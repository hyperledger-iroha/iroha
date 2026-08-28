#[test]
fn zk_attachment_global_count_must_be_nonzero() {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert("attachments_global_max_count".into(), Value::Integer(0));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("zero node-global attachment capacity must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("attachments_global_max_count must be greater than zero"),
        "{report}"
    );
}

#[test]
fn zk_attachment_global_count_must_not_exceed_quota_scan_ceiling() {
    let maximum = defaults::torii::ATTACHMENTS_GLOBAL_MAX_COUNT_MAX;
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "attachments_global_max_count".into(),
            Value::Integer(i64::try_from(maximum).expect("attachment count maximum fits TOML")),
        );
    assert_eq!(
        load_root(table).torii.attachments_global_max_count,
        maximum,
        "the closed quota-scan ceiling must remain valid"
    );

    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "attachments_global_max_count".into(),
            Value::Integer(i64::try_from(maximum + 1).expect("attachment count maximum fits TOML")),
        );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("a node-global attachment count above the scan ceiling must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("attachments_global_max_count must not exceed"),
        "{report}"
    );
}

#[test]
fn zk_attachment_global_bytes_must_fit_one_maximum_attachment() {
    let maximum_attachment = defaults::torii::ATTACHMENTS_MAX_BYTES;
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "attachments_global_max_bytes".into(),
            Value::Integer(
                i64::try_from(maximum_attachment - 1).expect("attachment maximum fits TOML"),
            ),
        );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("a node-global byte cap below one attachment must fail closed");
    let report = format!("{error:?}");
    assert!(
        report.contains("attachments_global_max_bytes must be at least"),
        "{report}"
    );

    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "attachments_global_max_bytes".into(),
            Value::Integer(
                i64::try_from(maximum_attachment).expect("attachment maximum fits TOML"),
            ),
        );
    assert_eq!(
        load_root(table).torii.attachments_global_max_bytes,
        maximum_attachment
    );
}
