//! Multiple child identity declarations are ambiguous.
use iroha_data_model_derive::EventSet;

#[derive(EventSet)]
#[event_set(schema_name = "fixture::First")]
#[event_set(schema_name = "fixture::Second")]
enum DuplicateIdentity {
    Created,
}

fn main() {}
