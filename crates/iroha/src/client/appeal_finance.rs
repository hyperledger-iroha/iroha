//! Appeal-finance HTTP client contract tests.

use super::*;

#[test]
fn sorafs_appeal_finance_readback_filter_sets_query_params() {
    let client = Client::new(config_factory());
    let url = join_torii_url(&client.torii_url, "v1/sorafs/appeals/finance/reports");
    let filter = SorafsAppealFinanceReadbackFilter { limit: Some(25) };
    let request = filter
        .apply(client.default_request(HttpMethod::GET, url))
        .build()
        .expect("build request");
    assert_eq!(request.uri().query(), Some("limit=25"));
}

#[test]
fn sorafs_appeal_pricing_readback_targets_endpoints() {
    let client = client_with_base_url(base_url());
    let config_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let status_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));

    with_mock_http(
        respond_with(&config_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .get_sorafs_appeal_pricing_config()
                .expect("pricing config request");
        },
    );
    with_mock_http(
        respond_with(&status_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .get_sorafs_appeal_pricing_status()
                .expect("pricing status request");
        },
    );

    assert_eq!(
        config_store
            .lock()
            .expect("config snapshots")
            .first()
            .expect("config snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/pricing/config"
    );
    assert_eq!(
        status_store
            .lock()
            .expect("status snapshots")
            .first()
            .expect("status snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/pricing/status"
    );
}

#[test]
fn sorafs_appeal_pricing_quote_sends_json_request() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, r#"{"schema":"quote"}"#);
    let payload = norito::json::to_vec(&norito::json!({
        "class": "content",
        "backlog": 4_u64,
        "evidence_size_mb": 12_u64,
        "urgency": "normal",
    }))
    .expect("encode quote payload");

    with_mock_http(respond_with(&store, response), || {
        client
            .post_sorafs_appeal_pricing_quote_json(&payload)
            .expect("appeal pricing quote request");
    });

    let snapshots = store.lock().expect("snapshot store");
    let snapshot = snapshots.first().expect("snapshot");
    assert_eq!(snapshot.method, HttpMethod::POST);
    assert_eq!(snapshot.url.path(), "/v1/sorafs/appeals/pricing/quote");
    let headers: HashMap<_, _> = snapshot.headers.iter().cloned().collect();
    assert_eq!(
        headers.get("content-type"),
        Some(&APPLICATION_JSON.to_owned())
    );
    assert_eq!(headers.get("accept"), Some(&APPLICATION_JSON.to_owned()));
    assert!(!headers.contains_key(HEADER_SIGNATURE));
    let body: JsonValue = norito::json::from_slice(&snapshot.body).expect("decode request body");
    assert_eq!(
        body.get("class").and_then(JsonValue::as_str),
        Some("content")
    );
}

#[test]
fn sorafs_appeal_finance_deposit_sends_signed_json_request() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, r#"{"schema":"deposit"}"#);
    let payload = norito::json::to_vec(&norito::json!({
        "case_id": "case-401",
        "payer_account": "payer",
        "destination_account": "treasury",
        "asset_definition_id": "xor#wonderland",
        "deposit_xor": "100",
        "idempotency_key": "case-401-round-7",
    }))
    .expect("encode deposit payload");

    with_mock_http(respond_with(&store, response), || {
        client
            .post_sorafs_appeal_finance_deposit_json(&payload)
            .expect("appeal finance deposit request");
    });

    let snapshots = store.lock().expect("snapshot store");
    let snapshot = snapshots.first().expect("snapshot");
    assert_eq!(snapshot.method, HttpMethod::POST);
    assert_eq!(snapshot.url.path(), "/v1/sorafs/appeals/finance/deposits");
    let headers: HashMap<_, _> = snapshot.headers.iter().cloned().collect();
    assert!(headers.contains_key(HEADER_ACCOUNT));
    assert!(headers.contains_key(HEADER_SIGNATURE));
    assert!(headers.contains_key(HEADER_TIMESTAMP_MS));
    assert!(headers.contains_key(HEADER_NONCE));
    assert_eq!(
        headers.get("content-type"),
        Some(&APPLICATION_JSON.to_owned())
    );
    assert_eq!(headers.get("accept"), Some(&APPLICATION_JSON.to_owned()));
    let body: JsonValue = norito::json::from_slice(&snapshot.body).expect("decode request body");
    assert_eq!(
        body.get("case_id").and_then(JsonValue::as_str),
        Some("case-401")
    );
}

#[test]
fn sorafs_appeal_finance_deposit_get_normalizes_escrow_id_and_signs() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, "{}");
    let escrow_id = format!("0x{}", "AB".repeat(32));

    with_mock_http(respond_with(&store, response), || {
        client
            .get_sorafs_appeal_finance_deposit(&escrow_id)
            .expect("appeal finance deposit get request");
    });

    let snapshots = store.lock().expect("snapshot store");
    let snapshot = snapshots.first().expect("snapshot");
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(
        snapshot.url.path(),
        format!("/v1/sorafs/appeals/finance/deposits/{}", "ab".repeat(32))
    );
    let headers: HashMap<_, _> = snapshot.headers.iter().cloned().collect();
    assert!(headers.contains_key(HEADER_ACCOUNT));
    assert!(headers.contains_key(HEADER_SIGNATURE));
    assert!(headers.contains_key(HEADER_TIMESTAMP_MS));
    assert!(headers.contains_key(HEADER_NONCE));
    assert_eq!(headers.get("accept"), Some(&APPLICATION_JSON.to_owned()));
}

