//! Smartcontract which creates new NFT for every user
#![no_std]

extern crate alloc;
#[cfg(not(test))]
extern crate panic_halt;

use alloc::{format, string::ToString};

use dlmalloc::GlobalDlmalloc;
use iroha_trigger::prelude::*;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) {
    iroha_trigger::log::info!("Executing trigger");

    if !matches!(context.event, EventBox::Time(_)) {
        dbg_panic!("Only work as a by call trigger");
    };

    let bad_domain_ids: [DomainId; 3] = [
        "system".parse().dbg_unwrap(),
        "genesis".parse().dbg_unwrap(),
        "garden_of_live_flowers".parse().dbg_unwrap(),
    ];

    for account in host.query(FindAccounts).execute().dbg_unwrap() {
        let account = account.dbg_unwrap();

        if bad_domain_ids.contains(account.id().domain()) {
            continue;
        }

        let mut metadata = Metadata::default();
        let name = format!(
            "nft_for_{}_in_{}",
            account.id().signatory(),
            account.id().domain()
        )
        .parse()
        .dbg_unwrap();
        metadata.insert(name, true);

        let nft_id = generate_new_nft_id(&host, account.id());
        let register_nft = Register::nft(Nft::new(nft_id.clone(), metadata));
        let transfer_nft = Transfer::nft(context.authority.clone(), nft_id, account.id().clone());
        host.submit_all::<InstructionBox>(&[register_nft.into(), transfer_nft.into()])
            .dbg_unwrap();
    }

    iroha_trigger::log::info!("Smart contract executed successfully");
}

fn generate_new_nft_id(host: &Iroha, account_id: &AccountId) -> NftId {
    let nfts = host
        .query(FindNfts)
        .filter_with(|nft| nft.owned_by.eq(account_id.clone()))
        .execute()
        .dbg_unwrap();

    let new_number = nfts
        .map(|res| res.dbg_unwrap())
        .filter(|nft| nft.id().to_string().starts_with("nft_"))
        .count()
        .checked_add(1)
        .dbg_unwrap();
    iroha_trigger::log::debug!(&format!("New number: {new_number}"));

    format!(
        "nft_number_{}_for_{}${}",
        new_number,
        account_id.signatory(),
        account_id.domain()
    )
    .parse()
    .dbg_unwrap()
}
