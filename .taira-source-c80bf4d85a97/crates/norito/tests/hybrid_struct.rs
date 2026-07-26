//! Ensure hybrid packed-struct layout (FIELD_BITSET) decodes without panics
//! for self-delimiting container fields like `Vec` and `String`.
#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};

#[derive(Debug, Clone, PartialEq, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SampleHybrid {
    id: u64,
    name: String,
    values: Vec<u32>,
}

#[test]
fn hybrid_struct_roundtrip() {
    let v = SampleHybrid {
        id: 42,
        name: "hello".repeat(5),
        values: (0..64).collect(),
    };
    let bytes = to_bytes(&v).expect("encode");
    let out: SampleHybrid = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(v, out);
}
