//! Fails when the `#[main]` entrypoint returns a result.
// compile-flags: --crate-type lib
use iroha_smart_contract::prelude::*;
use iroha_smart_contract_derive::main;

#[main]
fn contract_main(host: Iroha, context: Context) -> i32 {
    let _ = (host, context);
    0
}
