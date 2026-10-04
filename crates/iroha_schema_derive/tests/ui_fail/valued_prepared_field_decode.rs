//! Prepared binary field flags cannot accept values during schema derivation.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(decode_fields = true)]
struct Record {
    value: u8,
}
fn main() {}
