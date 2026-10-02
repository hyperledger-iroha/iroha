#[test]
fn retail_validation_fee_api_contract_has_calendar_rates_and_authenticated_reads() {
    for (label, document) in [
        ("package authority", canonical_document()),
        ("generated spec", generate_spec()),
    ] {
        let paths = document["paths"].as_object().unwrap();
        assert!(!paths.contains_key("/v1/validation-fee/hijiri/quote"));
        for (path, method, route_id, summary) in [
            (
                "/v1/validation-fee/quote",
                "post",
                "validation_fee.retail.quote",
                "Evaluate a native retail validation-fee quote.",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/status",
                "get",
                "validation_fee.retail.status",
                "Read a wallet's current monthly retail validation-fee status.",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/receipts",
                "get",
                "validation_fee.retail.receipts",
                "Read immutable native retail validation-fee receipts.",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/statement",
                "post",
                "validation_fee.retail.statement",
                "Read a private wallet retail validation-fee statement page.",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/statement/head",
                "get",
                "validation_fee.retail.statement_head",
                "Read the current cumulative wallet retail validation-fee statement head.",
            ),
        ] {
            let operation = openapi_operation(&document, path, method);
            assert_eq!(
                operation["x-iroha-operation"].as_str(),
                Some(route_id),
                "{label}: {path} operation identity"
            );
            assert_eq!(
                operation["x-iroha-route-auth"]["stableRouteId"].as_str(),
                Some(route_id),
                "{label}: {path} authenticated route identity"
            );
            assert_eq!(
                operation["summary"].as_str(),
                Some(summary),
                "{label}: {path} summary"
            );
        }
        for (path, method, response) in [
            (
                "/v1/validation-fee/quote",
                "post",
                "RetailFeeQuoteResponseV1",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/status",
                "get",
                "RetailFeeStatusResponseV1",
            ),
            (
                "/v1/validation-fee/accounts/{account_id}/receipts",
                "get",
                "RetailFeeReceiptsResponseV1",
            ),
        ] {
            let operation = openapi_operation(&document, path, method);
            assert!(
                operation.contains_key("security"),
                "{label}: {path} authentication"
            );
            assert!(operation.contains_key("x-iroha-canonical-auth-v1"));
            assert_eq!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
                    .as_str(),
                Some(format!("#/components/schemas/{response}").as_str())
            );
            assert_canonical_auth_required_response(
                operation,
                path,
                "canonical_authentication_required",
            );
            assert_eq!(
                documented_reject_codes(operation["responses"].as_object().unwrap(), "403"),
                vec!["retail_fee_account_mismatch"]
            );
        }
        let schemas = component_schemas(&document);
        assert_eq!(
            schemas["RetailFeeCurrentHeadResponseV1"]["properties"]["finality_proof"]["$ref"]
                .as_str(),
            Some("#/components/schemas/SumeragiFinalityProof"),
            "{label}: statement head uses the native finality owner",
        );
        assert!(!schemas.contains_key("ValidationFeeHijiriQuoteRequestV1"));
        assert!(
            !schemas
                .contains_key("GovernanceParliamentProposalPayloadValidationFeePayoutRecipientV1")
        );
        assert_eq!(
            schemas["RetailFeeReceiptsResponseV1"]["properties"]["finality_proofs"]["items"]["$ref"]
                .as_str(),
            Some("#/components/schemas/SumeragiFinalityProof"),
            "{label}: receipt proofs use the native finality owner",
        );
        let policy =
            &schemas["GovernanceParliamentProposalPayloadValidationFeePolicyV1"]["properties"];
        assert!(policy.get("effective_from_height").is_none());
        assert!(policy.get("expires_after_height").is_none());
        assert!(policy.get("treasury_payout_binding").is_none());
        assert!(policy.get("reward_custody").is_some());
        for field in [
            "effective_from_ms",
            "notice_published_at_ms",
            "retail_schedule",
        ] {
            assert!(policy.get(field).is_some(), "{label}: {field}");
        }
        let conversion = schemas["GovernanceParliamentProposalPayloadValidationFeePayoutBindingV1"]
            ["properties"]
            .as_object()
            .unwrap();
        assert!(!conversion.contains_key("recipients"));
        for field in [
            "reward_pool_account_id",
            "reference_provider_accounts",
            "max_sbd_per_attempt_minor",
            "max_sbd_per_day_minor",
        ] {
            assert!(conversion.contains_key(field));
        }
        let assessment = &schemas["RetailFeeAssessmentV1"]["properties"];
        for field in ["state_commitment", "intent_hash"] {
            assert_eq!(
                assessment[field]["pattern"].as_str(),
                Some("^[0-9A-F]{64}$")
            );
        }
    }
}
