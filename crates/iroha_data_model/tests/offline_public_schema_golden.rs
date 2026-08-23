//! Golden structural schemas for first-release public Offline requests and release provenance.
use iroha_data_model::isi::offline::{
    AuthorizeKagemushaTairaCanaryV4, RecordKagemushaTairaCanaryV4,
};
use iroha_data_model::offline::{
    KagemushaExactBytesDigestV1, KagemushaFinalizedBlockWireV1,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendCryptographicReviewSubjectV4, KagemushaRecursiveSpendPromotedReleaseV4,
    KagemushaRecursiveSpendQualificationReceiptV4, KagemushaRecursiveSpendRedeemRequestV4,
    KagemushaRecursiveSpendReleaseActivationV4, KagemushaRecursiveSpendReleaseAttestationSubjectV4,
    KagemushaRecursiveSpendReleaseRecordV4, KagemushaRecursiveSpendTopUpRequestV4,
    KagemushaV4ActivationFinalityProofChainV1, KagemushaV4ActivationFinalityReceiptBodyV1,
    KagemushaV4ActivationFinalityReceiptV1, KagemushaV4ActivationReceiptExpectationsArtifactV1,
    KagemushaV4ActivationReceiptExpectationsBodyV1, KagemushaV4GitHubPromotionRunV1,
    KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1,
    KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1,
    KagemushaV4PostCanaryValidatorLivenessChallengeV1,
    KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1,
    KagemushaV4PostCanaryValidatorLivenessEvidenceV1,
    KagemushaV4PostCanaryValidatorLivenessObservationV1,
    KagemushaV4PostCanaryValidatorLivenessTargetV1, KagemushaV4PromotionBindingV1,
    KagemushaV4PromotionReservationBodyV1, KagemushaV4PromotionReservationV1,
    KagemushaV4RuntimeEffectiveConfigProjectionV1, KagemushaV4RuntimeValidatorProjectionV1,
    KagemushaV4TairaCanaryAuthorizationBodyV1, KagemushaV4TairaCanaryAuthorizationPackageV1,
    KagemushaV4TairaCanaryAuthorizationV1, KagemushaV4TairaCanaryEvidenceBodyV1,
    KagemushaV4TairaCanaryEvidenceV1, KagemushaV4TairaCanaryPermitV1,
    KagemushaV4TairaCanaryQueryObservationV1, KagemushaV4TairaCanaryReservationBodyV1,
    KagemushaV4TairaCanaryReservationV1, KagemushaV4ValidatorQualificationSealBodyV1,
    KagemushaV4ValidatorQualificationSealV1,
};
use iroha_schema::IntoSchema;
use sha2::{Digest as _, Sha256};
const STRUCTURAL_SCHEMA_HASH_DOMAIN: &[u8] = b"norito:v1:structural-schema\0";
fn structural_schema_hash<T: IntoSchema>() -> String {
    let schema = T::schema();
    let canonical = norito::json::to_json(&schema).expect("serialize canonical structural schema");
    let mut hasher = Sha256::new();
    hasher.update(STRUCTURAL_SCHEMA_HASH_DOMAIN);
    hasher.update(canonical.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}
#[test]
fn public_offline_request_structural_schemas_are_frozen_for_abi21_v4() {
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendTopUpRequestV4>(),
        "7929db5019d35b407eddbccbd7b0529b"
    );
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendRedeemRequestV4>(),
        "1faf6a5464cf9fe068a27df61e15f563"
    );
}

#[test]
fn public_kagemusha_release_provenance_schemas_are_frozen_for_abi21_v4() {
    assert_eq!(
        [
            structural_schema_hash::<KagemushaRecursiveSpendArtifactManifestV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendCandidateV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendQualificationReceiptV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendCryptographicReviewSubjectV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendReleaseAttestationSubjectV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendPromotedReleaseV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendReleaseRecordV4>(),
            structural_schema_hash::<KagemushaRecursiveSpendReleaseActivationV4>(),
        ],
        [
            "d3793d12ffa1d5ecaf1d8c85b4ec778e".to_owned(),
            "4b19da66ff810a6dcc7092e6c11b2c04".to_owned(),
            "772903e6c2decde8f59cca71ae2af06b".to_owned(),
            "0bfc9b61c82dfbc4e87965aa3873ca58".to_owned(),
            "5dcb95b2da2967dea092ce73a7aeaa94".to_owned(),
            "946c1385d266363ceae083ed95bf5855".to_owned(),
            "74ee082e0e6722c579e28bcfd7b55572".to_owned(),
            "0e748d6348dcdd81fe50042e12affb9b".to_owned(),
        ]
    );
}

