mod kagemusha_v4_lifecycle_additional_domain_tests {
    use super::*;
    #[test]
    fn abi21_bundle_request_and_redemption_domains_are_unique() {
        let v4 = [
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4,
        ];
        assert_eq!(
            v4.into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            v4.len()
        );
        assert_ne!(
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V4,
            KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_DIGEST_DOMAIN_V2
        );
        assert_ne!(
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4,
            KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V2
        );
    }
    #[test]
    fn abi21_chain_request_size_caps_are_inclusive_and_fail_one_byte_over() {
        for maximum in [
            KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4,
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
        ] {
            ensure_kagemusha_encoded_size_at_most(maximum, maximum)
                .expect("the exact canonical request limit is accepted");
            assert!(matches!(
                ensure_kagemusha_encoded_size_at_most(maximum + 1, maximum),
                Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                    if actual == maximum + 1 && max == maximum
            ));
        }
    }
}
