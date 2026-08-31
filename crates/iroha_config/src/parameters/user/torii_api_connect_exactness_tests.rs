// Focused tests for exact Torii App API, proof, API-token, and Connect configuration.
fn exactness_torii_table_mut(table: &mut Table) -> &mut Table {
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
}

fn exactness_connect_table_mut(table: &mut Table) -> &mut Table {
    exactness_torii_table_mut(table)
        .entry("connect")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("torii.connect table")
}

fn exactness_rejection(table: Table, expected: &str) {
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("invalid exact Torii configuration must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains(expected),
        "expected {expected:?} in {report}"
    );
}

fn exactness_token(index: usize) -> String {
    format!("{index:032x}")
}

#[test]
fn torii_app_api_limits_reject_zero_instead_of_repairing_it() {
    for field in [
        "app_api_default_list_limit",
        "app_api_max_list_limit",
        "app_api_max_fetch_size",
        "app_api_rate_limit_cost_per_row",
    ] {
        let mut table = base_table();
        exactness_torii_table_mut(&mut table).insert(field.into(), Value::Integer(0));
        exactness_rejection(table, &format!("torii.{field} must be greater than zero"));
    }
}

#[test]
fn torii_app_api_limit_ordering_is_exact() {
    for (field, expected) in [
        (
            "app_api_max_list_limit",
            "app_api_max_list_limit must be at least torii.app_api_default_list_limit",
        ),
        (
            "app_api_max_fetch_size",
            "app_api_max_fetch_size must be at least torii.app_api_default_list_limit",
        ),
    ] {
        let mut table = base_table();
        exactness_torii_table_mut(&mut table).insert(field.into(), Value::Integer(99));
        exactness_rejection(table, expected);
    }

    let mut table = base_table();
    let torii = exactness_torii_table_mut(&mut table);
    for field in [
        "app_api_default_list_limit",
        "app_api_max_list_limit",
        "app_api_max_fetch_size",
        "app_api_rate_limit_cost_per_row",
    ] {
        torii.insert(field.into(), Value::Integer(1));
    }
    let parsed = load_root(table);
    assert_eq!(parsed.torii.app_api.default_list_limit.get(), 1);
    assert_eq!(parsed.torii.app_api.max_list_limit.get(), 1);
    assert_eq!(parsed.torii.app_api.max_fetch_size.get(), 1);
    assert_eq!(parsed.torii.app_api.rate_limit_cost_per_row.get(), 1);
}

#[test]
fn torii_optional_rate_limits_reject_explicit_zero() {
    for field in [
        "query_rate_per_authority_per_sec",
        "query_burst_per_authority",
        "tx_rate_per_authority_per_sec",
        "tx_burst_per_authority",
        "deploy_rate_per_origin_per_sec",
        "deploy_burst_per_origin",
        "soracloud_public_rate_per_ip_per_sec",
        "soracloud_public_burst_per_ip",
        "soracloud_mutation_rate_per_account_origin_per_sec",
        "soracloud_mutation_burst_per_account_origin",
        "preauth_rate_per_ip_per_sec",
        "preauth_burst_per_ip",
        "proof_rate_per_minute",
        "proof_burst",
        "proof_egress_bytes_per_sec",
        "proof_egress_burst_bytes",
    ] {
        let mut table = base_table();
        exactness_torii_table_mut(&mut table).insert(field.into(), Value::Integer(0));
        exactness_rejection(
            table,
            &format!("torii.{field} must be greater than zero when configured"),
        );
    }
}

