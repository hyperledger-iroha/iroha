//! Golden checks for hybrid packed-struct bitset sizing behavior.

use std::collections::BTreeMap;

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as norito_core, DecodeFlagsGuard, header_flags},
};

fn encode_bare_with_flags<T: NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter(flags);
    let mut payload = Vec::new();
    value.serialize(&mut payload).expect("serialize");
    payload
}

#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct SelfDelimExample {
    id: u32,
    maybe: Option<u64>,
    values: Vec<u8>,
    labels: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct Nested {
    name: String,
}

#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct NeedsSize {
    id: u32,
    nested: Nested,
}

#[test]
fn packed_struct_bitset_skips_self_delimiting_fields() {
    let mut labels = BTreeMap::new();
    labels.insert("a".to_string(), 1);
    let value = SelfDelimExample {
        id: 7,
        maybe: Some(9),
        values: vec![1, 2, 3],
        labels,
    };
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let payload = encode_bare_with_flags(&value, flags);

    let bitset_len = 1usize;
    assert_eq!(
        payload[0], 0,
        "bitset should be empty for fixed/self-delimiting fields"
    );
    let id_bytes = value.id.to_le_bytes();
    assert_eq!(&payload[bitset_len..bitset_len + id_bytes.len()], id_bytes);
}

#[test]
fn packed_struct_bitset_emits_size_for_nested_structs() {
    let value = NeedsSize {
        id: 1,
        nested: Nested {
            name: "hi".to_string(),
        },
    };
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let payload = encode_bare_with_flags(&value, flags);

    assert_eq!(payload[0], 0b10, "nested field should set bit 1");
    let (nested_len, hdr_len) = {
        let _guard = DecodeFlagsGuard::enter(flags);
        norito_core::read_len_dyn_slice(&payload[1..]).expect("nested size header")
    };
    let nested_payload = encode_bare_with_flags(&value.nested, flags);
    assert_eq!(nested_len, nested_payload.len());

    let data_start = 1 + hdr_len;
    let id_bytes = value.id.to_le_bytes();
    assert_eq!(&payload[data_start..data_start + id_bytes.len()], id_bytes);
    assert_eq!(
        &payload[data_start + id_bytes.len()..data_start + id_bytes.len() + nested_payload.len()],
        nested_payload.as_slice()
    );
}
