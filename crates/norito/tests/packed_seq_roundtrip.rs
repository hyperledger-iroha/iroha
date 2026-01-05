//! Roundtrip coverage for packed-sequence layouts.

use std::collections::BTreeMap;

use norito::{
    codec::encode_with_header_flags,
    core::{DecodeFlagsGuard, decode_from_bytes, frame_bare_with_header_flags, header_flags},
};

#[test]
fn packed_seq_vec_roundtrip() {
    let value = vec![1u32, 2, 3, 4];
    let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
    let (payload, flags) = encode_with_header_flags(&value);
    assert_ne!(flags & header_flags::PACKED_SEQ, 0);
    let bytes =
        frame_bare_with_header_flags::<Vec<u32>>(&payload, flags).expect("frame packed-seq vec");
    let decoded: Vec<u32> = decode_from_bytes(&bytes).expect("decode packed-seq vec");
    assert_eq!(decoded, value);
}

#[test]
fn packed_seq_empty_vec_roundtrip_fixed_offsets() {
    let value: Vec<u32> = Vec::new();
    let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
    let (payload, flags) = encode_with_header_flags(&value);
    assert_ne!(flags & header_flags::PACKED_SEQ, 0);
    let bytes = frame_bare_with_header_flags::<Vec<u32>>(&payload, flags)
        .expect("frame packed-seq empty vec");
    let decoded: Vec<u32> = decode_from_bytes(&bytes).expect("decode packed-seq empty vec");
    assert_eq!(decoded, value);
}

#[test]
fn packed_seq_btreemap_roundtrip_varint_offsets() {
    let mut map = BTreeMap::new();
    map.insert("a".to_string(), 1u64);
    map.insert("b".to_string(), 2u64);
    let flags = header_flags::PACKED_SEQ | header_flags::VARINT_OFFSETS;
    let _guard = DecodeFlagsGuard::enter(flags);
    let (payload, flags) = encode_with_header_flags(&map);
    assert_ne!(flags & header_flags::PACKED_SEQ, 0);
    assert_ne!(flags & header_flags::VARINT_OFFSETS, 0);
    let bytes = frame_bare_with_header_flags::<BTreeMap<String, u64>>(&payload, flags)
        .expect("frame packed-seq map");
    let decoded: BTreeMap<String, u64> = decode_from_bytes(&bytes).expect("decode packed map");
    assert_eq!(decoded, map);
}
