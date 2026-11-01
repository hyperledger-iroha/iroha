//! Aggregated integration tests for `iroha_schema`.

/// Evaluate at compile time whether a type implements a trait.
///
/// Supports `impls!(T: Trait)`, `impls!(T: !Trait)` and `impls!(T: A | B)`.
/// The inherent `IMPLEMENTS` constant only applies when the bound holds, and
/// associated-item resolution prefers it over the `false` trait fallback.
macro_rules! impls {
    ($ty:ty: !$trait:path) => {
        !impls!($ty: $trait)
    };
    ($ty:ty: $first:path | $second:path) => {
        impls!($ty: $first) || impls!($ty: $second)
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

#[test]
fn impls_macro_distinguishes_present_and_absent_traits() {
    struct Plain;
    assert!(impls!(u8: Copy));
    assert!(!impls!(Plain: Copy));
    assert!(impls!(Plain: !Clone));
    assert!(impls!(Plain: Clone | Send));
    assert!(!impls!(Plain: Clone | Copy));
}

mod common;
mod enum_composite_fields;

#[path = "architecture-dependent.rs"]
mod architecture_dependent;
#[path = "enum_with_default_discriminants.rs"]
mod enum_with_default_discriminants;
#[path = "enum_with_various_discriminants.rs"]
mod enum_with_various_discriminants;
#[path = "fieldless_enum.rs"]
mod fieldless_enum;
#[path = "floats.rs"]
mod floats;
#[path = "non_zero.rs"]
mod non_zero;
#[path = "numbers_compact_and_fixed.rs"]
mod numbers_compact_and_fixed;
#[path = "schema_json.rs"]
mod schema_json;
#[path = "struct_with_generic_bounds.rs"]
mod struct_with_generic_bounds;
#[path = "struct_with_named_fields.rs"]
mod struct_with_named_fields;
#[path = "struct_with_unnamed_fields.rs"]
mod struct_with_unnamed_fields;
#[path = "transparent_types.rs"]
mod transparent_types;
