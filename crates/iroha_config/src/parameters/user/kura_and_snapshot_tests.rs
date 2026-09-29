#[test]
fn kura_transaction_history_has_one_finite_file_configured_limit() {
    assert_eq!(
        load_root(base_table()).kura.transaction_history_bytes.get(),
        defaults::kura::TRANSACTION_HISTORY_BYTES.get()
    );
    for bytes in [0, 1, 16 * 1024 * 1024] {
        let mut table = base_table();
        table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("kura table")
            .insert("transaction_history_bytes".into(), Value::Integer(bytes));
        let result = actual::Root::from_toml_source(TomlSource::inline(table));
        if bytes == 0 {
            assert!(
                format!("{:?}", result.expect_err("zero is not unlimited"))
                    .contains("kura.transaction_history_bytes must be nonzero")
            );
        } else {
            assert_eq!(
                result
                    .expect("positive finite policy")
                    .kura
                    .transaction_history_bytes
                    .get(),
                bytes as u64
            );
        }
    }
}

#[test]
fn kura_hash_history_has_one_finite_file_configured_limit() {
    assert_eq!(
        load_root(base_table()).kura.block_hash_history_bytes.get(),
        defaults::kura::BLOCK_HASH_HISTORY_BYTES.get()
    );
    for bytes in [0, 16 * 1024 * 1024] {
        let mut table = base_table();
        table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("kura table")
            .insert("block_hash_history_bytes".into(), Value::Integer(bytes));
        let result = actual::Root::from_toml_source(TomlSource::inline(table));
        if bytes == 0 {
            assert!(
                format!("{:?}", result.expect_err("zero is not unlimited"))
                    .contains("kura.block_hash_history_bytes must be nonzero")
            );
        } else {
            assert_eq!(
                result
                    .expect("positive finite policy")
                    .kura
                    .block_hash_history_bytes
                    .get(),
                bytes as u64
            );
        }
    }
}

#[test]
fn snapshot_resource_defaults_respect_norito_structural_limit() {
    let resources = super::SnapshotResourcePolicy::default();
    resources
        .validate(defaults::snapshot::MAX_PAYLOAD_BYTES)
        .expect("snapshot defaults satisfy their production validation policy");
    assert_eq!(
        resources.max_decode_depth.get(),
        norito::core::MAX_VALUE_NESTING_DEPTH,
    );
    let actual = load_root(base_table());
    assert_eq!(
        actual.snapshot.resources.max_decode_depth,
        resources.max_decode_depth
    );
}

