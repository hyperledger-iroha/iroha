//! Return types are rejected for smart contract entrypoints.

use iroha_smart_contract::prelude::*;

#[iroha_smart_contract::main]
fn main(host: Iroha, context: Context) -> () {
    let _ = (host, context);
}
