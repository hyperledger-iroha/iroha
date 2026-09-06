//! A fixed frame identity cannot collapse generic instantiations.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "example::Generic", frame = "example.generic")]
struct Generic<T>(T);
fn main() {}
