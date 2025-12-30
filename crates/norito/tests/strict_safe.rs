//! Tests for nonpanic, structured-error Norito decode paths.
//! These validate that malformed inputs return errors instead of panicking.

use std::num::NonZeroU32;

use norito::{
    Error,
    core::{self, Header},
};
#[allow(unused_imports)]
use norito::{NoritoSerialize, decode_from_bytes, to_bytes};

fn rewrite_checksum(bytes: &mut [u8]) {
    // Header layout: magic(4) maj(1) min(1) schema(16) comp(1) len(8) crc(8) flags(1) = 40 bytes
    const HDR: usize = 40;
    assert!(bytes.len() >= HDR);
    let payload = &bytes[HDR..];
    let mut d = crc64fast::Digest::new();
    d.write(payload);
    let crc = d.sum64().to_le_bytes();
    // checksum starts at offset 4+1+1+16+1+8 = 31
    let off = 4 + 1 + 1 + 16 + 1 + 8;
    bytes[off..off + 8].copy_from_slice(&crc);
}

#[test]
fn decode_string_invalid_utf8_yields_error() {
    let ok = String::from("A");
    let mut bytes = to_bytes(&ok).expect("encode string");
    // Flip the single payload byte to 0xFF (invalid UTF-8).
    // Length header encoding may be compact or fixed; use a validated payload view
    // which also sets decode flags, then parse the first length header.
    let view = core::from_bytes_view(&bytes).expect("view");
    let payload = view.as_bytes();
    let (len, lhdr) = core::read_len_dyn_slice(payload).expect("length header");
    let hdr_len = Header::SIZE;
    assert!(len >= 1);
    // leave length as 1, corrupt the first content byte following the length header
    bytes[hdr_len + lhdr] = 0xFF;
    rewrite_checksum(&mut bytes);

    let err = decode_from_bytes::<String>(&bytes).expect_err("invalid utf8 should error");
    match err {
        Error::InvalidUtf8 => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn decode_option_invalid_tag_yields_error() {
    let v: Option<u32> = None;
    let mut bytes = to_bytes(&v).expect("encode option");
    // First payload byte after header is the tag; set to an invalid value 2.
    let hdr_len = Header::SIZE;
    bytes[hdr_len] = 2;
    rewrite_checksum(&mut bytes);

    let err = decode_from_bytes::<Option<u32>>(&bytes).expect_err("invalid tag should error");
    match err {
        Error::InvalidTag { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn decode_nonzero_zero_yields_error() {
    let nz = NonZeroU32::new(1).unwrap();
    let mut bytes = to_bytes(&nz).expect("encode nonzero");
    // Payload is 4 bytes (u32). Zero it out to make it invalid.
    let hdr_len = Header::SIZE;
    for b in &mut bytes[hdr_len..hdr_len + 4] {
        *b = 0;
    }
    rewrite_checksum(&mut bytes);

    let err = decode_from_bytes::<NonZeroU32>(&bytes).expect_err("zero nonzero should error");
    match err {
        Error::InvalidNonZero => {}
        other => panic!("unexpected error: {other:?}"),
    }
}
