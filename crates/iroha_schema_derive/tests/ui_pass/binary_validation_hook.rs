//! Binary validation has no effect on schema generation.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
#[norito(validate = "not_a_schema_dependency::validate")]
struct Record {
    value: u8,
}
fn main() {
    let _ = Record::schema();
}
