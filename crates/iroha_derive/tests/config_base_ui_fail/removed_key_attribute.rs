//! Ensures the retired configuration-key remapping attribute is rejected.

use iroha_config_base::ReadConfig;

#[derive(ReadConfig)]
struct Test {
    #[config(key = "legacy")]
    value: u64,
}

fn main() {}
