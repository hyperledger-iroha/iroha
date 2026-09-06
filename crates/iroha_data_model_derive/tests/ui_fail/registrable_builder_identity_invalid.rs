//! A generated child identity must be a valid nonempty literal.
use iroha_data_model_derive::RegistrableBuilder;

#[derive(RegistrableBuilder)]
#[registrable_builder(schema_name = " leading whitespace")]
struct InvalidIdentity {
    id: u64,
}

fn main() {}
