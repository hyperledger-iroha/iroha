//! Schema metadata test verifying compact vs fixed integer encodings.
mod common;
use common::{assert_schema, entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};
#[derive(IntoSchema, Encode, Decode)]
struct Foo {
    #[codec(compact)]
    u8_compact: u8,
    u8_fixed: u8,
    #[codec(compact)]
    u16_compact: u16,
    u16_fixed: u16,
    #[codec(compact)]
    u32_compact: u32,
    u32_fixed: u32,
    #[codec(compact)]
    u64_compact: u64,
    u64_fixed: u64,
    #[codec(compact)]
    u128_compact: u128,
    u128_fixed: u128,
}
#[test]
fn compact() {
    assert_schema::<Foo>(
        "compact_fixed.compact",
        &[
            entry::<Compact<u128>>("Compact<u128>"),
            entry::<Compact<u16>>("Compact<u16>"),
            entry::<Compact<u32>>("Compact<u32>"),
            entry::<Compact<u64>>("Compact<u64>"),
            entry::<Compact<u8>>("Compact<u8>"),
            entry::<Foo>("Foo"),
            entry::<u128>("u128"),
            entry::<u16>("u16"),
            entry::<u32>("u32"),
            entry::<u64>("u64"),
            entry::<u8>("u8"),
        ],
    );
}
