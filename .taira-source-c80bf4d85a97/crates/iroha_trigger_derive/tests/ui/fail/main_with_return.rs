//! Fails when trigger `#[main]` returns a value.
use iroha_trigger::prelude::*;
use iroha_trigger_derive::main;

#[main]
fn main(host: Iroha, context: Context) -> u32 {
    let _ = (host, context);
    7
}