#[test]
fn public_kagemusha_activation_receipt_schemas_are_frozen_for_first_release() {
    assert_eq!(
        [
            structural_schema_hash::<KagemushaExactBytesDigestV1>(),
            structural_schema_hash::<KagemushaV4GitHubPromotionRunV1>(),
            structural_schema_hash::<KagemushaV4PromotionReservationBodyV1>(),
            structural_schema_hash::<KagemushaV4PromotionReservationV1>(),
            structural_schema_hash::<KagemushaV4PromotionBindingV1>(),
            structural_schema_hash::<KagemushaV4RuntimeValidatorProjectionV1>(),
            structural_schema_hash::<KagemushaV4RuntimeEffectiveConfigProjectionV1>(),
            structural_schema_hash::<KagemushaV4ValidatorQualificationSealBodyV1>(),
            structural_schema_hash::<KagemushaV4ValidatorQualificationSealV1>(),
            structural_schema_hash::<KagemushaV4ActivationReceiptExpectationsBodyV1>(),
            structural_schema_hash::<KagemushaV4ActivationReceiptExpectationsArtifactV1>(),
            structural_schema_hash::<KagemushaFinalizedBlockWireV1>(),
            structural_schema_hash::<KagemushaV4ActivationFinalityProofChainV1>(),
            structural_schema_hash::<KagemushaV4ActivationFinalityReceiptBodyV1>(),
            structural_schema_hash::<KagemushaV4ActivationFinalityReceiptV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryAuthorizationBodyV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryPermitV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryReservationBodyV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryReservationV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryAuthorizationPackageV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryAuthorizationV1>(),
            structural_schema_hash::<AuthorizeKagemushaTairaCanaryV4>(),
            structural_schema_hash::<RecordKagemushaTairaCanaryV4>(),
            structural_schema_hash::<KagemushaV4TairaCanaryQueryObservationV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryEvidenceBodyV1>(),
            structural_schema_hash::<KagemushaV4TairaCanaryEvidenceV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessCanaryAnchorV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessTargetV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessChallengeBodyV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessChallengeV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessObservationV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessEvidenceBodyV1>(),
            structural_schema_hash::<KagemushaV4PostCanaryValidatorLivenessEvidenceV1>(),
        ],
        [
            "6ac84133729450e2392f324aae6d4e98".to_owned(),
            "2d43287df725fdac1dcaa156c9f419ba".to_owned(),
            "099e73f65ecc46c943b1a38a4a76ea52".to_owned(),
            "280cb539eb77f6f4b7754974522f07d7".to_owned(),
            "58bf4b648d507ecfaef604cc746b2228".to_owned(),
            "pending-runtime-validator-schema".to_owned(),
            "pending-runtime-config-schema".to_owned(),
            "ef3178e443bd626ca28c44c5900f3f6c".to_owned(),
            "692a53fdd5bb16cf2f6267564e696b59".to_owned(),
            "5ecdb55d5e634cca42c55ac7ee2c1da6".to_owned(),
            "1ee5a22e9979f3657c9d84a132ab253d".to_owned(),
            "ffa8c340c1cb5b1ac1fb3393f0278008".to_owned(),
            "e09dc896ca4af3edf286fb87e688fa9c".to_owned(),
            "6ba18f49bf3641fe423dddff52d6a363".to_owned(),
            "1397bcc0205a3fada9f8847936bad33c".to_owned(),
            "pending-taira-canary-authorization-body-schema".to_owned(),
            "pending-taira-canary-permit-schema".to_owned(),
            "pending-taira-canary-reservation-body-schema".to_owned(),
            "pending-taira-canary-reservation-schema".to_owned(),
            "pending-taira-canary-authorization-package-schema".to_owned(),
            "pending-taira-canary-authorization-schema".to_owned(),
            "pending-taira-canary-authorize-instruction-schema".to_owned(),
            "pending-taira-canary-record-instruction-schema".to_owned(),
            "pending-taira-canary-query-schema".to_owned(),
            "pending-taira-canary-body-schema".to_owned(),
            "pending-taira-canary-evidence-schema".to_owned(),
            "pending-post-canary-liveness-anchor-schema".to_owned(),
            "pending-post-canary-liveness-target-schema".to_owned(),
            "pending-post-canary-liveness-challenge-body-schema".to_owned(),
            "pending-post-canary-liveness-challenge-schema".to_owned(),
            "pending-post-canary-liveness-observation-schema".to_owned(),
            "pending-post-canary-liveness-evidence-body-schema".to_owned(),
            "pending-post-canary-liveness-evidence-schema".to_owned(),
        ]
    );
}
