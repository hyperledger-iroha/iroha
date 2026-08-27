#[test]
fn soracloud_runtime_json_deserialize_applies_inrou_archive_defaults() {
    let parsed: SoracloudRuntime =
        norito::json::from_json(r#"{"inrou":{}}"#).expect("runtime JSON should deserialize");
    assert_eq!(
        parsed.inrou.bundle_archive_max_compressed_bytes,
        defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_COMPRESSED_BYTES
    );
    assert_eq!(
        parsed.inrou.bundle_archive_max_decoded_bytes,
        defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_DECODED_BYTES
    );
    assert_eq!(
        parsed.inrou.bundle_archive_max_entries,
        defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_ENTRIES
    );
    assert_eq!(
        parsed.inrou.bundle_archive_max_file_bytes,
        defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_FILE_BYTES
    );
    assert_eq!(
        parsed.inrou.bundle_archive_max_total_file_bytes,
        defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_TOTAL_FILE_BYTES
    );
}
#[test]
fn soracloud_runtime_json_rejects_removed_proxy_only_field() {
    norito::json::from_json::<SoracloudRuntime>(r#"{"inrou":{"proxy_only":true}}"#)
        .expect_err("the removed proxy_only field must not be accepted as a compatibility alias");
}
#[test]
fn soracloud_runtime_json_rejects_retired_portable_vm_selectors() {
    for (name, field) in [
        ("max_concurrent_vms", r#""max_concurrent_vms":1"#),
        ("backends", r#""backends":["portable_vm"]"#),
        (
            "portable_vm_acceleration",
            r#""portable_vm_acceleration":"kvm""#,
        ),
        (
            "portable_vm_supplementary_gids",
            r#""portable_vm_supplementary_gids":[108]"#,
        ),
    ] {
        let error = norito::json::from_json::<crate::parameters::user::SoracloudRuntimeInrou>(
            &format!("{{{field}}}"),
        )
        .expect_err("retired one-value PortableVM selectors must fail closed");
        let message = error.to_string();
        assert!(
            message.contains("unknown field") && message.contains(name),
            "unexpected retired-selector diagnostic: {message}"
        );
    }
}
#[test]
fn soracloud_runtime_json_deserialize_applies_explicit_overrides() {
    let json = r#"{
            "state_dir":"./runtime/json",
            "reconcile_interval_ms":2500,
            "hydration_concurrency":7,
            "cache_budgets":{
                "bundle_bytes":1024,
                "static_asset_bytes":2048,
                "journal_bytes":3072,
                "checkpoint_bytes":4096,
                "model_artifact_bytes":5120,
                "model_weight_bytes":6144
            },
            "inrou":{
                "max_cpu_millis":5000,
                "max_memory_bytes":5368709120,
                "max_storage_bytes":10737418240,
                "bundle_archive_max_compressed_bytes":10000,
                "bundle_archive_max_decoded_bytes":40000,
                "bundle_archive_max_entries":123,
                "bundle_archive_max_file_bytes":20000,
                "bundle_archive_max_total_file_bytes":30000,
                "start_grace_ms":7500,
                "stop_grace_ms":9500
            },
            "egress":{
                "default_allow":true,
                "allowed_hosts":["cdn.sora.test"],
                "rate_per_minute":120,
                "max_bytes_per_minute":262144
            }
        }"#;
    let parsed: SoracloudRuntime =
        norito::json::from_json(json).expect("runtime JSON should deserialize");
    assert!(
        parsed
            .state_dir
            .value()
            .to_string_lossy()
            .ends_with("runtime/json")
    );
    assert_eq!(
        parsed.inrou.max_cpu_millis.expect("CPU budget").get(),
        5_000
    );
    assert_eq!(
        parsed.inrou.bundle_archive_max_compressed_bytes.get(),
        10_000
    );
    assert_eq!(parsed.inrou.bundle_archive_max_decoded_bytes.get(), 40_000);
    assert_eq!(parsed.inrou.bundle_archive_max_entries.get(), 123);
    assert_eq!(parsed.inrou.bundle_archive_max_file_bytes.get(), 20_000);
    assert_eq!(
        parsed.inrou.bundle_archive_max_total_file_bytes.get(),
        30_000
    );
    assert!(parsed.egress.default_allow);
}
#[test]
fn soracloud_runtime_parse_rejects_removed_legacy_runtime_section() {
    let mut table = base_table();
    let runtime = table
        .entry("soracloud_runtime")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("soracloud_runtime table");
    let removed_field = ["native", "process"].join("_");
    runtime.insert(removed_field, Value::Table(Table::new()));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("removed legacy runtime section must not parse");
    assert!(
        !error.to_string().is_empty(),
        "removed legacy runtime section should produce a parse error"
    );
}
#[test]
fn soracloud_runtime_json_deserialize_rejects_removed_legacy_runtime_field() {
    let removed_field = ["native", "process"].join("_");
    let json = r#"{
            "state_dir":"./runtime/json",
            "reconcile_interval_ms":2500,
            "hydration_concurrency":7,
            "cache_budgets":{
                "bundle_bytes":1024,
                "static_asset_bytes":2048,
                "journal_bytes":3072,
                "checkpoint_bytes":4096,
                "model_artifact_bytes":5120,
                "model_weight_bytes":6144
            },
            "__REMOVED_FIELD__":{},
            "inrou":{
                "start_grace_ms":7500,
                "stop_grace_ms":9500
            },
            "egress":{
                "default_allow":true,
                "allowed_hosts":["cdn.sora.test"]
            }
        }"#
    .replace(
        "\"__REMOVED_FIELD__\":{}",
        &format!("\"{removed_field}\":{{}}"),
    );
    let error = norito::json::from_json::<SoracloudRuntime>(&json)
        .expect_err("removed legacy runtime JSON field must be rejected");
    assert!(error.to_string().contains(&removed_field));
}
#[test]
fn nexus_hf_shared_leases_defaults_apply() {
    let actual = load_root(base_table());
    assert_eq!(
        actual.nexus.hf_shared_leases.drain_grace,
        StdDuration::from_millis(defaults::nexus::hf_shared_leases::DRAIN_GRACE_MS)
    );
}
#[test]
fn nexus_hf_shared_leases_parse_applies_explicit_overrides() {
    let mut table = base_table();
    let nexus = table
        .entry("nexus")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("nexus table");
    let mut hf_shared_leases = Table::new();
    hf_shared_leases.insert("drain_grace_ms".into(), Value::Integer(12_345));
    nexus.insert("hf_shared_leases".into(), Value::Table(hf_shared_leases));
    let actual = load_root(table);
    assert_eq!(
        actual.nexus.hf_shared_leases.drain_grace,
        StdDuration::from_millis(12_345)
    );
}
#[test]
fn nexus_uploaded_models_defaults_apply() {
    let actual = load_root(base_table());
    assert_eq!(
        actual.nexus.uploaded_models.max_plaintext_bytes_per_model,
        defaults::nexus::uploaded_models::MAX_PLAINTEXT_BYTES_PER_MODEL
    );
    assert_eq!(
        actual.nexus.uploaded_models.max_chunk_count_per_model,
        defaults::nexus::uploaded_models::MAX_CHUNK_COUNT_PER_MODEL
    );
}
#[test]
fn nexus_uploaded_models_parse_applies_explicit_overrides() {
    let mut table = base_table();
    let nexus = table
        .entry("nexus")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("nexus table");
    let mut uploaded_models = Table::new();
    uploaded_models.insert(
        "max_plaintext_bytes_per_model".into(),
        Value::Integer(2_097_152),
    );
    uploaded_models.insert("max_chunk_count_per_model".into(), Value::Integer(4_096));
    nexus.insert("uploaded_models".into(), Value::Table(uploaded_models));
    let actual = load_root(table);
    assert_eq!(
        actual.nexus.uploaded_models.max_plaintext_bytes_per_model,
        2_097_152
    );
    assert_eq!(
        actual.nexus.uploaded_models.max_chunk_count_per_model,
        4_096
    );
}
#[test]
fn tiered_state_parse_accepts_da_store_root() {
    let mut table = base_table();
    let tiered = table
        .entry("tiered_state")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("tiered_state table");
    tiered.insert(
        "da_store_root".into(),
        Value::String("./storage/da_wsv_custom".to_string()),
    );
    let actual = load_root(table);
    assert_eq!(
        actual
            .tiered_state
            .da_store_root
            .as_ref()
            .expect("da_store_root must parse"),
        &PathBuf::from("./storage/da_wsv_custom")
    );
}
#[test]
fn sumeragi_v2_rejects_retired_v1_tables() {
    for retired_table in [
        "collectors",
        "advanced",
        "recovery",
        "pacing_governor",
        "rbc",
        "da",
        "debug",
        "worker",
        "vnext",
    ] {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        sumeragi.insert(retired_table.into(), Value::Table(Table::new()));
        assert!(
            actual::Root::from_toml_source(TomlSource::inline(table)).is_err(),
            "retired sumeragi.{retired_table} must be rejected",
        );
    }
    for retired_field in [
        "protocol_version",
        "consensus_mode",
        "block_time_ms",
        "commit_time_ms",
        "round_timeout_ms",
    ] {
        let mut table = base_table();
        let sumeragi = table
            .entry("sumeragi")
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("sumeragi table");
        sumeragi.insert(retired_field.into(), Value::String("retired".to_owned()));
        assert!(
            actual::Root::from_toml_source(TomlSource::inline(table)).is_err(),
            "retired sumeragi.{retired_field} must be rejected",
        );
    }
}
#[test]
fn root_rejects_non_bls_consensus_keys() {
    let mut table = base_table();
    table.insert(
        "public_key".into(),
        Value::String(
            "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB".to_string(),
        ),
    );
    table.insert(
        "private_key".into(),
        Value::String(
            "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F".to_string(),
        ),
    );
    assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
}
#[test]
fn root_rejects_non_bls_trusted_peer_pop_key() {
    let mut table = base_table();
    let trusted_peers_pop = table
        .get_mut("trusted_peers_pop")
        .and_then(Value::as_array_mut)
        .expect("trusted_peers_pop array");
    let first = trusted_peers_pop
        .first_mut()
        .and_then(Value::as_table_mut)
        .expect("trusted_peers_pop entry");
    first.insert(
        "public_key".into(),
        Value::String(
            "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB".to_string(),
        ),
    );
    assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
}
#[test]
fn sumeragi_requires_bls_allowed_algorithms() {
    let mut table = base_table();
    let sumeragi = table
        .entry("sumeragi")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi table");
    let keys = sumeragi
        .entry("keys")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi.keys table");
    keys.insert(
        "allowed_algorithms".into(),
        Value::Array(vec![Value::String("ed25519".to_string())]),
    );
    assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
}
#[test]
fn retired_sumeragi_npos_config_is_rejected() {
    let mut table = base_table();
    let sumeragi = table
        .entry("sumeragi")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi table");
    let npos = sumeragi
        .entry("npos")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("npos table");
    let reconfig = npos
        .entry("reconfig")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("reconfig table");
    reconfig.insert("evidence_horizon_blocks".into(), Value::Integer(20));
    // NPoS policy is now governed state rather than a startup configuration
    // subtree, so stale file configuration must fail closed.
    assert!(actual::Root::from_toml_source(TomlSource::inline(table)).is_err());
}
#[test]
fn nts_parse_clamps_zero_sample_interval() {
    let mut table = base_table();
    let mut nts = Table::new();
    nts.insert("sample_interval_ms".into(), Value::Integer(0));
    table.insert("nts".into(), Value::Table(nts));
    let actual = load_root(table);
    assert_eq!(actual.nts.sample_interval, StdDuration::from_millis(100));
}
#[test]
fn nts_parse_rejects_unsafe_bounds() {
    for (field, value) in [
        ("sample_cap_per_round", Value::Integer(0)),
        ("max_rtt_ms", Value::Integer(0)),
        ("trim_percent", Value::Integer(46)),
        ("per_peer_buffer", Value::Integer(0)),
        ("min_samples", Value::Integer(0)),
        ("smoothing_alpha", Value::Float(-0.1)),
        ("smoothing_alpha", Value::Float(1.1)),
        ("smoothing_alpha", Value::Float(f64::NAN)),
    ] {
        let mut table = base_table();
        let mut nts = Table::new();
        nts.insert(field.into(), value);
        table.insert("nts".into(), Value::Table(nts));
        assert!(
            actual::Root::from_toml_source(TomlSource::inline(table)).is_err(),
            "unsafe NTS field `{field}` must fail configuration parsing"
        );
    }
}
#[test]
fn telemetry_clamps_zero_telegram_metrics_period() {
    let mut table = base_table();
    let mut telemetry = Table::new();
    telemetry.insert("name".into(), Value::String("ops".to_string()));
    telemetry.insert(
        "url".into(),
        Value::String("http://localhost:8180".to_string()),
    );
    telemetry.insert(
        "telegram_metrics_url".into(),
        Value::String("http://localhost:8180/metrics".to_string()),
    );
    telemetry.insert("telegram_metrics_period_ms".into(), Value::Integer(0));
    table.insert("telemetry".into(), Value::Table(telemetry));
    let actual = load_root(table);
    let telemetry = actual.telemetry.expect("telemetry configured");
    assert_eq!(
        telemetry.telegram_metrics_period,
        Some(StdDuration::from_millis(100))
    );
}

#[test]
fn network_parse_clamps_zero_periods() {
    let mut table = base_table();
    let network = table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table");
    network.insert("block_gossip_period_ms".into(), Value::Integer(0));
    network.insert("block_gossip_max_period_ms".into(), Value::Integer(0));
    network.insert("peer_gossip_period_ms".into(), Value::Integer(0));
    network.insert("peer_gossip_max_period_ms".into(), Value::Integer(0));
    network.insert("transaction_gossip_period_ms".into(), Value::Integer(0));
    network.insert(
        "transaction_gossip_public_target_reshuffle_ms".into(),
        Value::Integer(0),
    );
    network.insert(
        "transaction_gossip_restricted_target_reshuffle_ms".into(),
        Value::Integer(0),
    );
    network.insert("idle_timeout_ms".into(), Value::Integer(0));
    network.insert("reply_writer_flush_timeout_ms".into(), Value::Integer(0));
    let actual = load_root(table);
    let min = StdDuration::from_millis(100);
    assert_eq!(actual.block_sync.gossip_period, min);
    assert_eq!(actual.block_sync.gossip_max_period, min);
    assert_eq!(actual.transaction_gossiper.gossip_period, min);
    assert_eq!(
        actual
            .transaction_gossiper
            .dataspace
            .public_target_reshuffle,
        min
    );
    assert_eq!(
        actual
            .transaction_gossiper
            .dataspace
            .restricted_target_reshuffle,
        min
    );
    assert_eq!(actual.network.peer_gossip_period, min);
    assert_eq!(actual.network.peer_gossip_max_period, min);
    assert_eq!(actual.network.idle_timeout, min);
    assert_eq!(actual.network.reply_writer_flush_timeout, min);
}

#[test]
fn sumeragi_v2_exact_output_geometry_accepts_network_source_boundary() {
    let mut table = base_table();
    table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table")
        .insert("max_total_connections".into(), Value::Integer(2));
    let sumeragi = table
        .entry("sumeragi")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi table");
    let queues = sumeragi
        .entry("queues")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi.queues table");
    queues.insert("commands".into(), Value::Integer(8_192));
    let provisional = load_root(table.clone());
    let shared_capacity = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
        (provisional.sumeragi.queues.commands.get()
            / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
            .max(1),
        provisional.sumeragi.queues.bodies.get(),
    )
    .expect("fixture capacity must be representable");
    let source_boundary = shared_capacity / defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT;
    table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table")
        .insert(
            "max_total_connections".into(),
            Value::Integer(i64::try_from(source_boundary).expect("source boundary fits i64")),
        );
    let actual = load_root(table.clone());
    assert_eq!(actual.sumeragi.queues.commands.get(), 8_192);
    assert_eq!(
        actual
            .network
            .max_total_connections
            .map(std::num::NonZeroUsize::get),
        Some(source_boundary),
    );
    let rejected_source_capacity = source_boundary + 1;
    table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table")
        .insert(
            "max_total_connections".into(),
            Value::Integer(
                i64::try_from(rejected_source_capacity).expect("rejected source capacity fits i64"),
            ),
        );
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("the next reply-source slot exceeds exact-output capacity");
    let report = format!("{error:?}");
    let rejected_fanout = rejected_source_capacity
        .checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)
        .expect("fixture fanout fits usize");
    assert!(
        report.contains(&format!(
            "Sumeragi v2 outbound shared ownership capacity {shared_capacity} is below one maximum fanout {rejected_fanout}; configured network reply-source capacity is {rejected_source_capacity}"
        )),
        "{report}",
    );
}
#[test]
fn sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary() {
    let mut table = base_table();
    let network = table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table");
    network.insert("max_total_connections".into(), Value::Integer(93));
    let sumeragi = table
        .entry("sumeragi")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi table");
    let queues = sumeragi
        .entry("queues")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi.queues table");
    queues.insert("bodies".into(), Value::Integer(15));
    let actual = load_root(table);
    let shared_capacity = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
        (actual.sumeragi.queues.commands.get()
            / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
            .max(1),
        actual.sumeragi.queues.bodies.get(),
    )
    .expect("fixture capacity must be representable");
    let source_capacity = actual
        .network
        .max_total_connections
        .expect("fixture configures the source bound")
        .get();
    assert_eq!(
        shared_capacity,
        source_capacity * defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT,
    );
}
#[test]
fn sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources() {
    let mut table = base_table();
    let network = table
        .get_mut("network")
        .and_then(Value::as_table_mut)
        .expect("network table");
    network.insert("max_total_connections".into(), Value::Integer(93));
    let sumeragi = table
        .entry("sumeragi")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi table");
    let queues = sumeragi
        .entry("queues")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("sumeragi.queues table");
    queues.insert("bodies".into(), Value::Integer(14));
    let error = actual::Root::from_toml_source(TomlSource::inline(table))
        .expect_err("one maximum reply-source fanout must fit exact output");
    let report = format!("{error:?}");
    assert!(
        report.contains(
            "Sumeragi v2 outbound shared ownership capacity 278 is below one maximum fanout 279; configured network reply-source capacity is 93"
        ),
        "{report}",
    );
}
