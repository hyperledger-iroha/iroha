//! Sanity checks for slice-based decoding of streaming enum types.

use norito::{
    core::{DecodeFromSlice, PayloadCtxGuard, from_bytes_view},
    streaming::{CapabilityRole, EncryptionSuite, Hash, PrivacyBucketGranularity},
    to_bytes,
};

fn sample_hash(byte: u8) -> Hash {
    let mut hash = [0u8; 32];
    hash.fill(byte);
    hash
}

#[test]
fn capability_role_decodes_via_slice_adapter() {
    let original = CapabilityRole::Publisher;
    let bytes = to_bytes(&original).expect("encode capability role");
    let view = from_bytes_view(&bytes).expect("view");
    let payload = view.as_bytes();
    let _ctx = PayloadCtxGuard::enter(payload);
    let (decoded, used) =
        <CapabilityRole as DecodeFromSlice>::decode_from_slice(payload).expect("decode role");
    assert_eq!(decoded, original);
    assert_eq!(used, payload.len());
}

#[test]
fn encryption_suite_decodes_via_slice_adapter() {
    let original = EncryptionSuite::Kyber768XChaCha20Poly1305(sample_hash(0x42));
    let bytes = to_bytes(&original).expect("encode encryption suite");
    let view = from_bytes_view(&bytes).expect("view");
    let payload = view.as_bytes();
    let _ctx = PayloadCtxGuard::enter(payload);
    let (decoded, used) =
        <EncryptionSuite as DecodeFromSlice>::decode_from_slice(payload).expect("decode suite");
    assert_eq!(decoded, original);
    assert_eq!(used, payload.len());
}

#[test]
fn privacy_bucket_decodes_via_slice_adapter() {
    let original = PrivacyBucketGranularity::StandardV1;
    let bytes = to_bytes(&original).expect("encode privacy bucket");
    let view = from_bytes_view(&bytes).expect("view");
    let payload = view.as_bytes();
    let _ctx = PayloadCtxGuard::enter(payload);
    let (decoded, used) = <PrivacyBucketGranularity as DecodeFromSlice>::decode_from_slice(payload)
        .expect("decode bucket");
    assert_eq!(decoded, original);
    assert_eq!(used, payload.len());
}
