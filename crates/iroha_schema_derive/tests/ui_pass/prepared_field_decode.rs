//! Prepared binary field destinations do not alter schema generation.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(decode_fields)]
struct Record {
    value: u8,
}
fn main() {
    let _ = Record::schema();
}
