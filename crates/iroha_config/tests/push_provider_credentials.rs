//! Enforce the first-release push-provider credential schema.

use std::path::PathBuf;

use iroha_config::parameters::user::{Root as UserConfig, ToriiPush};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn canonical_push_json(extra_field: &str) -> String {
    format!(
        r#"{{
            "enabled": true,
            "rate_per_minute": 60,
            "burst": 30,
            "connect_timeout_ms": 5000,
            "request_timeout_ms": 10000,
            "max_topics_per_device": 32,
            "fcm_project_id": "taira-mobile",
            "fcm_service_account_path": "/run/secrets/fcm.json",
            "apns_environment": "production",
            "apns_topic": "org.sora.wallet",
            "apns_team_id": "TEAMID",
            "apns_key_id": "KEYID",
            "apns_private_key_path": "/run/secrets/AuthKey_KEYID.p8",
            "apns_endpoint": null
            {extra_field}
        }}"#
    )
}

#[test]
fn canonical_push_provider_bindings_are_accepted() {
    let json = canonical_push_json("");
    let parsed =
        norito::json::from_json::<ToriiPush>(&json).expect("canonical provider bindings parse");
    assert_eq!(parsed.fcm_project_id.as_deref(), Some("taira-mobile"));
    assert_eq!(parsed.apns_key_id.as_deref(), Some("KEYID"));
}

#[test]
fn retired_push_provider_toml_credentials_are_rejected() {
    for retired_field in ["fcm_api_key", "apns_auth_token"] {
        let table = format!("[torii.push]\n{retired_field} = \"must-not-be-accepted\"\n")
            .parse()
            .expect("retired push-provider TOML should be syntactically valid");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired push-provider credentials must be unknown");
        let report = format!("{error:?}");
        assert!(report.contains("unknown parameter"), "{report}");
        assert!(report.contains(retired_field), "{report}");
        assert!(
            !report.contains("must-not-be-accepted"),
            "unknown-field diagnostics leaked a retired credential value: {report}"
        );
    }
}

#[test]
fn retired_push_provider_json_credentials_are_rejected() {
    for retired_field in ["fcm_api_key", "apns_auth_token"] {
        let extra_field = format!(",\n\"{retired_field}\": \"must-not-be-accepted\"");
        let json = canonical_push_json(&extra_field);
        let error = norito::json::from_json::<ToriiPush>(&json)
            .expect_err("retired push-provider JSON credentials must be unknown");
        let report = error.to_string();
        assert!(report.contains(retired_field), "{report}");
        assert!(
            !report.contains("must-not-be-accepted"),
            "unknown-field diagnostics leaked a retired credential value: {report}"
        );
    }
}
