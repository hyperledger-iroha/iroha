//! Binary validation helpers must be quoted paths.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(validate = "Self::validate()")]
struct Record {
    value: u8,
}
fn main() {}
