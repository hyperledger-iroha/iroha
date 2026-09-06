//! Identity declarations belong to types, not fields.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "example::Nested")]
struct Nested {
    #[norito_schema(name = "example::Field")]
    field: u8,
}
fn main() {}
