// MCP canonical request-body builder regressions.
fn materialize_borrowed_body(body: &BorrowedMcpJson<'_>) -> Value {
    let encoded = encode_mcp_json_body(body, "encode borrowed test body").expect("encoded body");
    json::from_slice(&encoded).expect("decoded body")
}

#[test]
fn build_query_envelope_body_collects_shortcut_fields() {
    let args = norito::json!({
        "filter": { "op": "eq", "args": ["authority", TEST_ACCOUNT_I105] },
        "aggregate": {
            "group_by": ["primary_alias_domain"],
            "metrics": [
                { "alias": "holder_count", "fn": "count" }
            ]
        },
        "limit": 25,
        "offset": 5,
        "fetch_size": 10
    });
    let body = build_query_envelope_body(args.as_object().expect("object")).expect("body");
    let body = materialize_borrowed_body(&body);
    let body = body.as_object().expect("body object");
    assert!(body.contains_key("filter"));
    assert!(body.contains_key("aggregate"));
    let pagination = body
        .get("pagination")
        .and_then(Value::as_object)
        .expect("pagination");
    assert_eq!(pagination.get("limit").and_then(Value::as_u64), Some(25));
    assert_eq!(pagination.get("offset").and_then(Value::as_u64), Some(5));
    assert_eq!(body.get("fetch_size").and_then(Value::as_u64), Some(10));
}
#[test]
fn build_query_envelope_body_rejects_non_object_body() {
    let args = norito::json!({
        "body": "invalid"
    });
    let err = build_query_envelope_body(args.as_object().expect("object")).expect_err("error");
    assert!(err.contains("`body` must be an object"));
}
#[test]
fn build_accounts_onboard_plan_body_accepts_only_secret_free_intent() {
    let args = norito::json!({
        "body": {
            "version": 1,
            "alias": "alice@universal",
            "account_id": TEST_ACCOUNT_I105,
            "permissions": []
        }
    });
    let body =
        build_accounts_onboard_plan_body(args.as_object().expect("object")).expect("plan body");
    let body = materialize_borrowed_body(&body);
    let body = body.as_object().expect("object");
    assert_eq!(body.get("version").and_then(Value::as_u64), Some(1));
    assert_eq!(
        body.get("alias").and_then(Value::as_str),
        Some("alice@universal")
    );
    assert_eq!(
        body.get("account_id").and_then(Value::as_str),
        Some(TEST_ACCOUNT_I105)
    );
}
#[test]
fn sponsored_account_tools_reject_flat_and_dual_request_shapes() {
    let flat = norito::json!({
        "version": 1,
        "alias": "alice@universal",
        "account_id": TEST_ACCOUNT_I105,
        "permissions": []
    });
    assert!(
        build_accounts_onboard_plan_body(flat.as_object().expect("object"))
            .expect_err("flat onboarding plan shape")
            .contains("body")
    );
    let dual = norito::json!({
        "body": {
            "version": 1,
            "alias": "alice@universal",
            "account_id": TEST_ACCOUNT_I105,
            "permissions": []
        },
        "alias": "substituted@universal"
    });
    assert!(
        build_accounts_onboard_plan_body(dual.as_object().expect("object"))
            .expect_err("dual onboarding plan shape")
            .contains("only in `body`")
    );
}
#[test]
fn sponsored_signing_and_submit_tools_are_not_read_only() {
    assert_eq!(iroha_accounts_onboard_plan_tool().effect, ToolEffect::Read);
    assert_eq!(
        iroha_accounts_onboard_prepare_tool().effect,
        ToolEffect::Write
    );
    assert_eq!(
        iroha_accounts_onboard_submit_tool().effect,
        ToolEffect::Write
    );
}
#[test]
fn sponsored_account_tool_descriptors_require_one_exact_body_shape() {
    for tool in [
        iroha_accounts_onboard_plan_tool(),
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        let schema = tool.input_schema.as_object().expect("tool input object");
        assert!(schema.get("oneOf").is_none(), "{} has a union", tool.name);
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        assert_eq!(required.as_slice(), &[Value::String("body".to_owned())]);
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object");
        assert!(properties.contains_key("body"));
        assert!(
            properties
                .keys()
                .all(|field| matches!(field.as_str(), "body" | "headers" | "accept"))
        );
    }
}
#[test]
fn sponsored_account_tool_descriptors_require_v1_permissions_fee_and_pow_fields() {
    fn required_body_fields(tool: ToolSpec) -> Vec<String> {
        tool.input_schema["properties"]["body"]["required"]
            .as_array()
            .expect("body required array")
            .iter()
            .map(|field| field.as_str().expect("required field string").to_owned())
            .collect()
    }

    assert!(
        required_body_fields(iroha_accounts_onboard_plan_tool())
            .iter()
            .any(|field| field == "permissions")
    );
    for tool in [iroha_accounts_onboard_prepare_tool()] {
        assert!(
            required_body_fields(tool)
                .iter()
                .any(|field| field == "fee_payment")
        );
    }
}
#[test]
fn sponsored_account_tool_descriptors_close_nested_v1_objects() {
    fn assert_closed_object(schema: &Value, expected_fields: &[&str], context: &str) {
        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{context} must reject unknown fields"
        );
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{context} required array"));
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{context} properties object"));
        assert_eq!(
            required.len(),
            expected_fields.len(),
            "{context} required set"
        );
        assert_eq!(
            properties.len(),
            expected_fields.len(),
            "{context} property set"
        );
        for field in expected_fields {
            assert!(
                required
                    .iter()
                    .any(|required| required.as_str() == Some(field)),
                "{context} omitted required field {field}"
            );
            assert!(
                properties.contains_key(*field),
                "{context} omitted property {field}"
            );
        }
    }

    let binding_fields = [
        "schema",
        "authorization_sha256",
        "authorization_nonce",
        "kind",
        "phase",
        "idempotency_key",
        "execution_expires_at_unix_ms",
    ];
    for tool in [
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        let binding = &tool.input_schema["properties"]["body"]["properties"]["binding"];
        assert_closed_object(binding, &binding_fields, &format!("{} binding", tool.name));
    }

    let receipt_fields = ["body", "plan_hash", "signature"];
    let receipt_body_fields = [
        "version",
        "request",
        "authority",
        "network_id",
        "anchor",
        "resource",
        "acquisition",
        "quote_guard",
        "instructions",
        "owner_auto_renew_instruction",
        "valid_until_ms",
    ];
    for tool in [
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        let receipt = &tool.input_schema["properties"]["body"]["properties"]["receipt"];
        assert_closed_object(receipt, &receipt_fields, &format!("{} receipt", tool.name));
        assert_closed_object(
            &receipt["properties"]["body"],
            &receipt_body_fields,
            &format!("{} receipt body", tool.name),
        );
    }
}
#[test]
fn prepared_account_tool_descriptors_have_no_untyped_schema_holes() {
    fn assert_exact_schema(schema: &Value, path: &str) {
        let object = schema
            .as_object()
            .unwrap_or_else(|| panic!("{path} schema must be an object"));
        assert!(!object.is_empty(), "{path} must not admit arbitrary JSON");

        if object.get("type").and_then(Value::as_str) == Some("object") {
            assert_eq!(
                object.get("additionalProperties").and_then(Value::as_bool),
                Some(false),
                "{path} object must reject unknown fields"
            );
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{path} object properties"));
            let required = object
                .get("required")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{path} object required fields"));
            assert_eq!(
                required.len(),
                properties.len(),
                "{path} must require every first-release object slot"
            );
            for (field, child) in properties {
                assert!(
                    required
                        .iter()
                        .any(|required| required.as_str() == Some(field)),
                    "{path}.{field} must be required"
                );
                assert_exact_schema(child, &format!("{path}.{field}"));
            }
        }

        if let Some(items) = object.get("items") {
            assert_exact_schema(items, &format!("{path}.items"));
        }
        for keyword in ["oneOf", "anyOf", "allOf"] {
            if let Some(branches) = object.get(keyword).and_then(Value::as_array) {
                assert!(!branches.is_empty(), "{path}.{keyword} must not be empty");
                for (index, branch) in branches.iter().enumerate() {
                    assert_exact_schema(branch, &format!("{path}.{keyword}[{index}]"));
                }
            }
        }
    }

    for tool in [
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        assert_eq!(
            tool.input_schema[MCP_STRICT_BODY_SCHEMA_EXTENSION].as_bool(),
            Some(true),
            "{} must preserve closed typed bodies in tools/list",
            tool.name
        );
        let body = &tool.input_schema["properties"]["body"];
        assert_exact_schema(body, &format!("{}.body", tool.name));

        let advertised = tool.descriptor();
        assert_eq!(
            advertised["inputSchema"]["properties"]["body"]["additionalProperties"].as_bool(),
            Some(false),
            "{} advertised body must stay closed",
            tool.name
        );
    }
}
#[test]
fn prepared_account_tool_descriptors_encode_tagged_unions_and_required_null_slots() {
    fn has_null_branch(schema: &Value) -> bool {
        schema
            .get("oneOf")
            .and_then(Value::as_array)
            .is_some_and(|branches| {
                branches
                    .iter()
                    .any(|branch| branch.get("type").and_then(Value::as_str) == Some("null"))
            })
    }

    for tool in [
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        let properties = &tool.input_schema["properties"]["body"]["properties"];
        let receipt_body = &properties["receipt"]["properties"]["body"];
        let resource = &receipt_body["properties"]["resource"]["properties"];
        assert_eq!(
            resource["intent"]["properties"]["kind"]["const"].as_str(),
            Some("account_alias")
        );
        assert!(has_null_branch(&resource["quote"]));
        assert!(has_null_branch(&resource["instruction_index"]));
        assert!(has_null_branch(
            &receipt_body["properties"]["acquisition"]["properties"]["pricing_class_hint"]
        ));
        assert!(has_null_branch(
            &receipt_body["properties"]["owner_auto_renew_instruction"]
        ));
    }

    for tool in [
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
    ] {
        let fee = &tool.input_schema["properties"]["body"]["properties"]["fee_payment"];
        let branches = fee["oneOf"].as_array().expect("fee payer union");
        assert_eq!(branches.len(), 2);
        let payers = branches
            .iter()
            .map(|branch| {
                branch["properties"]["payer"]["const"]
                    .as_str()
                    .expect("fee payer const")
            })
            .collect::<Vec<_>>();
        assert_eq!(payers, ["authority", "sponsor"]);
        for branch in branches {
            let value = &branch["properties"]["value"];
            assert!(has_null_branch(&value["properties"]["gas_limit"]));
            let limit = &value["properties"]["charge_limits"]["items"];
            assert_eq!(
                limit["properties"]["kind"]["properties"]["value"]["type"].as_str(),
                Some("null")
            );
        }
    }
}
#[test]
fn build_accounts_onboard_plan_body_rejects_secret_and_legacy_fields() {
    for forbidden in ["private_key", "token", "uaid", "identity_commitment_hex"] {
        let mut args = norito::json!({
            "body": {
                "version": 1,
                "alias": "alice@universal",
                "account_id": TEST_ACCOUNT_I105
            }
        });
        args.as_object_mut()
            .expect("object")
            .insert(forbidden.to_owned(), Value::String("secret".to_owned()));
        let error = build_accounts_onboard_plan_body(args.as_object().expect("object"))
            .expect_err("forbidden field");
        assert!(error.contains(forbidden));
    }
}
#[test]
fn build_accounts_onboard_plan_body_requires_all_v1_fields() {
    let args = norito::json!({
        "body": {
            "version": 1,
            "alias": "alice@universal"
        }
    });
    let error = build_accounts_onboard_plan_body(args.as_object().expect("object"))
        .expect_err("missing account");
    assert!(error.contains("account_id"));

    let missing_permissions = norito::json!({
        "body": {
            "version": 1,
            "alias": "alice@universal",
            "account_id": TEST_ACCOUNT_I105
        }
    });
    let error = build_accounts_onboard_plan_body(missing_permissions.as_object().expect("object"))
        .expect_err("missing permissions");
    assert!(error.contains("permissions"));
}
#[test]
fn build_accounts_onboard_prepare_requires_schema_binding_receipt_and_fee() {
    let args = norito::json!({
        "body": {
            "schema": "iroha.accounts.onboard.prepare.v1",
            "binding": {},
            "receipt": {},
            "fee_payment": {}
        }
    });
    let body = build_accounts_onboard_prepare_body(args.as_object().expect("object"))
        .expect("prepare body");
    let body = materialize_borrowed_body(&body);
    assert!(body.as_object().is_some_and(|body| {
        body.contains_key("schema")
            && body.contains_key("binding")
            && body.contains_key("receipt")
            && body.contains_key("fee_payment")
    }));
    let missing_fee = norito::json!({
        "body": {
            "schema": "iroha.accounts.onboard.prepare.v1",
            "binding": {},
            "receipt": {}
        }
    });
    let error = build_accounts_onboard_prepare_body(missing_fee.as_object().expect("object"))
        .expect_err("missing fee payment");
    assert!(error.contains("fee_payment"));
    let null_fee = norito::json!({
        "body": {
            "schema": "iroha.accounts.onboard.prepare.v1",
            "binding": {},
            "receipt": {},
            "fee_payment": null
        }
    });
    let error = build_accounts_onboard_prepare_body(null_fee.as_object().expect("object"))
        .expect_err("null fee payment");
    assert!(error.contains("fee_payment"));
    let old = norito::json!({ "receipt": {} });
    let error = build_accounts_onboard_prepare_body(old.as_object().expect("object"))
        .expect_err("old one-shot shape");
    assert!(error.contains("only in `body`"));
}
#[test]
fn prepared_submit_builders_reject_old_one_shot_shapes() {
    let old_onboarding = norito::json!({ "receipt": {} });
    let error = build_accounts_onboard_submit_body(old_onboarding.as_object().expect("object"))
        .expect_err("old onboarding apply shape");
    assert!(error.contains("only in `body`"));
}
#[test]
fn build_object_body_or_default_uses_empty_object_when_missing() {
    let args = norito::json!({
        "headers": { "x-test": "1" }
    });
    let body = build_object_body_or_default(args.as_object().expect("object")).expect("body");
    let body = materialize_borrowed_body(&body);
    assert_eq!(
        body,
        Value::Object(Map::new()),
        "missing body should default to empty object"
    );
}
#[test]
fn build_object_body_or_default_rejects_non_object_body() {
    let args = norito::json!({
        "body": "invalid"
    });
    let err = build_object_body_or_default(args.as_object().expect("object"))
        .expect_err("should reject non-object body");
    assert!(err.contains("`body` must be an object"));
}
#[test]
fn build_object_body_or_flat_shortcuts_collects_top_level_fields() {
    let args = norito::json!({
        "authority": TEST_ACCOUNT_I105,
        "namespace": "nexus",
        "headers": { "x-test": "1" }
    });
    let body = build_object_body_or_flat_shortcuts(
        args.as_object().expect("object"),
        &["body", "headers", "accept"],
    )
    .expect("body");
    let body = materialize_borrowed_body(&body);
    let body = body.as_object().expect("object");
    assert_eq!(
        body.get("authority").and_then(Value::as_str),
        Some(TEST_ACCOUNT_I105)
    );
    assert_eq!(body.get("namespace").and_then(Value::as_str), Some("nexus"));
    assert!(body.get("headers").is_none());
}
#[test]
fn build_object_body_or_flat_shortcuts_rejects_missing_body_and_shortcuts() {
    let args = norito::json!({
        "headers": { "x-test": "1" }
    });
    let err = build_object_body_or_flat_shortcuts(
        args.as_object().expect("object"),
        &["body", "headers", "accept"],
    )
    .expect_err("should reject empty payload");
    assert!(err.contains("`body` is required"));
}
