//! Validate that SCCP consensus state cannot enter through node-local TOML.
use std::path::PathBuf;
use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
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
fn read_user_config_error(inline_toml: &str) -> String {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    let error = ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("retired node-local SCCP configuration must be rejected while reading");
    // `error-stack` styles attachment labels even when the report is captured as a string.
    strip_ansi_codes(&format!("{error:?}"))
}
#[test]
fn diagnostic_normalization_strips_ansi_styling() {
    let styled = "\u{1b}[3munknown parameter:\u{1b}[0m `zk.sccp_route_manifests`";
    assert_eq!(
        strip_ansi_codes(styled),
        "unknown parameter: `zk.sccp_route_manifests`"
    );
}
#[test]
fn route_manifest_array_is_rejected_as_an_unknown_parameter() {
    let message = read_user_config_error(
        r#"
[[zk.sccp_route_manifests]]
route_id = "taira_bsc_xor"
"#,
    );
    assert!(message.contains("unknown parameter: `zk.sccp_route_manifests`"));
}
#[test]
fn route_manifest_scalar_is_rejected_before_container_parsing() {
    let message = read_user_config_error(
        r#"
[zk]
sccp_route_manifests = "not-an-array"
"#,
    );
    assert!(message.contains("unknown parameter: `zk.sccp_route_manifests`"));
}
#[test]
fn every_retired_node_local_sccp_parameter_is_rejected() {
    for parameter in [
        "sccp_source_verifier_materials",
        "sccp_source_adapter_engine_deployments",
        "sccp_destination_rollouts",
        "sccp_route_allowlists",
        "sccp_route_manifests",
    ] {
        let source = format!("[zk]\n{parameter} = []\n");
        let message = read_user_config_error(&source);
        assert!(
            message.contains(&format!("unknown parameter: `zk.{parameter}`")),
            "unexpected error for retired parameter {parameter}: {message}"
        );
    }
}
#[test]
fn rejected_node_local_sccp_values_are_not_disclosed() {
    let secret = "operator-secret-verifier-key";
    let source = format!(
        r#"
[zk]
sccp_source_verifier_materials = [{{ verifier_key = "{secret}" }}]
"#
    );
    let message = read_user_config_error(&source);
    assert!(message.contains("unknown parameter: `zk.sccp_source_verifier_materials`"));
    assert!(!message.contains(secret));
}
