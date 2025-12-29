//! Negative tests for adaptive AoS rows decoding.
use norito::core::Error;

#[test]
fn decode_rows_u64_str_bool_adaptive_truncated_header() {
    // Tag + incomplete varint length
    let mut bytes = vec![norito::columnar::ADAPTIVE_TAG_AOS];
    // compact-len build encodes varint length; use a single continuation byte to force truncation
    bytes.push(0x80);
    let out = norito::columnar::decode_rows_u64_str_bool_adaptive(&bytes);
    assert!(matches!(
        out,
        Err(Error::LengthMismatch) | Err(Error::Message(_))
    ));
}

#[test]
fn decode_rows_u64_str_bool_adaptive_truncated_row() {
    // Tag + n=1 + id present but missing flag byte after name
    let mut body = Vec::new();
    // n = 1
    #[cfg(feature = "compact-len")]
    {
        norito::core::write_len_to_vec(&mut body, 1);
    }
    #[cfg(not(feature = "compact-len"))]
    {
        body.extend_from_slice(&(1u64).to_le_bytes());
    }
    // AoS version nibble (low=0x1)
    body.push(0x01);
    // id
    body.extend_from_slice(&42u64.to_le_bytes());
    // name length = 3, name bytes = "abc"
    #[cfg(feature = "compact-len")]
    {
        norito::core::write_len_to_vec(&mut body, 3);
    }
    #[cfg(not(feature = "compact-len"))]
    {
        body.extend_from_slice(&(3u64).to_le_bytes());
    }
    body.extend_from_slice(b"abc");
    // MISSING: flag byte → truncation

    let mut bytes = Vec::new();
    bytes.push(norito::columnar::ADAPTIVE_TAG_AOS);
    bytes.extend_from_slice(&body);
    let out = norito::columnar::decode_rows_u64_str_bool_adaptive(&bytes);
    assert!(matches!(
        out,
        Err(Error::LengthMismatch) | Err(Error::Message(_))
    ));
}
