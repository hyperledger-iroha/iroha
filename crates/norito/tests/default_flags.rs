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
        flags & !header_flags::COMPACT_LEN,
        0,
        "default payloads advertise no other layout bits"
    );
}
#[test]
fn decode_flags_guard_sanitizes_reserved_bits() {
    core::reset_decode_state();
    assert!(core::use_compact_len());
    {
        // 0x01, 0x04, 0x08, 0x10 and 0x20 are reserved layout bits.
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN | 0x05 | 0x08 | 0x10 | 0x20);
        assert!(core::use_compact_len());
        assert_eq!(
            core::get_decode_flags(),
            header_flags::COMPACT_LEN,
            "reserved flags should be masked out"
        );
    }
    {
        let _guard = DecodeFlagsGuard::enter(0x05);
        assert!(!core::use_compact_len());
        assert_eq!(
            core::get_decode_flags(),
            0,
            "reserved flags should be masked out"
        );
    }
    core::reset_decode_state();
}
