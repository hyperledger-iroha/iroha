//! Passing coverage for executor entrypoint attribute macros.
#![allow(dead_code)]

use iroha_executor::prelude::*;
use iroha_executor::data_model::query::AnyQueryBox;
use iroha_executor_derive::{entrypoint, migrate};

#[entrypoint]
fn execute_instruction(
    instruction: InstructionBox,
    host: Iroha,
    context: Context,
) -> Result {
    let _ = (instruction, host, context);
    Ok(())
}

#[entrypoint]
fn validate_query(query: AnyQueryBox, host: Iroha, context: Context) -> Result {
    let _ = (query, host, context);
    Ok(())
}

#[migrate]
fn migrate(host: Iroha, context: Context) {
    let _ = (host, context);
}

fn main() {}
