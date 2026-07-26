//! Fails when trigger `#[main]` receives arguments.
use iroha_trigger::prelude::*;
use iroha_trigger_derive::main;

#[main(trace)]
fn main(host: Iroha, context: Context) {
    let _ = (host, context);
}
