//! Smartcontract which creates new nft for every user
//!
//! This module isn't included in the build-tree,
//! but instead it is being built by a `client/build.rs`
#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use core::str::FromStr as _;

use iroha_wasm::data_model::prelude::*;

/// Mint 1 rose for authority
#[iroha_wasm::main(params = "[authority]")]
fn main(authority: <Account as Identifiable>::Id) {
    let rose_definition_id = <AssetDefinition as Identifiable>::Id::from_str("rose#wonderland")
        .dbg_expect("Failed to parse `rose#wonderland` asset definition id");
    let rose_id = <Asset as Identifiable>::Id::new(rose_definition_id, authority);

    MintBox::new(1_u32, rose_id).execute();
}
