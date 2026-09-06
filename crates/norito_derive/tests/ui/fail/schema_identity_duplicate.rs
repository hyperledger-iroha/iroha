//! Each type has exactly one declared nominal identity.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "example::A", name = "example::B")]
struct Duplicate;
fn main() {}
