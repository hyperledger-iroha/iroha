//! Restricted transaction gossip has one authenticated native-committee route.

use std::path::PathBuf;

use iroha_config::parameters::user::Root;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

#[test]
fn restricted_public_fallback_settings_are_unknown() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    for (field, values) in [
        (
            "transaction_gossip_restricted_fallback",
            ["drop", "public_overlay", "PUBLIC_OVERLAY"],
        ),
        (
            "transaction_gossip_restricted_public_payload",
            ["refuse", "forward", "FORWARD"],
        ),
    ] {
        for value in values {
            let overlay = format!("[network]\n{field} = {value:?}\n")
                .parse()
                .expect("test TOML");
            let error = ConfigReader::new()
                .without_env()
                .read_toml_with_extends(&base)
                .expect("base configuration")
                .with_toml_source(TomlSource::inline(overlay))
                .read_and_complete::<Root>()
                .expect_err("retired settings must not be accepted as no-op aliases");
            let report = format!("{error:?}");
            assert!(report.contains("unknown parameter"), "{report}");
            assert!(report.contains(&format!("network.{field}")), "{report}");
        }
    }
}
