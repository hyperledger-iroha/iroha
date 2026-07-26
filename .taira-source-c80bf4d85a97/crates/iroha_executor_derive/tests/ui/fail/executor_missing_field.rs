//! Executor derives should enforce required fields.

use iroha_executor::data_model::isi::register::RegisterPeerWithPop;
use iroha_executor::prelude::*;

#[derive(Visit)]
struct IncompleteExecutor {
    /// Host handle.
    host: Iroha,
    /// Execution context.
    context: Context,
}

fn main() {
    let _ = RegisterPeerWithPop::new;
}
