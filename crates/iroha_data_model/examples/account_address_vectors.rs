//! Generate dual-format account address compliance vectors.
//!
//! Preferred workflow: `cargo xtask address-vectors --out fixtures/account/address_vectors.json`
//! Use this example only when you need to experiment with alternative targets or formats.

use iroha_data_model::account::address::compliance_vectors::compliance_vectors_json;
use norito::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = compliance_vectors_json();
    println!("{}", json::to_string_pretty(&root)?);
    Ok(())
}
