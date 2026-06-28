//! Validate Nexus autoscale configuration parsing and guardrails.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

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

fn autoscale_config_error(inline_toml: &str) -> String {
    parse_actual_config(inline_toml).expect_err("autoscale config should be rejected")
}

fn assert_ratio_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= f64::EPSILON,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn autoscale_overrides_parse_from_toml() {
    let config = parse_actual_config(
        r"
[nexus]
enabled = true

[nexus.autoscale]
enabled = true
min_lanes = 3
max_lanes = 5
target_block_ms = 2500
scale_out_latency_ratio = 1.25
scale_in_latency_ratio = 0.75
scale_out_utilization_ratio = 0.90
scale_in_utilization_ratio = 0.35
scale_out_window_blocks = 12
scale_in_window_blocks = 48
cooldown_blocks = 9
per_lane_target_tps = 75
",
    )
    .expect("valid autoscale config should parse");

    let autoscale = config.nexus.autoscale;
    assert!(autoscale.enabled);
    assert_eq!(autoscale.min_lanes.get(), 3);
    assert_eq!(autoscale.max_lanes.get(), 5);
    assert_eq!(autoscale.target_block_ms.get(), 2_500);
    assert_ratio_eq(autoscale.scale_out_latency_ratio, 1.25);
    assert_ratio_eq(autoscale.scale_in_latency_ratio, 0.75);
    assert_ratio_eq(autoscale.scale_out_utilization_ratio, 0.90);
    assert_ratio_eq(autoscale.scale_in_utilization_ratio, 0.35);
    assert_eq!(autoscale.scale_out_window_blocks.get(), 12);
    assert_eq!(autoscale.scale_in_window_blocks.get(), 48);
    assert_eq!(autoscale.cooldown_blocks.get(), 9);
    assert_eq!(autoscale.per_lane_target_tps.get(), 75);
    assert_eq!(autoscale.last_transition_height, 0);
}

#[test]
fn autoscale_rejects_enabled_autoscale_when_nexus_disabled() {
    let message = autoscale_config_error(
        r"
[nexus]
enabled = false

[nexus.autoscale]
enabled = true
",
    );

    assert!(
        message.contains("nexus.autoscale.enabled requires nexus.enabled = true"),
        "error should require Nexus before enabling autoscale: {message}"
    );
}

#[test]
fn autoscale_rejects_zero_sizing_fields() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
min_lanes = 0
max_lanes = 0
target_block_ms = 0
scale_out_window_blocks = 0
scale_in_window_blocks = 0
cooldown_blocks = 0
per_lane_target_tps = 0
",
    );

    for field in [
        "min_lanes",
        "max_lanes",
        "target_block_ms",
        "scale_out_window_blocks",
        "scale_in_window_blocks",
        "cooldown_blocks",
        "per_lane_target_tps",
    ] {
        assert!(
            message.contains(field),
            "error should identify invalid autoscale field {field}: {message}"
        );
    }
}

#[test]
fn autoscale_rejects_invalid_lane_bounds() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
min_lanes = 6
max_lanes = 5
",
    );

    assert!(
        message.contains("min_lanes must be <= max_lanes"),
        "error should identify inverted autoscale lane bounds: {message}"
    );
}

#[test]
fn autoscale_rejects_max_lanes_above_safety_cap() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
max_lanes = 9
",
    );

    assert!(
        message.contains("max_lanes must be <= 8"),
        "error should identify autoscale max_lanes above the safety cap: {message}"
    );
}

#[test]
fn autoscale_rejects_reserved_lane_metadata_claim() {
    let message = autoscale_config_error(
        r#"
[nexus]
enabled = true
lane_count = 1

[[nexus.lane_catalog]]
index = 0
alias = "default"
metadata = { "autoscale.managed" = "true" }
"#,
    );

    assert!(
        message.contains("autoscale.managed") && message.contains("reserved"),
        "error should identify reserved autoscale-managed lane metadata: {message}"
    );
}

