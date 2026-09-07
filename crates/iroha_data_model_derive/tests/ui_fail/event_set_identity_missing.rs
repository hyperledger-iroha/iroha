//! Generated event sets require their own identity declaration.
use iroha_data_model_derive::EventSet;

#[derive(EventSet)]
enum MissingIdentity {
    Created,
}

fn main() {}
