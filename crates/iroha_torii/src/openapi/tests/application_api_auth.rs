#[test]
fn subscription_mutations_document_exact_account_authentication() {
    let document = generate_spec();
    let expected = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);

    for descriptor in [
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_PLANS_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_PAUSE_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_RESUME_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CANCEL_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_KEEP_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_USAGE_POST,
        iroha_torii_shared::route_catalog::application_api::SUBSCRIPTIONS_BY_SUBSCRIPTION_ID_CHARGE_NOW_POST,
    ] {
        let headers = operation_header_requirements(openapi_operation(
            &document,
            descriptor.path(),
            "post",
        ));
        let names = headers
            .into_iter()
            .filter_map(|(name, required)| {
                name.starts_with("X-Iroha-").then(|| {
                    assert!(
                        !required,
                        "{} must permit the witness alternative",
                        descriptor.path()
                    );
                    name
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(names, expected, "POST {}", descriptor.path());
    }
}

#[test]
fn application_drafts_and_cryptographic_services_document_exact_account_authentication() {
    let document = generate_spec();
    let expected_headers = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);

    for descriptor in [
        iroha_torii_shared::route_catalog::application_api::SPACE_DIRECTORY_MANIFESTS_POST,
        iroha_torii_shared::route_catalog::application_api::SPACE_DIRECTORY_MANIFESTS_REVOKE_POST,
        iroha_torii_shared::route_catalog::application_api::RAM_LFE_PROGRAMS_BY_PROGRAM_ID_EXECUTE_POST,
        iroha_torii_shared::route_catalog::application_api::RAM_LFE_RECEIPTS_VERIFY_POST,
        iroha_torii_shared::route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_IDENTIFIERS_CLAIM_RECEIPT_POST,
        iroha_torii_shared::route_catalog::application_api::IDENTIFIERS_RESOLVE_POST,
    ] {
        let operation = openapi_operation(&document, descriptor.path(), "post");
        let headers = operation_header_requirements(operation)
            .into_iter()
            .filter_map(|(name, required)| {
                name.starts_with("X-Iroha-").then(|| {
                    assert!(
                        !required,
                        "{} must permit the witness alternative",
                        descriptor.path()
                    );
                    name
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(headers, expected_headers, "POST {}", descriptor.path());
        assert!(
            operation.get("security").is_some(),
            "POST {} security",
            descriptor.path()
        );
    }
}

#[test]
fn expensive_application_queries_document_exact_account_authentication() {
    let document = generate_spec();
    let expected_headers = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);

    for descriptor in [
        iroha_torii_shared::route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::DOMAINS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::ACCOUNTS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::TRANSACTIONS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::TRANSACTIONS_VISIBLE_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::REPO_AGREEMENTS_QUERY_POST,
        iroha_torii_shared::route_catalog::telemetry::ASSET_HOLDERS_QUERY,
        iroha_torii_shared::route_catalog::application_api::ASSETS_DEFINITIONS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::NFTS_QUERY_POST,
        iroha_torii_shared::route_catalog::application_api::RWAS_QUERY_POST,
    ] {
        let operation = openapi_operation(&document, descriptor.path(), "post");
        let headers = operation_header_requirements(operation)
            .into_iter()
            .filter_map(|(name, required)| {
                name.starts_with("X-Iroha-").then(|| {
                    assert!(!required, "{} permits the witness alternative", descriptor.path());
                    name
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(headers, expected_headers, "POST {}", descriptor.path());
        assert!(operation.get("security").is_some(), "POST {} security", descriptor.path());
    }
}

#[test]
fn proof_query_documents_body_signed_authentication_without_account_headers() {
    let document = generate_spec();
    let descriptor = iroha_torii_shared::route_catalog::application_api::PROOFS_QUERY_POST;
    let operation = openapi_operation(&document, descriptor.path(), "post");
    assert!(
        operation_header_requirements(operation)
            .keys()
            .all(|name| !name.starts_with("X-Iroha-")),
        "the SignedQuery body, not account headers, authenticates the proof query"
    );
    assert!(
        operation
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| description.contains("SignedQuery"))
    );
}

#[test]
fn offline_receiver_lineage_documents_exact_account_authentication() {
    let document = generate_spec();
    let descriptor = iroha_torii_shared::route_catalog::offline::RECIPIENT_LINEAGE;
    let operation = openapi_operation(&document, descriptor.path(), "post");
    let expected_headers = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);
    let headers = operation_header_requirements(operation)
        .into_iter()
        .filter_map(|(name, required)| {
            name.starts_with("X-Iroha-").then(|| {
                assert!(
                    !required,
                    "the multisig witness alternative must remain valid"
                );
                name
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(headers, expected_headers);
    assert!(operation.get("security").is_some());

    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .expect("receiver-lineage responses");
    for response in responses.values() {
        let cache_control = response
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cache-Control"))
            .and_then(Value::as_object)
            .and_then(|header| header.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("const"))
            .and_then(Value::as_str);
        assert_eq!(cache_control, Some("private, no-store"));
    }
    let bad_request_headers = responses
        .get("400")
        .and_then(Value::as_object)
        .and_then(|response| response.get("headers"))
        .and_then(Value::as_object)
        .expect("receiver-lineage validation headers");
    assert!(
        bad_request_headers.contains_key("x-iroha-reject-code"),
        "private caching must not erase the exact reject-code contract"
    );
}

#[test]
fn soracloud_sensitive_reads_document_exact_account_authentication() {
    let document = generate_spec();
    let expected_headers = BTreeSet::from([
        "X-Iroha-Account".to_owned(),
        "X-Iroha-Signature".to_owned(),
        "X-Iroha-Timestamp-Ms".to_owned(),
        "X-Iroha-Nonce".to_owned(),
        "X-Iroha-Witness".to_owned(),
    ]);

    for descriptor in [
        iroha_torii_shared::route_catalog::application_api::SORACLOUD_STATUS_GET,
        iroha_torii_shared::route_catalog::application_api::SORACLOUD_MODEL_UPLOAD_PRIVATE_RECEIPTS_GET,
    ] {
        let operation = openapi_operation(&document, descriptor.path(), "get");
        let headers = operation_header_requirements(operation)
            .into_iter()
            .filter_map(|(name, required)| {
                name.starts_with("X-Iroha-").then(|| {
                    assert!(!required, "{} permits the witness alternative", descriptor.path());
                    name
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(headers, expected_headers, "GET {}", descriptor.path());
        assert!(operation.get("security").is_some(), "GET {} security", descriptor.path());

        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("Soracloud GET responses");
        for response in responses.values() {
            let cache_control = response
                .get("headers")
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cache-Control"))
                .and_then(Value::as_object)
                .and_then(|header| header.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str);
            assert_eq!(cache_control, Some("private, no-store"));
        }
    }
}

#[test]
fn webhook_registry_documents_required_operator_signatures() {
    let document = generate_spec();
    let expected = BTreeSet::from([
        "X-Iroha-Operator-Public-Key".to_owned(),
        "X-Iroha-Operator-Timestamp-Ms".to_owned(),
        "X-Iroha-Operator-Nonce".to_owned(),
        "X-Iroha-Operator-Signature".to_owned(),
    ]);
    for (descriptor, method) in [
        (
            iroha_torii_shared::route_catalog::application_api::WEBHOOKS_GET,
            "get",
        ),
        (
            iroha_torii_shared::route_catalog::application_api::WEBHOOKS_POST,
            "post",
        ),
        (
            iroha_torii_shared::route_catalog::application_api::WEBHOOKS_BY_ID_DELETE,
            "delete",
        ),
    ] {
        let headers =
            operation_header_requirements(openapi_operation(&document, descriptor.path(), method));
        let names = headers
            .into_iter()
            .filter_map(|(name, required)| {
                name.starts_with("X-Iroha-Operator-").then(|| {
                    assert!(required, "{method} {} {name}", descriptor.path());
                    name
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(names, expected, "{method} {}", descriptor.path());
    }
}
