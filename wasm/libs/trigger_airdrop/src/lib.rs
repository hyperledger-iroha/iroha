//! Airdrop: mint roses on every account registration

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use dlmalloc::GlobalDlmalloc;
use iroha_trigger::prelude::*;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) {
    let EventBox::Data(DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(account)))) =
        context.event
    else {
        dbg_panic!("only account-created events should pass");
    };
    let rose_def: AssetDefinitionId = "rose#wonderland".parse().dbg_unwrap();
    let rose = AssetId::new(rose_def, account.id().clone());

    host.submit(&Mint::asset_numeric(100u32, rose))
        .dbg_expect("should mint");
}
