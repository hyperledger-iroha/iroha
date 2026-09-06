//! A registration builder requires its own explicit child identity.
use iroha_data_model_derive::RegistrableBuilder;

#[derive(RegistrableBuilder)]
struct MissingIdentity {
    id: u64,
}

fn main() {}
