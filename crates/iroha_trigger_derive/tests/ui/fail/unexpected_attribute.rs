use iroha_trigger::prelude::*;
use iroha_trigger_derive::main;

#[main(unexpected)]
fn trigger_main(host: Iroha, context: Context) {
    let _ = (host, context);
}
