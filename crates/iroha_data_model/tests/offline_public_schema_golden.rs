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
fn public_offline_request_structural_schemas_are_frozen_for_abi20_v4() {
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendTopUpRequestV4>(),
        "61c0b26a37f66702fec5f6fe3ec0fd7b"
    );
    assert_eq!(
        structural_schema_hash::<KagemushaRecursiveSpendRedeemRequestV4>(),
        "9baef4065151b262dd09536997e51933"
    );
}
