//! Golden structural schemas for the first-release public Offline requests.
use iroha_data_model::offline::{
    KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendTopUpRequestV4,
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
        "cc4b252cf164cf026616483eda9e4085"
    );
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendRedeemRequestV4>(),
        "f93f8c4af5ac999d49c04b6f1639e03a"
    );
}
