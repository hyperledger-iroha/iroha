//! Validate the V1 hard cut for the aggregate pipeline signature-batch alias.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};

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
fn retired_signature_batch_max_config_is_rejected_as_unknown() {
    let table = "[pipeline]\nsignature_batch_max = 32\n"
        .parse()
        .expect("retired inline TOML should parse");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("the aggregate signature-batch alias must be rejected");
    let message = strip_ansi_codes(&format!("{error:?}"));
    assert!(
        message.contains("unknown parameter: `pipeline.signature_batch_max`"),
        "unexpected retired-field diagnostic: {message}"
    );
}

#[test]
fn retired_signature_batch_max_environment_alias_is_unvisited() {
    const RETIRED_ALIAS: &str = "PIPELINE_SIGNATURE_BATCH_MAX";
    let env = MockEnv::new().set(RETIRED_ALIAS, "1");
    let actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("retired environment alias is not a schema input")
        .parse()
        .expect("retired environment alias cannot alter V1 configuration");

    assert!(
        env.unvisited().contains(RETIRED_ALIAS),
        "retired environment alias must remain unvisited"
    );
    assert_eq!(
        actual.pipeline.signature_batch_max_ed25519,
        defaults::pipeline::SIGNATURE_BATCH_MAX_ED25519
    );
}
