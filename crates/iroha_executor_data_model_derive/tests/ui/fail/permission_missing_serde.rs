//! Fails when derived permission lacks serialization support.
use iroha_executor_data_model_derive::Permission;

struct NotSerializable;

#[derive(Permission)]
struct MissingSerde {
    field: NotSerializable,
}

fn main() {
    let _ = MissingSerde {
        field: NotSerializable,
    };
}
