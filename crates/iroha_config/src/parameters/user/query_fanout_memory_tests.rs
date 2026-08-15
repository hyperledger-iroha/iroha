// Focused tests for Torii cross-dataspace fanout memory configuration.
#[test]
fn query_fanout_pool_may_be_smaller_than_the_general_body_cap() {
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(
            i64::try_from(defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1)
                .expect("protocol minimum fits TOML integer"),
        ),
    );
    let root = load_root(table);
    assert_eq!(
        root.torii.query_fanout_max_retained_bytes.get(),
        defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1
    );
    assert!(
        root.torii.query_fanout_max_retained_bytes.get() < root.torii.max_content_len.get(),
        "the query pool derives a smaller phase-bounded body limit instead of rejecting the general listener cap"
    );
}
#[test]
fn query_fanout_retention_budget_rejects_below_protocol_pool() {
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(
            i64::try_from(defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1 - 1)
                .expect("protocol minimum fits TOML integer"),
        ),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("a pool below the source-coupled geometry must fail closed");
    assert!(format!("{error:?}").contains(&format!(
        "query_fanout_max_retained_bytes must be at least {} bytes",
        defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1
    )));
}
#[test]
fn zero_torii_content_bound_is_rejected() {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert("max_content_len".into(), Value::Integer(0));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("zero maximum response size must fail closed");
    assert!(format!("{error:?}").contains("torii.max_content_len must be greater than zero"));
}
#[test]
fn minimum_transport_content_limit_keeps_a_complete_query_envelope() {
    let exact = i64::try_from(defaults::torii::HTTP_READ_CHUNK_BYTES_V1)
        .expect("HTTP read chunk fits TOML integer");
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert("max_content_len".into(), Value::Integer(exact));
    let root = load_root(table);
    assert_eq!(root.torii.max_content_len.get(), exact as u64);
    assert!(
        root.torii.max_content_len.get() < root.torii.query_fanout_max_retained_bytes.get(),
        "the exact transport minimum remains far below the aggregate query-memory pool"
    );
}
#[test]
fn undersized_aggregate_query_memory_pool_is_rejected() {
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(
            i64::try_from(defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1 - 1)
                .expect("protocol minimum fits TOML integer"),
        ),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("an aggregate pool below the V1 split must fail closed");
    assert!(format!("{error:?}").contains(&format!(
        "query_fanout_max_retained_bytes must be at least {} bytes for four bounded ingress slots",
        defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1
    )));
}
#[test]
fn internal_proxy_memory_geometry_overflow_is_rejected_at_config_load() {
    let headroom = usize::try_from(defaults::torii::TORII_PROXY_HTTP_FIXED_MEMORY_HEADROOM_V1)
        .expect("proxy headroom fits usize");
    let phases = usize::try_from(defaults::torii::TORII_PROXY_HTTP_MEMORY_PHASE_UNITS_V1)
        .expect("proxy phase count fits usize");
    let first_overflow = (usize::MAX - headroom) / phases + 1;
    let Ok(first_overflow) = i64::try_from(first_overflow) else {
        return;
    };
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert("max_content_len".into(), Value::Integer(first_overflow));
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(first_overflow),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("overflowing proxy memory geometry must fail before router construction");
    assert!(
        format!("{error:?}")
            .contains("max_content_len is too large for the first-release internal proxy")
    );
}
#[test]
fn content_limit_above_proxy_protocol_body_cap_is_rejected() {
    let first_invalid = defaults::torii::TORII_PROXY_MAX_INNER_BODY_BYTES_V1 + 1;
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert(
        "max_content_len".into(),
        Value::Integer(i64::try_from(first_invalid).expect("protocol bound fits TOML integer")),
    );
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(i64::try_from(first_invalid).expect("protocol bound fits TOML integer")),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("content above the proxy protocol bound must fail at config load");
    assert!(format!("{error:?}").contains("proxy inner-body maximum"));
}
