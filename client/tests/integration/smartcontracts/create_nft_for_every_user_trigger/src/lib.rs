//! Smartcontract which creates new nft for every user
//!
//! This module isn't included in the build-tree,
//! but instead it is being built by a `client/build.rs`

#![no_std]
#![no_main]
#![allow(clippy::all)]

extern crate alloc;
#[cfg(not(test))]
extern crate panic_halt;

use alloc::{format, string::ToString, vec::Vec};

use iroha_wasm::{
    data_model::{prelude::*, Registered},
    debug::DebugUnwrapExt as _,
    ExecuteOnHost as _,
};

#[iroha_wasm::entrypoint]
fn trigger_entrypoint() {
    iroha_wasm::info!("Executing trigger");

    let query = QueryBox::from(FindAllAccounts);
    let accounts: Vec<Account> = query.execute().try_into().dbg_unwrap();

    let limits = MetadataLimits::new(256, 256);

    for account in accounts {
        let mut metadata = Metadata::new();
        let name = format!(
            "nft_for_{}_in_{}",
            account.id().name(),
            account.id().domain_id()
        )
        .parse()
        .dbg_unwrap();
        metadata
            .insert_with_limits(name, true.into(), limits)
            .dbg_unwrap();

        let nft_id = generate_new_nft_id(account.id());
        let nft_definition = AssetDefinition::store(nft_id.clone())
            .mintable_once()
            .with_metadata(metadata);
        let account_nft_id = <Asset as Identifiable>::Id::new(nft_id, account.id().clone());
        let account_nft = <Asset as Registered>::With::new(account_nft_id, Metadata::new());

        InstructionBox::from(RegisterBox::new(nft_definition)).execute();
        InstructionBox::from(RegisterBox::new(account_nft)).execute();
    }

    iroha_wasm::info!("Smart contract executed successfully");
}

fn generate_new_nft_id(account_id: &<Account as Identifiable>::Id) -> AssetDefinitionId {
    let query = QueryBox::from(FindAssetsByAccountId::new(account_id.clone()));
    let assets: Vec<Asset> = query.execute().try_into().dbg_unwrap();

    let new_number = assets
        .into_iter()
        .filter(|asset| asset.id().definition_id().to_string().starts_with("nft_"))
        .count()
        .checked_add(1)
        .dbg_unwrap();
    iroha_wasm::debug!(&format!("New number: {}", new_number));

    format!(
        "nft_number_{}_for_{}#{}",
        new_number,
        account_id.name(),
        account_id.domain_id()
    )
    .parse()
    .dbg_unwrap()
}
