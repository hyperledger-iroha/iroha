//! Fails when smart contract `#[main]` returns a value.
use iroha_smart_contract::prelude::*;
use iroha_smart_contract_derive::main;

#[main]
fn main(host: Iroha, context: Context) -> i32 {
    let _ = (host, context);
    1
}
