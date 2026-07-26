//! Happy-path executor derives and entrypoints.

use iroha_executor::prelude::*;
use iroha_executor::data_model::isi::register::RegisterPeerWithPop;

#[derive(Visit, Execute, Entrypoints)]
struct Executor {
    /// Host handle passed by the runtime.
    host: Iroha,
    /// Execution context for the current call.
    context: Context,
    /// Verdict accumulated during validation.
    verdict: Result,
}

#[iroha_executor::migrate]
fn migrate(host: Iroha, context: Context) {
    let _ = (host, context);
}

fn main() {
    // Ensure generated methods are reachable.
    let _ = Executor::visit_instruction as fn(&mut Executor, &InstructionBox);
    let _ = RegisterPeerWithPop::new;
}
