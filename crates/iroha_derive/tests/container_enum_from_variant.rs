//! Container enum derive tests for `FromVariant`.
#[allow(dead_code)]
#[path = "enum_from_variant_attrs.rs"]
mod enum_from_variant_attrs;
use impls::impls;
struct Variant1;
struct Variant2;
#[derive(iroha_derive::FromVariant)]
enum Enum {
    Variant1(Box<Variant1>),
    Variant2(Variant2),
}
macro_rules! check_variant {
    ($variant:ty) => {
        assert!(impls!(Enum: From<$variant>), "Enum does not implement From<{}>", stringify!($variant));
        assert!(impls!($variant: TryFrom<Enum>), "{} does not implement TryFrom<Enum>", stringify!($variant));
    };
}
#[test]
fn conversions_use_only_the_exact_field_type() {
    check_variant!(Box<Variant1>);
    check_variant!(Variant2);
    assert!(impls!(Enum: !From<Variant1>));
}
