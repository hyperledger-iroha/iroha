// Focused tests for verified-source compiler/body admission configuration.

#[test]
fn verified_source_ingress_defaults_are_single_worker_and_bounded_time() {
    let root = load_root(base_table());
    assert_eq!(
        root.torii
            .transaction_ingress
            .verified_source_max_concurrent_compiles
            .get(),
        1
    );
    assert_eq!(
        root.torii
            .transaction_ingress
            .verified_source_body_read_timeout,
        defaults::torii::VERIFIED_SOURCE_BODY_READ_TIMEOUT
    );
}

#[test]
fn verified_source_compile_concurrency_accepts_v1_max_and_rejects_next() {
    fn table_with_ingress(slots: usize, timeout_ms: i64) -> Table {
        let mut table = base_table();
        table
            .get_mut("torii")
            .and_then(Value::as_table_mut)
            .expect("torii table")
            .insert(
                "transaction_ingress".into(),
                Value::Table(Table::from_iter([
                    (
                        "verified_source_max_concurrent_compiles".into(),
                        Value::Integer(i64::try_from(slots).expect("slot fixture fits i64")),
                    ),
                    (
                        "verified_source_body_read_timeout_ms".into(),
                        Value::Integer(timeout_ms),
                    ),
                ])),
            );
        table
    }

    let exact = load_root(table_with_ingress(
        defaults::torii::VERIFIED_SOURCE_MAX_CONCURRENT_COMPILES_V1,
        1,
    ));
    assert_eq!(
        exact
            .torii
            .transaction_ingress
            .verified_source_max_concurrent_compiles
            .get(),
        defaults::torii::VERIFIED_SOURCE_MAX_CONCURRENT_COMPILES_V1
    );
    assert_eq!(
        exact
            .torii
            .transaction_ingress
            .verified_source_body_read_timeout,
        std::time::Duration::from_millis(1)
    );

    let error = actual::Root::from_toml_source(TomlSource::inline(table_with_ingress(
        defaults::torii::VERIFIED_SOURCE_MAX_CONCURRENT_COMPILES_V1 + 1,
        1,
    )))
    .expect_err("the first compile slot beyond the V1 envelope must fail closed");
    assert!(
        format!("{error:?}")
            .contains("verified_source_max_concurrent_compiles must be within 1..=4")
    );

    let error = actual::Root::from_toml_source(TomlSource::inline(table_with_ingress(1, 0)))
        .expect_err("a zero absolute body-read deadline must fail closed at startup");
    assert!(
        format!("{error:?}").contains("verified_source_body_read_timeout_ms must be at least 1 ms")
    );
}
