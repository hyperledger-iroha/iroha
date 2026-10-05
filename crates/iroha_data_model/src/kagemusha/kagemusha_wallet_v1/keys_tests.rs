//! P-256 device key and signature validation, conversion and codec checks.

use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

use super::*;

const MESSAGE: &[u8] = b"kagemusha wallet device key test message";

fn signing_key(seed: u8) -> SigningKey {
    // Public deterministic test seeds, never runtime device authority keys.
    SigningKey::from_bytes((&[seed; 32]).into()).expect("fixed scalar")
}

fn public_key(signing: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical key")
}

/// Low-S and high-S fixed-width forms of one signature over [`MESSAGE`].
fn low_and_high(signing: &SigningKey) -> ([u8; 64], [u8; 64]) {
    let signature: P256Signature = signing.sign(MESSAGE);
    let low = signature.normalize_s().unwrap_or(signature);
    let high =
        P256Signature::from_scalars(low.r().to_bytes(), (-*low.s()).to_bytes()).expect("high");
    assert!(high.normalize_s().is_some());
    (
        low.to_bytes().as_slice().try_into().expect("64 bytes"),
        high.to_bytes().as_slice().try_into().expect("64 bytes"),
    )
}

fn rejected_field<T: core::fmt::Debug>(result: WalletResult<T>, expected: &'static str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidField { field }) => {
            assert_eq!(field, expected);
        }
        other => panic!("expected InvalidField `{expected}`, got {other:?}"),
    }
}

#[test]
fn kagemusha_wallet_v1_device_keys_declare_wallet_schema_names() {
    assert_eq!(KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1, 65);
    assert_eq!(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1, 64);
    assert_eq!(
        <KagemushaDevicePublicKeyV1 as norito::NoritoSchema>::frame_name(),
        "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaDevicePublicKeyV1"
    );
    assert_eq!(
        <KagemushaDeviceSignatureV1 as norito::NoritoSchema>::frame_name(),
        "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaDeviceSignatureV1"
    );
}

#[test]
fn kagemusha_wallet_v1_device_public_key_accepts_only_canonical_uncompressed_points() {
    let signing = signing_key(0x39);
    let key = public_key(&signing);
    let uncompressed = signing.verifying_key().to_encoded_point(false);
    assert_eq!(key.as_sec1_bytes().as_slice(), uncompressed.as_bytes());
    assert_eq!(key.as_ref(), uncompressed.as_bytes());
    key.validate().expect("canonical key validates");
    assert_eq!(
        KagemushaDevicePublicKeyV1::try_from(uncompressed.as_bytes()).expect("slice"),
        key
    );
    assert_eq!(
        KagemushaDevicePublicKeyV1::try_from(*key.as_sec1_bytes()).expect("array"),
        key
    );

    let compressed = signing.verifying_key().to_encoded_point(true);
    rejected_field(
        KagemushaDevicePublicKeyV1::from_sec1_bytes(compressed.as_bytes()),
        "device_public_key",
    );
    rejected_field(
        KagemushaDevicePublicKeyV1::from_sec1_bytes(&key.as_sec1_bytes()[..64]),
        "device_public_key",
    );
    let mut long = key.as_sec1_bytes().to_vec();
    long.push(0);
    rejected_field(
        KagemushaDevicePublicKeyV1::from_sec1_bytes(&long),
        "device_public_key",
    );
    let mut wrong_prefix = *key.as_sec1_bytes();
    wrong_prefix[0] = 0x02;
    rejected_field(
        KagemushaDevicePublicKeyV1::from_sec1_bytes(&wrong_prefix),
        "device_public_key",
    );
    let mut off_curve = [0; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1];
    off_curve[0] = 0x04;
    rejected_field(
        KagemushaDevicePublicKeyV1::try_from(off_curve),
        "device_public_key",
    );
    rejected_field(
        KagemushaDevicePublicKeyV1([0; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]).validate(),
        "device_public_key",
    );
}

