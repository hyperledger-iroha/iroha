//! Grant roles upon each account registration.

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
    let volunteers: RoleId = "volunteers".parse().dbg_unwrap();

    host.submit(&Grant::account_role(volunteers, account.id().clone()))
        .dbg_expect("should succeed");
}
