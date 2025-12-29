//! `Parameter` derive should complain when JSON traits are missing.

use iroha_executor_data_model_derive::Parameter;

#[derive(Default, Parameter)]
struct MissingSerialization {
    amount: u64,
}

fn main() {
    let _ = MissingSerialization::default();
}
