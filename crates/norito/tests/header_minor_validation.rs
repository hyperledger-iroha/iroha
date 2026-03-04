//! Validate header minor-version handling.

use crc64fast::Digest;
use norito::{
    core::{Compression, Error, NoritoSerialize, VERSION_MAJOR, VERSION_MINOR, header_flags},
    decode_from_bytes,
};

fn frame_payload<T: NoritoSerialize>(minor: u8, flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(norito::core::Header::SIZE + payload.len());
    bytes.extend_from_slice(b"NRT0");
    bytes.push(VERSION_MAJOR);
    bytes.push(minor);
    bytes.extend_from_slice(&T::schema_hash());
    bytes.push(Compression::None as u8);
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    let mut digest = Digest::new();
    digest.write(payload);
    bytes.extend_from_slice(&digest.sum64().to_le_bytes());
    bytes.push(flags);
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn header_minor_mismatch_is_rejected() {
    let minor = VERSION_MINOR.wrapping_add(1);
    let bytes = frame_payload::<()>(minor, 0, &[]);
    let err = decode_from_bytes::<()>(&bytes).expect_err("minor mismatch must be rejected");
    assert!(matches!(
        err,
        Error::UnsupportedMinorVersion { found, .. } if found == minor
    ));
}

#[test]
fn header_flags_are_accepted_when_supported() {
    let bytes = frame_payload::<()>(
        VERSION_MINOR,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        &[],
    );
    decode_from_bytes::<()>(&bytes).expect("supported flags must be accepted");
}

#[test]
fn header_flags_outside_supported_mask_are_rejected() {
    let bytes = frame_payload::<()>(VERSION_MINOR, 0x80, &[]);
    let err = decode_from_bytes::<()>(&bytes).expect_err("unknown flags must be rejected");
    assert!(matches!(err, Error::UnsupportedFeature("layout flag")));
}

#[test]
fn header_checksum_mismatch_is_rejected() {
    let value = vec![11u32, 22, 33];
    let mut bytes = norito::to_bytes(&value).expect("serialize vector");
    let payload_offset = norito::core::Header::SIZE;
    bytes[payload_offset] ^= 0xFF;

    let err =
        decode_from_bytes::<Vec<u32>>(&bytes).expect_err("tampered payload must fail checksum");
    assert!(matches!(err, Error::ChecksumMismatch));
}
