//! Enforce the first-release removal of caller-defined citizen-service policy.

use iroha_config::parameters::user;
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};

#[test]
fn retired_citizen_service_table_is_rejected_as_unknown() {
    let table: toml::Table = toml::from_str(
        r#"
[citizen_service]
seat_cooldown_blocks = 4
max_seats_per_epoch = 2
free_declines_per_epoch = 1
decline_slash_bps = 100
no_show_slash_bps = 200
misconduct_slash_bps = 300
role_bond_multipliers = { parliament = 2 }
"#,
    )
    .expect("retired citizen-service TOML is syntactically valid");
    let error = ConfigReader::new()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<user::Governance>()
        .expect_err("retired citizen-service configuration must be rejected as unknown");
    let report = format!("{error:?}");
    assert!(report.contains("citizen_service"), "{report}");
}

#[test]
fn retired_citizen_service_environment_aliases_are_unvisited() {
    const RETIRED: [&str; 6] = [
        "GOV_CITIZEN_SEAT_COOLDOWN_BLOCKS",
        "GOV_CITIZEN_MAX_SEATS_PER_EPOCH",
        "GOV_CITIZEN_FREE_DECLINES_PER_EPOCH",
        "GOV_CITIZEN_DECLINE_SLASH_BPS",
        "GOV_CITIZEN_NO_SHOW_SLASH_BPS",
        "GOV_CITIZEN_MISCONDUCT_SLASH_BPS",
    ];
    let env = RETIRED
        .iter()
        .fold(MockEnv::new(), |env, name| env.set(*name, "1"));
    ConfigReader::new()
        .with_env(env.clone())
        .read_and_complete::<user::Governance>()
        .expect("retired citizen-service aliases are not schema inputs")
        .parse();
    for name in RETIRED {
        assert!(
            env.unvisited().contains(name),
            "retired environment alias {name} must remain unvisited"
        );
    }
}
