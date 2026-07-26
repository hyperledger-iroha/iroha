//! Entry point attribute should restrict allowed function names.

use iroha_executor::prelude::*;

#[iroha_executor::entrypoint]
fn wrong_entrypoint(host: Iroha, context: Context) -> Result {
    let _ = (host, context);
    Ok(())
}

fn main() {}