#[test]
fn autoscale_rejects_manual_lane_inside_elastic_id_range() {
    let message = autoscale_config_error(
        r#"
[nexus]
enabled = true
lane_count = 2

[nexus.autoscale]
enabled = true
min_lanes = 1
max_lanes = 2

[[nexus.lane_catalog]]
index = 0
alias = "default"
metadata = {}

[[nexus.lane_catalog]]
index = 1
alias = "manual-elastic-range"
metadata = {}
"#,
    );

    assert!(
        message.contains("reserved autoscale elastic lane id range [1, 2)")
            && message.contains("manual lanes outside"),
        "error should identify manual lanes inside the autoscale elastic range: {message}"
    );
}

#[test]
fn autoscale_rejects_default_lane_inside_elastic_id_range() {
    let message = autoscale_config_error(
        r#"
[nexus]
enabled = true
lane_count = 2

[[nexus.lane_catalog]]
index = 0
alias = "default"
metadata = {}

[[nexus.lane_catalog]]
index = 1
alias = "elastic-default"
metadata = {}

[nexus.autoscale]
enabled = true
min_lanes = 1
max_lanes = 2

[nexus.routing_policy]
default_lane = 1
default_dataspace = "universal"
rules = []
"#,
    );

    assert!(
        message.contains("nexus.routing_policy.default_lane")
            && message.contains("reserved autoscale elastic lane id range [1, 2)")
            && message.contains("base default lane outside"),
        "error should identify default lanes inside the autoscale elastic range: {message}"
    );
}

#[test]
fn autoscale_rejects_hysteresis_without_gap() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
scale_out_latency_ratio = 1.0
scale_in_latency_ratio = 1.0
scale_out_utilization_ratio = 0.5
scale_in_utilization_ratio = 0.5
",
    );

    assert!(
        message.contains("scale_in_latency_ratio must be < scale_out_latency_ratio"),
        "error should identify latency hysteresis violation: {message}"
    );
    assert!(
        message.contains("scale_in_utilization_ratio must be < scale_out_utilization_ratio"),
        "error should identify utilization hysteresis violation: {message}"
    );
}

#[test]
fn autoscale_rejects_hysteresis_that_collapses_after_permille_rounding() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
scale_out_latency_ratio = 1.00049
scale_in_latency_ratio = 1.0004
scale_out_utilization_ratio = 0.50049
scale_in_utilization_ratio = 0.5004
",
    );

    assert!(
        message.contains("scale_in_latency_ratio must round below scale_out_latency_ratio"),
        "error should identify collapsed latency hysteresis: {message}"
    );
    assert!(
        message.contains("scale_in_utilization_ratio must round below scale_out_utilization_ratio"),
        "error should identify collapsed utilization hysteresis: {message}"
    );
}

#[test]
fn autoscale_rejects_non_finite_ratios() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
scale_out_latency_ratio = inf
scale_in_latency_ratio = nan
scale_out_utilization_ratio = inf
scale_in_utilization_ratio = nan
",
    );

    for field in [
        "scale_out_latency_ratio",
        "scale_in_latency_ratio",
        "scale_out_utilization_ratio",
        "scale_in_utilization_ratio",
    ] {
        assert!(
            message.contains(field),
            "error should identify non-finite autoscale field {field}: {message}"
        );
    }
    assert!(
        message.contains("invalid float value") && message.contains("NaN or infinite"),
        "config reader should reject non-finite autoscale ratios before runtime parse: {message}"
    );
}

#[test]
fn autoscale_rejects_sub_permille_ratios() {
    let message = autoscale_config_error(
        r"
[nexus.autoscale]
scale_out_latency_ratio = 0.0004
scale_in_latency_ratio = 0.0002
scale_out_utilization_ratio = 0.0004
scale_in_utilization_ratio = 0.0002
",
    );

    for field in [
        "scale_out_latency_ratio",
        "scale_in_latency_ratio",
        "scale_out_utilization_ratio",
        "scale_in_utilization_ratio",
    ] {
        assert!(
            message.contains(field),
            "error should identify sub-permille autoscale field {field}: {message}"
        );
    }
    assert!(
        message.contains("1 permille"),
        "error should explain representable permille precision: {message}"
    );
}
