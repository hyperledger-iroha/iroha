//! Fails when parameter type does not implement `Default`.
use iroha_executor_data_model::json_macros::{JsonDeserialize, JsonSerialize};
use iroha_executor_data_model_derive::Parameter;
use iroha_schema::IntoSchema;

#[derive(JsonSerialize, JsonDeserialize, IntoSchema, Parameter)]
struct MissingDefault {
    value: String,
}

fn main() {
    let _ = MissingDefault {
        value: "x".to_owned(),
    };
}
