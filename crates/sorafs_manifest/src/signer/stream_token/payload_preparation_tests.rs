//! Exact shared payload admission; structural claims do not qualify hardware or token release.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn fixture() -> (StreamTokenBodyV1, SignerCustodyBindingV1) {
    let body = StreamTokenBodyV1 {
        token_id: "0123456789abcdef0123456789abcdef".into(),
        manifest_cid: vec![0xff; 128],
        provider_id: [0x62; 32],
        profile_handle: "b".repeat(128),
        max_streams: 1024,
        ttl_epoch: 1200,
        rate_limit_bytes: 1_073_741_824,
        issued_at: 1000,
        requests_per_minute: 10000,
        token_pk_version: 7,
    };
    let key = KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519).unwrap();
    let binding = SignerCustodyBindingV1 {
        chain_id: "sorafs-reference".into(),
        network_id: [0x11; 32],
        runtime_handle: "hsm://sorafs/stream/primary".into(),
        key_handle: "pkcs11:production/stream/key-7".into(),
        service_id: "stream-primary".into(),
        administrator_id: "stream-security-primary".into(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: body.provider_id,
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: key.public_key().clone(),
        key_revision: 7,
        policy_revision: 9,
        policy_digest: [0x51; 32],
    };
    (body, binding)
}
fn layouts() -> [u8; 10] {
    use norito::core::header_flags::{
        COMPACT_LEN as C, FIELD_BITSET as F, PACKED_SEQ as Q, PACKED_STRUCT as S,
    };
    [
        0,
        C,
        Q,
        Q | C,
        S,
        S | C,
        Q | S,
        Q | S | C,
        S | C | F,
        Q | S | C | F,
    ]
}
fn payload_with_frame(frame: &[u8]) -> Vec<u8> {
    let mut payload = b"sorafs.stream-token.signature.v1\0".to_vec();
    payload.extend_from_slice(frame);
    payload
}

#[test]
fn maximum_prepared_body_uses_the_same_identity_in_every_layout_with_finite_decode_cost() {
    let (body, binding) = fixture();
    validate_token_body(&body).unwrap();
    let frame = norito::encode_canonical(&body).unwrap();
    let payload = payload_with_frame(&frame);
    let expected = SignerStreamTokenExpectedV1::new(&body, &binding).unwrap();
    let allocation = 64 * 1024 + 8 * frame.len();
    assert!(payload.len() <= 2048);
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let (actual, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(128, frame.len(), 2048, allocation, 16),
            || prepare_stream_token_signing_payload_v1(&payload, &binding),
        );
        let (decoded, prepared) = actual.expect("actual maximum variable leaves admit");
        assert_eq!(decoded, body);
        assert_eq!(prepared, expected);
        assert_eq!(decoded.signing_payload_bytes().unwrap(), payload);
        assert_eq!(
            stream_token_binding_digest_v1(&binding).unwrap(),
            expected.binding_digest()
        );
        assert!(usage.total_allocated_bytes() > 0);
        assert!(usage.total_allocated_bytes() <= allocation);
    }
}

#[test]
fn empty_oversized_wrong_domain_and_forbidden_headers_fail_before_decode_allocation() {
    let (body, binding) = fixture();
    let canonical = body.signing_payload_bytes().unwrap();
    prepare_stream_token_signing_payload_v1(&canonical, &binding).unwrap();
    let compressed = crate::canonical_test_support::with_compression_tag(&body);
    let mut huge = norito::encode_canonical(&body).unwrap();
    huge[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut wrong_domain = canonical.clone();
    wrong_domain[0] ^= 1;
    for invalid in [
        Vec::new(),
        vec![0; 2049],
        b"sorafs.stream-token.signature.v1\0".to_vec(),
        wrong_domain,
        payload_with_frame(&compressed),
        payload_with_frame(&compressed[..norito::core::Header::SIZE]),
        payload_with_frame(&huge),
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(4096, 4096, 8192, 128 * 1024, 24),
            || prepare_stream_token_signing_payload_v1(&invalid, &binding),
        );
        assert_eq!(
            result.err(),
            Some(SignerStreamTokenReceiptErrorV1::InvalidReceipt)
        );
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
}

#[test]
fn same_value_alternate_frames_suffixes_and_changed_authority_never_prepare() {
    let (body, binding) = fixture();
    let canonical = body.signing_payload_bytes().unwrap();
    prepare_stream_token_signing_payload_v1(&canonical, &binding).unwrap();
    let mut saw_alternate = false;
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let frame = norito::to_bytes(&body).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<StreamTokenBodyV1>(&frame).unwrap(),
            body
        );
        let alternative = payload_with_frame(&frame);
        if alternative != canonical {
            saw_alternate = true;
            assert_eq!(
                prepare_stream_token_signing_payload_v1(&alternative, &binding).err(),
                Some(SignerStreamTokenReceiptErrorV1::InvalidReceipt)
            );
        }
    }
    assert!(saw_alternate);
    let mut suffix = canonical.clone();
    suffix.push(0);
    assert_eq!(
        prepare_stream_token_signing_payload_v1(&suffix, &binding).err(),
        Some(SignerStreamTokenReceiptErrorV1::InvalidReceipt)
    );
    assert_eq!(
        prepare_stream_token_signing_payload_v1(&canonical[..canonical.len() - 1], &binding).err(),
        Some(SignerStreamTokenReceiptErrorV1::InvalidReceipt)
    );
    let mut wrong = binding.clone();
    wrong.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x63; 32],
    };
    assert!(
        stream_token_binding_digest_v1(&wrong).is_ok(),
        "different valid authority is not malformed"
    );
    assert_eq!(
        prepare_stream_token_signing_payload_v1(&canonical, &wrong).err(),
        Some(SignerStreamTokenReceiptErrorV1::WrongPurpose)
    );
    wrong = binding.clone();
    wrong.key_revision += 1;
    assert_eq!(
        prepare_stream_token_signing_payload_v1(&canonical, &wrong).err(),
        Some(SignerStreamTokenReceiptErrorV1::TokenMismatch)
    );
    wrong = binding.clone();
    wrong.key_revision = u64::from(u32::MAX) + 1;
    assert_eq!(
        stream_token_binding_digest_v1(&wrong),
        Err(SignerStreamTokenReceiptErrorV1::TokenMismatch)
    );
    wrong = binding.clone();
    wrong.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0; 32],
    };
    assert!(stream_token_binding_digest_v1(&wrong).is_err());
    wrong = binding;
    wrong.service_id = "x".repeat(129);
    assert_eq!(
        stream_token_binding_digest_v1(&wrong),
        Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt)
    );
}
