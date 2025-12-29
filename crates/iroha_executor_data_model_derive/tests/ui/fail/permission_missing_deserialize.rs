//! `Permission` derive should require JSON (de)serialization.

use iroha_executor_data_model_derive::Permission;

/// Skipping deserialization support should fail the derive.
#[derive(norito::derive::JsonSerialize, iroha_schema::IntoSchema, Permission)]
pub struct MissingDeserialize {
    /// Marker field.
    pub scope: String,
}

fn main() {
    let _ = MissingDeserialize {
        scope: "x".to_owned(),
    };
}
