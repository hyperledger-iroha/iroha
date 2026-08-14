#[test]
fn kura_replica_advert_defaults_reserve_an_evictable_window_after_the_tail() {
    let actual = load_root(base_table());
    assert_eq!(
        actual.kura.replica_advert.evictable_window,
        defaults::kura::REPLICA_ADVERT_EVICTABLE_WINDOW,
    );
    assert_eq!(
        actual.kura.replica_advert.ttl,
        defaults::kura::REPLICA_ADVERT_TTL,
    );
    assert_eq!(
        actual.kura.replica_advert.refresh_interval,
        defaults::kura::REPLICA_ADVERT_REFRESH_INTERVAL,
    );
    let keys = actual::kura_replica_advert_registry_key_capacity(
        actual.kura.blocks_in_memory,
        actual.kura.replica_advert.evictable_window,
    )
    .expect("default replica-advert key capacity is representable");
    assert_eq!(
        keys.get() - actual.kura.blocks_in_memory.get(),
        actual.kura.replica_advert.evictable_window.get(),
        "the protected body tail must not consume the historical evictable window",
    );
    let entries = actual::kura_replica_advert_registry_entry_capacity(
        actual.kura.blocks_in_memory,
        actual.kura.replica_advert.evictable_window,
    )
    .expect("default replica-advert peer capacity is representable");
    assert_eq!(
        entries.get(),
        keys.get() * actual::KURA_REPLICA_ADVERT_KEEPERS_PER_KEY_LIMIT,
    );
}
#[test]
fn kura_replica_advert_ttl_is_nonzero_and_bounded() {
    for (ttl_ms, expected) in [
        (0, "below the 2 ms minimum"),
        (1, "below the 2 ms minimum"),
        (
            i64::try_from(actual::KURA_REPLICA_ADVERT_TTL_MAX.as_millis())
                .expect("TTL maximum fits TOML integer")
                + 1,
            "exceeds the 1 hour maximum",
        ),
    ] {
        let mut table = base_table();
        let kura = table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("kura table");
        kura.insert("replica_advert_ttl_ms".into(), Value::Integer(ttl_ms));
        let error = actual::Root::from_toml_source(TomlSource::inline(table))
            .expect_err("invalid replica-advert TTL must fail configuration parsing");
        let report = format!("{error:?}");
        assert!(report.contains(expected), "{report}");
    }
}
#[test]
fn kura_replica_advert_refresh_is_nonzero_and_at_most_half_the_ttl() {
    for (refresh_ms, expected) in [
        (0, Some("below the 1 ms minimum")),
        (501, Some("exceeds half of TTL")),
        (500, None),
    ] {
        let mut table = base_table();
        let kura = table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("kura table");
        kura.insert("replica_advert_ttl_ms".into(), Value::Integer(1_000));
        kura.insert(
            "replica_advert_refresh_interval_ms".into(),
            Value::Integer(refresh_ms),
        );
        let parsed = actual::Root::from_toml_source(TomlSource::inline(table));
        if let Some(expected) = expected {
            let error = parsed.expect_err("invalid refresh interval must fail parsing");
            let report = format!("{error:?}");
            assert!(report.contains(expected), "{report}");
        } else {
            let actual = parsed.expect("exactly half the TTL is a valid refresh interval");
            assert_eq!(
                actual.kura.replica_advert.refresh_interval,
                Duration::from_millis(500),
            );
        }
    }
    let mut sub_millisecond = defaults::kura::REPLICA_ADVERT_POLICY;
    sub_millisecond.refresh_interval = Duration::from_nanos(1);
    let error = sub_millisecond
        .validate(defaults::kura::BLOCKS_IN_MEMORY)
        .expect_err("typed runtime policy must reject a sub-millisecond refresh cadence");
    assert!(
        error.to_string().contains("below the 1 ms minimum"),
        "{error}",
    );
    let mut exact_minimum = base_table();
    let kura = exact_minimum
        .entry("kura")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("kura table");
    kura.insert("replica_advert_ttl_ms".into(), Value::Integer(2));
    kura.insert(
        "replica_advert_refresh_interval_ms".into(),
        Value::Integer(1),
    );
    let parsed = actual::Root::from_toml_source(TomlSource::inline(exact_minimum))
        .expect("the exact 2 ms TTL and 1 ms refresh floors must compose");
    assert_eq!(
        parsed.kura.replica_advert.ttl,
        actual::KURA_REPLICA_ADVERT_TTL_MIN,
    );
    assert_eq!(
        parsed.kura.replica_advert.refresh_interval,
        actual::KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN,
    );
}
#[test]
fn kura_replica_advert_geometry_uses_checked_arithmetic() {
    let maximum = NonZeroUsize::new(usize::MAX).expect("usize maximum is nonzero");
    let one = NonZeroUsize::new(1).expect("one is nonzero");
    assert!(actual::kura_replica_advert_registry_key_capacity(maximum, one).is_none());
    let largest_key_capacity_without_addition_overflow =
        NonZeroUsize::new(usize::MAX / actual::KURA_REPLICA_ADVERT_KEEPERS_PER_KEY_LIMIT)
            .expect("protocol keeper divisor leaves a nonzero capacity");
    assert!(
        actual::kura_replica_advert_registry_entry_capacity(
            largest_key_capacity_without_addition_overflow,
            one,
        )
        .is_none(),
        "peer multiplication must fail closed even when the outer key sum still fits",
    );
}
#[test]
fn kura_eviction_replica_floor_must_fit_the_protocol_validator_bound() {
    let mut table = base_table();
    let kura = table
        .entry("kura")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("kura table");
    kura.insert(
        "eviction_required_replicas".into(),
        Value::Integer(
            i64::try_from(actual::KURA_REPLICA_ADVERT_KEEPERS_PER_KEY_LIMIT)
                .expect("validator limit fits TOML integer")
                + 1,
        ),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("impossible keeper floor must fail configuration parsing");
    let report = format!("{error:?}");
    assert!(
        report.contains("Kura eviction replica floor 129 exceeds the protocol validator limit 128"),
        "{report}",
    );
}
#[cfg(target_pointer_width = "64")]
#[test]
fn kura_config_rejects_unrepresentable_replica_advert_peer_geometry() {
    let mut table = base_table();
    let kura = table
        .entry("kura")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("kura table");
    kura.insert("blocks_in_memory".into(), Value::Integer(i64::MAX));
    kura.insert(
        "replica_advert_evictable_window".into(),
        Value::Integer(i64::MAX),
    );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("unrepresentable nested registry geometry must fail parsing");
    let report = format!("{error:?}");
    assert!(
        report.contains(
            "times the protocol keeper limit 128 exceeds the platform size representation"
        ),
        "{report}",
    );
}
#[test]
fn default_snapshot_store_dir_follows_explicit_kura_store_dir() {
    let mut table = base_table();
    let kura = table
        .entry("kura")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("kura table");
    kura.insert(
        "store_dir".into(),
        Value::String("/var/lib/iroha/peer0".into()),
    );
    let actual = load_root(table);
    assert_eq!(
        actual.snapshot.store_dir.value(),
        &PathBuf::from("/var/lib/iroha/peer0/snapshot")
    );
}
#[test]
fn explicit_snapshot_store_dir_is_preserved() {
    let mut table = base_table();
    let kura = table
        .entry("kura")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("kura table");
    kura.insert(
        "store_dir".into(),
        Value::String("/var/lib/iroha/peer0".into()),
    );
    let snapshot = table
        .entry("snapshot")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("snapshot table");
    snapshot.insert(
        "store_dir".into(),
        Value::String("/snapshots/paynet-1".into()),
    );
    let actual = load_root(table);
    assert_eq!(
        actual.snapshot.store_dir.value(),
        &PathBuf::from("/snapshots/paynet-1")
    );
}
#[test]
fn snapshot_bootstrap_policy_parses_only_complete_exact_authority() {
    let digest = "1a0861b04fa35fd0d8ea4c2f38baaa478c7430df3466e9401c53f934671747bd";
    let mut table = base_table();
    let snapshot = table
        .entry("snapshot")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("snapshot table");
    let mut bootstrap = Table::new();
    bootstrap.insert("enabled".into(), Value::Boolean(true));
    bootstrap.insert("audited_sha256".into(), Value::String(digest.to_owned()));
    bootstrap.insert("audited_height".into(), Value::Integer(42));
    snapshot.insert("bootstrap".into(), Value::Table(bootstrap));
    let actual = load_root(table);
    assert!(actual.snapshot.bootstrap.authorizes(digest, 42));
}
#[test]
fn snapshot_bootstrap_policy_rejects_partial_or_invalid_authority() {
    for bootstrap in [
        {
            let mut value = Table::new();
            value.insert("enabled".into(), Value::Boolean(true));
            value.insert("audited_height".into(), Value::Integer(42));
            value
        },
        {
            let mut value = Table::new();
            value.insert("enabled".into(), Value::Boolean(true));
            value.insert("audited_sha256".into(), Value::String("AA".repeat(32)));
            value.insert("audited_height".into(), Value::Integer(42));
            value
        },
        {
            let mut value = Table::new();
            value.insert("enabled".into(), Value::Boolean(false));
            value.insert("audited_sha256".into(), Value::String("00".repeat(32)));
            value.insert("audited_height".into(), Value::Integer(42));
            value
        },
    ] {
        let mut table = base_table();
        let snapshot = table
            .entry("snapshot")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("snapshot table");
        snapshot.insert("bootstrap".into(), Value::Table(bootstrap));
        assert!(
            actual::Root::from_toml_source(TomlSource::inline(table)).is_err(),
            "invalid snapshot bootstrap authority must fail configuration parsing"
        );
    }
}
