//! Feature-independent fixtures for rejecting noncanonical archive headers.

/// Representative supported layouts used to test fixed V1 boundaries.
pub(crate) fn supported_layouts() -> [u8; 8] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}

/// Change only the compression tag of a canonical frame.
///
/// The payload remains uncompressed on purpose: a canonical boundary must
/// reject this header before it attempts payload decoding or decompression.
pub(crate) fn with_compression_tag<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
    let mut frame = norito::encode_canonical(value).expect("canonical fixture");
    let header = norito::core::Header::read(frame.as_slice()).expect("fixture header");
    assert_eq!(header.compression, norito::Compression::None);
    // Norito V1: magic, major, minor, schema, then the compression tag.
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    frame[compression_offset] = norito::Compression::Zstd as u8;
    assert_eq!(
        norito::core::Header::read(frame.as_slice())
            .expect("valid header with forbidden compression")
            .compression,
        norito::Compression::Zstd
    );
    frame
}

#[test]
fn compression_tag_is_rejected_before_missing_or_uncompressed_payload() {
    let original = norito::encode_canonical(&vec![1_u64, 2, 3]).expect("canonical fixture");
    let frame = with_compression_tag(&vec![1_u64, 2, 3]);
    assert_eq!(
        original.iter().zip(&frame).filter(|(a, b)| a != b).count(),
        1
    );
    for bytes in [&frame[..], &frame[..norito::core::Header::SIZE]] {
        assert!(matches!(
            norito::decode_canonical::<Vec<u64>>(bytes),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }
}
