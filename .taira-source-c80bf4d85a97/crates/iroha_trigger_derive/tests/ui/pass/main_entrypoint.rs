//! Happy-path trigger entrypoint.
use iroha_trigger::prelude::*;
use iroha_trigger_derive::main;

#[main]
fn trigger_main(host: Iroha, context: Context) {
    let _ = (host, context);
}

fn main() {}
