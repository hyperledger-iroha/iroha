// compile-flags: --crate-type lib
//! Compile-pass coverage for the public smart-contract prelude.

use iroha_smart_contract::prelude::*;

#[iroha_smart_contract::main]
fn contract_main(host: Iroha, context: Context) {
    let _context = context;
    let _ = host.submit(&Log::new(Level::INFO, "trybuild contract".to_owned()));
    let _ = host.query_single(FindParameters);
}

fn main() {}
