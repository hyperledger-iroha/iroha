//! Enforce the exact first-release Connect relay-strategy vocabulary.

use std::path::PathBuf;

use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn reader_with_strategy(strategy: &str) -> ConfigReader {
    let table = format!("[torii.connect]\nrelay_strategy = {strategy:?}\n")
        .parse()
        .expect("Connect relay TOML should be syntactically valid");
    base_reader().with_toml_source(TomlSource::inline(table))
}

#[test]
fn exact_connect_relay_strategies_are_accepted() {
    for strategy in ["broadcast", "local_only"] {
        reader_with_strategy(strategy)
            .read_and_complete::<UserConfig>()
            .unwrap_or_else(|error| panic!("canonical strategy {strategy} rejected: {error:?}"));
    }
}

#[test]
fn connect_relay_aliases_and_normalization_are_rejected() {
    for rejected in ["local-only", "local", "BROADCAST", " broadcast ", "unknown"] {
        let error = reader_with_strategy(rejected)
            .read_and_complete::<UserConfig>()
            .expect_err("non-canonical Connect relay strategy must be rejected");
        let report = format!("{error:?}");
        assert!(report.contains("relay_strategy"), "{report}");
        assert!(report.contains("broadcast"), "{report}");
        assert!(report.contains("local_only"), "{report}");
    }
}
