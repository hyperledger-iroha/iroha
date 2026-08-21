//! Golden structural schemas for first-release public Offline requests and release provenance.
use iroha_data_model::offline::{
    KagemushaExactBytesDigestV1, KagemushaFinalizedBlockWireV1,
    KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendCryptographicReviewSubjectV4, KagemushaRecursiveSpendPromotedReleaseV4,
    KagemushaRecursiveSpendQualificationReceiptV4, KagemushaRecursiveSpendRedeemRequestV4,
    KagemushaRecursiveSpendReleaseActivationV4, KagemushaRecursiveSpendReleaseAttestationSubjectV4,
    KagemushaRecursiveSpendReleaseRecordV4, KagemushaRecursiveSpendTopUpRequestV4,
    KagemushaV4ActivationFinalityReceiptBodyV1, KagemushaV4ActivationFinalityReceiptV1,
    KagemushaV4PromotionBindingV1, KagemushaV4ValidatorQualificationSealBodyV1,
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
            structural_schema_hash::<KagemushaV4PromotionBindingV1>(),
            structural_schema_hash::<KagemushaV4ValidatorQualificationSealBodyV1>(),
            structural_schema_hash::<KagemushaV4ValidatorQualificationSealV1>(),
            structural_schema_hash::<KagemushaFinalizedBlockWireV1>(),
            structural_schema_hash::<KagemushaV4ActivationFinalityReceiptBodyV1>(),
            structural_schema_hash::<KagemushaV4ActivationFinalityReceiptV1>(),
        ],
        [
            "6ac84133729450e2392f324aae6d4e98".to_owned(),
            "0fadd89106b4d767a2f3fa08e5f347b2".to_owned(),
            "1515a8f6642258e5fa36149b3f65c7cd".to_owned(),
            "984109c9a925c4724380f32cb90277fb".to_owned(),
            "ffa8c340c1cb5b1ac1fb3393f0278008".to_owned(),
            "52437162d2eddc16d47de914ed850a7c".to_owned(),
            "c57a922d03191b30e9eadfae4b2a5327".to_owned(),
        ]
    );
}
