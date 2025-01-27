//! Airdrop: mint roses on every account registration

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use dlmalloc::GlobalDlmalloc;
use iroha_trigger::prelude::*;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

const AIRDROP_KEY: &str = "airdrop";

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) {
    let EventBox::Data(DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(account)))) =
        context.event
    else {
        dbg_panic!("only account-created events should pass");
    };
    let asset_defs_unfiltered = host
        .query(FindAssetsDefinitions)
        .execute_all()
        .dbg_unwrap();

    for asset_def in asset_defs_unfiltered {
        if asset_def.owned_by() != account.id() {
            continue;
        }
        let Some(amount) = asset_def
            .metadata()
            .get(AIRDROP_KEY)
            .and_then(|v| v.try_into_any::<u32>().ok())
        else {
            continue;
        };
        let asset = AssetId::new(asset_def.id().clone(), account.id().clone());

        host.submit(&Mint::asset_numeric(amount, asset))
            .dbg_expect("should mint");
    }
}