#[test]
fn snapshot_resource_depth_boundary_is_enforced_without_other_invalid_budgets() {
    for (depth, valid) in [
        (norito::core::MAX_VALUE_NESTING_DEPTH, true),
        (norito::core::MAX_VALUE_NESTING_DEPTH + 1, false),
    ] {
        let resources = super::SnapshotResourcePolicy {
            max_decode_depth: NonZeroUsize::new(depth).expect("positive depth"),
            ..super::SnapshotResourcePolicy::default()
        };
        let result = resources.validate(defaults::snapshot::MAX_PAYLOAD_BYTES);
        if valid {
            result.expect("the exact structural limit remains valid");
        } else {
            assert!(
                result
                    .expect_err("excess depth must fail")
                    .contains("structural limit")
            );
        }
    }
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
fn default_snapshot_decode_depth_matches_norito() {
    let actual = load_root(base_table());
    assert_eq!(
        actual.snapshot.resources.max_decode_depth.get(),
        norito::core::MAX_VALUE_NESTING_DEPTH,
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

#[test]
fn kura_fastpq_artifact_defaults_have_explicit_finite_byte_and_count_caps() {
    let root = load_root(base_table());
    assert_eq!(
        root.kura.fastpq_artifacts,
        defaults::kura::FASTPQ_ARTIFACT_POLICY
    );
    assert_eq!(
        root.kura.fastpq_artifacts.max_artifact_bytes.get() as u64,
        defaults::zk::fastpq::PROOF_SIDECAR_MAX_BYTES.get()
    );
    assert_eq!(root.kura.fastpq_artifacts.max_artifacts.get(), 1024);
    assert_eq!(
        root.kura.fastpq_artifacts.max_total_bytes.get(),
        256 * 1024 * 1024
    );
    assert!(root.kura.fastpq_artifacts.validate().is_ok());
}

#[test]
fn kura_fastpq_artifact_policy_parses_nested_file_config_and_rejects_zero_or_inconsistent_caps() {
    for (artifact, records, total, valid) in [
        (8, 2, 16, true),
        (8, 2, 8, true),
        (0, 2, 16, false),
        (8, 0, 16, false),
        (8, 2, 0, false),
        (9, 2, 8, false),
    ] {
        let mut table = base_table();
        let mut policy = Table::new();
        policy.insert("max_artifact_bytes".into(), Value::Integer(artifact));
        policy.insert("max_artifacts".into(), Value::Integer(records));
        policy.insert("max_total_bytes".into(), Value::Integer(total));
        let kura = table
            .entry("kura")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .unwrap();
        kura.insert("fastpq_artifacts".into(), Value::Table(policy));
        let parsed = actual::Root::from_toml_source(TomlSource::inline(table));
        assert_eq!(
            parsed.is_ok(),
            valid,
            "artifact={artifact} records={records} total={total}"
        );
        if valid {
            let parsed = parsed.unwrap().kura.fastpq_artifacts;
            assert_eq!(parsed.max_artifact_bytes.get(), artifact as usize);
            assert_eq!(parsed.max_artifacts.get(), records as usize);
            assert_eq!(parsed.max_total_bytes.get(), total as u64);
        }
    }
}

#[test]
fn kura_fastpq_artifact_policy_checks_overflow_probe_and_temporary_slot_geometry() {
    let policy = defaults::kura::FASTPQ_ARTIFACT_POLICY;
    let no_probe = actual::KuraFastpqArtifactPolicy {
        max_artifact_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
        max_total_bytes: NonZeroU64::new(u64::MAX).unwrap(),
        ..policy
    };
    assert_eq!(
        no_probe.validate(),
        Err(actual::KuraFastpqArtifactPolicyError::ArtifactReadOverflow)
    );
    let no_temporary = actual::KuraFastpqArtifactPolicy {
        max_artifacts: NonZeroUsize::new(usize::MAX).unwrap(),
        ..policy
    };
    assert_eq!(
        no_temporary.validate(),
        Err(actual::KuraFastpqArtifactPolicyError::TemporaryCountOverflow)
    );
    let exact_geometry = actual::KuraFastpqArtifactPolicy {
        max_artifact_bytes: NonZeroUsize::new(1).unwrap(),
        max_artifacts: NonZeroUsize::new(usize::MAX - 1).unwrap(),
        max_total_bytes: NonZeroU64::new(1).unwrap(),
    };
    assert!(exact_geometry.validate().is_ok());
}

#[test]
fn membership_storage_limits_are_finite_file_configured_and_independent() {
    let actual = load_root(base_table());
    assert_eq!(actual.kura.membership_storage, defaults::kura::MEMBERSHIP_STORAGE_POLICY);
    for field in ["max_bytes", "memory_bytes"] {
        for value in [0, 4096] {
            let mut table = base_table();
            table.entry("kura").or_insert_with(|| Value::Table(Table::new()))
                .as_table_mut().expect("kura table")
                .entry("membership_storage").or_insert_with(|| Value::Table(Table::new()))
                .as_table_mut().expect("membership policy")
                .insert(field.into(), Value::Integer(value));
            let loaded = actual::Root::from_toml_source(TomlSource::inline(table));
            if value == 0 { assert!(loaded.is_err(), "zero must never select unlimited storage"); }
            else {
                let loaded = loaded.expect("explicit finite membership policy");
                let actual = if field == "max_bytes" { loaded.kura.membership_storage.max_bytes.get() }
                    else { loaded.kura.membership_storage.memory_bytes.get() as u64 };
                assert_eq!(actual, 4096);
                assert_eq!(loaded.kura.block_hash_history_bytes.get(), defaults::kura::BLOCK_HASH_HISTORY_BYTES.get());
            }
        }
    }
}
