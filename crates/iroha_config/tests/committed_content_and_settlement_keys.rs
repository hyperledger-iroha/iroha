//! A configuration file cannot set a content-bundle limit, content access
//! control or a settlement router input.
//!
//! The content limits and defaults (size, file-count, path and retention caps,
//! chunk size, cache defaults, default stripe layout) and the settlement router
//! inputs are protocol constants. Who may publish a bundle is the executor's
//! admission decision, and a bundle manifest that names no read authorization
//! mode is public. The node schema keeps the gateway resources of `[content]`
//! and has no `[settlement]` section; the settings value a node builds carries
//! the protocol value of every fixed field.
use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use std::{path::PathBuf, time::Duration};

/// Every retired key of `[content]`, with a value that differs from the
/// protocol value where one exists.
const CONTENT_POLICY_KEYS: [(&str, &str); 11] = [
    ("publish_allow_accounts", "[]"),
    ("default_auth_mode", "\"public\""),
    ("max_bundle_bytes", "7"),
    ("max_files", "7"),
    ("max_path_len", "7"),
    ("max_retention_blocks", "7"),
    ("chunk_size_bytes", "7"),
    ("default_cache_max_age_secs", "7"),
    ("max_cache_max_age_secs", "7"),
    ("immutable_bundles", "false"),
    (
        "stripe_layout",
        "{ total_stripes = 4, shards_per_stripe = 16, row_parity_stripes = 1 }",
    ),
];

/// Every key of the retired `[settlement.router]` section, with a value that
/// differs from the protocol default.
const ROUTER_POLICY_KEYS: [(&str, &str); 7] = [
    ("twap_window_seconds", "90"),
    ("epsilon_bps", "40"),
    ("buffer_alert_pct", "80"),
    ("buffer_throttle_pct", "30"),
    ("buffer_xor_only_pct", "12"),
    ("buffer_halt_pct", "3"),
    ("buffer_horizon_hours", "48"),
];

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn inline_source(source: &str) -> TomlSource {
    let table: toml::Table = source.parse().expect("inline TOML should parse");
    TomlSource::inline(table)
}

fn assert_protocol_defaults(config: &ActualConfig) {
    let content = &config.content;
    assert_eq!(
        content.max_bundle_bytes,
        defaults::content::MAX_BUNDLE_BYTES
    );
    assert_eq!(content.max_files, defaults::content::MAX_FILES);
    assert_eq!(content.max_path_len, defaults::content::MAX_PATH_LEN);
    assert_eq!(
        content.max_retention_blocks,
        defaults::content::MAX_RETENTION_BLOCKS
    );
    assert_eq!(
        content.chunk_size_bytes,
        defaults::content::CHUNK_SIZE_BYTES
    );
    assert_eq!(
        content.default_cache_max_age_secs,
        defaults::content::DEFAULT_CACHE_MAX_AGE_SECS
    );
    assert_eq!(
        content.max_cache_max_age_secs,
        defaults::content::MAX_CACHE_MAX_AGE_SECS
    );
    assert_eq!(
        content.immutable_bundles,
        defaults::content::IMMUTABLE_BUNDLES
    );
    assert_eq!(
        content.stripe_layout,
        defaults::content::default_stripe_layout()
    );
    let router = &config.settlement.router;
    assert_eq!(
        router.twap_window,
        Duration::from_secs(defaults::settlement::router::TWAP_WINDOW_SECS)
    );
    assert_eq!(
        router.epsilon_bps,
        defaults::settlement::router::EPSILON_BPS
    );
    assert_eq!(
        router.buffer_alert_pct,
        defaults::settlement::router::ALERT_PCT
    );
    assert_eq!(
        router.buffer_throttle_pct,
        defaults::settlement::router::THROTTLE_PCT
    );
    assert_eq!(
        router.buffer_xor_only_pct,
        defaults::settlement::router::XOR_ONLY_PCT
    );
    assert_eq!(
        router.buffer_halt_pct,
        defaults::settlement::router::HALT_PCT
    );
    assert_eq!(
        router.buffer_horizon_hours,
        defaults::settlement::router::BUFFER_HORIZON_HOURS
    );
}

#[test]
fn moved_values_of_a_node_settings_value_are_the_protocol_defaults() {
    let config = base_reader()
        .read_and_complete::<UserConfig>()
        .expect("default user config should read")
        .parse()
        .expect("default actual config should parse");
    assert_protocol_defaults(&config);
}

#[test]
fn configuration_file_cannot_set_a_content_limit() {
    for (key, value) in CONTENT_POLICY_KEYS {
        let error = base_reader()
            .with_toml_source(inline_source(&format!("[content]\n{key} = {value}\n")))
            .read_and_complete::<UserConfig>()
            .expect_err("a content limit is not node configuration");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("unknown parameter")
                && diagnostic.contains(&format!("`content.{key}")),
            "{key}: {diagnostic}"
        );
    }
    // Gateway resources of the same section still load.
    let config: ActualConfig = base_reader()
        .with_toml_source(inline_source(
            "[content]\nmax_requests_per_second = 3\npow_difficulty_bits = 2\n",
        ))
        .read_and_complete::<UserConfig>()
        .expect("gateway resources are node configuration")
        .parse()
        .expect("config should parse");
    assert_eq!(config.content.limits.max_requests_per_second.get(), 3);
    assert_eq!(config.content.pow.difficulty_bits, 2);
    assert_protocol_defaults(&config);
}

#[test]
fn configuration_file_cannot_set_a_settlement_router_input() {
    for (key, value) in ROUTER_POLICY_KEYS {
        let error = base_reader()
            .with_toml_source(inline_source(&format!(
                "[settlement.router]\n{key} = {value}\n"
            )))
            .read_and_complete::<UserConfig>()
            .expect_err("a settlement router input is not node configuration");
        // The whole section is gone from the schema, so the reader names its root.
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("unknown parameter") && diagnostic.contains("`settlement`"),
            "{key}: {diagnostic}"
        );
    }
}
