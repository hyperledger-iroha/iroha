//! A generated child identity cannot be declared twice.
use iroha_data_model_derive::RegistrableBuilder;

#[derive(RegistrableBuilder)]
#[registrable_builder(schema_name = "fixture::First")]
#[registrable_builder(schema_name = "fixture::Second")]
struct DuplicateIdentity {
    id: u64,
}

fn main() {}
