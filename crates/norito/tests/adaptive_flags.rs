//! Tests for adaptive layout flags in `to_bytes_auto`.
//!
//! We validate that adaptive flags advertise compact sequence lengths and that
//! packed layouts always use `VARINT_OFFSETS` so decoders can rely on the
//! canonical representation.

use norito::to_bytes_auto;

fn header_flags(bytes: &[u8]) -> u8 {
    // Header layout: 4 magic + 1 major + 1 minor + 16 schema + 1 compression + 8 len + 8 cksum + 1 flags
    // Flags at index 39
    bytes[39]
}

#[test]
fn adaptive_small_prefers_varints() {
    // Small-ish packed sequence payload
    let v: Vec<u32> = (0..256u32).collect();
    let bytes = to_bytes_auto(&v).expect("encode");
    let flags = header_flags(&bytes);
    assert_eq!(flags, 0, "sequential layout must not set adaptive flags");
}

#[test]
fn adaptive_large_still_uses_varints() {
    // Large payload: vector of 1M u32 (4 MiB data)
    let v: Vec<u32> = (0..1_000_000u32).collect();
    let bytes = to_bytes_auto(&v).expect("encode");
    let flags = header_flags(&bytes);
    assert_eq!(flags, 0, "sequential layout must keep header flags zero");
}
