//! Passing derive coverage for executor macros.
#![allow(dead_code)]

use iroha_executor::prelude::*;
use iroha_executor::data_model::isi::register::RegisterPeerWithPop;

#[derive(Visit, Entrypoints, Execute)]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

fn main() {
    let _ = RegisterPeerWithPop::new;
}
