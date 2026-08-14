// Focused tests for the ISO 20022 durable-store memory bound.
fn iso_bridge_table_mut(table: &mut Table) -> &mut Table {
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .entry("iso_bridge")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("torii.iso_bridge table")
}
#[test]
fn iso_bridge_store_count_uses_a_nonzero_bounded_default() {
    let configured = load_root(base_table()).torii.iso_bridge.store_max_records;
    assert_eq!(configured, defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS);
    assert!(configured > 0);
    assert!(configured <= defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS_HARD_LIMIT_V1);
}
#[test]
fn iso_bridge_store_count_rejects_zero() {
    let mut table = base_table();
    iso_bridge_table_mut(&mut table).insert("store_max_records".into(), Value::Integer(0));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("an unbounded ISO durable store must fail closed");
    assert!(
        format!("{error:?}").contains("iso_bridge.store_max_records must be greater than zero")
    );
}
#[test]
fn iso_bridge_store_count_rejects_values_above_v1_hard_limit() {
    let mut table = base_table();
    let excessive = defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS_HARD_LIMIT_V1 + 1;
    iso_bridge_table_mut(&mut table).insert(
        "store_max_records".into(),
        Value::Integer(i64::try_from(excessive).expect("hard limit fits TOML integer")),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("an ISO durable store above the hard limit must fail closed");
    let report = format!("{error:?}");
    assert!(report.contains("iso_bridge.store_max_records must not exceed"));
    assert!(report.contains("first-release hard maximum"));
    let mut table = base_table();
    iso_bridge_table_mut(&mut table).insert(
        "store_max_records".into(),
        Value::Integer(
            i64::try_from(defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS_HARD_LIMIT_V1)
                .expect("hard limit fits TOML integer"),
        ),
    );
    assert_eq!(
        load_root(table).torii.iso_bridge.store_max_records,
        defaults::torii::ISO_BRIDGE_STORE_MAX_RECORDS_HARD_LIMIT_V1
    );
}
