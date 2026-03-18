#![cfg(feature = "compression")]

//! Ensure zstd compression path roundtrips complex payloads and preserves flags.

use norito::{CompressionConfig, core::header_flags, decode_from_bytes, to_compressed_bytes};

#[test]
fn compressed_roundtrip_preserves_layout_flags() {
    let value = vec![
        (1u8, vec![0x10, 0x20, 0x30], Some(String::from("alpha"))),
        (2u8, vec![0x99; 128], None),
        (
            3u8,
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            Some(String::from("beta")),
        ),
    ];

    let bytes =
        to_compressed_bytes(&value, Some(CompressionConfig::default())).expect("compress payload");
    let decoded: Vec<(u8, Vec<u8>, Option<String>)> =
        decode_from_bytes(&bytes).expect("decode compressed payload");
    assert_eq!(decoded, value);

    let flags = bytes[norito::core::Header::SIZE - 1];
    assert_eq!(
        flags & header_flags::PACKED_SEQ,
        0,
        "sequential layout must not set packed sequence flag"
    );
}
