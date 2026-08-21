//! Validate the first-release hard cut to the canonical Kura lane-history retention setting.

use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;

const RETIRED_TOML_FIELDS: [&str; 2] = ["block_sync_roster_retention", "roster_sidecar_retention"];
const RETIRED_ENV_NAMES: [&str; 2] = [
    "KURA_BLOCK_SYNC_ROSTER_RETENTION",
    "KURA_ROSTER_SIDECAR_RETENTION",
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
fn canonical_lane_history_retention_parses_from_toml() {
    let table = "[kura]\nlane_history_retention = 73\n"
        .parse()
        .expect("canonical inline TOML should parse");
    let actual: ActualConfig = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("canonical lane-history retention should be accepted")
        .parse()
        .expect("canonical lane-history retention should reach the actual config");

    assert_eq!(actual.kura.lane_history_retention.get(), 73);
}

#[test]
fn canonical_lane_history_retention_parses_from_environment() {
    let env = MockEnv::new().set("KURA_LANE_HISTORY_RETENTION", "89");
    let actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("canonical lane-history environment setting should be accepted")
        .parse()
        .expect("canonical lane-history environment setting should reach the actual config");

    assert_eq!(actual.kura.lane_history_retention.get(), 89);
    assert!(
        !env.unvisited().contains("KURA_LANE_HISTORY_RETENTION"),
        "the canonical lane-history environment setting must be consumed"
    );
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
        .set(RETIRED_ENV_NAMES[1], "19");
    let actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("retired environment names are not schema inputs")
        .parse()
        .expect("retired environment names cannot alter Kura configuration");

    assert_eq!(
        actual.kura.lane_history_retention,
        defaults::kura::LANE_HISTORY_RETENTION
    );
    let unvisited = env.unvisited();
    for name in RETIRED_ENV_NAMES {
        assert!(
            unvisited.contains(name),
            "retired environment name must remain unvisited: {name}"
        );
    }
}
