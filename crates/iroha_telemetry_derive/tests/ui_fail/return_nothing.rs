use iroha_core::state::StateTransaction;
use iroha_telemetry_derive::metrics;

#[metrics(+"test_query", "another_test_query_without_timing")]
fn execute(_state_transaction: &StateTransaction) {
    Ok::<(), ()>(());
}

fn main() {}
