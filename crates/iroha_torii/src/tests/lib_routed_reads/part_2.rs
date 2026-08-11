    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_applies_post_merge_pagination() {
        let response = merged_space_directory_manifests_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "total": 1,
                    "manifests": [{
                        "dataspace_id": 2,
                        "manifest_hash": "aaaa",
                        "status": "Active"
                    }]
                }),
                norito::json!({
                    "uaid": "uaid:alice",
                    "total": 1,
                    "manifests": [{
                        "dataspace_id": 7,
                        "manifest_hash": "bbbb",
                        "status": "Revoked"
                    }, {
                        "dataspace_id": 2,
                        "manifest_hash": "aaaa",
                        "status": "Active"
                    }]
                }),
            ],
            1,
            Some(1),
            "proxy",
            routed_read_test_budget(),
        )
        .expect("manifest merge should succeed");

        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy")
        );
        assert!(
            response.headers().get("x-iroha-route-lane-id").is_none(),
            "fanout merge must not report a singular lane"
        );
        assert!(
            response
                .headers()
                .get("x-iroha-route-dataspace-id")
                .is_none(),
            "fanout merge must not report a singular dataspace"
        );

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["total"].as_u64(), Some(2));
        let manifests = json["manifests"]
            .as_array()
            .expect("manifest merge should expose manifests");
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0]["dataspace_id"].as_u64(), Some(7));
        assert_eq!(manifests[0]["manifest_hash"].as_str(), Some("bbbb"));
        assert_eq!(manifests[0]["status"].as_str(), Some("Revoked"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_uses_route_totals_before_status_filtering() {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!({
                "uaid": "uaid:alice",
                "total": 2,
                "manifests": [{
                    "dataspace_id": 7,
                    "manifest_hash": "bbbb",
                    "status": "Revoked"
                }]
            })],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect("manifest merge should succeed");

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(
            json["manifests"].as_array().expect("manifests array").len(),
            1
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_deduplicates_identical_rows_without_explicit_totals()
     {
        let response = merged_space_directory_manifests_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "manifests": [{
                        "dataspace_id": 7,
                        "manifest_hash": "bbbb",
                        "status": "Revoked"
                    }]
                }),
                norito::json!({
                    "uaid": "uaid:alice",
                    "manifests": [{
                        "dataspace_id": 7,
                        "manifest_hash": "bbbb",
                        "status": "Revoked"
                    }]
                }),
            ],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect("manifest merge should succeed");

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["total"].as_u64(), Some(1));
        let manifests = json["manifests"].as_array().expect("manifests array");
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0]["manifest_hash"].as_str(), Some("bbbb"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_clears_page_when_offset_exceeds_merged_len()
    {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!({
                "uaid": "uaid:alice",
                "total": 2,
                "manifests": [{
                    "dataspace_id": 2,
                    "manifest_hash": "aaaa",
                    "status": "Active"
                }, {
                    "dataspace_id": 7,
                    "manifest_hash": "bbbb",
                    "status": "Revoked"
                }]
            })],
            5,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect("manifest merge should succeed");

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["total"].as_u64(), Some(2));
        assert_eq!(
            json["manifests"].as_array().expect("manifests array").len(),
            0
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_conflicting_uaid_roots() {
        let response = merged_space_directory_manifests_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "manifests": []
                }),
                norito::json!({
                    "uaid": "uaid:bob",
                    "manifests": []
                }),
            ],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("conflicting UAID roots should fail");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_conflicting_duplicate_rows() {
        let response = merged_space_directory_manifests_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "manifests": [{
                        "dataspace_id": 7,
                        "manifest_hash": "bbbb",
                        "status": "Active"
                    }]
                }),
                norito::json!({
                    "uaid": "uaid:alice",
                    "manifests": [{
                        "dataspace_id": 7,
                        "manifest_hash": "bbbb",
                        "status": "Revoked"
                    }]
                }),
            ],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("conflicting manifest rows should fail");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_non_object_payloads() {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!(["not-an-object"])],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("non-object manifest payloads should fail");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_missing_manifests_array() {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!({
                "uaid": "uaid:alice",
                "total": 1
            })],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("manifest payloads without manifests array should fail");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_missing_dataspace_id() {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!({
                "uaid": "uaid:alice",
                "manifests": [{
                    "manifest_hash": "bbbb",
                    "status": "Revoked"
                }]
            })],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("manifest rows without dataspace ids should fail");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn merged_space_directory_manifests_response_rejects_missing_manifest_hash() {
        let response = merged_space_directory_manifests_response(
            vec![norito::json!({
                "uaid": "uaid:alice",
                "manifests": [{
                    "dataspace_id": 7,
                    "status": "Revoked"
                }]
            })],
            0,
            None,
            "proxy",
            routed_read_test_budget(),
        )
        .expect_err("manifest rows without manifest hashes should fail");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("invalid_proxy_response")
        );
    }

    #[tokio::test]
    async fn merged_portfolio_response_aggregates_dataspaces_and_totals() {
        let response = merged_portfolio_response(
            vec![
                norito::json!({
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 2,
                        "accounts": [
                            {"assets": [1, 2]}
                        ]
                    }]
                }),
                norito::json!({
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 7,
                        "accounts": [
                            {"assets": [1]},
                            {"assets": []}
                        ]
                    }]
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("portfolio merge should succeed");

        let json = response_json(response).await;
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["totals"]["accounts"].as_u64(), Some(3));
        assert_eq!(json["totals"]["positions"].as_u64(), Some(3));
        assert_eq!(
            json["dataspaces"]
                .as_array()
                .expect("dataspaces should be an array")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn merged_dataspace_summary_response_accumulates_totals() {
        let response = merged_dataspace_summary_response(
            vec![
                norito::json!({
                    "account": "alice@sora",
                    "account_id": "alice@sora",
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 2,
                        "accounts": ["alice@sora", "alice@vault"],
                        "portfolio": {"accounts": 2, "positions": 5},
                        "manifest": {"present": true, "active": true},
                        "consensus": {
                            "entries": 3,
                            "tx_count": 8,
                            "total_chunks": 13,
                            "rbc_bytes_total": 21,
                            "teu_total": 34
                        }
                    }]
                }),
                norito::json!({
                    "account": "alice@sora",
                    "account_id": "alice@sora",
                    "uaid": "uaid:alice",
                    "dataspaces": [{
                        "dataspace_id": 7,
                        "accounts": ["alice@vault", "alice@ops"],
                        "portfolio": {"accounts": 1, "positions": 2},
                        "manifest": {"present": true, "active": false},
                        "consensus": {
                            "entries": 5,
                            "tx_count": 13,
                            "total_chunks": 8,
                            "rbc_bytes_total": 55,
                            "teu_total": 89
                        }
                    }]
                }),
            ],
            "proxy",
            routed_read_test_budget(),
        )
        .expect("dataspace summary merge should succeed");

        let json = response_json(response).await;
        assert_eq!(json["account"].as_str(), Some("alice@sora"));
        assert_eq!(json["account_id"].as_str(), Some("alice@sora"));
        assert_eq!(json["uaid"].as_str(), Some("uaid:alice"));
        assert_eq!(json["totals"]["dataspaces"].as_u64(), Some(2));
        assert_eq!(json["totals"]["accounts_bound"].as_u64(), Some(3));
        assert_eq!(json["totals"]["portfolio_accounts"].as_u64(), Some(3));
        assert_eq!(json["totals"]["portfolio_positions"].as_u64(), Some(7));
        assert_eq!(json["totals"]["manifests_total"].as_u64(), Some(2));
        assert_eq!(json["totals"]["manifests_active"].as_u64(), Some(1));
        assert_eq!(json["totals"]["consensus_entries"].as_u64(), Some(8));
        assert_eq!(json["totals"]["consensus_tx_count"].as_u64(), Some(21));
        assert_eq!(json["totals"]["consensus_chunks_total"].as_u64(), Some(21));
        assert_eq!(
            json["totals"]["consensus_rbc_bytes_total"].as_u64(),
            Some(76)
        );
        assert_eq!(json["totals"]["consensus_teu_total"].as_u64(), Some(123));
        assert_eq!(
            json["dataspaces"]
                .as_array()
                .expect("dataspaces should be an array")
                .len(),
            2
        );
    }
