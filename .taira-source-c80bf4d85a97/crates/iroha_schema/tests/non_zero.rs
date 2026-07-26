//! Schema coverage for core `NonZero*` integers.

use core::any::TypeId;

use impls::impls;
use iroha_schema::{IntoSchema, MetaMap, Metadata, UnnamedFieldsMeta};

macro_rules! check_non_zero {
    ($($ty:ty => $inner:ty),* $(,)?) => {
        $(
            let mut map = MetaMap::new();
            <$ty>::update_schema_map(&mut map);

            let schema = map
                .get::<$ty>()
                .unwrap_or_else(|| panic!("missing schema for {}", stringify!($ty)));
            assert_eq!(
                schema,
                &Metadata::Tuple(UnnamedFieldsMeta {
                    types: vec![TypeId::of::<$inner>()],
                })
            );

            assert!(map.contains_key::<$inner>());
        )*
    };
}

#[test]
fn non_zero_integers_schema() {
    check_non_zero! {
        core::num::NonZeroU8 => u8,
        core::num::NonZeroU16 => u16,
        core::num::NonZeroU32 => u32,
        core::num::NonZeroU64 => u64,
        core::num::NonZeroU128 => u128,
        core::num::NonZeroI8 => i8,
        core::num::NonZeroI16 => i16,
        core::num::NonZeroI32 => i32,
        core::num::NonZeroI64 => i64,
        core::num::NonZeroI128 => i128,
    }
}

#[test]
fn arch_dependent_non_zero_are_excluded() {
    // Keeping architecture-dependent `isize`/`usize` out of the schema surface
    // ensures cross-platform determinism.
    assert!(!impls!(core::num::NonZeroUsize: IntoSchema));
    assert!(!impls!(core::num::NonZeroIsize: IntoSchema));
}
