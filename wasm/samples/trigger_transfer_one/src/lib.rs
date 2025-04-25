//! Transfer one rose to Bob

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use dlmalloc::GlobalDlmalloc;
use iroha_trigger::prelude::*;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) {
    let rose = AssetId::new("rose#wonderland".parse().unwrap(), context.authority);
    let bob: AccountId =
        "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016@wonderland"
            .parse()
            .unwrap();

    host.submit(&Transfer::asset_numeric(rose, Numeric::ONE, bob))
        .dbg_expect("should transfer a rose");
}
