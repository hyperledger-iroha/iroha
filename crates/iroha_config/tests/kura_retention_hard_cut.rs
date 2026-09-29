//! Validate that retired Kura retention settings are not configuration inputs.

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;

const RETIRED_TOML_FIELDS: [&str; 3] = [
    "block_sync_roster_retention",
    "roster_sidecar_retention",
    "lane_history_retention",
];
const RETIRED_ENV_NAMES: [&str; 3] = [
    "KURA_BLOCK_SYNC_ROSTER_RETENTION",
    "KURA_ROSTER_SIDECAR_RETENTION",
    "KURA_LANE_HISTORY_RETENTION",
];

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
fn retired_kura_retention_toml_fields_are_unknown() {
    for field in RETIRED_TOML_FIELDS {
        let table = format!("[kura]\n{field} = 17\n")
            .parse()
            .expect("retired inline TOML should parse lexically");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired Kura retention fields must be rejected");
        let message = strip_ansi_codes(&format!("{error:?}"));
        assert!(
            message.contains(&format!("unknown parameter: `kura.{field}`")),
            "unexpected retired-field diagnostic for {field}: {message}"
        );
    }
}

#[test]
fn retired_kura_retention_environment_names_are_unvisited() {
    let env = MockEnv::new()
        .set(RETIRED_ENV_NAMES[0], "17")
        .set(RETIRED_ENV_NAMES[1], "19")
        .set(RETIRED_ENV_NAMES[2], "23");
    let _actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("retired environment names are not schema inputs")
        .parse()
        .expect("retired environment names cannot alter Kura configuration");

    let unvisited = env.unvisited();
    for name in RETIRED_ENV_NAMES {
        assert!(
            unvisited.contains(name),
            "retired environment name must remain unvisited: {name}"
        );
    }
}
