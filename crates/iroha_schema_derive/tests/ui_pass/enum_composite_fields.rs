//! Composite enum payloads derive their complete schema.
use iroha_schema::IntoSchema;
#[derive(IntoSchema)]
enum NamedVariant {
    Foo { first: u8, second: u8 },
    Pair(u32, u64),
    Empty(),
    EmptyNamed {},
}
fn main() {}
