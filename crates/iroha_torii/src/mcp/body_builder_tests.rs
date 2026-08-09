// MCP canonical request-body builder regressions.

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
        "version": 1,
        "alias": "alice@universal",
        "account_id": TEST_ACCOUNT_I105
    });
    let body =
        build_accounts_onboard_plan_body(args.as_object().expect("object")).expect("plan body");
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
fn sponsored_onboarding_plan_is_read_only_and_apply_is_write() {
    assert_eq!(iroha_accounts_onboard_plan_tool().effect, ToolEffect::Read);
    assert_eq!(iroha_accounts_onboard_tool().effect, ToolEffect::Write);
}

#[test]
fn build_accounts_onboard_plan_body_rejects_secret_and_legacy_fields() {
    for forbidden in ["private_key", "token", "uaid", "identity_commitment_hex"] {
        let mut args = norito::json!({
            "version": 1,
            "alias": "alice@universal",
            "account_id": TEST_ACCOUNT_I105
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
        "version": 1,
        "alias": "alice@universal"
    });
    let error = build_accounts_onboard_plan_body(args.as_object().expect("object"))
        .expect_err("missing account");
    assert!(error.contains("account_id"));
}

#[test]
fn build_accounts_onboard_apply_body_accepts_only_receipt() {
    let args = norito::json!({ "receipt": { "body": {}, "plan_hash": "hash", "signature": {} } });
    let body =
        build_accounts_onboard_apply_body(args.as_object().expect("object")).expect("apply body");
    assert!(
        body.as_object()
            .is_some_and(|body| body.contains_key("receipt"))
    );

    let forbidden = norito::json!({ "receipt": {}, "private_key": "secret" });
    let error = build_accounts_onboard_apply_body(forbidden.as_object().expect("object"))
        .expect_err("forbidden key");
    assert!(error.contains("private_key"));
}

#[test]
fn build_accounts_faucet_body_collects_shortcut_field() {
    let args = norito::json!({
        "account_id": TEST_ACCOUNT_I105
    });
    let body = build_accounts_faucet_body(args.as_object().expect("object")).expect("body");
    let body = body.as_object().expect("object");
    assert_eq!(
        body.get("account_id").and_then(Value::as_str),
        Some(TEST_ACCOUNT_I105)
    );
}

#[test]
fn build_accounts_faucet_body_rejects_missing_account_id() {
    let args = norito::json!({
        "headers": { "x-test": "1" }
    });
    let err = build_accounts_faucet_body(args.as_object().expect("object")).expect_err("error");
    assert!(err.contains("`account_id` is required"));
}

#[test]
fn build_object_body_or_default_uses_empty_object_when_missing() {
    let args = norito::json!({
        "headers": { "x-test": "1" }
    });
    let body = build_object_body_or_default(args.as_object().expect("object")).expect("body");
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
