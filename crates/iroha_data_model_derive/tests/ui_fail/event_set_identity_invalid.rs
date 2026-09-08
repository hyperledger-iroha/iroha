//! Generated event-set identities cannot be inferred by an expression.
use iroha_data_model_derive::EventSet;

#[derive(EventSet)]
#[event_set(schema_name = concat!("fixture", "::Identity"))]
enum InvalidIdentity {
    Created,
}

fn main() {}
