//! Enforce removal of the retired operator-selected SCION configuration path.

use std::path::PathBuf;

use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

#[test]
fn retired_scion_configuration_keys_are_rejected() {
    for (retired_field, value) in [
        ("scion_enabled", "true"),
        ("scion_fallback_to_legacy", "false"),
        ("scion_listen_endpoint", "\"127.0.0.1:30257\""),
        ("scion_routes", "{}"),
    ] {
        let table = format!("[network]\n{retired_field} = {value}\n")
            .parse()
            .expect("retired SCION TOML should be syntactically valid");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired SCION configuration must be unknown");
        let report = format!("{error:?}");
        assert!(report.contains("unknown parameter"), "{report}");
        assert!(report.contains(retired_field), "{report}");
    }
}
