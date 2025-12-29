//! Fails when required executor fields are missing.
use iroha_executor::data_model::isi::register::RegisterPeerWithPop;
use iroha_executor::prelude::*;

#[derive(Visit, Entrypoints, Execute)]
struct BrokenExecutor {
    host: Iroha,
}

fn main() {
    let _ = RegisterPeerWithPop::new;
}
