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
            "account_id": TEST_ACCOUNT_I105
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
        "account_id": TEST_ACCOUNT_I105
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
            "account_id": TEST_ACCOUNT_I105
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
fn sponsored_prepare_tools_are_read_only_and_submit_tools_are_write() {
    assert_eq!(iroha_accounts_onboard_plan_tool().effect, ToolEffect::Read);
    assert_eq!(
        iroha_accounts_onboard_prepare_tool().effect,
        ToolEffect::Read
    );
    assert_eq!(
        iroha_accounts_onboard_submit_tool().effect,
        ToolEffect::Write
    );
    assert_eq!(
        iroha_accounts_faucet_prepare_tool().effect,
        ToolEffect::Read
    );
    assert_eq!(
        iroha_accounts_faucet_submit_tool().effect,
        ToolEffect::Write
    );
}
#[test]
fn sponsored_account_tool_descriptors_require_one_exact_body_shape() {
    for tool in [
        iroha_accounts_onboard_plan_tool(),
        iroha_accounts_onboard_prepare_tool(),
        iroha_accounts_onboard_submit_tool(),
        iroha_accounts_faucet_prepare_tool(),
        iroha_accounts_faucet_submit_tool(),
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
fn build_accounts_onboard_plan_body_requires_version_alias_and_account() {
    let args = norito::json!({
        "body": {
            "version": 1,
            "alias": "alice@universal"
        }
    });
    let error = build_accounts_onboard_plan_body(args.as_object().expect("object"))
        .expect_err("missing account");
    assert!(error.contains("account_id"));
}
#[test]
fn build_accounts_onboard_prepare_requires_schema_binding_and_receipt() {
    let args = norito::json!({
        "body": {
            "schema": "iroha.accounts.onboard.prepare.v1",
            "binding": {},
            "receipt": {}
        }
    });
    let body = build_accounts_onboard_prepare_body(args.as_object().expect("object"))
        .expect("prepare body");
    let body = materialize_borrowed_body(&body);
    assert!(body.as_object().is_some_and(|body| {
        body.contains_key("schema") && body.contains_key("binding") && body.contains_key("receipt")
    }));
    let old = norito::json!({ "receipt": {} });
    let error = build_accounts_onboard_prepare_body(old.as_object().expect("object"))
        .expect_err("old one-shot shape");
    assert!(error.contains("schema") || error.contains("binding"));
}
#[test]
fn prepared_submit_builders_reject_old_one_shot_shapes() {
    let old_onboarding = norito::json!({ "receipt": {} });
    let error = build_accounts_onboard_submit_body(old_onboarding.as_object().expect("object"))
        .expect_err("old onboarding apply shape");
    assert!(error.contains("schema"));

    let old_faucet = norito::json!({ "account_id": TEST_ACCOUNT_I105 });
    let error = build_accounts_faucet_submit_body(old_faucet.as_object().expect("object"))
        .expect_err("old faucet claim shape");
    assert!(error.contains("unsupported") || error.contains("schema"));
}
#[test]
fn build_accounts_faucet_prepare_requires_closed_shape() {
    let args = norito::json!({
        "body": {
            "schema": "iroha.accounts.faucet.prepare.v1",
            "binding": {},
            "claim": { "account_id": TEST_ACCOUNT_I105 }
        }
    });
    let body = build_accounts_faucet_prepare_body(args.as_object().expect("object"))
        .expect("faucet prepare body");
    let body = materialize_borrowed_body(&body);
    assert!(body.as_object().is_some_and(|body| {
        body.contains_key("schema") && body.contains_key("binding") && body.contains_key("claim")
    }));
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
