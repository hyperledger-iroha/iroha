//! The `#[main]` attribute must not accept arguments.

use iroha_smart_contract::prelude::*;

#[iroha_smart_contract::main(unexpected)]
fn main(host: Iroha, context: Context) {
    let _ = (host, context);
}
