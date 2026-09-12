//! Duplicate binary validation declarations are rejected by schema derivation.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(validate = "Self::first", validate = "Self::second")]
struct Record {
    value: u8,
}
fn main() {}
