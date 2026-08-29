//! Golden structural schemas for first-release public Offline requests and release provenance.
use iroha_data_model::isi::offline::{
    AuthorizeKagemushaTairaCanaryV4, RecordKagemushaTairaCanaryV4,
};
use iroha_data_model::offline::{
    KagemushaExactBytesDigestV1, KagemushaFinalizedBlockWireV1,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendCryptographicReviewSubjectV4,
    KagemushaRecursiveSpendInternalValidationReceiptV1, KagemushaRecursiveSpendPromotedReleaseV4,
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
    KagemushaV4ValidatorQualificationSealV1, OfflineAndroidAttestationStatusSnapshotV1,
    OfflineDeviceAttestationPolicy,
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
            "74c49db4a7409b01b69c4be3736f9fa0".to_owned(),
            "13e748a8145c226c48a46cfb8d2a9829".to_owned(),
            "d3f45905ef4fd933799654aadd00e00e".to_owned(),
            "2e93da6fb419e1dc4fd8e8898f7e48b0".to_owned(),
            "2c02da011b36781cb59d258a8d9e3507".to_owned(),
            "ada8d473d390a8fa9d372bc96b29e6c9".to_owned(),
            "766967fc902acf6db96f89f4b3b33c59".to_owned(),
            "5574a5e7ece0696bbbfdb2c3ccd8dd32".to_owned(),
        ]
    );
}

#[test]
fn public_kagemusha_internal_validation_receipt_schema_is_frozen_for_first_release() {
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendInternalValidationReceiptV1>(),
        "1033ca4e95d00e51a54c72812ba2bbed"
    );
}

#[test]
fn public_offline_device_attestation_policy_schemas_are_frozen_for_first_release() {
    assert_eq!(
        structural_schema_hash::<OfflineAndroidAttestationStatusSnapshotV1>(),
        "e35ecec076f7d14680b82c42ef072c87"
    );
    assert_eq!(
        structural_schema_hash::<OfflineDeviceAttestationPolicy>(),
        "8aee64787ce094a054aac19be8ddd7c6"
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
            "c678723f564279fc0e34c511d5949bc6".to_owned(),
            "bed74c06e3a528ecb21eb074144c11c6".to_owned(),
            "58bf4b648d507ecfaef604cc746b2228".to_owned(),
            "8fd2a3fc47baf857340965468acc6e4e".to_owned(),
            "cf5fad8e8d29bce96025624938b98978".to_owned(),
            "257741e512ac4724a13d1a16fd1d5f80".to_owned(),
            "83b59b3316b56396cb8019a6b147c958".to_owned(),
            "891c839a1c4c4c2aee05da7d2870fe25".to_owned(),
            "cb7cff07b7b2514bdafd29cac519be38".to_owned(),
            "ffa8c340c1cb5b1ac1fb3393f0278008".to_owned(),
            "e09dc896ca4af3edf286fb87e688fa9c".to_owned(),
            "e031a0fefbd7f3bdd1a17595c8771373".to_owned(),
            "688793c8ffdeb0cde0301f55094ceb59".to_owned(),
            "b455e2a6866e273eb3e91ef7f32e1e19".to_owned(),
            "338c2d0bebc0b86729cd22a6e6828900".to_owned(),
            "f365d43994e5daf2d2978aac950e75c2".to_owned(),
            "b5138aa1fb22e991c64cc35d220df84b".to_owned(),
            "1a589413f0d177fcd5b40f9a335ea8d5".to_owned(),
            "3b1b4a192d2edfb888577f0b1c902f12".to_owned(),
            "9a3608396160a7870e0c2a643a565863".to_owned(),
            "35aca72eb7aaac337d749874a59b9d92".to_owned(),
            "fe97ebc1f9facc78bbda6d5e7191b4dc".to_owned(),
            "e58d72f36f15e2bc775846dec21fae4f".to_owned(),
            "b8e301e76c9d368f4b2af44040837716".to_owned(),
            "cc646466c3d4031793e692dd00528f03".to_owned(),
            "68003281351de11ebbd136637f261056".to_owned(),
            "36e3fa477655e33de981f235a8953dbb".to_owned(),
            "8652cbe7186d08bc6cadd1b9e6f58c7f".to_owned(),
            "2b400d54c4ecc6dfc77cd435b39272e9".to_owned(),
            "2fd0bf37851f6713dbfedb8f39cc8c61".to_owned(),
            "d82914fc3ec57466da744a9251706a57".to_owned(),
        ]
    );
}
