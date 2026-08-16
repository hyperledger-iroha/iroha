//! Validate the exact-signature hard cut for SoraNet privacy telemetry ingress.
use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;
fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}
fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}
#[test]
fn retired_soranet_privacy_bearer_fields_are_rejected_without_disclosure() {
    for (field, value) in [
        ("require_token", "true"),
        ("tokens", "[\"retired-collector-secret\"]"),
    ] {
        let table = format!("[torii.soranet_privacy_ingest]\n{field} = {value}\n")
            .parse()
            .expect("retired inline TOML should parse");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired collector bearer fields must be rejected");
        let message = strip_ansi_codes(&format!("{error:?}"));
        assert!(
            message.contains("unknown parameter: `torii.soranet_privacy_ingest"),
            "unexpected retired-field diagnostic for {field}: {message}"
        );
        assert!(
            !message.contains("retired-collector-secret"),
            "unknown-field diagnostics must not disclose retired bearer values"
        );
    }
}
