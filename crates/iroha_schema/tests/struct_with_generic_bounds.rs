//! `IntoSchema` derive tests for generic bounds.
mod common;
use common::{assert_schema, entry};
#[derive(iroha_schema::IntoSchema)]
struct Foo<V> {
    _value: Option<V>,
}
#[test]
fn check_generic() {
    assert_schema::<Foo<bool>>(
        "generic.check_generic",
        &[
            entry::<bool>("bool"),
            entry::<Option<bool>>("Option<bool>"),
            entry::<Foo<bool>>("Foo<bool>"),
        ],
    );
}
