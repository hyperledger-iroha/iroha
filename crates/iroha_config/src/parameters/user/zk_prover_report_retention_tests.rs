#[test]
fn zk_prover_scan_time_budget_must_allow_progress() {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert("zk_prover_max_scan_millis".into(), Value::Integer(0));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("zero scan time budget must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("zk_prover_max_scan_millis must be greater than zero"),
        "{report}"
    );
}
#[test]
fn zk_prover_report_store_count_must_be_nonzero() {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert("zk_prover_reports_max_count".into(), Value::Integer(0));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("zero report capacity must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("zk_prover_reports_max_count must be greater than zero"),
        "{report}"
    );
}
#[test]
fn zk_prover_report_store_bytes_must_fit_one_maximum_report() {
    let minimum = defaults::torii::ZK_PROVER_REPORT_MAX_BYTES_V1
        .saturating_add(defaults::torii::ZK_PROVER_REPORT_SUMMARY_MAX_BYTES_V1);
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "zk_prover_reports_max_bytes".into(),
            Value::Integer(i64::try_from(minimum - 1).expect("minimum fits TOML integer")),
        );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("impossible report byte geometry must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("zk_prover_reports_max_bytes must be at least"),
        "{report}"
    );
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "zk_prover_reports_max_bytes".into(),
            Value::Integer(i64::try_from(minimum).expect("minimum fits TOML integer")),
        );
    assert_eq!(load_root(table).torii.zk_prover_reports_max_bytes, minimum);
}
