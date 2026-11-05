//! The schema adapter preserves malformed validator diagnostics.
#[derive(iroha_schema_derive::IntoSchema)]
#[norito(validate = "Self::validate()")]
struct InvalidValidator {
    value: u8,
}

fn main() {}
