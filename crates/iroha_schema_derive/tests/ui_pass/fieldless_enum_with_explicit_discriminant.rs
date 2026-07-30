//! Literal Rust discriminants are valid schema discriminants.

use iroha_schema::IntoSchema;

#[derive(IntoSchema)]
enum EnumWithExplicitDiscriminant {
    A = 1,
    B,
    C = 9,
    D,
}

fn main() {}
