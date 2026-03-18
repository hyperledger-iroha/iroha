//! Fails when smart contract `#[main]` receives arguments.
use iroha_smart_contract::prelude::*;
use iroha_smart_contract_derive::main;

#[main(debug)]
fn main(host: Iroha, context: Context) {
    let _ = (host, context);
}
