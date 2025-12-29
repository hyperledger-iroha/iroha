// compile-flags: --crate-type lib
use iroha_smart_contract::prelude::*;
use iroha_smart_contract_derive::main;

#[main(unexpected)]
fn contract_main(host: Iroha, context: Context) {
    let _ = (host, context);
}
