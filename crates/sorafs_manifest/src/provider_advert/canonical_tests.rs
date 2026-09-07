// Fixed V1 advert wire/signature admission across foreign codec layouts.

#[test]
fn advert_canonical_decoding_and_size_bounds_ignore_caller_layout() {
    let now = 1_700_000_000;
    let advert = signed_sample_advert(now);
    let canonical = norito::encode_canonical(&advert).unwrap();
    let alternate = encode_frame_with_flags(&advert, 0);
    assert_ne!(canonical, alternate);
    assert_eq!(
        decode_from_bytes::<ProviderAdvertV1>(&alternate).unwrap(),
        advert
    );
    let exact = {
        let _context = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::core::encoded_payload_len(&advert).unwrap()
    };
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        let decoded = decode_provider_advert_v1(&canonical).unwrap();
        assert_eq!(decoded, advert);
        decoded.validate_with_body(now).unwrap();
        decoded.verify_signature().unwrap();
        assert_eq!(preflight_provider_advert_len(&advert, exact), Ok(exact));
        assert_eq!(
            preflight_provider_advert_len(&advert, exact - 1),
            Err(AdvertValidationError::AdvertTooLarge {
                found: exact,
                maximum: exact - 1
            })
        );
        assert!(matches!(
            decode_provider_advert_v1(&alternate),
            Err(norito::core::Error::NonCanonicalEncoding)
        ));
    }
}

#[test]
fn advert_signatures_use_canonical_preimages_across_every_caller_layout() {
    let now = 1_700_000_000;
    let key = SigningKey::from_bytes(&[0x67; 32]);
    let mut advert = sample_advert(now);
    advert.signature.public_key = key.verifying_key().to_bytes().to_vec();
    let mut expected = PROVIDER_ADVERT_SIGNATURE_DOMAIN_V1.to_vec();
    expected.extend_from_slice(&norito::encode_canonical(&advert.signature_payload()).unwrap());
    let expected_signature = key.sign(&expected).to_bytes();
    for signing_flags in supported_layouts() {
        let _signing_context = norito::core::DecodeFlagsGuard::enter(signing_flags);
        let payload = advert.signature_payload_bytes().unwrap();
        assert_eq!(payload, expected);
        advert.signature.signature = key.sign(&payload).to_bytes().to_vec();
        assert_eq!(advert.signature.signature, expected_signature);
        for verifying_flags in supported_layouts() {
            let _verification_context = norito::core::DecodeFlagsGuard::enter(verifying_flags);
            advert.validate_with_body(now).unwrap();
            advert.verify_signature().unwrap();
        }
    }
}

#[test]
fn advert_signature_over_an_alternate_frame_has_no_fallback() {
    let now = 1_700_000_000;
    let key = SigningKey::from_bytes(&[0x68; 32]);
    let mut advert = sample_advert(now);
    advert.signature.public_key = key.verifying_key().to_bytes().to_vec();
    let mut alternate = PROVIDER_ADVERT_SIGNATURE_DOMAIN_V1.to_vec();
    alternate.extend_from_slice(&encode_frame_with_flags(&advert.signature_payload(), 0));
    assert_ne!(alternate, advert.signature_payload_bytes().unwrap());
    let signature = key.sign(&alternate);
    key.verifying_key()
        .verify_strict(&alternate, &signature)
        .unwrap();
    advert.signature.signature = signature.to_bytes().to_vec();
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        advert.validate_with_body(now).unwrap();
        assert!(matches!(
            advert.verify_signature(),
            Err(AdvertSignatureError::Verification(_))
        ));
    }
}
