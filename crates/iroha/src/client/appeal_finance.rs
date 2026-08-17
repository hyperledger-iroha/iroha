use super::*;

#[test]
fn sorafs_appeal_finance_readback_filter_sets_query_params() {
    let mut url = join_torii_url(&base_url(), "v1/sorafs/appeals/finance/reports");
    SorafsAppealFinanceReadbackFilter { limit: Some(25) }.apply_to_url(&mut url);
    assert_eq!(url.query(), Some("limit=25"));
}

#[test]
fn sorafs_appeal_pricing_readback_targets_endpoints() {
    type PricingReadRequest = fn(&Client) -> Result<Response<Vec<u8>>>;
    let requests: [(&str, PricingReadRequest); 2] = [
        (
            "/v1/sorafs/appeals/pricing/config",
            Client::get_sorafs_appeal_pricing_config,
        ),
        (
            "/v1/sorafs/appeals/pricing/status",
            Client::get_sorafs_appeal_pricing_status,
        ),
    ];
    for (path, request) in requests {
        let (_, snapshot) = capture_sorafs_endpoint(StatusCode::OK, "{}", request);
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), path);
        assert_single_accept_header(&snapshot, APPLICATION_JSON);
    }
}

#[test]
fn sorafs_appeal_pricing_quote_sends_json_request() {
    let payload = br#"{"class":"content"}"#.to_vec();
    let (_, snapshot) = capture_sorafs_endpoint(StatusCode::OK, "{}", |client| {
        client.post_sorafs_appeal_pricing_quote_json(&payload)
    });
    assert_eq!(snapshot.method, HttpMethod::POST);
    assert_eq!(snapshot.url.path(), "/v1/sorafs/appeals/pricing/quote");
    assert_unsigned_json_request(&snapshot);
    assert_single_accept_header(&snapshot, APPLICATION_JSON);
    assert_eq!(snapshot.body, payload);
}

#[test]
fn sorafs_appeal_finance_deposit_sends_signed_json_request() {
    let payload = br#"{"case_id":"case-401"}"#.to_vec();
    assert_signed_json_endpoint(
        StatusCode::OK,
        "/v1/sorafs/appeals/finance/deposits",
        true,
        &payload,
        |client| client.post_sorafs_appeal_finance_deposit_json(&payload),
    );
}

#[test]
fn sorafs_appeal_finance_deposit_get_normalizes_escrow_id_and_signs() {
    let escrow_id = format!("0x{}", "AB".repeat(32));
    let (_, snapshot) = capture_sorafs_endpoint(StatusCode::OK, "{}", |client| {
        client.get_sorafs_appeal_finance_deposit(&escrow_id)
    });
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(
        snapshot.url.path(),
        format!("/v1/sorafs/appeals/finance/deposits/{}", "ab".repeat(32))
    );
    let headers = assert_signed_headers(&snapshot);
    assert_eq!(headers.get("accept"), Some(&APPLICATION_JSON.to_owned()));
}

#[test]
fn sorafs_appeal_finance_deposit_settle_sends_signed_json_request() {
    let payload = settlement_payload("11", "case-401", "uphold");
    assert_signed_json_endpoint(
        StatusCode::OK,
        "/v1/sorafs/appeals/finance/deposits/settle",
        true,
        &payload,
        |client| client.post_sorafs_appeal_finance_deposit_settle_json(&payload),
    );
}

fn settlement_payload(escrow_byte: &str, case_id: &str, outcome: &str) -> Vec<u8> {
    let escrow_id_hex = escrow_byte.repeat(32);
    norito::json::to_vec(&norito::json!({
        "escrow_id_hex": escrow_id_hex,
        "case_id": case_id,
        "outcome": outcome,
    }))
    .expect("encode settlement payload")
}

#[test]
fn sorafs_appeal_finance_deposit_reconcile_and_submit_target_endpoints() {
    type SettlementRequest = fn(&Client, &[u8]) -> Result<Response<Vec<u8>>>;

    let payload = settlement_payload("22", "case-402", "frivolous");
    let cases: [(StatusCode, &str, SettlementRequest); 2] = [
        (
            StatusCode::OK,
            "/v1/sorafs/appeals/finance/deposits/reconcile",
            Client::post_sorafs_appeal_finance_deposit_reconcile_json,
        ),
        (
            StatusCode::ACCEPTED,
            "/v1/sorafs/appeals/finance/deposits/submit-settlement",
            Client::post_sorafs_appeal_finance_deposit_submit_settlement_json,
        ),
    ];
    for (status, path, request) in cases {
        let (_, snapshot) =
            capture_sorafs_endpoint(status, "{}", |client| request(client, &payload));
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert_eq!(snapshot.url.path(), path);
        assert_signed_json_headers(&snapshot);
    }
}

#[test]
fn sorafs_appeal_finance_readback_targets_endpoints() {
    type FinanceReadbackRequest =
        fn(&Client, SorafsAppealFinanceReadbackFilter) -> Result<Response<Vec<u8>>>;

    let filter = SorafsAppealFinanceReadbackFilter { limit: Some(7) };
    let cases: [(&str, FinanceReadbackRequest); 3] = [
        (
            "/v1/sorafs/appeals/finance/reports",
            Client::get_sorafs_appeal_finance_reports,
        ),
        (
            "/v1/sorafs/appeals/finance/weekly-rollups",
            Client::get_sorafs_appeal_finance_weekly_rollups,
        ),
        (
            "/v1/sorafs/appeals/finance/settlement-receipts",
            Client::get_sorafs_appeal_finance_settlement_receipts,
        ),
    ];
    for (path, request) in cases {
        let (_, snapshot) =
            capture_sorafs_endpoint(StatusCode::OK, "{}", |client| request(client, filter));
        assert_eq!(snapshot.method, HttpMethod::GET);
        assert_eq!(snapshot.url.path(), path);
        assert_eq!(snapshot.url.query(), Some("limit=7"));
        assert_single_accept_header(&snapshot, APPLICATION_JSON);
    }
}

#[test]
fn sorafs_appeal_finance_json_rejects_empty_payload() {
    let error = client_with_base_url(base_url())
        .post_sorafs_appeal_finance_deposit_json(&[])
        .expect_err("empty payload must be rejected");
    assert!(error.to_string().contains("appeal finance deposit"));
}
