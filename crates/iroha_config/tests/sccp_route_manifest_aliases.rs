//! Validate canonical SCCP route manifest fields through the real TOML loader.

use std::{panic, path::PathBuf};

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

const SOURCE_BRIDGE: &str = "0x3333333333333333333333333333333333333333";
const VERIFIER: &str = "0x4444444444444444444444444444444444444444";

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn route_manifest_toml(address_fields: &str) -> String {
    let blocker_fields = [
        "production_blockers",
        "post_deploy_production_blockers",
        "full_toml_production_blockers",
        "source_event_transaction_production_blockers",
        "route_canary_production_blockers",
    ]
    .into_iter()
    .filter(|field| !address_fields.contains(field))
    .map(|field| format!("{field} = []"))
    .collect::<Vec<_>>()
    .join("\n");

    format!(
        r#"
[zk]
[[zk.sccp_route_manifests]]
version = 1
route_id = "taira_bsc_xor"
asset_key = "xor"
network = "bsc-testnet"
chain = "bsc-testnet"
chain_id_hex = "0x61"
counterparty_domain = 2
verifier_target = "EvmContract"
production_ready = false
disabled_reason = "test route"
{blocker_fields}
network_id_hex = "0x0000000000000000000000000000000000000000000000000000000000000061"
taira_xor_token_address = "0x1111111111111111111111111111111111111111"
taira_xor_bridge_address = "0x2222222222222222222222222222222222222222"
{address_fields}
verifier_code_hash = "0x4545454545454545454545454545454545454545454545454545454545454545"
verifier_key_hash = "0x4646464646464646464646464646464646464646464646464646464646464646"
destination_binding_key = "evm:0:2:test-binding"
destination_binding_hash = "0x4747474747474747474747474747474747474747474747474747474747474747"
taira_burn_record_settlement_asset_definition_id = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
taira_burn_record_contract_artifact_b64 = "QUJDREVGRw=="
taira_burn_record_artifact_sha256 = "0x4848484848484848484848484848484848484848484848484848484848484848"
taira_burn_record_code_hash = "0x4949494949494949494949494949494949494949494949494949494949494949"
taira_burn_record_vk_backend = "halo2_ipa"
taira_burn_record_vk_name = "taira_bsc_xor_burn_record_v1"
taira_burn_record_gas_limit = 2000000
"#,
    )
}

fn read_user_config(inline_toml: &str) -> UserConfig {
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("user config should read")
}

fn read_user_config_error(inline_toml: &str) -> String {
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("user config should reject malformed route manifest");
    format!("{error:?}")
}

fn load_actual_config(inline_toml: &str) -> ActualConfig {
    read_user_config(inline_toml)
        .parse()
        .expect("user config should parse")
}

fn parse_panic_message(inline_toml: &str) -> String {
    let user_config = read_user_config(inline_toml);
    let panic = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let _ = user_config.parse();
    }))
    .expect_err("route manifest aliases should be rejected");

    panic.downcast_ref::<String>().cloned().unwrap_or_else(|| {
        panic.downcast_ref::<&str>().map_or_else(
            || "<non-string panic>".to_owned(),
            |message| (*message).to_owned(),
        )
    })
}

#[test]
fn route_manifest_toml_rejects_scalar_post_deploy_blocker_container() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
post_deploy_production_blockers = "operator hold"
"#
    ));

    let message = read_user_config_error(&toml);
    assert!(message.contains("post_deploy_production_blockers"));
}

#[test]
fn route_manifest_toml_rejects_non_string_post_deploy_blocker_entry() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
full_toml_production_blockers = [123]
"#
    ));

    let message = read_user_config_error(&toml);
    assert!(message.contains("full_toml_production_blockers"));
}

#[test]
fn canonical_bsc_route_manifest_toml_parses() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let actual = load_actual_config(&toml);
    let route = actual
        .zk
        .sccp_route_manifests
        .first()
        .expect("route manifest");

    assert_eq!(route.route_id, "taira_bsc_xor");
    assert_eq!(route.source_bridge_address, SOURCE_BRIDGE);
    assert_eq!(route.destination_verifier_address, VERIFIER);
}

