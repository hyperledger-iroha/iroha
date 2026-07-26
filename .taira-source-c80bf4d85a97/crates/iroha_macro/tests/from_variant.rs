//! Integration test for the `FromVariant` derive macro.
//! Verifies that converting to and from enum variants works.

use iroha_macro::FromVariant;

#[derive(FromVariant, Debug, PartialEq)]
enum Enum {
    A(Box<u32>),
}

#[test]
fn from_variant_converts() {
    let en = Enum::from(Box::new(10_u32));
    let boxed: Box<u32> = Box::try_from(en).expect("convert back");
    assert_eq!(*boxed, 10);
}
