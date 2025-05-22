//! Runtime Executor which changes amounts of fuel availible in the runtime.

#![no_std]

extern crate alloc;
#[cfg(not(test))]
extern crate panic_halt;

use alloc::{format, string::ToString};

use dlmalloc::GlobalDlmalloc;
use iroha_executor::prelude::*;
use iroha_trigger::log;
use serde::{Deserialize, Serialize};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[derive(Visit, Execute, Entrypoints)]
#[visit(custom(visit_instruction, visit_transaction))]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

// Transaction metadata contains fuel adjustments
fn visit_transaction(executor: &mut Executor, tx: &SignedTransaction) {
    let fuel_config: u64 = tx
        .metadata()
        .get("fuel")
        .expect("missing `fuel` metadata entry")
        .try_into_any()
        .expect("invalid `fuel` configuraion");

    let fuel = runtime::get_fuel();
    log::info!(&format!("initial fuel: {}", fuel));

    runtime::add_fuel(fuel_config);

    let fuel = runtime::get_fuel();
    log::info!(&format!("updated fuel amounts: {}", fuel));

    iroha_executor::default::visit_transaction(executor, tx)
}

// Custom visit_instruction is more computationally expensive
fn visit_instruction(executor: &mut Executor, isi: &InstructionBox) {
    runtime::consume_fuel(30_000_000);
    execute!(executor, isi);
}

#[iroha_executor::migrate]
fn migrate(_host: Iroha, _context: Context) {}
