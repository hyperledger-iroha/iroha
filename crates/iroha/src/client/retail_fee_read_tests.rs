// Account-context authentication and exact intent binding for retail fee reads.
#[test]
fn retail_fee_reads_bind_service_authority_network_path_body_and_fresh_nonce() {
    use iroha_data_model::validation_fee::{
        RetailFeeAssessmentV1, RetailFeePaymentLegV1, RetailFeeQuoteRequestV1, RetailFeeScheduleV1,
    };
    let client = client_with_base_url(base_url());
    let wallet = AccountId::new(KeyPair::random().public_key().clone());
    let request = RetailFeeQuoteRequestV1 {
        account_id: wallet.clone(),
        asset_definition_id: "7ZepsJTHCVLKsrFFNZGSRGZgvBhv".parse().unwrap(),
        transfers: vec![RetailFeePaymentLegV1 {
            destination_account_id: client.account.clone(),
            amount_minor_units: 100,
        }],
    };
    let quote = RetailFeeQuoteResponseV1 {
        request: request.clone(),
        assessment: RetailFeeAssessmentV1 {
            account_id: wallet.clone(),
            retail_enrolled: true,
            billing_month_start_ms: 1,
            policy_revision: 1,
            payments_used_before: 49,
            qualifying_payments: 1,
            fee_minor: 0,
            state_commitment: [1; 32],
            intent_hash: request.intent_hash().unwrap(),
            expires_at_ms: 60_000,
        },
        policy_hash_hex: "11".repeat(32),
        ledger_finalised_height: 20,
    };
    let quote_json = norito::json::to_json(&quote).unwrap();
    let status_json = norito::json::to_json(&RetailFeeStatusResponseV1 {
        account_state: None,
        policy_hash_hex: "11".repeat(32),
        ledger_finalised_height: 20,
        institutional_fee_minor: 10,
        retail_schedule: RetailFeeScheduleV1::default(),
        estimated_maintenance_minor: 0,
        forthcoming_policy: None,
    })
    .unwrap();
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let captured = snapshots.clone();
    with_mock_http(
        move |snapshot| {
            let json = if snapshot.url.path().ends_with("/status") {
                status_json.clone()
            } else if snapshot.url.path().ends_with("/quote") {
                quote_json.clone()
            } else {
                r#"{"receipts":[],"receipt_proofs":[],"finality_proofs":[],"ledger_finalised_height":20,"next_receipt_id":null,"assurance":"NATIVE_RECEIPT_MEMBERSHIP_PROOFS_REQUIRING_TRUSTED_FINALITY_VERIFICATION"}"#.to_owned()
            };
            captured.lock().unwrap().push(snapshot);
            Ok(json_response(StatusCode::OK, &json))
        },
        |transport| {
            let bound = client
                .clone()
                .with_test_http_transport(transport)
                .account_client()
                .unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                bound.retail_fee_status(&wallet).await.unwrap();
                let response = bound.retail_fee_quote(&request).await.unwrap();
                assert_eq!(
                    response.assessment.fee_minor, 0,
                    "included payment keeps its explicit assessment"
                );
                bound
                    .retail_fee_receipts(&wallet, Some(&"ab".repeat(32)), 5)
                    .await
                    .unwrap();
            });
        },
    );
    let snapshots = snapshots.lock().unwrap();
    assert_eq!(snapshots.len(), 3);
    let mut nonces = std::collections::HashSet::new();
    for snapshot in snapshots.iter() {
        assert_canonical_account_signed_request(&client, snapshot);
        nonces.insert(
            snapshot
                .headers
                .iter()
                .find(|(key, _)| key == HEADER_NONCE)
                .unwrap()
                .1
                .clone(),
        );
    }
    assert_eq!(nonces.len(), 3);
    assert!(snapshots[0].url.path().ends_with("/status"));
    assert_eq!(snapshots[1].method, HttpMethod::POST);
    assert_eq!(snapshots[1].body, norito::json::to_vec(&request).unwrap());
    assert_eq!(
        snapshots[2]
            .url
            .query_pairs()
            .find(|(key, _)| key == "limit")
            .unwrap()
            .1,
        "5"
    );
}

#[test]
fn retail_fee_reads_reject_oversized_page_before_network_and_untrusted_response() {
    let client = client_with_base_url(base_url());
    with_mock_http(
        |_| panic!("invalid receipt page must fail before network"),
        |transport| {
            let bound = client
                .clone()
                .with_test_http_transport(transport)
                .account_client()
                .unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                assert!(
                    bound
                        .retail_fee_receipts(&client.account, None, 101)
                        .await
                        .is_err()
                );
                assert!(
                    bound
                        .retail_fee_receipts(&client.account, Some("invalid"), 1)
                        .await
                        .is_err()
                );
            });
        },
    );
    with_mock_http(
        |_| Ok(json_response(StatusCode::FOUND, "{}")),
        |transport| {
            let bound = client
                .clone()
                .with_test_http_transport(transport)
                .account_client()
                .unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            assert!(
                runtime
                    .block_on(bound.retail_fee_status(&client.account))
                    .is_err()
            );
        },
    );
}

#[test]
fn retail_fee_quote_rejects_changed_intent_even_when_request_echo_is_unchanged() {
    use iroha_data_model::validation_fee::{
        RetailFeeAssessmentV1, RetailFeePaymentLegV1, RetailFeeQuoteRequestV1,
    };
    let client = client_with_base_url(base_url());
    let request = RetailFeeQuoteRequestV1 {
        account_id: client.account.clone(),
        asset_definition_id: "7ZepsJTHCVLKsrFFNZGSRGZgvBhv".parse().unwrap(),
        transfers: vec![RetailFeePaymentLegV1 {
            destination_account_id: AccountId::new(KeyPair::random().public_key().clone()),
            amount_minor_units: 100,
        }],
    };
    let response = RetailFeeQuoteResponseV1 {
        request: request.clone(),
        assessment: RetailFeeAssessmentV1 {
            account_id: request.account_id.clone(),
            retail_enrolled: true,
            billing_month_start_ms: 1,
            policy_revision: 1,
            payments_used_before: 0,
            qualifying_payments: 1,
            fee_minor: 0,
            state_commitment: [1; 32],
            intent_hash: [0; 32],
            expires_at_ms: 60_000,
        },
        policy_hash_hex: "11".repeat(32),
        ledger_finalised_height: 20,
    };
    let json = norito::json::to_json(&response).unwrap();
    with_mock_http(
        move |_| Ok(json_response(StatusCode::OK, &json)),
        |transport| {
            let bound = client
                .with_test_http_transport(transport)
                .account_client()
                .unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            assert!(
                runtime
                    .block_on(bound.retail_fee_quote(&request))
                    .unwrap_err()
                    .to_string()
                    .contains("exact requested payment intent")
            );
        },
    );
}
