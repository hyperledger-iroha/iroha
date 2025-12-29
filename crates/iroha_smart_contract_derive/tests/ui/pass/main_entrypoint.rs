// compile-flags: --crate-type lib
//! Happy-path smart contract entrypoint.
use iroha_smart_contract::prelude::*;
use iroha_smart_contract_derive::main;

#[main]
fn contract_main(host: Iroha, context: Context) {
    let _ = (host, context);
}

fn main() {}