#[test]
fn sorafs_appeal_finance_deposit_settle_sends_signed_json_request() {
    let client = client_with_base_url(base_url());
    let store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let response = json_response(StatusCode::OK, r#"{"schema":"settle"}"#);
    let payload = norito::json::to_vec(&norito::json!({
        "deposit_confirmation": {
            "escrow_id_hex": ("11".repeat(32)),
            "case_id": "case-401",
            "payer_account": "payer",
            "destination_account": "treasury",
            "asset_definition_id": "xor#wonderland",
            "deposit_xor": "100",
            "idempotency_key": "case-401-round-7"
        },
        "outcome": "uphold"
    }))
    .expect("encode settle payload");

    with_mock_http(respond_with(&store, response), || {
        client
            .post_sorafs_appeal_finance_deposit_settle_json(&payload)
            .expect("appeal finance deposit settle request");
    });

    let snapshots = store.lock().expect("snapshot store");
    let snapshot = snapshots.first().expect("snapshot");
    assert_eq!(snapshot.method, HttpMethod::POST);
    assert_eq!(
        snapshot.url.path(),
        "/v1/sorafs/appeals/finance/deposits/settle"
    );
    let headers: HashMap<_, _> = snapshot.headers.iter().cloned().collect();
    assert!(headers.contains_key(HEADER_SIGNATURE));
    let body: JsonValue = norito::json::from_slice(&snapshot.body).expect("decode request body");
    assert_eq!(
        body.get("outcome").and_then(JsonValue::as_str),
        Some("uphold")
    );
}

#[test]
fn sorafs_appeal_finance_deposit_reconcile_and_submit_target_endpoints() {
    let client = client_with_base_url(base_url());
    let reconcile_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let submit_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let payload = norito::json::to_vec(&norito::json!({
        "deposit_confirmation": {
            "escrow_id_hex": ("22".repeat(32)),
            "case_id": "case-402",
            "payer_account": "payer",
            "destination_account": "treasury",
            "asset_definition_id": "xor#wonderland",
            "deposit_xor": "100",
            "idempotency_key": "case-402-round-7"
        },
        "outcome": "frivolous"
    }))
    .expect("encode reconcile payload");

    with_mock_http(
        respond_with(&reconcile_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .post_sorafs_appeal_finance_deposit_reconcile_json(&payload)
                .expect("appeal finance deposit reconcile request");
        },
    );
    with_mock_http(
        respond_with(&submit_store, json_response(StatusCode::ACCEPTED, "{}")),
        || {
            client
                .post_sorafs_appeal_finance_deposit_submit_settlement_json(&payload)
                .expect("appeal finance deposit submit-settlement request");
        },
    );

    assert_eq!(
        reconcile_store
            .lock()
            .expect("reconcile snapshots")
            .first()
            .expect("reconcile snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/finance/deposits/reconcile"
    );
    assert_eq!(
        submit_store
            .lock()
            .expect("submit snapshots")
            .first()
            .expect("submit snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/finance/deposits/submit-settlement"
    );
}

#[test]
fn sorafs_appeal_finance_readback_targets_endpoints() {
    let client = client_with_base_url(base_url());
    let reports_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let rollups_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let receipts_store: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let filter = SorafsAppealFinanceReadbackFilter { limit: Some(7) };

    with_mock_http(
        respond_with(&reports_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .get_sorafs_appeal_finance_reports(filter)
                .expect("appeal finance reports request");
        },
    );
    with_mock_http(
        respond_with(&rollups_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .get_sorafs_appeal_finance_weekly_rollups(filter)
                .expect("appeal finance weekly rollups request");
        },
    );
    with_mock_http(
        respond_with(&receipts_store, json_response(StatusCode::OK, "{}")),
        || {
            client
                .get_sorafs_appeal_finance_settlement_receipts(filter)
                .expect("appeal finance settlement receipts request");
        },
    );

    assert_eq!(
        reports_store
            .lock()
            .expect("reports snapshots")
            .first()
            .expect("reports snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/finance/reports"
    );
    assert_eq!(
        rollups_store
            .lock()
            .expect("rollups snapshots")
            .first()
            .expect("rollups snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/finance/weekly-rollups"
    );
    assert_eq!(
        receipts_store
            .lock()
            .expect("receipts snapshots")
            .first()
            .expect("receipts snapshot")
            .url
            .path(),
        "/v1/sorafs/appeals/finance/settlement-receipts"
    );
}

#[test]
fn sorafs_appeal_finance_json_rejects_empty_payload() {
    let client = client_with_base_url(base_url());
    let err = client
        .post_sorafs_appeal_finance_deposit_json(&[])
        .expect_err("empty payload must be rejected");
    assert!(err.to_string().contains("appeal finance deposit"));
}
