//! Tests for truncated payloads and checksum mismatches.

use iroha_schema::IntoSchema;
use norito::{decode_from_bytes, to_bytes};

#[test]
fn truncated_uncompressed_payload_yields_length_mismatch() {
    let v = vec![0u8; 256];
    let mut bytes = to_bytes(&v).expect("encode");
    // Drop the last 5 bytes to simulate premature EOF.
    bytes.truncate(bytes.len() - 5);
    let err = decode_from_bytes::<Vec<u8>>(&bytes).expect_err("truncation should error");
    // Length mismatch is returned when the reader cannot fill the declared length.
    assert!(format!("{err}").contains("length mismatch"), "got: {err:?}");
}

#[test]
fn checksum_mismatch_detected() {
    let s = String::from("checksum");
    let mut bytes = to_bytes(&s).expect("encode");
    // Flip a payload byte without updating the checksum.
    let hdr_len = norito::core::Header::SIZE;
    bytes[hdr_len] ^= 0xFF;
    let err = decode_from_bytes::<String>(&bytes).expect_err("checksum mismatch expected");
    assert!(
        format!("{err}").contains("checksum mismatch"),
        "got: {err:?}"
    );
}

#[test]
fn truncated_compressed_payload_yields_length_mismatch() {
    // Ensure compression is enabled in default features; the helper will decode either form.
    #[derive(Debug, PartialEq, IntoSchema, norito::NoritoSerialize, norito::NoritoDeserialize)]
    struct Blob(Vec<u8>);

    let v = Blob((0..8192u32).map(|i| (i as u8).wrapping_mul(31)).collect());
    let mut bytes = norito::to_compressed_bytes(&v, Some(norito::CompressionConfig::default()))
        .expect("encode compressed");
    // Truncate near the end to simulate a broken compressed stream.
    bytes.truncate(bytes.len() - 7);
    let err = decode_from_bytes::<Blob>(&bytes).expect_err("truncation should error");
    match err {
        norito::Error::LengthMismatch => {}
        norito::Error::Io(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}
