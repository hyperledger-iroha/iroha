use iroha_telemetry_derive::metrics;

#[metrics("metric with space")]
fn execute(state: &StateTransaction) -> Result<(), ()> {
    let _ = state;
    Ok(())
}

fn main() {}