#[test]
fn torii_proof_limits_reject_zero_instead_of_repairing_it() {
    for (field, expected) in [
        (
            "proof_max_body_bytes",
            "torii.proof_max_body_bytes must be greater than zero",
        ),
        (
            "proof_body_read_timeout_ms",
            "torii.proof_body_read_timeout_ms must be at least 1 ms",
        ),
        (
            "proof_max_list_limit",
            "torii.proof_max_list_limit must be greater than zero",
        ),
        (
            "proof_request_timeout_ms",
            "torii.proof_request_timeout_ms must be at least 1 ms",
        ),
        (
            "proof_cache_max_age_secs",
            "torii.proof_cache_max_age_secs must be greater than zero",
        ),
        (
            "proof_retry_after_secs",
            "torii.proof_retry_after_secs must be greater than zero",
        ),
    ] {
        let mut table = base_table();
        exactness_torii_table_mut(&mut table).insert(field.into(), Value::Integer(0));
        exactness_rejection(table, expected);
    }

    let mut table = base_table();
    let torii = exactness_torii_table_mut(&mut table);
    for field in [
        "proof_max_body_bytes",
        "proof_body_read_timeout_ms",
        "proof_max_list_limit",
        "proof_request_timeout_ms",
        "proof_cache_max_age_secs",
        "proof_retry_after_secs",
        "proof_rate_per_minute",
        "proof_burst",
        "proof_egress_bytes_per_sec",
        "proof_egress_burst_bytes",
    ] {
        torii.insert(field.into(), Value::Integer(1));
    }
    let parsed = load_root(table);
    assert_eq!(parsed.torii.proof_api.max_body_bytes.get(), 1);
    assert_eq!(parsed.torii.proof_api.max_list_limit.get(), 1);
    assert_eq!(
        parsed.torii.proof_api.request_timeout,
        Duration::from_millis(1)
    );
    assert_eq!(parsed.torii.proof_api.cache_max_age, Duration::from_secs(1));
    assert_eq!(parsed.torii.proof_api.retry_after, Duration::from_secs(1));
}

#[test]
fn torii_connect_required_limits_reject_zero() {
    for field in [
        "ws_max_sessions",
        "session_ttl_ms",
        "frame_max_bytes",
        "session_buffer_max_bytes",
        "ping_interval_ms",
        "ping_miss_tolerance",
        "ping_min_interval_ms",
        "dedupe_ttl_ms",
        "dedupe_cap",
    ] {
        let mut table = base_table();
        exactness_connect_table_mut(&mut table).insert(field.into(), Value::Integer(0));
        exactness_rejection(
            table,
            &format!("torii.connect.{field} must be greater than zero"),
        );
    }
}

#[test]
fn torii_connect_ping_interval_is_not_silently_raised() {
    let mut table = base_table();
    exactness_connect_table_mut(&mut table)
        .insert("ping_interval_ms".into(), Value::Integer(14_999));
    exactness_rejection(
        table,
        "torii.connect.ping_interval_ms must be at least torii.connect.ping_min_interval_ms",
    );
}

#[test]
fn torii_connect_preserves_only_documented_zero_disable_values() {
    let mut table = base_table();
    let connect = exactness_connect_table_mut(&mut table);
    for field in [
        "ws_max_sessions",
        "session_ttl_ms",
        "frame_max_bytes",
        "session_buffer_max_bytes",
        "ping_interval_ms",
        "ping_miss_tolerance",
        "ping_min_interval_ms",
        "dedupe_ttl_ms",
        "dedupe_cap",
    ] {
        connect.insert(field.into(), Value::Integer(1));
    }
    for field in [
        "ws_per_ip_max_sessions",
        "ws_rate_per_ip_per_min",
        "p2p_ttl_hops",
    ] {
        connect.insert(field.into(), Value::Integer(0));
    }
    let parsed = load_root(table);
    assert_eq!(parsed.torii.connect.ws_max_sessions, 1);
    assert_eq!(parsed.torii.connect.ping_miss_tolerance, 1);
    assert_eq!(parsed.torii.connect.ws_per_ip_max_sessions, 0);
    assert_eq!(parsed.torii.connect.ws_rate_per_ip_per_min, 0);
    assert_eq!(parsed.torii.connect.p2p_ttl_hops, 0);
}

