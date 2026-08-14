//! Fieldless enum schema tests.
mod common;
use common::{assert_schema, entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};
#[derive(IntoSchema, Encode, Decode)]
enum Foo {
    A,
    B,
    C,
    D,
}
#[test]
fn discriminant() {
    assert_schema::<Foo>("enum_fieldless.discriminant", &[entry::<Foo>("Foo")]);
}
