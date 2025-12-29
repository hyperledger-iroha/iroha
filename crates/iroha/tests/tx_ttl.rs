//! Tests for transaction TTL configuration parsing

use iroha::config::UserConfig;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn config_with_ttl(ttl_ms: u64, timeout_ms: u64) -> toml::Table {
    toml::toml! {
        chain = "00000000-0000-0000-0000-000000000000"
        torii_url = "http://127.0.0.1:8080/"
        [account]
        domain = "wonderland"
        public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
        [transaction]
        time_to_live_ms = ttl_ms
        status_timeout_ms = timeout_ms
        nonce = false
    }
}

#[test]
fn accepts_ttl_not_smaller_than_minimum() {
    ConfigReader::new()
        .with_toml_source(TomlSource::inline(config_with_ttl(1000, 500)))
        .read_and_complete::<UserConfig>()
        .unwrap()
        .parse()
        .expect("should accept minimal ttl");
}

#[test]
fn rejects_ttl_smaller_than_minimum() {
    let err = ConfigReader::new()
        .with_toml_source(TomlSource::inline(config_with_ttl(999, 500)))
        .read_and_complete::<UserConfig>()
        .unwrap()
        .parse()
        .expect_err("should reject too-small ttl");
    let message = format!("{err:?}");
    assert!(
        message.contains("Transaction time-to-live should be at least"),
        "unexpected error report: {message}"
    );
}
