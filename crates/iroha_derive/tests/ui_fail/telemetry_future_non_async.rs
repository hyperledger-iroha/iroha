//! Ensures telemetry instrumentation rejects non-async functions.

#[iroha_derive::telemetry_future]
fn not_async() {}

fn main() {}
