//! Ensures confidential key derivations match the published fixtures.

use hex::decode;
use iroha_crypto::derive_keyset;

#[test]
fn confidential_vectors_match_fixture() {
    let fixture = include_str!("../../../docs/source/confidential_key_vectors.json");
    let values: norito::json::Value = norito::json::from_str(fixture).expect("parse fixture");
    let items = values.as_array().expect("fixture array");
    for item in items {
        let obj = item.as_object().expect("fixture object");
        let seed_hex = obj
            .get("seed_hex")
            .and_then(|v| v.as_str())
            .expect("seed hex");
        let seed_bytes = decode(seed_hex).expect("seed decode");
        let mut seed_array = [0u8; 32];
        seed_array.copy_from_slice(&seed_bytes);
        let derived = derive_keyset(seed_array);

        assert_eq!(
            hex::encode(derived.nullifier_key()),
            obj.get("nullifier_key_hex")
                .and_then(|v| v.as_str())
                .expect("nk hex")
        );
        assert_eq!(
            hex::encode(derived.incoming_view_key()),
            obj.get("incoming_view_key_hex")
                .and_then(|v| v.as_str())
                .expect("ivk hex")
        );
        assert_eq!(
            hex::encode(derived.outgoing_view_key()),
            obj.get("outgoing_view_key_hex")
                .and_then(|v| v.as_str())
                .expect("ovk hex")
        );
        assert_eq!(
            hex::encode(derived.full_view_key()),
            obj.get("full_view_key_hex")
                .and_then(|v| v.as_str())
                .expect("fvk hex")
        );
    }
}
