//! Validate the mandatory non-zero IVM cycle admission ceiling.
use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;
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
#[test]
fn cycle_ceiling_defaults_to_one_million() {
    let config = base_reader()
        .read_and_complete::<UserConfig>()
        .expect("default user config should read")
        .parse()
        .expect("default actual config should parse");
    assert_eq!(config.pipeline.ivm_max_cycles_upper_bound.get(), 1_000_000);
}
#[test]
fn positive_cycle_ceiling_deserializes() {
    let config: ActualConfig = base_reader()
        .with_toml_source(inline_source(
            r"
[pipeline]
ivm_max_cycles_upper_bound = 42
",
        ))
        .read_and_complete::<UserConfig>()
        .expect("positive cycle ceiling should read")
        .parse()
        .expect("positive cycle ceiling should parse");
    assert_eq!(config.pipeline.ivm_max_cycles_upper_bound.get(), 42);
}
#[test]
fn zero_cycle_ceiling_is_rejected_during_deserialization() {
    let error = base_reader()
        .with_toml_source(inline_source(
            r"
[pipeline]
ivm_max_cycles_upper_bound = 0
",
        ))
        .read_and_complete::<UserConfig>()
        .expect_err("zero must not disable IVM cycle admission");
    let message = format!("{error:?}");
    assert!(
        message.contains("ivm_max_cycles_upper_bound"),
        "error should identify the invalid cycle ceiling: {message}"
    );
}
#[test]
fn cycle_ceiling_has_no_environment_override() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    let env = MockEnv::new().set("PIPELINE_IVM_MAX_CYCLES_UPPER_BOUND", "42");
    let config: ActualConfig = ConfigReader::new()
        .with_env(env.clone())
        .read_toml_with_extends(base_path)
        .expect("base config should load")
        .read_and_complete::<UserConfig>()
        .expect("unrecognized environment variable must not affect configuration")
        .parse()
        .expect("default actual config should parse");
    assert_eq!(config.pipeline.ivm_max_cycles_upper_bound.get(), 1_000_000);
    assert!(
        env.unvisited()
            .contains("PIPELINE_IVM_MAX_CYCLES_UPPER_BOUND"),
        "the production cycle ceiling must not have an environment alias"
    );
}
