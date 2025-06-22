//! Smart contract which executes [`MintAsset`] for the caller.
//! TODO: Extend the smartcontact interface to accept input arguments (same as triggers)?

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use dlmalloc::GlobalDlmalloc;
use iroha_smart_contract::{
    data_model::query::{
        builder::QueryExecutor,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::{ForwardCursor, QueryParams},
        QueryWithFilter, QueryWithParams,
    },
    prelude::*,
};
use nonzero_ext::nonzero;
use parity_scale_codec::{Decode, DecodeAll, Encode};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

/// Execute [`MintAsset`] for the caller.
/// NOTE: DON'T TAKE THIS AS AN EXAMPLE, THIS IS ONLY FOR TESTING INTERNALS OF IROHA
#[iroha_smart_contract::main]
fn main(host: Iroha, context: Context) {
    let rose_definition_id = "rose#wonderland".parse().unwrap();
    let rose_id = AssetId::new(rose_definition_id, context.authority);

    host.submit(&Mint::asset_numeric(1_u32, rose_id))
        .dbg_expect("Failed to mint rose");
}
