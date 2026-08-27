//! Enforce removal of retired first-release P2P configuration surfaces.

use std::path::PathBuf;

use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

#[test]
fn retired_packet_loss_configuration_keys_are_rejected() {
    for retired_field in [
        "debug_packet_loss_inbound_percent",
        "debug_packet_loss_outbound_percent",
    ] {
        let table = format!("[network]\n{retired_field} = 5\n")
            .parse()
            .expect("retired packet-loss TOML should be syntactically valid");
        let error = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect_err("retired packet-loss configuration must be unknown");
        let report = format!("{error:?}");
        assert!(report.contains("unknown parameter"), "{report}");
        assert!(report.contains(retired_field), "{report}");
    }
}

#[test]
fn retired_signed_ticket_verifier_key_is_rejected_as_unknown() {
    let table = r#"
[network.soranet_handshake.pow]
signed_ticket_public_key_hex = "00"
"#
    .parse()
    .expect("retired signed-ticket TOML should be syntactically valid");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("retired signed-ticket verifier configuration must be unknown");
    let report = format!("{error:?}");
    assert!(report.contains("unknown parameter"), "{report}");
    assert!(report.contains("signed_ticket_public_key_hex"), "{report}");
}
