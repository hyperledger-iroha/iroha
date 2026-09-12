//! Canonical token transport tests using the actual independently signed issuer fixture.
use super::hardware_test_support::{PROVIDER, SignedFixture, TestSignerMode};
use super::*;

fn layouts() -> [u8; 10] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}
fn issued() -> StreamTokenV1 {
    let fixture = SignedFixture::new(1, TestSignerMode::Sign);
    let issuer = fixture
        .issuer()
        .expect("independent signed startup fixture");
    let public =
        iroha_crypto::KeyPair::try_from_seed(vec![0x74; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    issuer
        .issue_token(
            StreamTokenQuotaSubject::from_authenticated_operator(public.public_key()),
            vec![0xaa],
            PROVIDER,
            "sorafs.sf1@1.0.0".into(),
            TokenOverrides::default(),
        )
        .expect("real four-signature and completed-observer fixture")
        .token
}
#[test]
fn signed_token_transport_is_canonical_in_all_ten_layouts() {
    let token = issued();
    let canonical = norito::encode_canonical(&token).unwrap();
    let expected = base64::engine::general_purpose::STANDARD.encode(&canonical);
    let mut different = 0;
    for flags in layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(encode_token_base64(&token).unwrap(), expected);
        assert_eq!(decode_token_base64(&expected).unwrap(), token);
        let alternate = norito::to_bytes(&token).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<StreamTokenV1>(&alternate).unwrap(),
            token
        );
        if alternate != canonical {
            different += 1;
            assert!(matches!(
                decode_token_base64(&base64::engine::general_purpose::STANDARD.encode(alternate)),
                Err(StreamTokenHeaderError::InvalidPayload(
                    norito::Error::NonCanonicalEncoding
                ))
            ));
        }
    }
    assert!(
        different > 0,
        "the negative must contain genuine same-value alternate frames"
    );
}
#[test]
fn token_transport_rejects_compression_before_allocation_and_intersects_outer_limits() {
    let token = issued();
    let canonical = norito::encode_canonical(&token).unwrap();
    let header = norito::core::Header::read(canonical.as_slice()).unwrap();
    let compression = header.magic.len() + 2 + header.schema.len();
    let mut tagged = canonical.clone();
    tagged[compression] = 1;
    assert_ne!(
        norito::core::Header::read(tagged.as_slice())
            .unwrap()
            .compression,
        header.compression
    );
    let mut advertised = tagged[..norito::core::Header::SIZE].to_vec();
    advertised[compression + 1..compression + 9].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        norito::core::Header::read(advertised.as_slice())
            .unwrap()
            .length,
        u64::MAX
    );
    let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    for flags in layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        for bytes in [&tagged, &advertised] {
            assert!(matches!(
                norito::with_decode_limits_scope(zero, || decode_token_wire(bytes)),
                Err(StreamTokenHeaderError::InvalidPayload(
                    norito::Error::NonCanonicalEncoding
                ))
            ));
        }
        assert!(
            matches!(norito::with_decode_limits_scope(zero, || decode_token_wire(&canonical)),
            Err(StreamTokenHeaderError::InvalidPayload(error)) if error.is_decode_resource_limit())
        );
    }
    for limits in [
        norito::DecodeLimits::new(1, usize::MAX, usize::MAX, usize::MAX, 128),
        norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, 128),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, 1, usize::MAX, 128),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 1),
    ] {
        assert!(
            matches!(norito::with_decode_limits_scope(limits, || decode_token_wire(&canonical)),
            Err(StreamTokenHeaderError::InvalidPayload(error)) if error.is_decode_resource_limit())
        );
    }
    assert!(matches!(
        decode_token_wire(&vec![0; MAX_STREAM_TOKEN_WIRE_BYTES + 1]),
        Err(StreamTokenHeaderError::PayloadTooLong {
            maximum: MAX_STREAM_TOKEN_WIRE_BYTES
        })
    ));
}