#[test]
fn torii_global_api_token_configuration_is_exact() {
    let min_token = "a".repeat(defaults::torii::API_TOKEN_MIN_BYTES_V1);
    let max_token = "b".repeat(defaults::torii::API_TOKEN_MAX_BYTES_V1);
    let mut table = base_table();
    let torii = exactness_torii_table_mut(&mut table);
    torii.insert("require_api_token".into(), Value::Boolean(true));
    torii.insert(
        "api_tokens".into(),
        Value::Array(vec![
            Value::String(min_token.clone()),
            Value::String(max_token.clone()),
        ]),
    );
    let parsed = load_root(table);
    assert_eq!(
        parsed.torii.api_tokens.as_ref(),
        &[min_token.clone(), max_token.clone()]
    );
    let actual_debug = format!("{:?}", parsed.torii);
    assert!(actual_debug.contains("REDACTED"), "{actual_debug}");
    assert!(!actual_debug.contains(&min_token), "{actual_debug}");
    assert!(!actual_debug.contains(&max_token), "{actual_debug}");

    let mut table = base_table();
    let torii = exactness_torii_table_mut(&mut table);
    torii.insert("require_api_token".into(), Value::Boolean(true));
    torii.insert(
        "api_tokens".into(),
        Value::Array(vec![Value::String(min_token.clone())]),
    );
    let user = load_user_root(table);
    let user_debug = format!("{:?}", user.torii);
    assert!(user_debug.contains("REDACTED"), "{user_debug}");
    assert!(!user_debug.contains(&min_token), "{user_debug}");
}

#[test]
fn torii_global_api_tokens_reject_ambiguous_or_unbounded_inputs() {
    let valid = exactness_token(1);

    let mut required_without_tokens = base_table();
    exactness_torii_table_mut(&mut required_without_tokens)
        .insert("require_api_token".into(), Value::Boolean(true));
    exactness_rejection(
        required_without_tokens,
        "torii.require_api_token=true requires at least one torii.api_tokens entry",
    );

    let mut disabled_with_token = base_table();
    exactness_torii_table_mut(&mut disabled_with_token).insert(
        "api_tokens".into(),
        Value::Array(vec![Value::String(valid.clone())]),
    );
    exactness_rejection(
        disabled_with_token,
        "torii.api_tokens must be empty when torii.require_api_token=false",
    );

    for token in ["a".repeat(31), "a".repeat(257)] {
        let mut table = base_table();
        let torii = exactness_torii_table_mut(&mut table);
        torii.insert("require_api_token".into(), Value::Boolean(true));
        torii.insert(
            "api_tokens".into(),
            Value::Array(vec![Value::String(token)]),
        );
        exactness_rejection(
            table,
            "torii.api_tokens entries must contain 32..=256 bytes",
        );
    }

    for token in [
        format!("{} ", "a".repeat(31)),
        format!("{}\u{7f}", "a".repeat(31)),
    ] {
        let mut table = base_table();
        let torii = exactness_torii_table_mut(&mut table);
        torii.insert("require_api_token".into(), Value::Boolean(true));
        torii.insert(
            "api_tokens".into(),
            Value::Array(vec![Value::String(token)]),
        );
        exactness_rejection(
            table,
            "torii.api_tokens entries must use visible ASCII without whitespace",
        );
    }

    let mut duplicates = base_table();
    let torii = exactness_torii_table_mut(&mut duplicates);
    torii.insert("require_api_token".into(), Value::Boolean(true));
    torii.insert(
        "api_tokens".into(),
        Value::Array(vec![Value::String(valid.clone()), Value::String(valid)]),
    );
    exactness_rejection(duplicates, "torii.api_tokens must not contain duplicates");

    let mut too_many = base_table();
    let torii = exactness_torii_table_mut(&mut too_many);
    torii.insert("require_api_token".into(), Value::Boolean(true));
    torii.insert(
        "api_tokens".into(),
        Value::Array(
            (0..=defaults::torii::API_TOKEN_MAX_COUNT_V1)
                .map(exactness_token)
                .map(Value::String)
                .collect(),
        ),
    );
    exactness_rejection(
        too_many,
        "torii.api_tokens must not contain more than 256 entries",
    );
}
