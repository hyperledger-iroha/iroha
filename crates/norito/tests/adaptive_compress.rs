//! Tests for adaptive compression selection in Norito.
//!
//! These tests validate header compression tags chosen by `to_bytes_auto`
//! without depending on GPU availability. They do not assert specific sizes,
//! only that compression is skipped for small payloads and used for large ones.

use norito::{Compression, to_bytes_auto};

fn header_compression_tag(bytes: &[u8]) -> u8 {
    // Offsets: 4 magic + 1 major + 1 minor + 16 schema = 22
    bytes[22]
}

#[test]
fn auto_small_payload_no_compress() {
    let val = vec![1u8, 2, 3, 4, 5];
    let bytes = to_bytes_auto(&val).expect("encode");
    assert_eq!(header_compression_tag(&bytes), Compression::None as u8);
}

#[test]
fn auto_large_payload_uses_compress() {
    // 256 KiB payload exceeds default CPU threshold (2 KiB) and large threshold (128 KiB)
    let val = vec![0xABu8; 256 * 1024];
    let bytes = to_bytes_auto(&val).expect("encode");
    assert_eq!(header_compression_tag(&bytes), Compression::Zstd as u8);
}
