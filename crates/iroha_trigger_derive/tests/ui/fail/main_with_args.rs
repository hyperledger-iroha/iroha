//! The `#[main]` attribute should not accept arguments.

use iroha_trigger::prelude::*;

#[iroha_trigger::main(extra)]
fn main(host: Iroha, context: Context) {
    let _ = (host, context);
}
