//! Return types are forbidden for trigger entrypoints.

use iroha_trigger::prelude::*;

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) -> () {
    let _ = (host, context);
}
