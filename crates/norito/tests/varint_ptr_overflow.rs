//! Compact length prefixes reject overlong and overflowing varints.
use norito::core::{self, header_flags};

#[test]
fn compact_length_prefix_rejects_overlong_and_overflowing_varints() {
    let flags = header_flags::COMPACT_LEN;
    // Eleven continuation bytes: longer than any `u64` varint.
    assert!(matches!(
        core::read_len_from_slice_with_flags(&[0xff; 11], flags),
        Err(norito::Error::LengthMismatch)
    ));
    // Ten bytes whose final group overflows 64 bits.
    let mut overflow = [0xff_u8; 10];
    overflow[9] = 0x02;
    assert!(matches!(
        core::read_len_from_slice_with_flags(&overflow, flags),
        Err(norito::Error::LengthMismatch)
    ));
    // A redundant continuation byte is not the shortest encoding.
    assert!(matches!(
        core::read_len_from_slice_with_flags(&[0x81, 0x00], flags),
        Err(norito::Error::LengthMismatch)
    ));
    // The longest valid prefix, `u64::MAX` reads in ten bytes. Its value
    // exceeds every field ceiling only when a decode scope is active.
    let mut longest = [0xff_u8; 10];
    longest[9] = 0x01;
    let (value, used) = core::inspect_raw_len_from_slice_with_flags(&longest, flags)
        .expect("ten-byte maximum varint");
    assert_eq!((value as u64, used), (u64::MAX, 10));
}
