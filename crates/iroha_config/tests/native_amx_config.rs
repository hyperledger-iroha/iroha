//! Validate native AMX Sumeragi cache limit configuration.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn load_actual_config(inline_toml: &str) -> ActualConfig {
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("user config should read")
        .parse()
        .expect("user config should parse")
}

#[test]
fn native_amx_cache_limits_parse_from_toml() {
    let config = load_actual_config(
        r"
[sumeragi.advanced.native_amx]
session_cache_max = 17
session_body_bucket_max = 3
",
    );

    assert_eq!(config.sumeragi.native_amx.session_cache_max.get(), 17);
    assert_eq!(config.sumeragi.native_amx.session_body_bucket_max.get(), 3);
}

#[test]
fn native_amx_cache_limits_reject_zero_values() {
    let table: toml::Table = r"
[sumeragi.advanced.native_amx]
session_cache_max = 0
session_body_bucket_max = 1
"
    .parse()
    .expect("inline TOML should parse");

    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("zero native AMX cache limits should be rejected");
    let message = format!("{error:?}");
    assert!(
        message.contains("session_cache_max"),
        "error should identify the invalid native AMX field: {message}"
    );
}
