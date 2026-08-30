//! Enforce the single first-credential operator-token bootstrap configuration surface.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

#[test]
fn retired_operator_bearer_mode_fields_are_rejected() {
    for (field, value) in [
        ("token_fallback", "\"always\""),
        ("token_source", "\"api\""),
    ] {
        let table = format!("[torii.operator_auth]\n{field} = {value}\n")
            .parse()
            .expect("retired inline TOML should parse");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired operator bearer mode must be unknown");
        let report = format!("{error:?}");
        assert!(
            report.contains(&format!("unknown parameter: `torii.operator_auth.{field}`")),
            "unexpected retired-field diagnostic: {report}"
        );
    }
}

#[test]
fn dedicated_operator_bootstrap_tokens_remain_configurable() {
    let table = concat!(
        "[torii.operator_auth]\n",
        "tokens = [\"first-credential-bootstrap\"]\n",
        "ephemeral_state_capacity = 64\n",
        "credential_capacity = 8\n",
    )
    .parse()
    .expect("operator bootstrap TOML should parse");
    let actual: ActualConfig = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("operator bootstrap token should remain configurable")
        .parse()
        .expect("operator bootstrap config should be valid");
    assert_eq!(
        actual.torii.operator_auth.tokens,
        ["first-credential-bootstrap"]
    );
    assert_eq!(
        actual.torii.operator_auth.ephemeral_state_capacity.get(),
        64
    );
    assert_eq!(actual.torii.operator_auth.credential_capacity.get(), 8);
}

#[test]
fn operator_auth_ephemeral_state_capacity_must_be_nonzero() {
    let table = "[torii.operator_auth]\nephemeral_state_capacity = 0\n"
        .parse()
        .expect("operator auth TOML should parse");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("zero ephemeral state capacity must be rejected");
}

#[test]
fn operator_auth_credential_capacity_must_be_nonzero() {
    let table = "[torii.operator_auth]\ncredential_capacity = 0\n"
        .parse()
        .expect("operator auth TOML should parse");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("zero credential capacity must be rejected");
}

#[test]
fn operator_auth_lockout_durations_must_be_nonzero() {
    for field in ["lockout_window_secs", "lockout_duration_secs"] {
        let table = format!("[torii.operator_auth]\n{field} = 0\n")
            .parse()
            .expect("operator auth TOML should parse");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("zero lockout duration must be rejected");
        assert!(format!("{error:?}").contains(field));
    }
}
