//! Enforce first-release Torii provider and optional-runtime schemas.

use std::path::PathBuf;

use iroha_config::parameters::user::{Root as UserConfig, ToriiPush};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn ram_lfe_reader() -> ConfigReader {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/torii_ram_lfe.toml");
    ConfigReader::new()
        .read_toml_with_extends(fixture)
        .expect("RAM-LFE fixture should load")
}

fn canonical_push_json(extra_field: &str) -> String {
    format!(
        r#"{{
            "enabled": true,
            "rate_limit_enabled": true,
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
fn zero_valued_push_limits_are_rejected_by_the_schema() {
    for field in ["rate_per_minute", "burst", "max_topics_per_device"] {
        let table = format!("[torii.push]\n{field} = 0\n")
            .parse()
            .expect("push TOML should be syntactically valid");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("zero-valued push limits must not be normalized");
        let report = format!("{error:?}");
        assert!(report.contains(field), "{report}");
    }
    let json = canonical_push_json("").replacen(
        "\"max_topics_per_device\": 32",
        "\"max_topics_per_device\": 0",
        1,
    );
    let error = norito::json::from_json::<ToriiPush>(&json)
        .expect_err("zero JSON topic limit must not be normalized");
    assert!(error.to_string().contains("max_topics_per_device"));
}

#[test]
fn excessive_push_topic_limit_is_rejected_during_actual_parse() {
    let table = format!(
        "[torii.push]\nmax_topics_per_device = {}\n",
        iroha_config::parameters::defaults::torii::PUSH_MAX_TOPICS_PER_DEVICE_V1 + 1
    )
    .parse()
    .expect("push TOML should be syntactically valid");
    let user = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("positive topic limit should pass schema decoding");
    let error = user
        .parse()
        .expect_err("excessive push topic limit must fail actual parsing");
    assert!(
        format!("{error:?}").contains("max_topics_per_device must not exceed"),
        "{error:?}"
    );
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

#[test]
fn ram_lfe_optional_table_rejects_redundant_enable_switch() {
    let table = "[torii.ram_lfe]\nenabled = true\n"
        .parse()
        .expect("RAM-LFE TOML should be syntactically valid");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("presence of the optional RAM-LFE table is the only enable switch");
    let report = format!("{error:?}");
    assert!(report.contains("unknown parameter"), "{report}");
    assert!(report.contains("enabled"), "{report}");
}

#[test]
fn ram_lfe_hidden_program_material_is_required_by_the_schema() {
    let table = r#"
[torii.ram_lfe]

[[torii.ram_lfe.programs]]
program_id = "phone_retail"
secret_hex = "0x01020304"
signer_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#
    .parse()
    .expect("RAM-LFE TOML should be syntactically valid");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("hidden program material must never be fabricated from a default");
    assert!(
        format!("{error:?}").contains("hidden_program_hex"),
        "{error:?}"
    );
}

#[test]
fn ram_lfe_rejects_empty_duplicate_and_malformed_program_lists() {
    let mut empty = ram_lfe_reader()
        .read_and_complete::<UserConfig>()
        .expect("canonical RAM-LFE fixture should decode");
    empty
        .torii
        .ram_lfe
        .as_mut()
        .expect("fixture RAM-LFE table")
        .programs
        .clear();
    let error = empty
        .parse()
        .expect_err("configured RAM-LFE runtime must not silently disable itself");
    assert!(format!("{error:?}").contains("must contain at least one program"));

    let mut duplicate = ram_lfe_reader()
        .read_and_complete::<UserConfig>()
        .expect("canonical RAM-LFE fixture should decode");
    let runtime = duplicate
        .torii
        .ram_lfe
        .as_mut()
        .expect("fixture RAM-LFE table");
    runtime.programs.push(runtime.programs[0].clone());
    let error = duplicate
        .parse()
        .expect_err("duplicate program ids must not overwrite runtime material");
    assert!(format!("{error:?}").contains("program_id duplicates"));

    let mut malformed = ram_lfe_reader()
        .read_and_complete::<UserConfig>()
        .expect("canonical RAM-LFE fixture should decode");
    malformed
        .torii
        .ram_lfe
        .as_mut()
        .expect("fixture RAM-LFE table")
        .programs[0]
        .hidden_program_hex = "0xnot-hex".to_owned();
    let error = malformed
        .parse()
        .expect_err("malformed hidden-program material must be a parse error");
    assert!(format!("{error:?}").contains("hidden_program_hex"));
}
