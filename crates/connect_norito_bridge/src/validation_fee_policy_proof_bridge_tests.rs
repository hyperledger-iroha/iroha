#[cfg(test)]
mod validation_fee_policy_proof_bridge_tests {
    use super::*;

    #[test]
    fn request_encoder_validates_context_without_serializing_it() {
        let first_context = [1_u8; 32];
        let mut second_context = [3_u8; 32];
        second_context[0] = 9;
        let first = validation_fee_current_policy_proof_request_v1(17, first_context)
            .expect("encode first request");
        let second = validation_fee_current_policy_proof_request_v1(17, second_context)
            .expect("encode second request");
        assert_eq!(first, second);
        let decoded: ValidationFeeCurrentPolicyProofRequestV1 =
            decode_from_bytes(&first).expect("decode request");
        assert_eq!(
            decoded,
            ValidationFeeCurrentPolicyProofRequestV1 {
                version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
                trusted_checkpoint_height: 17,
            }
        );
        assert!(
            validation_fee_current_policy_proof_request_v1(0, first_context).is_err(),
            "zero height must fail closed"
        );
        assert!(
            validation_fee_current_policy_proof_request_v1(17, [0; 32]).is_err(),
            "zero context must fail closed"
        );
        assert!(
            validation_fee_current_policy_proof_request_v1(17, [2; 32]).is_err(),
            "unmarked context must fail closed"
        );
    }

    #[test]
    fn proof_verifier_rejects_malformed_archive() {
        assert!(
            validation_fee_current_policy_proof_verify_v1(
                b"not norito",
                NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::prehashed([1; 32]),
                )),
                [3; 32],
                1,
                [5; 32],
            )
            .is_err()
        );
    }
}
