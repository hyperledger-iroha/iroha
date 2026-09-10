//! Canonical signer-authority exclusion preserves decoded material without provider attribution.
use super::*;

#[test]
fn signer_authority_unavailable_is_canonical_and_requires_decoded_material() {
    let outcome = StreamTokenValidationOutcomeV1 {
        binding: StreamTokenValidationBindingV1::try_new([1; 32], 1, [2; 32]).unwrap(),
        token_body_digest: Some([3; 32]),
        token_key_version: Some(7),
        validated_at_unix_ms: 1_800_000_000_000,
        status: StreamTokenValidationStatusV1::Excluded(
            StreamTokenExcludedKindV1::SignerAuthorityUnavailable,
        ),
    };
    outcome.validate(outcome.validated_at_unix_ms).unwrap();
    assert!(!outcome.status.counts_for_provider());
    assert!(!outcome.status.is_violation());
    let bytes = norito::encode_canonical(&outcome).unwrap();
    assert_eq!(
        norito::decode_canonical::<StreamTokenValidationOutcomeV1>(&bytes).unwrap(),
        outcome
    );
    {
        let bytes = norito::json::to_vec(&outcome).unwrap();
        assert_eq!(
            norito::json::from_slice::<StreamTokenValidationOutcomeV1>(&bytes).unwrap(),
            outcome
        );
        assert!(
            std::str::from_utf8(&bytes)
                .unwrap()
                .contains("signer_authority_unavailable")
        );
    }
    for (body, version) in [(None, Some(7)), (Some([3; 32]), None), (None, None)] {
        let invalid = StreamTokenValidationOutcomeV1 {
            token_body_digest: body,
            token_key_version: version,
            ..outcome
        };
        assert_eq!(
            invalid.validate(outcome.validated_at_unix_ms),
            Err(ReputationJournalValidationError::TokenMaterialMismatch)
        );
    }
}
