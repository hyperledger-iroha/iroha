//! Trigger entrypoints must not return a value.
use iroha_trigger::prelude::*;
use iroha_trigger_derive::main;

#[main]
fn trigger_main(host: Iroha, context: Context) -> Result {
    let _ = (host, context);
    Ok(())
}
