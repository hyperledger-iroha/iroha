//! Ensure Norito defaults to compact sequential encoding.

use norito::core::{self, DecodeFlagsGuard, header_flags};

#[test]
fn default_encode_flags_use_compact_lengths() {
    assert_eq!(core::default_encode_flags(), header_flags::COMPACT_LEN);
}

#[test]
fn serialize_sets_only_compact_len_by_default() {
    let payload: Vec<u32> = (0..128u32).collect();
    let bytes = norito::to_bytes(&payload).expect("encode");
    let header_size = core::Header::SIZE;
    assert!(bytes.len() >= header_size);
    let flags = bytes[header_size - 1];
    assert_eq!(
        flags & header_flags::COMPACT_LEN,
        header_flags::COMPACT_LEN,
        "default payloads should advertise compact length prefixes"
    );
    assert_eq!(
        flags
            & (header_flags::PACKED_SEQ | header_flags::PACKED_STRUCT | header_flags::FIELD_BITSET),
        0,
        "default payloads should remain sequential"
    );
}

#[test]
fn decode_flags_guard_sanitizes_reserved_bits() {
    core::reset_decode_state();
    assert!(core::use_compact_len());
    assert!(!core::use_packed_seq());
    assert!(!core::use_packed_struct());

    {
        let _guard = DecodeFlagsGuard::enter(
            header_flags::COMPACT_LEN
                | header_flags::COMPACT_SEQ_LEN
                | header_flags::VARINT_OFFSETS
                | header_flags::PACKED_SEQ
                | header_flags::PACKED_STRUCT,
        );
        assert!(core::use_compact_len());
        assert!(core::use_packed_seq());
        assert!(core::use_packed_struct());
        assert_eq!(
            core::get_decode_flags(),
            header_flags::COMPACT_LEN | header_flags::PACKED_SEQ | header_flags::PACKED_STRUCT,
            "reserved flags should be masked out"
        );
    }

    core::reset_decode_state();
}
