// Focused tests for operator-signature request-body deadlines.

fn table_with_operator_signature_body_timeout(timeout_ms: i64) -> Table {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "operator_signatures".into(),
            Value::Table(Table::from_iter([(
                "body_read_timeout_ms".into(),
                Value::Integer(timeout_ms),
            )])),
        );
    table
}

#[test]
fn operator_signature_body_timeout_defaults_and_accepts_one_millisecond() {
    let default = load_root(base_table());
    assert_eq!(
        default.torii.operator_signatures.body_read_timeout,
        defaults::torii::operator_signatures::BODY_READ_TIMEOUT
    );

    let exact = load_root(table_with_operator_signature_body_timeout(1));
    assert_eq!(
        exact.torii.operator_signatures.body_read_timeout,
        std::time::Duration::from_millis(1)
    );
}

#[test]
fn operator_signature_body_timeout_rejects_zero() {
    let error = actual::Root::from_toml_source(TomlSource::inline(
        table_with_operator_signature_body_timeout(0),
    ))
    .expect_err("a zero operator-signature body-read deadline must fail closed");
    assert!(
        format!("{error:?}")
            .contains("torii.operator_signatures.body_read_timeout_ms must be at least 1 ms")
    );
}
