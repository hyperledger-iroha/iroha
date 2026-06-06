//! Validate SCCP route manifest address aliases through the real TOML loader.

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
    format!(
        r#"
[zk]
sccp_allow_unready_transparent_proofs = true

[[zk.sccp_route_manifests]]
version = 1
route_id = "taira_bsc_xor"
asset_key = "xor"
tron_network = "bsc-testnet"
chain = "bsc-testnet"
chain_id_hex = "0x61"
counterparty_domain = 2
verifier_target = "EvmContract"
production_ready = false
disabled_reason = "test route"
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

    if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_owned()
    } else {
        "<non-string panic>".to_owned()
    }
}

#[test]
fn generic_bsc_route_manifest_toml_parses_without_legacy_tron_address_fields() {
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
    assert_eq!(route.sccp_tron_source_bridge_address, SOURCE_BRIDGE);
    assert_eq!(route.tron_verifier_address, VERIFIER);
}

#[test]
fn generated_bsc_route_manifest_toml_accepts_matching_alias_mirrors() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
sccp_bsc_source_bridge_address = "{SOURCE_BRIDGE}"
bsc_source_bridge_address = "{SOURCE_BRIDGE}"
sccp_tron_source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
verifier_address = "{VERIFIER}"
sccp_bsc_destination_verifier_address = "{VERIFIER}"
bsc_verifier_address = "{VERIFIER}"
evm_verifier_address = "{VERIFIER}"
tron_verifier_address = "{VERIFIER}"
"#
    ));

    let actual = load_actual_config(&toml);
    let route = actual
        .zk
        .sccp_route_manifests
        .first()
        .expect("route manifest");

    assert_eq!(route.sccp_tron_source_bridge_address, SOURCE_BRIDGE);
    assert_eq!(route.tron_verifier_address, VERIFIER);
}

#[test]
fn legacy_tron_route_manifest_toml_still_parses() {
    let toml = route_manifest_toml(&format!(
        r#"
sccp_tron_source_bridge_address = "{SOURCE_BRIDGE}"
tron_verifier_address = "{VERIFIER}"
"#
    ));

    let actual = load_actual_config(&toml);
    let route = actual
        .zk
        .sccp_route_manifests
        .first()
        .expect("route manifest");

    assert_eq!(route.sccp_tron_source_bridge_address, SOURCE_BRIDGE);
    assert_eq!(route.tron_verifier_address, VERIFIER);
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

#[test]
fn route_manifest_toml_rejects_source_bridge_alias_drift() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
bsc_source_bridge_address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
destination_verifier_address = "{VERIFIER}"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("source bridge address aliases disagree"));
}

#[test]
fn route_manifest_toml_rejects_destination_verifier_alias_drift() {
    let toml = route_manifest_toml(&format!(
        r#"
source_bridge_address = "{SOURCE_BRIDGE}"
destination_verifier_address = "{VERIFIER}"
bsc_verifier_address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
"#
    ));

    let message = parse_panic_message(&toml);
    assert!(message.contains("destination verifier address aliases disagree"));
}
