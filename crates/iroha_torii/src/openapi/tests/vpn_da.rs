#[test]
fn vpn_openapi_paths_are_typed_signed_and_use_runtime_success_statuses() {
    let document = generate_spec();
    let cases = [
        (
            "/v1/vpn/profile",
            "get",
            None,
            "200",
            "#/components/schemas/VpnProfileResponse",
            false,
        ),
        (
            "/v1/vpn/quotes",
            "post",
            Some("#/components/schemas/VpnQuoteCreateRequest"),
            "201",
            "#/components/schemas/VpnQuoteResponse",
            true,
        ),
        (
            "/v1/vpn/sessions",
            "post",
            Some("#/components/schemas/VpnSessionCreateRequest"),
            "201",
            "#/components/schemas/VpnSessionResponse",
            true,
        ),
        (
            "/v1/vpn/sessions/{session_id}",
            "get",
            None,
            "200",
            "#/components/schemas/VpnSessionResponse",
            true,
        ),
        (
            "/v1/vpn/sessions/{session_id}",
            "delete",
            None,
            "200",
            "#/components/schemas/VpnReceiptResponse",
            true,
        ),
        (
            "/v1/vpn/receipts",
            "get",
            None,
            "200",
            "#/components/schemas/VpnReceiptListResponse",
            true,
        ),
        (
            "/v1/vpn/receipts",
            "post",
            Some("#/components/schemas/VpnReceiptSubmitRequest"),
            "201",
            "#/components/schemas/VpnReceiptResponse",
            true,
        ),
    ];
    let expected_auth_headers = [
        "X-Iroha-Account",
        "X-Iroha-Signature",
        "X-Iroha-Timestamp-Ms",
        "X-Iroha-Nonce",
        "X-Iroha-Witness",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for (path, method, request_ref, success_status, response_ref, signed) in cases {
        let operation = openapi_operation(&document, path, method);
        match request_ref {
            Some(expected) => {
                assert_eq!(operation_request_schema_ref(operation, path), expected);
                assert_eq!(
                    operation
                        .get("requestBody")
                        .and_then(Value::as_object)
                        .and_then(|body| body.get("required")),
                    Some(&Value::Bool(true)),
                    "{method} {path} request body"
                );
            }
            None => assert!(
                operation.get("requestBody").is_none(),
                "{method} {path} must not advertise a request body"
            ),
        }
        assert_eq!(
            operation_response_schema_ref(operation, success_status, path),
            response_ref,
            "{method} {path} response"
        );
        let responses = operation
            .get("responses")
            .and_then(Value::as_object)
            .expect("operation responses");
        assert!(
            responses.contains_key(success_status),
            "{method} {path} must advertise HTTP {success_status}"
        );
        if method == "post" {
            assert!(
                !responses.contains_key("200"),
                "{method} {path} must not advertise the retired creation status"
            );
        }
        let auth_headers = operation
            .get("parameters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|parameter| parameter.get("in").and_then(Value::as_str) == Some("header"))
            .map(|parameter| {
                parameter
                    .get("name")
                    .and_then(Value::as_str)
                    .expect("header parameter name")
            })
            .collect::<BTreeSet<_>>();
        if signed {
            assert_eq!(auth_headers, expected_auth_headers, "{method} {path} auth");
        } else {
            assert!(
                auth_headers.is_empty(),
                "{method} {path} must remain a public route"
            );
        }
    }
    for method in ["get", "delete"] {
        let operation = openapi_operation(&document, "/v1/vpn/sessions/{session_id}", method);
        let session_id = operation
            .get("parameters")
            .and_then(Value::as_array)
            .and_then(|parameters| {
                parameters.iter().find(|parameter| {
                    parameter.get("name").and_then(Value::as_str) == Some("session_id")
                        && parameter.get("in").and_then(Value::as_str) == Some("path")
                })
            })
            .expect("session_id path parameter");
        assert_eq!(
            session_id
                .get("schema")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some("^[0-9a-f]{64}$"),
            "{method} session id must use the canonical lowercase 32-byte hex form"
        );
    }
}
#[test]
fn vpn_openapi_schemas_are_strict_and_use_canonical_quantities() {
    let document = generate_spec();
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "VpnTxInstruction",
        &["wire_id", "payload_hex"],
        &[],
    );
    assert_strict_object_schema(
        schemas,
        "VpnQuoteCreateRequest",
        &["metering_public_key_hex"],
        &["exit_class"],
    );
    assert_strict_object_schema(
        schemas,
        "VpnSessionCreateRequest",
        &["quote_id", "payment_tx_hash", "metering_public_key_hex"],
        &["exit_class"],
    );
    assert_strict_object_schema(
        schemas,
        "VpnReceiptSubmitRequest",
        &["relay_receipt_hex", "client_voucher_hex"],
        &["lease_id_hex"],
    );
    let profile_fields = [
        "available",
        "relay_endpoint",
        "supported_exit_classes",
        "default_exit_class",
        "lease_secs",
        "dns_push_interval_secs",
        "meter_family",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "display_billing_label",
        "operator_account_id",
        "lease_fee",
        "settlement_grace_secs",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
    ];
    assert_strict_object_schema(schemas, "VpnProfileResponse", &profile_fields, &[]);
    let quote_fields = [
        "quote_id",
        "lease_id_hex",
        "session_id_hex",
        "payment_reference",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "quote_expires_at_ms",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "meter_family",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "metering_public_key_hex",
        "open_lease_instruction",
    ];
    assert_strict_object_schema(schemas, "VpnQuoteResponse", &quote_fields, &[]);
    let session_fields = [
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "lease_secs",
        "expires_at_ms",
        "connected_at_ms",
        "meter_family",
        "quote_id",
        "payment_reference",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "flow_label_bits",
        "padding_budget_ms",
        "relay_id_hex",
        "descriptor_commit_hex",
        "tls_server_name",
        "relay_tls_spki_sha256_hex",
        "relay_certificate_sha256_hex",
        "directory_snapshot_digest_hex",
        "route_pushes",
        "excluded_routes",
        "dns_servers",
        "tunnel_addresses",
        "mtu_bytes",
        "helper_ticket_hex",
        "bytes_in",
        "bytes_out",
        "status",
    ];
    assert_strict_object_schema(schemas, "VpnSessionResponse", &session_fields, &[]);
    for (schema_name, field) in [
        ("VpnSessionResponse", "payment_tx_hash"),
        ("VpnReceiptResponse", "payment_tx_hash"),
        ("VpnReceiptResponse", "lease_id_hex"),
    ] {
        let pattern = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field))
            .and_then(Value::as_object)
            .and_then(|property| property.get("pattern"))
            .and_then(Value::as_str);
        assert_eq!(
            pattern,
            Some("^[0-9a-f]{64}$"),
            "{schema_name}.{field} must advertise canonical lowercase hex"
        );
    }
    for (schema_name, field, pattern) in [
        (
            "VpnSessionCreateRequest",
            "payment_tx_hash",
            "^(?:0[xX])?[0-9A-Fa-f]{64}$",
        ),
        (
            "VpnReceiptSubmitRequest",
            "lease_id_hex",
            "^(?:$|(?:0[xX])?[0-9A-Fa-f]{64})$",
        ),
    ] {
        let actual = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field))
            .and_then(Value::as_object)
            .and_then(|property| property.get("pattern"))
            .and_then(Value::as_str);
        assert_eq!(
            actual,
            Some(pattern),
            "{schema_name}.{field} must continue accepting mixed-case prefixed input"
        );
    }
    let helper_ticket = schemas
        .get("VpnSessionResponse")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("helper_ticket_hex"))
        .and_then(Value::as_object)
        .expect("VPN helper ticket schema");
    assert_eq!(
        helper_ticket.get("pattern").and_then(Value::as_str),
        Some("^[0-9a-f]{1328}$")
    );
    assert_eq!(
        helper_ticket.get("minLength").and_then(Value::as_u64),
        Some(1328)
    );
    assert_eq!(
        helper_ticket.get("maxLength").and_then(Value::as_u64),
        Some(1328)
    );
    let receipt_fields = [
        "session_id",
        "account_id",
        "exit_class",
        "relay_endpoint",
        "meter_family",
        "connected_at_ms",
        "disconnected_at_ms",
        "duration_ms",
        "bytes_in",
        "bytes_out",
        "status",
        "receipt_source",
        "quote_id",
        "payment_tx_hash",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "lease_fee",
        "earned_fee",
        "refunded_fee",
        "lease_id_hex",
        "settle_lease_instruction",
    ];
    assert_strict_object_schema(schemas, "VpnReceiptResponse", &receipt_fields, &[]);
    assert_strict_object_schema(schemas, "VpnReceiptListResponse", &["items", "total"], &[]);
    for (schema_name, field) in [
        ("VpnTxInstruction", "wire_id"),
        ("VpnProfileResponse", "relay_endpoint"),
        ("VpnProfileResponse", "meter_family"),
        ("VpnProfileResponse", "display_billing_label"),
        ("VpnProfileResponse", "operator_account_id"),
        ("VpnQuoteResponse", "payment_reference"),
        ("VpnQuoteResponse", "account_id"),
        ("VpnQuoteResponse", "relay_endpoint"),
        ("VpnQuoteResponse", "fee_asset_id"),
        ("VpnQuoteResponse", "escrow_account_id"),
        ("VpnQuoteResponse", "operator_account_id"),
        ("VpnQuoteResponse", "meter_family"),
        ("VpnSessionResponse", "account_id"),
        ("VpnSessionResponse", "relay_endpoint"),
        ("VpnSessionResponse", "meter_family"),
        ("VpnSessionResponse", "payment_reference"),
        ("VpnSessionResponse", "fee_asset_id"),
        ("VpnSessionResponse", "escrow_account_id"),
        ("VpnSessionResponse", "operator_account_id"),
        ("VpnReceiptResponse", "account_id"),
        ("VpnReceiptResponse", "relay_endpoint"),
        ("VpnReceiptResponse", "meter_family"),
        ("VpnReceiptResponse", "fee_asset_id"),
        ("VpnReceiptResponse", "escrow_account_id"),
        ("VpnReceiptResponse", "operator_account_id"),
    ] {
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|property| property.get("minLength"))
                .and_then(Value::as_u64),
            Some(1),
            "{schema_name}.{field} must reject empty runtime identifiers"
        );
    }
    for (schema_name, field) in [
        ("VpnQuoteResponse", "quote_expires_at_ms"),
        ("VpnSessionResponse", "expires_at_ms"),
        ("VpnSessionResponse", "connected_at_ms"),
        ("VpnSessionResponse", "bytes_in"),
        ("VpnSessionResponse", "bytes_out"),
        ("VpnReceiptResponse", "connected_at_ms"),
        ("VpnReceiptResponse", "disconnected_at_ms"),
        ("VpnReceiptResponse", "duration_ms"),
        ("VpnReceiptResponse", "bytes_in"),
        ("VpnReceiptResponse", "bytes_out"),
        ("VpnReceiptListResponse", "total"),
    ] {
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|property| property.get("minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "{schema_name}.{field} must advertise its unsigned lower bound"
        );
    }
    let quantity = schemas
        .get("Quantity")
        .and_then(Value::as_object)
        .expect("Quantity schema");
    assert_eq!(quantity.get("type").and_then(Value::as_str), Some("string"));
    for (schema_name, fee_fields) in [
        ("VpnProfileResponse", &["lease_fee"][..]),
        ("VpnQuoteResponse", &["lease_fee"][..]),
        ("VpnSessionResponse", &["lease_fee"][..]),
        (
            "VpnReceiptResponse",
            &["lease_fee", "earned_fee", "refunded_fee"][..],
        ),
    ] {
        let properties = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{schema_name} properties"));
        for field in fee_fields {
            assert_eq!(
                properties
                    .get(*field)
                    .and_then(Value::as_object)
                    .and_then(|property| property.get("$ref"))
                    .and_then(Value::as_str),
                Some("#/components/schemas/Quantity"),
                "{schema_name}.{field}"
            );
        }
    }
    for name in [
        "VpnProfileResponse",
        "VpnQuoteCreateRequest",
        "VpnQuoteResponse",
        "VpnSessionCreateRequest",
        "VpnSessionResponse",
        "VpnReceiptSubmitRequest",
        "VpnReceiptResponse",
        "VpnReceiptListResponse",
        "VpnTxInstruction",
    ] {
        assert_no_retired_vpn_fee_fields(
            schemas.get(name).unwrap_or_else(|| panic!("{name} schema")),
            name,
        );
    }
    let vpn_paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths");
    for (path, path_item) in vpn_paths {
        if path.starts_with("/v1/vpn/") {
            assert_no_retired_vpn_fee_fields(path_item, path);
        }
    }
}
#[test]
fn sorafs_tag_documents_exact_canonical_quantity_contract() {
    let tags = tags_section().as_array().expect("tags array").to_vec();
    let description = tags
        .iter()
        .find(|tag| tag.get("name").and_then(Value::as_str) == Some("SoraFS"))
        .and_then(|tag| tag.get("description"))
        .and_then(Value::as_str)
        .expect("SoraFS tag description");
    for required in [
        "canonical exact decimal JSON strings",
        "`amount`",
        "`rent_due`",
        "`reserve_shortfall`",
        "`top_up_shortfall`",
        "JSON numbers",
        "`_micro`/`_micro_xor`",
    ] {
        assert!(
            description.contains(required),
            "SoraFS quantity contract must document {required}"
        );
    }
}
#[test]
fn tags_section_includes_push_tag() {
    let tags = match tags_section() {
        Value::Array(tags) => tags,
        _ => panic!("tags section should be an array"),
    };
    let mut has_push = false;
    let mut has_soracloud = false;
    let mut has_vpn = false;
    for tag in tags {
        let Some(obj) = tag.as_object() else { continue };
        if obj.get("name").and_then(Value::as_str) == Some("Push") {
            has_push = true;
        }
        if obj.get("name").and_then(Value::as_str) == Some("Soracloud") {
            has_soracloud = true;
        }
        if obj.get("name").and_then(Value::as_str) == Some("VPN") {
            has_vpn = true;
        }
    }
    assert!(has_push, "tags should include Push");
    assert!(has_soracloud, "tags should include Soracloud");
    assert!(has_vpn, "tags should include VPN");
}
#[test]
fn detached_asset_transfer_openapi_is_strict_and_two_phase() {
    let doc = generate_spec();
    let operation = doc
        .get("paths")
        .and_then(Value::as_object)
        .and_then(|paths| paths.get("/v1/assets/transfer"))
        .and_then(Value::as_object)
        .and_then(|path| path.get("post"))
        .and_then(Value::as_object)
        .expect("detached asset transfer POST operation");
    let request_ref = operation
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    assert_eq!(
        request_ref,
        Some("#/components/schemas/AssetTransferRequest")
    );
    let response_ref = operation
        .get("responses")
        .and_then(Value::as_object)
        .and_then(|responses| responses.get("200"))
        .and_then(Value::as_object)
        .and_then(|response| response.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    assert_eq!(
        response_ref,
        Some("#/components/schemas/AssetTransferResponse")
    );
    assert!(
        operation
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(
                |description| description.contains("exact signed transaction is idempotent")
            ),
        "asset transfer operation must document exact signed replay semantics"
    );
    let schemas = doc
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("schemas");
    let request = schemas
        .get("AssetTransferRequest")
        .and_then(Value::as_object)
        .expect("asset transfer request schema");
    assert_eq!(
        request.get("additionalProperties"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        request.get("oneOf").and_then(Value::as_array).map(Vec::len),
        Some(2)
    );
    let properties = request
        .get("properties")
        .and_then(Value::as_object)
        .expect("asset transfer request properties");
    for field in [
        "authority",
        "asset_definition_id",
        "asset_balance_scope",
        "amount",
        "destination",
        "fee_payment",
        "creation_time_ms",
        "transaction_ttl_ms",
        "public_key_hex",
        "signature_base64",
    ] {
        assert!(properties.contains_key(field), "missing `{field}`");
    }
    for forbidden in [
        "private_key",
        "nonce",
        "metadata",
        "signature_b64",
        "fee_sponsor",
        "gas_asset_id",
        "gas_limit",
    ] {
        assert!(
            !properties.contains_key(forbidden),
            "legacy signing field `{forbidden}` must not be documented"
        );
    }
    assert_eq!(
        schemas
            .get("AssetTransferResponse")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("additionalProperties")),
        Some(&Value::Bool(false))
    );
    let receipt_statuses = schemas
        .get("AssetTransferReceipt")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("status"))
        .and_then(Value::as_object)
        .and_then(|status| status.get("enum"))
        .and_then(Value::as_array)
        .expect("asset transfer receipt status enum");
    for status in ["pending_signature", "submitted", "applied"] {
        assert!(
            receipt_statuses
                .iter()
                .any(|value| value.as_str() == Some(status)),
            "asset transfer receipt status enum must include `{status}`"
        );
    }
}
#[test]
fn zk_ivm_openapi_uses_compact_state_dependent_schemas() {
    let doc = generate_spec();
    let paths = doc.get("paths").and_then(Value::as_object).expect("paths");
    assert!(paths.contains_key("/v1/zk/ivm/derive"));
    let prove_post = paths
        .get("/v1/zk/ivm/prove")
        .and_then(Value::as_object)
        .and_then(|path| path.get("post"))
        .and_then(Value::as_object)
        .expect("prove post");
    let request_ref = prove_post
        .get("requestBody")
        .and_then(Value::as_object)
        .and_then(|body| body.get("content"))
        .and_then(Value::as_object)
        .and_then(|content| content.get("application/json"))
        .and_then(Value::as_object)
        .and_then(|media| media.get("schema"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("$ref"))
        .and_then(Value::as_str);
    assert_eq!(request_ref, Some("#/components/schemas/ZkIvmProveRequest"));
    assert!(prove_post.get("security").is_some(), "prove POST auth");
    assert_eq!(
        prove_post
            .get("responses")
            .and_then(Value::as_object)
            .and_then(|responses| responses.get("200"))
            .and_then(Value::as_object)
            .and_then(|response| response.get("headers"))
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cache-Control"))
            .and_then(Value::as_object)
            .and_then(|header| header.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("const"))
            .and_then(Value::as_str),
        Some("private, no-store")
    );
    let job_path = paths
        .get("/v1/zk/ivm/prove/{job_id}")
        .and_then(Value::as_object)
        .expect("prove job path");
    for method in ["get", "delete"] {
        let operation = job_path
            .get(method)
            .and_then(Value::as_object)
            .expect("prove job operation");
        let pattern = operation
            .get("parameters")
            .and_then(Value::as_array)
            .and_then(|parameters| parameters.first())
            .and_then(Value::as_object)
            .and_then(|parameter| parameter.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("pattern"))
            .and_then(Value::as_str);
        assert_eq!(pattern, Some("^[0-9a-f]{32}$"), "{method} path id");
        assert!(operation.get("security").is_some(), "{method} job auth");
        assert_eq!(
            operation
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("200"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("headers"))
                .and_then(Value::as_object)
                .and_then(|headers| headers.get("Cache-Control"))
                .and_then(Value::as_object)
                .and_then(|header| header.get("schema"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("const"))
                .and_then(Value::as_str),
            Some("private, no-store"),
            "{method} cache policy"
        );
    }
    let schemas = doc
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("schemas");
    let job = schemas
        .get("ZkIvmProveJob")
        .and_then(Value::as_object)
        .expect("job schema");
    assert_eq!(
        job.get("oneOf").and_then(Value::as_array).map(Vec::len),
        Some(3)
    );
    let done = schemas
        .get("ZkIvmProveJobDone")
        .and_then(Value::as_object)
        .expect("done schema");
    assert_eq!(done.get("additionalProperties"), Some(&Value::Bool(false)));
    let required = done
        .get("required")
        .and_then(Value::as_array)
        .expect("done required");
    assert_eq!(
        required
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["job_id", "status", "proved", "attachment"]
    );
    let proof_properties = schemas
        .get("ZkIvmCompactProof")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("compact proof properties");
    assert!(proof_properties.contains_key("bytes_b64"));
    assert!(!proof_properties.contains_key("bytes"));
    assert_eq!(
        proof_properties
            .get("bytes_b64")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("maxLength"))
            .and_then(Value::as_u64),
        Some(11_184_812)
    );
    for state in [
        "ZkIvmProveJobPendingOrRunning",
        "ZkIvmProveJobError",
        "ZkIvmProveJobDone",
    ] {
        let pattern = schemas
            .get(state)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("job_id"))
            .and_then(Value::as_object)
            .and_then(|job_id| job_id.get("pattern"))
            .and_then(Value::as_str);
        assert_eq!(pattern, Some("^[0-9a-f]{32}$"), "{state}");
    }
}
#[test]
fn retired_server_contract_deployment_paths_are_absent() {
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    for retired_path in [
        "/v1/contracts/deploy",
        "/v1/contracts/deploy-bundle",
        "/v1/contracts/deploy-bundles/{bundle_digest}",
    ] {
        assert!(
            !paths.contains_key(retired_path),
            "retired server-side contract deployment path leaked into OpenAPI: {retired_path}"
        );
    }
}
#[test]
fn governance_mutation_openapi_is_typed_closed_and_secret_free() {
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    let schemas = document
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("OpenAPI schemas");
    assert!(
        !paths.contains_key("/v1/gov/ballots/zk"),
        "the legacy ZK ballot route must not enter the first-release OpenAPI"
    );
    assert!(
        !schemas.contains_key("GovernanceZkBallotRequestV1")
            && !schemas.contains_key("GovernanceZkPublicInputsV1"),
        "legacy ZK ballot schemas must not enter the first-release OpenAPI"
    );
    let capabilities_path = iroha_torii_shared::uri::GOV_CAPABILITIES;
    let capabilities_operation = openapi_operation(&document, capabilities_path, "get");
    assert_eq!(
        operation_response_schema_ref(capabilities_operation, "200", capabilities_path),
        "#/components/schemas/GovernanceCapabilitiesV1"
    );
    let capability_properties = schemas
        .get("GovernanceCapabilitiesV1")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("governance capabilities properties");
    assert_eq!(
        capability_properties
            .get("network_id")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str),
        Some("#/components/schemas/NetworkId")
    );
    assert!(!capability_properties.contains_key("chain_id"));
    assert!(!capability_properties.contains_key("genesis_hash"));
    let cases: [(&str, &str, &[&str]); 9] = [
        (
            "/v1/ministry/agenda/proposals/draft",
            "MinistryAgendaProposalDraftRequestV1",
            &["authority", "proposal"],
        ),
        (
            "/v1/gov/proposals/deploy-contract",
            "GovernanceProposeDeployContractRequestV1",
            &[
                "abi_hash",
                "abi_version",
                "code_hash",
                "contract_address",
                "contract_alias",
                "manifest_provenance",
                "mode",
                "window",
            ],
        ),
        (
            "/v1/gov/ballots/zk-v1",
            "GovernanceZkBallotEnvelopeRequestV1",
            &[
                "amount",
                "authority",
                "backend",
                "direction",
                "duration_blocks",
                "election_id",
                "envelope_b64",
                "network_id",
                "nullifier",
                "owner",
                "root_hint",
            ],
        ),
        (
            "/v1/gov/ballots/zk-v1/ballot-proof",
            "GovernanceZkBallotProofRequestV1",
            &["authority", "ballot", "election_id", "network_id"],
        ),
        (
            "/v1/gov/ballots/plain",
            "GovernancePlainBallotRequestV1",
            &[
                "amount",
                "authority",
                "direction",
                "duration_blocks",
                "network_id",
                "owner",
                "referendum_id",
            ],
        ),
        (
            "/v1/gov/parliament/ballots",
            "GovernanceParliamentBallotRequestV1",
            &["authority", "body", "decision", "network_id", "proposal_id"],
        ),
        (
            "/v1/gov/finalize",
            "GovernanceFinalizeRequestV1",
            &["proposal_id", "referendum_id"],
        ),
        (
            "/v1/gov/enact",
            "GovernanceEnactRequestV1",
            &["proposal_id"],
        ),
        (
            "/v1/gov/protected-namespaces",
            "GovernanceProtectedNamespacesRequestV1",
            &["authority", "namespaces"],
        ),
    ];
    for path in [
        "/v1/gov/ballots/zk-v1",
        "/v1/gov/ballots/zk-v1/ballot-proof",
        "/v1/gov/ballots/plain",
        "/v1/gov/parliament/ballots",
    ] {
        let operation = openapi_operation(&document, path, "post");
        assert!(
            operation.get("x-iroha-canonical-auth-v1").is_some(),
            "POST {path} must publish canonical one-shot authentication"
        );
        let description = operation
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            description.contains("exact-network request authority"),
            "POST {path} must document exact-network authentication"
        );
        let headers = operation_header_requirements(operation);
        for expected in [
            "X-Iroha-Account",
            "X-Iroha-Signature",
            "X-Iroha-Timestamp-Ms",
            "X-Iroha-Nonce",
            "X-Iroha-Witness",
        ] {
            assert!(
                headers.iter().any(|(name, _required)| name == expected),
                "POST {path} must document canonical auth header `{expected}`"
            );
        }
    }
    for (path, schema_name, expected_properties) in cases {
        let request_ref = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|item| item.get("post"))
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("requestBody"))
            .and_then(Value::as_object)
            .and_then(|body| body.get("content"))
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|media| media.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing governance request schema for `{path}`"));
        assert_eq!(
            request_ref,
            format!("#/components/schemas/{schema_name}"),
            "{path}"
        );
        let schema = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema_name}`"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false)),
            "{schema_name}"
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema_name}` properties"));
        let mut actual_properties = properties.keys().map(String::as_str).collect::<Vec<_>>();
        actual_properties.sort_unstable();
        assert_eq!(actual_properties, expected_properties, "{schema_name}");
    }
    for (schema_name, expected_required) in [
        (
            "GovernanceProposeDeployContractRequestV1",
            &["abi_hash", "abi_version", "code_hash"][..],
        ),
        (
            "GovernanceZkBallotEnvelopeRequestV1",
            &[
                "authority",
                "backend",
                "election_id",
                "envelope_b64",
                "network_id",
            ][..],
        ),
        (
            "GovernanceZkBallotProofRequestV1",
            &["authority", "ballot", "election_id", "network_id"][..],
        ),
        (
            "GovernancePlainBallotRequestV1",
            &[
                "amount",
                "authority",
                "direction",
                "duration_blocks",
                "network_id",
                "owner",
                "referendum_id",
            ][..],
        ),
        (
            "GovernanceParliamentBallotRequestV1",
            &["authority", "body", "decision", "network_id", "proposal_id"][..],
        ),
        (
            "GovernanceFinalizeRequestV1",
            &["proposal_id", "referendum_id"][..],
        ),
        ("GovernanceEnactRequestV1", &["proposal_id"][..]),
        (
            "GovernanceProtectedNamespacesRequestV1",
            &["namespaces"][..],
        ),
        (
            "MinistryAgendaProposalDraftRequestV1",
            &["authority", "proposal"][..],
        ),
    ] {
        let schema = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema_name}`"));
        let mut actual_required = schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("missing `{schema_name}` required set"))
            .iter()
            .map(|value| value.as_str().expect("required field is a string"))
            .collect::<Vec<_>>();
        actual_required.sort_unstable();
        assert_eq!(actual_required, expected_required, "{schema_name}");
    }
    let deploy = schemas
        .get("GovernanceProposeDeployContractRequestV1")
        .and_then(Value::as_object)
        .expect("deploy request schema");
    assert_eq!(
        deploy.get("oneOf").and_then(Value::as_array).map(Vec::len),
        Some(2),
        "deploy target must be exactly one address or alias"
    );
    for target_case in deploy
        .get("oneOf")
        .and_then(Value::as_array)
        .expect("deploy target variants")
    {
        let target_case = target_case.as_object().expect("deploy target variant");
        let required = target_case
            .get("required")
            .and_then(Value::as_array)
            .and_then(|required| required.first())
            .and_then(Value::as_str)
            .expect("selected deploy target");
        assert_eq!(
            target_case
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(required))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("minLength"))
                .and_then(Value::as_u64),
            Some(1),
            "selected deploy target `{required}` must be nonempty"
        );
    }
    let deploy_properties = deploy
        .get("properties")
        .and_then(Value::as_object)
        .expect("deploy request properties");
    assert_eq!(
        deploy_properties
            .get("abi_version")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("const"))
            .and_then(Value::as_str),
        Some("1"),
        "first-release deploy requests must advertise exactly ABI V1"
    );
    assert_eq!(
        deploy_properties
            .get("mode")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum")),
        Some(&norito::json!(["Zk", "Plain", null])),
        "deploy voting mode must use the closed canonical wire labels"
    );
    for field in ["code_hash", "abi_hash"] {
        assert_eq!(
            deploy_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some(GOVERNANCE_HASH_LITERAL_PATTERN),
            "deploy `{field}` must document every accepted canonicalizable hash form"
        );
    }
    let ballot = schemas
        .get("GovernanceBallotProofV1")
        .and_then(Value::as_object)
        .expect("GovernanceBallotProofV1 schema");
    assert_eq!(
        ballot.get("additionalProperties"),
        Some(&Value::Bool(false))
    );
    let mut ballot_properties = ballot
        .get("properties")
        .and_then(Value::as_object)
        .expect("GovernanceBallotProofV1 properties")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    ballot_properties.sort_unstable();
    assert_eq!(
        ballot_properties,
        [
            "amount",
            "backend",
            "direction",
            "duration_blocks",
            "envelope_bytes",
            "nullifier",
            "owner",
            "root_hint",
        ]
    );
    let u64_maximum = Value::from(u64::MAX);
    let governance_window = schemas
        .get("GovernanceAtWindowV1")
        .and_then(Value::as_object)
        .expect("GovernanceAtWindowV1 schema");
    assert!(
        governance_window
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(
                |description| description.contains("upper") && description.contains("lower")
            ),
        "window ordering must be explicit"
    );
    for field in ["lower", "upper"] {
        assert_eq!(
            governance_window
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum")),
            Some(&u64_maximum),
            "window `{field}` must publish the exact u64 maximum"
        );
    }
    for (schema_name, field) in [
        ("GovernanceBallotProofV1", "duration_blocks"),
        ("GovernanceZkBallotEnvelopeRequestV1", "duration_blocks"),
    ] {
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maximum")),
            Some(&u64_maximum),
            "{schema_name}.{field} must publish the exact u64 maximum"
        );
    }
    assert_eq!(
        schemas
            .get("GovernancePlainBallotRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("duration_blocks"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("pattern"))
            .and_then(Value::as_str),
        Some(GOVERNANCE_U64_DECIMAL_PATTERN),
        "plain-ballot duration must publish the exact canonical u64 grammar"
    );
    for schema_name in [
        "GovernanceBallotProofV1",
        "GovernanceZkBallotEnvelopeRequestV1",
    ] {
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("backend"))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some(GOVERNANCE_EXACT_TOKEN_PATTERN),
            "{schema_name}.backend must be an exact nonempty token"
        );
    }
    for schema_name in [
        "GovernanceZkBallotEnvelopeRequestV1",
        "GovernanceZkBallotProofRequestV1",
        "GovernancePlainBallotRequestV1",
        "GovernanceParliamentBallotRequestV1",
    ] {
        let properties = schemas
            .get(schema_name)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema_name}` properties"));
        assert_eq!(
            properties
                .get("network_id")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/NetworkId"),
            "{schema_name}.network_id must use the exact typed network identity"
        );
        assert!(
            !properties.contains_key("chain_id"),
            "{schema_name} must reject the retired chain_id key"
        );
    }
    for (schema_name, field) in [
        ("GovernanceZkBallotEnvelopeRequestV1", "election_id"),
        ("GovernanceZkBallotProofRequestV1", "election_id"),
        ("GovernancePlainBallotRequestV1", "referendum_id"),
    ] {
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("pattern"))
                .and_then(Value::as_str),
            Some(GOVERNANCE_SELECTOR_V1_PATTERN),
            "{schema_name}.{field} must publish the canonical V1 selector grammar"
        );
        assert_eq!(
            schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .and_then(|properties| properties.get(field))
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maxLength"))
                .and_then(Value::as_u64),
            Some(iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_MAX_BYTES as u64),
            "{schema_name}.{field} must publish the selector byte ceiling"
        );
    }
    assert_eq!(
        schemas
            .get("GovernanceParliamentBallotRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("decision"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("enum")),
        Some(&norito::json!(["approve", "reject", "abstain"])),
        "Parliament decisions must expose only the exact lowercase wire labels"
    );
    assert_eq!(
        schemas
            .get("GovernanceEnactRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("proposal_id"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("pattern"))
            .and_then(Value::as_str),
        Some(GOVERNANCE_LOWER_HEX32_PATTERN),
        "enactment must accept only the exact committed proposal-key grammar"
    );
    for field in ["referendum_id", "proposal_id"] {
        let finalization_id = schemas
            .get("GovernanceFinalizeRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field))
            .and_then(Value::as_object)
            .expect("finalization identifier schema");
        assert_eq!(
            finalization_id.get("pattern").and_then(Value::as_str),
            Some(GOVERNANCE_LOWER_HEX32_PATTERN),
            "finalization {field} must use the exact committed proposal-key grammar"
        );
        assert_eq!(
            finalization_id.get("minLength").and_then(Value::as_u64),
            Some(64),
            "finalization {field} must publish the exact digest length"
        );
        assert_eq!(
            finalization_id.get("maxLength").and_then(Value::as_u64),
            Some(64),
            "finalization {field} must publish the exact digest length"
        );
    }
    let provenance_properties = schemas
        .get("GovernanceManifestProvenanceV1")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .expect("manifest provenance properties");
    for field in ["signer", "signature"] {
        assert_eq!(
            provenance_properties
                .get(field)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("type"))
                .and_then(Value::as_str),
            Some("string"),
            "manifest provenance `{field}` must be a typed public string"
        );
    }
    assert_eq!(
        schemas
            .get("GovernancePlainBallotRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("duration_blocks"))
            .and_then(Value::as_object)
            .and_then(|duration| duration.get("type"))
            .and_then(Value::as_str),
        Some("string")
    );
    assert_eq!(
        schemas
            .get("GovernanceZkBallotEnvelopeRequestV1")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("direction"))
            .and_then(Value::as_object)
            .and_then(|direction| direction.get("enum")),
        Some(&norito::json!(["Aye", "Nay", "Abstain", null])),
        "ZK-v1 direction must match the closed runtime ballot enum"
    );
    for schema_name in [
        "GovernanceProposeDeployContractRequestV1",
        "GovernanceZkBallotEnvelopeRequestV1",
        "GovernanceZkBallotProofRequestV1",
        "GovernanceBallotProofV1",
        "GovernancePlainBallotRequestV1",
        "GovernanceParliamentBallotRequestV1",
        "GovernanceFinalizeRequestV1",
        "GovernanceEnactRequestV1",
        "GovernanceProtectedNamespacesRequestV1",
        "MinistryAgendaProposalDraftRequestV1",
        "MinistryAgendaProposalV1",
        "MinistryAgendaProposalSummaryV1",
        "MinistryAgendaProposalTargetV1",
        "MinistryAgendaEvidenceAttachmentV1",
        "MinistryAgendaProposalSubmitterV1",
    ] {
        let encoded = norito::json::to_json(
            schemas
                .get(schema_name)
                .unwrap_or_else(|| panic!("missing `{schema_name}`")),
        )
        .expect("schema JSON");
        for forbidden in [
            "private_key",
            "privateKey",
            "private_key_hex",
            "privateKeyHex",
            "private_key_bytes",
            "privateKeyBytes",
            "private_key_seed",
            "privateKeySeed",
            "private_key_multihash",
            "privateKeyMultihash",
            "private_key_algorithm",
            "privateKeyAlgorithm",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "`{schema_name}` leaked retired signing field `{forbidden}`"
            );
        }
    }
    for schema_name in [
        "GovernanceProposeDeployContractRequestV1",
        "GovernanceFinalizeRequestV1",
        "GovernanceEnactRequestV1",
    ] {
        assert!(
            !schemas
                .get(schema_name)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .is_some_and(|properties| properties.contains_key("authority")),
            "`{schema_name}` must not restore retired server-side authority"
        );
    }
}
#[test]
fn governance_read_path_parameters_publish_exact_runtime_grammars() {
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    for (path, method, parameter_name, expected_pattern) in [
        (
            "/v1/ministry/agenda/proposals/{proposal_id}",
            "get",
            "proposal_id",
            "^AC-[0-9]{4}-[0-9]{3}$",
        ),
        (
            "/v1/gov/proposals/{id}",
            "get",
            "id",
            GOVERNANCE_LOWER_HEX32_PATTERN,
        ),
        (
            "/v1/validation-fee/proposals/{proposal_id}",
            "get",
            "proposal_id",
            GOVERNANCE_LOWER_HEX32_PATTERN,
        ),
        (
            "/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft",
            "post",
            "proposal_id",
            GOVERNANCE_LOWER_HEX32_PATTERN,
        ),
        (
            "/v1/gov/locks/{rid}",
            "get",
            "rid",
            GOVERNANCE_SELECTOR_V1_PATTERN,
        ),
        (
            "/v1/gov/referenda/{id}",
            "get",
            "id",
            GOVERNANCE_SELECTOR_V1_PATTERN,
        ),
        (
            "/v1/gov/tally/{id}",
            "get",
            "id",
            GOVERNANCE_SELECTOR_V1_PATTERN,
        ),
    ] {
        let parameters = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|item| item.get(method))
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("parameters"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("missing {method} parameters for `{path}`"));
        let pattern = parameters
            .iter()
            .filter_map(Value::as_object)
            .find(|parameter| parameter.get("name").and_then(Value::as_str) == Some(parameter_name))
            .and_then(|parameter| parameter.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("pattern"))
            .and_then(Value::as_str);
        assert_eq!(
            pattern,
            Some(expected_pattern),
            "`{path}` must publish the exact runtime selector grammar"
        );
    }
}
#[test]
fn subscription_mutations_publish_exact_unsigned_v1_draft_contract() {
    let paths = subscription_paths();
    for path in [
        "/v1/subscriptions",
        "/v1/subscriptions/{subscription_id}/pause",
        "/v1/subscriptions/{subscription_id}/resume",
        "/v1/subscriptions/{subscription_id}/cancel",
        "/v1/subscriptions/{subscription_id}/keep",
        "/v1/subscriptions/{subscription_id}/charge-now",
    ] {
        let post = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|item| item.get("post"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing subscription draft POST `{path}`"));
        let description = post
            .get("description")
            .and_then(Value::as_str)
            .expect("draft description");
        assert!(description.contains("does not sign or queue"));
    }
    let mut schemas = Map::new();
    subscription_schemas(&mut schemas);
    for request in [
        "SubscriptionCreateDraftRequestV1",
        "SubscriptionAuthorityDraftRequestV1",
        "SubscriptionChargeDraftRequestV1",
        "SubscriptionCancelDraftRequestV1",
    ] {
        let schema = schemas
            .get(request)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{request}`"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("request properties");
        assert!(!properties.contains_key("private_key"));
    }
    for response in [
        "SubscriptionCreateDraftResponseV1",
        "SubscriptionActionDraftResponseV1",
    ] {
        let schema = schemas
            .get(response)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{response}`"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("response properties");
        assert!(properties.contains_key("version"));
        assert!(properties.contains_key("authority"));
        assert!(properties.contains_key("action"));
        assert!(properties.contains_key("tx_instructions"));
        assert!(!properties.contains_key("ok"));
        assert!(!properties.contains_key("tx_hash_hex"));
    }
    let cancel_mode = schemas
        .get("SubscriptionCancelModeV1")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("oneOf"))
        .and_then(Value::as_array)
        .expect("exact cancellation mode cases");
    assert_eq!(cancel_mode.len(), 2);
}
#[test]
fn local_signing_openapi_contracts_are_closed_and_secret_free() {
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    for path in [
        "/v1/contracts/aliases",
        "/v1/proofs/query",
        "/v1/space-directory/manifests",
        "/v1/space-directory/manifests/revoke",
        "/v1/subscriptions/plans",
        "/v1/subscriptions/{subscription_id}/usage",
    ] {
        assert!(
            paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|item| item.get("post"))
                .is_some(),
            "missing local-signing POST `{path}`",
        );
    }
    let schemas = document
        .get("components")
        .and_then(Value::as_object)
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("OpenAPI schemas");
    for request in [
        "ProofFindByIdSignedQueryRequestV1",
        "ContractAliasDraftRequestV1",
        "SpaceDirectoryManifestPublishDraftRequestV1",
        "SpaceDirectoryManifestRevokeDraftRequestV1",
        "SubscriptionPlanDraftRequestV1",
        "SubscriptionUsageDraftRequestV1",
    ] {
        let schema = schemas
            .get(request)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{request}`"));
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&Value::Bool(false))
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("request properties");
        assert!(
            !properties.contains_key("private_key"),
            "`{request}` must not advertise private_key",
        );
    }
}
#[test]
fn da_proof_openapi_contracts_match_exact_norito_json_wire_shapes() {
    fn operation_responses<'a>(paths: &'a Map, path: &str) -> &'a Map {
        paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|item| item.get("post"))
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("responses"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing DA POST responses for `{path}`"))
    }
    fn schema_properties<'a>(schemas: &'a Map, schema: &str) -> &'a Map {
        schemas
            .get(schema)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema}` properties"))
    }
    fn operation_schema_ref<'a>(paths: &'a Map, path: &str, request: bool) -> &'a str {
        let operation = paths
            .get(path)
            .and_then(Value::as_object)
            .and_then(|item| item.get("post"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing DA POST `{path}`"));
        let media = if request {
            operation
                .get("requestBody")
                .and_then(Value::as_object)
                .and_then(|body| body.get("content"))
        } else {
            operation
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|responses| responses.get("200"))
                .and_then(Value::as_object)
                .and_then(|response| response.get("content"))
        };
        media
            .and_then(Value::as_object)
            .and_then(|content| content.get("application/json"))
            .and_then(Value::as_object)
            .and_then(|json| json.get("schema"))
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("$ref"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing DA schema reference for `{path}`"))
    }
    let document = generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/commitments", true),
        "#/components/schemas/DaCommitmentListRequest"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/commitments", false),
        "#/components/schemas/DaCommitmentListResponse"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/commitments/prove", true),
        "#/components/schemas/DaCommitmentProofRequest"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/commitments/prove", false),
        "#/components/schemas/DaCommitmentProofResponse"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/commitments/verify", true),
        "#/components/schemas/DaCommitmentProof"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/pin-intents", true),
        "#/components/schemas/DaPinIntentListRequest"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/pin-intents", false),
        "#/components/schemas/DaPinIntentListResponse"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/pin-intents/prove", true),
        "#/components/schemas/DaPinIntentQueryRequest"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/pin-intents/prove", false),
        "#/components/schemas/DaPinIntentProofResponse"
    );
    assert_eq!(
        operation_schema_ref(paths, "/v1/da/pin-intents/verify", true),
        "#/components/schemas/DaPinIntentProof"
    );
    for (path, method, operation_id) in [
        ("/v1/da/proof-policies", "get", "daProofPoliciesList"),
        (
            "/v1/da/proof-policies/snapshot",
            "get",
            "daProofPolicySnapshot",
        ),
        ("/v1/da/pin-intents", "post", "daPinIntentsList"),
        ("/v1/da/pin-intents/prove", "post", "daPinIntentsProve"),
        ("/v1/da/pin-intents/verify", "post", "daPinIntentsVerify"),
    ] {
        assert_eq!(
            paths
                .get(path)
                .and_then(Value::as_object)
                .and_then(|item| item.get(method))
                .and_then(Value::as_object)
                .and_then(|operation| operation.get("operationId"))
                .and_then(Value::as_str),
            Some(operation_id),
            "{path}"
        );
    }
    for path in [
        "/v1/da/commitments",
        "/v1/da/commitments/prove",
        "/v1/da/commitments/verify",
        "/v1/da/pin-intents",
        "/v1/da/pin-intents/prove",
        "/v1/da/pin-intents/verify",
    ] {
        let responses = operation_responses(paths, path);
        assert!(responses.contains_key("400"), "{path} malformed JSON");
        assert!(responses.contains_key("413"), "{path} 64 KiB body limit");
    }
    for path in ["/v1/da/commitments", "/v1/da/pin-intents"] {
        assert!(
            operation_responses(paths, path).contains_key("409"),
            "{path} mid-read snapshot change"
        );
    }
    for path in [
        "/v1/da/commitments/prove",
        "/v1/da/commitments/verify",
        "/v1/da/pin-intents/prove",
        "/v1/da/pin-intents/verify",
    ] {
        assert!(
            !operation_responses(paths, path).contains_key("409"),
            "{path} does not issue list snapshots"
        );
    }
    assert!(
        operation_responses(paths, "/v1/da/pin-intents/prove")
            .get("400")
            .and_then(Value::as_object)
            .and_then(|response| response.get("description"))
            .and_then(Value::as_str)
            .is_some_and(|description| description.contains("256 UTF-8 bytes")),
        "pin-intent proof errors must document the alias byte limit"
    );
    let schemas = component_schemas(&document);
    assert!(
        !schemas.contains_key("DaPagination"),
        "offset pagination must not remain in the first-release DA list contract"
    );
    assert!(
        !schemas.contains_key("DaCommitmentWithLocationList"),
        "the commitment list route uses its cursor-bearing page envelope"
    );
    assert!(
        !schemas.contains_key("DaPinIntentWithLocationList"),
        "the pin-intent list route uses its cursor-bearing page envelope"
    );
    for (request, cursor) in [
        ("DaCommitmentListRequest", "DaCommitmentListCursor"),
        ("DaPinIntentListRequest", "DaPinIntentListCursor"),
    ] {
        let request_schema = schemas
            .get(request)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{request}` schema"));
        assert_eq!(
            request_schema
                .get("additionalProperties")
                .and_then(Value::as_bool),
            Some(false),
            "{request}"
        );
        let properties = schema_properties(schemas, request);
        let mut property_names = properties.keys().map(String::as_str).collect::<Vec<_>>();
        property_names.sort_unstable();
        assert_eq!(property_names, vec!["cursor", "limit"], "{request}");
        let limit = properties
            .get("limit")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{request}.limit` schema"));
        assert_eq!(limit.get("minimum").and_then(Value::as_u64), Some(1));
        assert!(
            !limit.contains_key("maximum"),
            "{request}.limit accepts the full nonzero u64 range before server capping"
        );
        assert_eq!(limit.get("default").and_then(Value::as_u64), Some(100));
        assert!(
            limit
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|description| description.contains("capped at 1,000")),
            "{request}.limit must document the raw-row cap"
        );
        let expected_cursor_ref = format!("#/components/schemas/{cursor}");
        assert_eq!(
            properties
                .get("cursor")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(expected_cursor_ref.as_str()),
            "{request}"
        );
    }
    for (request, expected) in [
        (
            "DaCommitmentProofRequest",
            vec!["epoch", "lane_id", "manifest_hash", "sequence"],
        ),
        (
            "DaPinIntentQueryRequest",
            vec![
                "alias",
                "epoch",
                "lane_id",
                "manifest_hash",
                "sequence",
                "storage_ticket",
            ],
        ),
    ] {
        let mut property_names = schema_properties(schemas, request)
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        property_names.sort_unstable();
        assert_eq!(property_names, expected, "{request}");
    }
    let snapshot = schemas
        .get("DaListSnapshot")
        .and_then(Value::as_object)
        .expect("DA list snapshot schema");
    assert_eq!(
        snapshot
            .get("required")
            .and_then(Value::as_array)
            .map(|required| required
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()),
        Some(vec!["block_height", "block_hash"])
    );
    assert_eq!(
        snapshot
            .get("oneOf")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "empty and non-empty canonical snapshot shapes"
    );
    for (cursor, after) in [
        ("DaCommitmentListCursor", "DaCommitmentKey"),
        ("DaPinIntentListCursor", "DaCommitmentLocation"),
    ] {
        let properties = schema_properties(schemas, cursor);
        assert_eq!(
            properties
                .get("snapshot")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/DaListSnapshot"),
            "{cursor}"
        );
        let expected_after_ref = format!("#/components/schemas/{after}");
        assert_eq!(
            properties
                .get("after")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(expected_after_ref.as_str()),
            "{cursor}"
        );
    }
    for (response, items, cursor) in [
        (
            "DaCommitmentListResponse",
            "commitments",
            "DaCommitmentListCursor",
        ),
        (
            "DaPinIntentListResponse",
            "intents",
            "DaPinIntentListCursor",
        ),
    ] {
        let response_schema = schemas
            .get(response)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{response}` schema"));
        assert!(
            response_schema
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| {
                    required
                        .iter()
                        .any(|field| field.as_str() == Some("next_cursor"))
                }),
            "{response}.next_cursor must be explicit, including null"
        );
        assert!(
            schema_properties(schemas, response).contains_key(items),
            "{response}.{items}"
        );
        let next_cursor = schema_properties(schemas, response)
            .get("next_cursor")
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("anyOf"))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("missing `{response}.next_cursor` union"));
        let expected_cursor_ref = format!("#/components/schemas/{cursor}");
        assert_eq!(
            next_cursor
                .first()
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some(expected_cursor_ref.as_str()),
            "{response}"
        );
        assert_eq!(
            next_cursor
                .get(1)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("type"))
                .and_then(Value::as_str),
            Some("null"),
            "{response}"
        );
    }
    let digest = schemas
        .get("DaDigest32")
        .and_then(Value::as_object)
        .expect("DA digest schema");
    assert_eq!(digest.get("minItems").and_then(Value::as_u64), Some(1));
    assert_eq!(digest.get("maxItems").and_then(Value::as_u64), Some(1));
    let digest_bytes = digest
        .get("items")
        .and_then(Value::as_object)
        .expect("DA digest byte-array schema");
    assert_eq!(
        digest_bytes.get("minItems").and_then(Value::as_u64),
        Some(32)
    );
    assert_eq!(
        digest_bytes.get("maxItems").and_then(Value::as_u64),
        Some(32)
    );
    assert!(
        !schemas.contains_key("DaBytes48"),
        "removed KZG wire values must not remain in the first-release schema"
    );
    let proof_scheme = schemas
        .get("DaTaggedProofScheme")
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("type"))
        .and_then(Value::as_object)
        .and_then(|schema| schema.get("enum"))
        .and_then(Value::as_array)
        .expect("DA proof-scheme enum");
    assert_eq!(
        proof_scheme
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        vec!["MerkleSha256"]
    );
    let commitment_record = schemas
        .get("DaCommitmentRecord")
        .and_then(Value::as_object)
        .expect("DA commitment record schema");
    assert!(
        !commitment_record
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key("kzg_commitment")),
        "removed KZG commitments must not remain in DA record properties"
    );
    assert!(
        !commitment_record
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                required
                    .iter()
                    .any(|field| field.as_str() == Some("kzg_commitment"))
            }),
        "removed KZG commitments must not remain in DA record requirements"
    );
    let direction = schemas
        .get("MerkleDirection")
        .and_then(Value::as_object)
        .expect("DA Merkle direction schema");
    assert_eq!(
        direction.get("type").and_then(Value::as_str),
        Some("object")
    );
    assert_eq!(
        direction
            .get("required")
            .and_then(Value::as_array)
            .map(|required| required
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()),
        Some(vec!["direction", "value"])
    );
    for proof in ["DaCommitmentProof", "DaPinIntentProof"] {
        let properties = schemas
            .get(proof)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{proof}` properties"));
        assert_eq!(
            properties
                .get("bundle_hash")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("allOf"))
                .and_then(Value::as_array)
                .and_then(|schemas| schemas.first())
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("$ref"))
                .and_then(Value::as_str),
            Some("#/components/schemas/DaIrohaHash")
        );
        assert!(
            properties
                .get("bundle_hash")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("description"))
                .and_then(Value::as_str)
                .is_some_and(|description| description.contains("tree version")),
            "{proof}.bundle_hash must describe the committed tree descriptor"
        );
        assert_eq!(
            properties
                .get("path")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("maxItems"))
                .and_then(Value::as_u64),
            Some(32)
        );
    }
    for (schema, field) in [
        ("DaPinIntentQueryRequest", "alias"),
        ("DaPinIntent", "alias"),
    ] {
        let alias = schemas
            .get(schema)
            .and_then(Value::as_object)
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(field))
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing `{schema}.{field}` schema"));
        assert_eq!(alias.get("maxLength").and_then(Value::as_u64), Some(256));
        assert_eq!(
            alias.get("x-iroha-maxUtf8Bytes").and_then(Value::as_u64),
            Some(256)
        );
    }
    for verify in ["DaCommitmentVerifyResponse", "DaPinIntentVerifyResponse"] {
        assert_eq!(
            schemas
                .get(verify)
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("oneOf"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2),
            "{verify}"
        );
    }
}
