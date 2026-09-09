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

/// A nested field with payload encoding and no typed-frame identity or marker.
#[derive(norito::derive::SerializePayload)]
pub(crate) struct PayloadOnly(pub(crate) u64);

/// Check field bytes, exact lengths and counting across all supported layouts.
pub(crate) fn assert_same_payload(
    borrowed: &impl norito::SerializePayload,
    owned: &impl norito::SerializePayload,
) {
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        norito::core::serialize_to_buffer(borrowed, &mut actual).expect("borrowed field");
        norito::core::serialize_to_buffer(owned, &mut expected).expect("owned field");
        assert_eq!(
            actual, expected,
            "field payload changed for flags {flags:#04x}"
        );
        assert_eq!(borrowed.encoded_len_exact(), owned.encoded_len_exact());
        assert_eq!(borrowed.encoded_len_exact(), Some(actual.len()));
        assert_eq!(
            norito::core::encoded_payload_len(borrowed).unwrap(),
            actual.len()
        );
    }
}
