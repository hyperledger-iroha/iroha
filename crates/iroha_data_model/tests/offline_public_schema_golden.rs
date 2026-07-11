//! Golden structural schemas for the first-release public Offline requests.

use iroha_data_model::offline::{
    KagemushaRecursiveSpendRedeemRequestV2, KagemushaRecursiveSpendTopUpRequestV2,
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
fn public_offline_request_structural_schemas_are_frozen_for_v1() {
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendTopUpRequestV2>(),
        "e4df2ad21939cb32b251406f431c3d7a"
    );
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendRedeemRequestV2>(),
        "6bcd95587f27af1626d014a2b2494eb0"
    );
}
