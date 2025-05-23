//! A trigger that recursively invokes itself, incrementing both its current depth and the maximum allowed depth on each invocation.
//! The intended use case is unclear; this simply documents its behavior.
//! Note that the current execution depth cannot be inspected.
//! It loops indefinitely under `Repeats::Indefinitely`.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use dlmalloc::GlobalDlmalloc;
use iroha_trigger::{
    data_model::parameter::SmartContractParameter,
    prelude::{Parameter::SmartContract, *},
};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[iroha_trigger::main]
fn main(host: Iroha, context: Context) {
    let EventBox::Data(_data_event) = context.event else {
        dbg_panic!("expected: any DataEvent");
    };

    let max_depth = {
        let parameters: Parameters = host.query_single(FindParameters).dbg_unwrap();
        parameters.smart_contract().execution_depth()
    };

    let instruction = SetParameter::new(SmartContract(SmartContractParameter::ExecutionDepth(
        max_depth + 1,
    )));
    host.submit(&instruction).dbg_unwrap();
}
