//! Container enum derive tests for `FromVariant`.

/// Evaluate at compile time whether a type implements a trait.
///
/// Supports `impls!(T: Trait)` and `impls!(T: !Trait)`. The inherent
/// `IMPLEMENTS` constant only applies when the bound holds, and
/// associated-item resolution prefers it over the `false` trait fallback.
/// Defined before `enum_from_variant_attrs` so that module can use it too.
macro_rules! impls {
    ($ty:ty: !$trait:path) => {
        !impls!($ty: $trait)
    };
    ($ty:ty: $trait:path) => {{
        #[allow(dead_code)]
        trait DoesNotImplement {
            const IMPLEMENTS: bool = false;
        }
        impl<T: ?Sized> DoesNotImplement for T {}
        struct Probe<T: ?Sized>(::core::marker::PhantomData<T>);
        #[allow(dead_code)]
        impl<T: ?Sized + $trait> Probe<T> {
            const IMPLEMENTS: bool = true;
        }
        <Probe<$ty>>::IMPLEMENTS
    }};
}

#[allow(dead_code)]
#[path = "enum_from_variant_attrs.rs"]
mod enum_from_variant_attrs;
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

#[test]
fn impls_macro_distinguishes_present_and_absent_traits() {
    assert!(impls!(u8: Copy));
    assert!(!impls!(Variant1: Copy));
    assert!(impls!(Variant1: !Clone));
}
