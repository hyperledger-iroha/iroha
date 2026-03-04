//! Architecture-dependent schema tests.
use impls::impls;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[test]
fn usize_isize_not_into_schema() {
    // The architecture-dependent
    assert!(!impls!(usize: IntoSchema));
    assert!(!impls!(isize: IntoSchema));

    // `usize` and `isize` can now be encoded and decoded in a
    // platform-independent manner but remain excluded from schema
    // generation.
    assert!(impls!(usize: Encode | Decode));
    assert!(impls!(isize: Encode | Decode));

    // There are no other primitive architecture-dependent types, so
    // as long as `IntoSchema` requires all variants and all fields to
    // also be `IntoSchema`, we are guaranteed that all schema types
    // are safe to exchange.
}
