//! Duplicate canonical Norito enum indices must be rejected.

use iroha_schema::IntoSchema;

#[derive(IntoSchema)]
enum DuplicateIndex {
    #[codec(index = 1)]
    First,
    Second,
}

fn main() {}
