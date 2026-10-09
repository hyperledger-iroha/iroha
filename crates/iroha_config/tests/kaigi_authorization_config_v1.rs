//! Final Kaigi verifier configuration has one authorization key and no aliases.

#[path = "publisher_config_fixture.rs"]
mod publisher_config_fixture;
use publisher_config_fixture::{ParserOnlyPublisherFiles, with_fixture_refs};

use iroha_config::parameters::user::Root;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;

fn reader() -> ConfigReader {
    with_fixture_refs(
        ConfigReader::new()
            .read_toml_with_extends(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),
            )
            .unwrap(),
    )
}

#[test]
fn every_authorization_action_uses_one_explicit_governed_key_reference() {
    let table = "[zk.kaigi_authorization_vk]\nbackend = \"pipa-r/pasta/kaigi-authorization-v1\"\nname = \"kaigi-final\"\n"
        .parse()
        .unwrap();
    let config = reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<Root>()
        .unwrap()
        .parse_with_file_source(&ParserOnlyPublisherFiles)
        .unwrap();
    let key = config.zk.kaigi_authorization_vk.unwrap();
    assert_eq!(key.backend, "pipa-r/pasta/kaigi-authorization-v1");
    assert_eq!(key.name, "kaigi-final");
    assert!(config.zk.kaigi_usage_vk.is_none());
}

#[test]
fn retired_per_action_key_configuration_is_unknown() {
    for name in ["kaigi_roster_join_vk", "kaigi_roster_leave_vk"] {
        let table = format!(
            "[zk.{name}]\nbackend = \"pipa-r/pasta/kaigi-authorization-v1\"\nname = \"retired\"\n"
        )
        .parse()
        .unwrap();
        let error = reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<Root>()
            .unwrap_err();
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("unknown parameter") && diagnostic.contains(name),
            "{diagnostic}"
        );
    }
}