#[test]
fn route_manifest_toml_rejects_noncanonical_aliases_with_canonical_fields() {
    let cases = [
        ("sccp_bsc_source_bridge_address", "source_bridge_address"),
        ("bsc_source_bridge_address", "source_bridge_address"),
        ("sccp_tron_source_bridge_address", "source_bridge_address"),
        ("verifier_address", "destination_verifier_address"),
        (
            "sccp_bsc_destination_verifier_address",
            "destination_verifier_address",
        ),
        ("bsc_verifier_address", "destination_verifier_address"),
        ("evm_verifier_address", "destination_verifier_address"),
        ("tron_verifier_address", "destination_verifier_address"),
        (
            "sccp_tron_destination_verifier_address",
            "destination_verifier_address",
        ),
        ("prover_artifact_hash", "proof_artifact_hash"),
        ("circuit_artifact_hash", "proof_artifact_hash"),
    ];

    for (field, replacement) in cases {
        let alias_value = format!("{field}-secret-value");
        let toml = route_manifest_toml(&format!(
            r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
{field} = "{alias_value}"
"#
        ));

        let message = parse_panic_message(&toml);
        assert!(
            message.contains(&format!(
                "must not use noncanonical {field}; use {replacement}"
            )),
            "unexpected alias rejection for {field}: {message}"
        );
        assert!(!message.contains(&alias_value));
    }
}

#[test]
fn route_manifest_toml_rejects_noncanonical_bsc_aliases_without_canonical_fields() {
    let toml = route_manifest_toml(&format!(
        r#"
sccp_bsc_source_bridge_address = "{SOURCE_BRIDGE}"
sccp_bsc_destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical sccp_bsc_source_bridge_address"));
    assert!(message.contains("use source_bridge_address"));
    assert!(!message.contains(SOURCE_BRIDGE));
}

#[test]
fn route_manifest_toml_rejects_noncanonical_tron_source_bridge_alias() {
    let toml = route_manifest_toml(&format!(
        r#"
sccp_tron_source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical sccp_tron_source_bridge_address"));
    assert!(message.contains("use source_bridge_address"));
    assert!(!message.contains(SOURCE_BRIDGE));
}

#[test]
fn route_manifest_toml_rejects_noncanonical_tron_verifier_alias() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
tron_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical tron_verifier_address"));
    assert!(message.contains("use destination_verifier_address"));
    assert!(!message.contains(VERIFIER));
}

#[test]
fn route_manifest_toml_rejects_noncanonical_tron_network_alias_with_canonical_network() {
    let legacy_network = "legacy-network-token";
    let toml = route_manifest_toml(&format!(
        r#"
tron_network = "{legacy_network}"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical tron_network"));
    assert!(message.contains("use network"));
    assert!(!message.contains(legacy_network));
}

#[test]
fn route_manifest_toml_rejects_noncanonical_tron_source_alias_with_canonical_address() {
    let legacy_source = "legacy-source-bridge";
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
sccp_tron_source_bridge_address = "{legacy_source}"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical sccp_tron_source_bridge_address"));
    assert!(message.contains("use source_bridge_address"));
    assert!(!message.contains(legacy_source));
}

#[test]
fn route_manifest_toml_rejects_noncanonical_tron_destination_alias_with_canonical_address() {
    let legacy_verifier = "legacy-destination-verifier";
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
sccp_tron_destination_verifier_address = "{legacy_verifier}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("must not use noncanonical sccp_tron_destination_verifier_address"));
    assert!(message.contains("use destination_verifier_address"));
    assert!(!message.contains(legacy_verifier));
}

#[test]
fn route_manifest_toml_rejects_missing_source_bridge_alias() {
    let toml = route_manifest_toml(&format!(
        r#"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("requires source bridge address"));
}

#[test]
fn route_manifest_toml_rejects_missing_destination_verifier_alias() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("requires destination verifier address"));
}
