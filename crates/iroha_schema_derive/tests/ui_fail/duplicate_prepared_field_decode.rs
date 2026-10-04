//! Duplicate prepared binary field flags are rejected by schema derivation.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(decode_fields, decode_fields)]
struct Record {
    value: u8,
}
fn main() {}
