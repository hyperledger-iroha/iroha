//! Enforce the first-release removal of the public-lane withdrawal-expiry setting.

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
fn retired_withdraw_grace_is_rejected() {
    let table = "[nexus.staking]\nwithdraw_grace_ms = 60000\n"
        .parse()
        .expect("retired staking configuration should parse as TOML");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("the retired withdrawal-expiry setting must be unknown");
    let message = format!("{error:?}");
    assert!(
        message.contains("unknown parameter")
            && message.contains("nexus.staking.withdraw_grace_ms"),
        "unexpected retired-field diagnostic: {message}"
    );
}
