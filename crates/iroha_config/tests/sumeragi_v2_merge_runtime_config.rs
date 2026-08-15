//! Validate explicit Sumeragi v2 merge-sidecar and signing-guard overrides.
use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use std::{path::PathBuf, time::Duration};
fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}
fn parse_actual_config(inline_toml: &str) -> Result<ActualConfig, String> {
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    let user = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .map_err(|error| format!("{error:?}"))?;
    user.parse().map_err(|error| format!("{error:?}"))
}
#[test]
fn every_merge_runtime_override_reaches_the_actual_config() {
    let config = parse_actual_config(
        r"
[sumeragi.limits]
merge_sidecar_inbound_session_capacity = 11
merge_sidecar_inbound_sessions_per_peer = 3
merge_sidecar_inbound_assembly_bytes = 50000000
merge_sidecar_inbound_assembly_bytes_per_peer = 40000000
merge_sidecar_deferred_block_capacity = 17
merge_sidecar_future_block_distance = 19
merge_sidecar_request_timeout_ms = 2300
merge_sidecar_outbound_sessions_per_source = 5
merge_sidecar_outbound_bytes_per_source = 20000000
merge_sidecar_server_request_gates_per_source = 7
pending_certified_merge_entry_capacity = 37
pending_queue_plan_admission_capacity = 41
pending_control_sidecar_bytes = 30000000
merge_signing_guard_record_capacity = 31
merge_signing_guard_record_bytes = 17000000
merge_signing_guard_total_bytes = 18000000
",
    )
    .expect("valid explicit merge runtime overrides should parse");
    let limits = config.sumeragi.limits;
    assert_eq!(limits.merge_sidecar_inbound_session_capacity.get(), 11);
    assert_eq!(limits.merge_sidecar_inbound_sessions_per_peer.get(), 3);
    assert_eq!(
        limits.merge_sidecar_inbound_assembly_bytes.get(),
        50_000_000
    );
    assert_eq!(
        limits.merge_sidecar_inbound_assembly_bytes_per_peer.get(),
        40_000_000
    );
    assert_eq!(limits.merge_sidecar_deferred_block_capacity.get(), 17);
    assert_eq!(limits.merge_sidecar_future_block_distance.get(), 19);
    assert_eq!(
        limits.merge_sidecar_request_timeout,
        Duration::from_millis(2_300)
    );
    assert_eq!(limits.merge_sidecar_outbound_sessions_per_source.get(), 5);
    assert_eq!(
        limits.merge_sidecar_outbound_bytes_per_source.get(),
        20_000_000
    );
    assert_eq!(
        limits.merge_sidecar_server_request_gates_per_source.get(),
        7
    );
    assert_eq!(limits.pending_certified_merge_entry_capacity.get(), 37);
    assert_eq!(limits.pending_queue_plan_admission_capacity.get(), 41);
    assert_eq!(limits.pending_control_sidecar_bytes.get(), 30_000_000);
    assert_eq!(limits.merge_signing_guard_record_capacity.get(), 31);
    assert_eq!(limits.merge_signing_guard_record_bytes.get(), 17_000_000);
    assert_eq!(limits.merge_signing_guard_total_bytes.get(), 18_000_000);
}
#[test]
fn tight_valid_merge_runtime_geometry_is_admitted() {
    let inbound_bytes = defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_MIN;
    let outbound_bytes = defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE_MIN;
    let record_bytes = defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_BYTES_MIN;
    let pending_control_bytes = defaults::sumeragi::V2_PENDING_CONTROL_SIDECAR_BYTES_MIN;
    let total_bytes = record_bytes
        .checked_add(defaults::sumeragi::V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES)
        .expect("static tight signing geometry fits usize");
    let config = parse_actual_config(&format!(
        r"
[sumeragi.limits]
merge_sidecar_inbound_session_capacity = 2
merge_sidecar_inbound_sessions_per_peer = 2
merge_sidecar_inbound_assembly_bytes = {inbound_bytes}
merge_sidecar_inbound_assembly_bytes_per_peer = {inbound_bytes}
merge_sidecar_deferred_block_capacity = 2
merge_sidecar_future_block_distance = 1
merge_sidecar_request_timeout_ms = 1
merge_sidecar_outbound_sessions_per_source = 1
merge_sidecar_outbound_bytes_per_source = {outbound_bytes}
merge_sidecar_server_request_gates_per_source = 1
pending_certified_merge_entry_capacity = 1
pending_queue_plan_admission_capacity = 1
pending_control_sidecar_bytes = {pending_control_bytes}
merge_signing_guard_record_capacity = 1
merge_signing_guard_record_bytes = {record_bytes}
merge_signing_guard_total_bytes = {total_bytes}
"
    ))
    .expect("every inclusive production minimum should form a valid runtime geometry");
    let limits = config.sumeragi.limits;
    assert_eq!(limits.merge_sidecar_inbound_session_capacity.get(), 2);
    assert_eq!(limits.merge_sidecar_inbound_sessions_per_peer.get(), 2);
    assert_eq!(
        limits.merge_sidecar_inbound_assembly_bytes.get(),
        inbound_bytes
    );
    assert_eq!(
        limits.merge_sidecar_inbound_assembly_bytes_per_peer.get(),
        inbound_bytes
    );
    assert_eq!(limits.merge_sidecar_deferred_block_capacity.get(), 2);
    assert_eq!(limits.merge_sidecar_future_block_distance.get(), 1);
    assert_eq!(
        limits.merge_sidecar_request_timeout,
        Duration::from_millis(1)
    );
    assert_eq!(limits.merge_sidecar_outbound_sessions_per_source.get(), 1);
    assert_eq!(
        limits.merge_sidecar_outbound_bytes_per_source.get(),
        outbound_bytes
    );
    assert_eq!(
        limits.merge_sidecar_server_request_gates_per_source.get(),
        1
    );
    assert_eq!(limits.pending_certified_merge_entry_capacity.get(), 1);
    assert_eq!(limits.pending_queue_plan_admission_capacity.get(), 1);
    assert_eq!(
        limits.pending_control_sidecar_bytes.get(),
        pending_control_bytes
    );
    assert_eq!(limits.merge_signing_guard_record_capacity.get(), 1);
    assert_eq!(limits.merge_signing_guard_record_bytes.get(), record_bytes);
    assert_eq!(limits.merge_signing_guard_total_bytes.get(), total_bytes);
}
