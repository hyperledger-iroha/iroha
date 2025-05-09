//! Runtime Executor which copies default logic but forbids any queries and fails to migrate.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use dlmalloc::GlobalDlmalloc;
use iroha_executor::prelude::*;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[derive(Visit, Execute, Entrypoints)]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

#[iroha_executor::migrate]
fn migrate(host: Iroha, _context: Context) {
    // Performing side-effects to check in the test that it won't be applied after failure

    // Registering a new domain (using ISI)
    let domain_id = "failed_migration_test_domain".parse().unwrap();
    host.submit(&Register::domain(Domain::new(domain_id)))
        .unwrap();

    dbg_panic!("This executor always fails to migrate");
}
