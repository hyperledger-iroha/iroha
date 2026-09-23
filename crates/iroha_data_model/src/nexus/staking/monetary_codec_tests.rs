//! Codec and schema coverage for signed public-lane monetary bindings.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use std::any::TypeId;

#[test]
fn monetary_scope_roundtrips_both_variants_and_rejects_unknown_envelope_fields() {
    let network = crate::NetworkId::from_genesis_hash(
        HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed([1; 32])),
    );
    for (scope, tag) in [
        (PublicLaneMonetaryScopeV1::Genesis, "genesis"),
        (PublicLaneMonetaryScopeV1::Network(network), "network"),
    ] {
        let bytes = norito::to_bytes(&scope).unwrap();
        let decoded: PublicLaneMonetaryScopeV1 = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(decoded, scope);
        let json = norito::json::to_json(&scope).unwrap();
        let decoded: PublicLaneMonetaryScopeV1 = norito::json::from_json(&json).unwrap();
        assert_eq!(decoded, scope);
        let mut value = norito::json::to_value(&scope).unwrap();
        assert_eq!(value.as_object().unwrap()["kind"].as_str(), Some(tag));
        let decoded: PublicLaneMonetaryScopeV1 = norito::json::from_value(value.clone()).unwrap();
        assert_eq!(decoded, scope);
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), norito::json::Value::Null);
        assert!(norito::json::from_value::<PublicLaneMonetaryScopeV1>(value.clone()).is_err());
        assert!(
            norito::json::from_json::<PublicLaneMonetaryScopeV1>(
                &norito::json::to_json(&value).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn monetary_preconditions_roundtrip_complete_payloads_and_register_schema() {
    let peer_id = PeerId::new(
        KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let cases = [
        (
            PublicLaneMonetaryPreconditionV1::Registration(PublicLaneRegistrationPreconditionV1 {
                activation_height: 7,
            }),
            "registration",
            TypeId::of::<PublicLaneRegistrationPreconditionV1>(),
        ),
        (
            PublicLaneMonetaryPreconditionV1::Bond(PublicLaneBondPreconditionV1 {
                activation_height: 8,
                peer_id,
            }),
            "bond",
            TypeId::of::<PublicLaneBondPreconditionV1>(),
        ),
        (
            PublicLaneMonetaryPreconditionV1::Unbond(PublicLaneUnbondPreconditionV1 {
                activation_height: 9,
                request_hash: Hash::new(b"withdrawal"),
            }),
            "unbond",
            TypeId::of::<PublicLaneUnbondPreconditionV1>(),
        ),
        (
            PublicLaneMonetaryPreconditionV1::Slash(PublicLaneSlashPreconditionV1 {
                activation_height: 10,
                slashable_exposure: Quantity::from(99_u64),
            }),
            "slash",
            TypeId::of::<PublicLaneSlashPreconditionV1>(),
        ),
    ];
    let schema = PublicLaneMonetaryPreconditionV1::schema();
    let Some(iroha_schema::Metadata::Enum(metadata)) =
        schema.get::<PublicLaneMonetaryPreconditionV1>()
    else {
        panic!("precondition enum schema")
    };
    assert_eq!(metadata.variants.len(), cases.len());
    for (index, (precondition, tag, payload_type)) in cases.into_iter().enumerate() {
        assert_eq!(metadata.variants[index].discriminant, index as u32);
        assert_eq!(metadata.variants[index].ty, Some(payload_type));
        let bytes = norito::to_bytes(&precondition).unwrap();
        let decoded: PublicLaneMonetaryPreconditionV1 = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(decoded, precondition);
        let json = norito::json::to_json(&precondition).unwrap();
        let decoded: PublicLaneMonetaryPreconditionV1 = norito::json::from_json(&json).unwrap();
        assert_eq!(decoded, precondition);
        let mut value = norito::json::to_value(&precondition).unwrap();
        assert_eq!(value.as_object().unwrap()["kind"].as_str(), Some(tag));
        let decoded: PublicLaneMonetaryPreconditionV1 =
            norito::json::from_value(value.clone()).unwrap();
        assert_eq!(decoded, precondition);
        value
            .as_object_mut()
            .unwrap()
            .get_mut("value")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<PublicLaneMonetaryPreconditionV1>(value.clone()).is_err()
        );
        assert!(
            norito::json::from_json::<PublicLaneMonetaryPreconditionV1>(
                &norito::json::to_json(&value).unwrap()
            )
            .is_err()
        );
    }
}
