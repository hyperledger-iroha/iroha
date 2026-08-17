//! Schema metadata test covering enums with implicit discriminants.
use crate::common::{assert_schema, entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};
#[derive(IntoSchema, Encode, Decode)]
enum Foo {
    Variant1(bool),
    Variant2(String),
    Variant3(Result<bool, String>),
    #[codec(skip)]
    _Variant4,
    Variant5(i32),
}
#[test]
fn default_discriminants() {
    assert_schema::<Foo>(
        "enum_default.default_discriminants",
        &[
            entry::<Result<bool, String>>("Result<bool, String>"),
            entry::<String>("String"),
            entry::<bool>("bool"),
            entry::<Foo>("Foo"),
            entry::<i32>("i32"),
        ],
    );
}