#[test]
fn kagemusha_wallet_v1_device_signature_accepts_only_fixed_width_low_s() {
    let signing = signing_key(0x57);
    let (low, high) = low_and_high(&signing);
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    assert_eq!(signature.as_raw_bytes(), &low);
    assert_eq!(signature.as_ref(), low.as_slice());
    signature.validate().expect("low-S validates");
    assert_eq!(
        KagemushaDeviceSignatureV1::try_from(low.as_slice()).expect("slice"),
        signature
    );
    assert_eq!(
        KagemushaDeviceSignatureV1::try_from(low).expect("array"),
        signature
    );

    rejected_field(
        KagemushaDeviceSignatureV1::from_raw_bytes(&high),
        "device_signature",
    );
    rejected_field(
        KagemushaDeviceSignatureV1::from_raw_bytes(&low[..63]),
        "device_signature",
    );
    for range in [0..32, 32..64] {
        let mut zero_scalar = low;
        zero_scalar[range].fill(0);
        rejected_field(
            KagemushaDeviceSignatureV1::try_from(zero_scalar),
            "device_signature",
        );
    }
    rejected_field(
        KagemushaDeviceSignatureV1(high).validate(),
        "device_signature",
    );
}

#[test]
fn kagemusha_wallet_v1_device_signature_der_conversion_normalizes_high_s() {
    let signing = signing_key(0x57);
    let (low, high) = low_and_high(&signing);
    let expected = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    let low_der = P256Signature::from_slice(&low).expect("low").to_der();
    let high_der = P256Signature::from_slice(&high).expect("high").to_der();
    assert_eq!(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(low_der.as_bytes())
            .expect("low DER"),
        expected
    );
    assert_eq!(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(high_der.as_bytes())
            .expect("high DER"),
        expected
    );

    let mut padded = high_der.as_bytes().to_vec();
    padded.push(0);
    rejected_field(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(&padded),
        "device_signature",
    );
    rejected_field(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(&high_der.as_bytes()[..7]),
        "device_signature",
    );
    rejected_field(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(&[0x30; 73]),
        "device_signature",
    );
    rejected_field(
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(&low),
        "device_signature",
    );
}

#[test]
fn kagemusha_wallet_v1_device_signature_verify_binds_key_and_message() {
    let signing = signing_key(0x39);
    let key = public_key(&signing);
    let other = public_key(&signing_key(0x57));
    let (low, _) = low_and_high(&signing);
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");
    signature.verify(&key, MESSAGE).expect("verifies");
    rejected_field(signature.verify(&other, MESSAGE), "device_signature");
    rejected_field(
        signature.verify(&key, b"different message"),
        "device_signature",
    );
    rejected_field(
        signature.verify(
            &KagemushaDevicePublicKeyV1([0; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]),
            MESSAGE,
        ),
        "device_public_key",
    );
}

#[test]
fn kagemusha_wallet_v1_device_key_codec_is_raw_and_validating() {
    let signing = signing_key(0x39);
    let key = public_key(&signing);
    let (low, high) = low_and_high(&signing);
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&low).expect("low-S");

    let key_frame = norito::encode_canonical(&key).expect("encode key");
    let signature_frame = norito::encode_canonical(&signature).expect("encode signature");
    assert_eq!(
        norito::decode_canonical::<KagemushaDevicePublicKeyV1>(&key_frame).expect("decode key"),
        key
    );
    assert_eq!(
        norito::decode_canonical::<KagemushaDeviceSignatureV1>(&signature_frame)
            .expect("decode signature"),
        signature
    );
    assert!(key_frame.ends_with(key.as_sec1_bytes()));
    assert!(signature_frame.ends_with(&low));

    let decoded = <KagemushaDevicePublicKeyV1 as norito::core::DecodeFromSlice>::decode_from_slice(
        key.as_sec1_bytes(),
    )
    .expect("raw key payload");
    assert_eq!(decoded, (key, KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1));
    let mut wrong_prefix = *key.as_sec1_bytes();
    wrong_prefix[0] = 0x02;
    assert!(
        <KagemushaDevicePublicKeyV1 as norito::core::DecodeFromSlice>::decode_from_slice(
            &wrong_prefix
        )
        .is_err()
    );
    assert!(
        <KagemushaDevicePublicKeyV1 as norito::core::DecodeFromSlice>::decode_from_slice(
            &key.as_sec1_bytes()[..64]
        )
        .is_err()
    );
    assert!(
        <KagemushaDeviceSignatureV1 as norito::core::DecodeFromSlice>::decode_from_slice(&high)
            .is_err()
    );

    assert!(
        norito::encode_canonical(&KagemushaDevicePublicKeyV1(
            [0; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]
        ))
        .is_err()
    );
    assert!(norito::encode_canonical(&KagemushaDeviceSignatureV1(high)).is_err());
}
