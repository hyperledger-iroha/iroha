//! Fails when entrypoint attribute has arguments.
use iroha_executor::prelude::*;
use iroha_executor_derive::entrypoint;

#[entrypoint(invalid)]
fn execute_transaction(
    transaction: SignedTransaction,
    host: Iroha,
    context: Context,
) -> Result {
    let _ = (transaction, host, context);
    Ok(())
}

fn main() {}
