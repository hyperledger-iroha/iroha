//! Closed monetary staking codecs and operation-specific schema boundaries.

use super::*;
use crate::{NetworkId, asset::AssetDefinitionId, block::BlockHeader};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use norito::json::Value;
use std::any::TypeId;

fn keypair() -> KeyPair {
    KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).expect("fixture keypair")
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"staking monetary codec fixture",
    )))
}

fn preconditions(height: u64) -> [PublicLaneMonetaryPreconditionV1; 4] {
    [
        PublicLaneMonetaryPreconditionV1::Registration(PublicLaneMonetaryRegistrationV1 {
            activation_height: height,
        }),
        PublicLaneMonetaryPreconditionV1::Bond(PublicLaneMonetaryBondV1 {
            activation_height: height,
            peer_id: PeerId::new(keypair().public_key().clone()),
        }),
        PublicLaneMonetaryPreconditionV1::Unbond(PublicLaneMonetaryUnbondV1 {
            activation_height: height,
            request_hash: Hash::new(b"exact pending withdrawal"),
        }),
        PublicLaneMonetaryPreconditionV1::Slash(PublicLaneMonetarySlashV1 {
            activation_height: height,
            slashable_exposure: "9007199254740993.000000001"
                .parse()
                .expect("exact exposure"),
        }),
    ]
}

fn plan() -> PublicLaneMonetaryPlanV1 {
    let definition = AssetDefinitionId::from_uuid_bytes([
        0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd, 0xcd,
        0x2f,
    ])
    .expect("canonical asset definition");
    let asset = AssetId::new(definition, AccountId::new(keypair().public_key().clone()));
    PublicLaneMonetaryPlanV1::genesis_registration(asset.clone(), asset, Quantity::from(17_u64))
}

fn assert_precondition_json_refused(value: Value) {
    let text = norito::json::to_json(&value).expect("malformed fixture JSON");
    assert!(
        norito::json::from_str::<PublicLaneMonetaryPreconditionV1>(&text).is_err(),
        "{text}"
    );
    assert!(norito::json::from_value::<PublicLaneMonetaryPreconditionV1>(value).is_err());
}

#[test]
fn monetary_variants_roundtrip_canonical_binary_and_tagged_json() {
    for (tag, scope) in [
        ("Genesis", PublicLaneMonetaryScopeV1::Genesis),
        ("Network", PublicLaneMonetaryScopeV1::Network(network())),
    ] {
        let bytes = norito::encode_canonical(&scope).expect("scope encoding");
        assert_eq!(
            norito::decode_canonical::<PublicLaneMonetaryScopeV1>(&bytes).unwrap(),
            scope
        );
        let value = norito::json::to_value(&scope).expect("scope JSON");
        assert_eq!(value.get("scope"), Some(&Value::String(tag.into())));
        assert_eq!(value.as_object().unwrap().len(), 2);
        let text = norito::json::to_json(&scope).unwrap();
        assert_eq!(
            norito::json::from_str::<PublicLaneMonetaryScopeV1>(&text).unwrap(),
            scope
        );
        assert_eq!(
            norito::json::from_value::<PublicLaneMonetaryScopeV1>(value).unwrap(),
            scope
        );
    }
    for height in [0, 1, u64::MAX] {
        for (tag, condition) in ["Registration", "Bond", "Unbond", "Slash"]
            .into_iter()
            .zip(preconditions(height))
        {
            let bytes = norito::encode_canonical(&condition).expect("precondition encoding");
            assert_eq!(
                norito::decode_canonical::<PublicLaneMonetaryPreconditionV1>(&bytes).unwrap(),
                condition
            );
            let value = norito::json::to_value(&condition).unwrap();
            assert_eq!(value.get("operation"), Some(&Value::String(tag.into())));
            assert_eq!(value.as_object().unwrap().len(), 2);
            let text = norito::json::to_json(&condition).unwrap();
            assert_eq!(
                norito::json::from_str::<PublicLaneMonetaryPreconditionV1>(&text).unwrap(),
                condition
            );
            assert_eq!(
                norito::json::from_value::<PublicLaneMonetaryPreconditionV1>(value).unwrap(),
                condition
            );
        }
    }
}

#[test]
fn monetary_schema_names_every_variant_and_distinct_named_payload() {
    let schema = PublicLaneMonetaryPreconditionV1::schema();
    let iroha_schema::Metadata::Enum(metadata) =
        schema.get::<PublicLaneMonetaryPreconditionV1>().unwrap()
    else {
        panic!("precondition must have enum schema");
    };
    let expected = [
        (
            "Registration",
            TypeId::of::<PublicLaneMonetaryRegistrationV1>(),
            vec!["activation_height"],
        ),
        (
            "Bond",
            TypeId::of::<PublicLaneMonetaryBondV1>(),
            vec!["activation_height", "peer_id"],
        ),
        (
            "Unbond",
            TypeId::of::<PublicLaneMonetaryUnbondV1>(),
            vec!["activation_height", "request_hash"],
        ),
        (
            "Slash",
            TypeId::of::<PublicLaneMonetarySlashV1>(),
            vec!["activation_height", "slashable_exposure"],
        ),
    ];
    assert_eq!(metadata.variants.len(), expected.len());
    for (index, (variant, (tag, ty, _))) in metadata.variants.iter().zip(&expected).enumerate() {
        assert_eq!(variant.tag.as_str(), *tag);
        assert_eq!(variant.discriminant, u32::try_from(index).unwrap());
        assert_eq!(variant.ty, Some(*ty));
    }
    macro_rules! fields {
        ($ty:ty, $index:expr) => {
            let iroha_schema::Metadata::Struct(metadata) = schema.get::<$ty>().unwrap() else {
                panic!("payload must have named field schema");
            };
            assert_eq!(
                metadata
                    .declarations
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                expected[$index].2
            );
        };
    }
    fields!(PublicLaneMonetaryRegistrationV1, 0);
    fields!(PublicLaneMonetaryBondV1, 1);
    fields!(PublicLaneMonetaryUnbondV1, 2);
    fields!(PublicLaneMonetarySlashV1, 3);
    let scope_schema = PublicLaneMonetaryScopeV1::schema();
    let iroha_schema::Metadata::Enum(scope) =
        scope_schema.get::<PublicLaneMonetaryScopeV1>().unwrap()
    else {
        panic!("scope must have enum schema");
    };
    assert_eq!(scope.variants.len(), 2);
    assert_eq!(
        (
            scope.variants[0].tag.as_str(),
            scope.variants[0].discriminant,
            scope.variants[0].ty
        ),
        ("Genesis", 0, None)
    );
    assert_eq!(
        (
            scope.variants[1].tag.as_str(),
            scope.variants[1].discriminant,
            scope.variants[1].ty
        ),
        ("Network", 1, Some(TypeId::of::<NetworkId>()))
    );
}

#[test]
fn monetary_json_rejects_relabelled_and_unknown_operation_payloads() {
    let tags = ["Registration", "Bond", "Unbond", "Slash"];
    for (index, condition) in preconditions(1).into_iter().enumerate() {
        let value = norito::json::to_value(&condition).unwrap();
        for tag in tags
            .into_iter()
            .filter(|tag| *tag != tags[index])
            .chain(["Unknown", "bond"])
        {
            let mut relabelled = value.clone();
            relabelled
                .as_object_mut()
                .unwrap()
                .insert("operation".into(), Value::String(tag.into()));
            assert_precondition_json_refused(relabelled);
        }
        let mut unknown_envelope = value.clone();
        unknown_envelope
            .as_object_mut()
            .unwrap()
            .insert("unreviewed".into(), Value::Null);
        assert_precondition_json_refused(unknown_envelope);
        let mut unknown_payload = value.clone();
        unknown_payload
            .as_object_mut()
            .unwrap()
            .get_mut("value")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unreviewed".into(), Value::Null);
        assert_precondition_json_refused(unknown_payload);
        for required in ["operation", "value"] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(required);
            assert_precondition_json_refused(missing);
        }
    }
    for malformed in [
        "null",
        "0",
        "\"Bond\"",
        "{}",
        "{\"operation\":\"Registration\",\"value\":{\"activation_height\":-1}}",
        "{\"operation\":\"Registration\",\"value\":{\"activation_height\":18446744073709551616}}",
        "{\"operation\":\"Slash\",\"value\":{\"activation_height\":1,\"slashable_exposure\":\"-1\"}}",
    ] {
        assert_precondition_json_refused(norito::json::from_str(malformed).unwrap());
    }
}

#[test]
fn monetary_scope_json_rejects_relabelled_or_unscoped_authority() {
    let mut network =
        norito::json::to_value(&PublicLaneMonetaryScopeV1::Network(network())).unwrap();
    network
        .as_object_mut()
        .unwrap()
        .insert("scope".into(), Value::String("Genesis".into()));
    let network_as_genesis = norito::json::to_json(&network).unwrap();
    for malformed in [
        "\"Genesis\"",
        "null",
        "{}",
        "{\"scope\":\"Unknown\",\"value\":null}",
        "{\"scope\":\"Network\",\"value\":null}",
        "{\"scope\":\"Genesis\",\"value\":null,\"extra\":1}",
        network_as_genesis.as_str(),
    ] {
        assert!(
            norito::json::from_str::<PublicLaneMonetaryScopeV1>(malformed).is_err(),
            "{malformed}"
        );
        let value: Value = norito::json::from_str(malformed).unwrap();
        assert!(norito::json::from_value::<PublicLaneMonetaryScopeV1>(value).is_err());
    }
}

#[test]
fn monetary_binary_rejects_truncation_and_wrong_schema() {
    for condition in preconditions(u64::MAX) {
        let bytes = norito::encode_canonical(&condition).unwrap();
        for end in 0..bytes.len() {
            assert!(
                norito::decode_canonical::<PublicLaneMonetaryPreconditionV1>(&bytes[..end])
                    .is_err(),
                "accepted truncation at {end}"
            );
        }
        assert!(norito::decode_canonical::<PublicLaneMonetaryScopeV1>(&bytes).is_err());
    }
    let scope = norito::encode_canonical(&PublicLaneMonetaryScopeV1::Genesis).unwrap();
    assert!(norito::decode_canonical::<PublicLaneMonetaryPreconditionV1>(&scope).is_err());
}

#[test]
fn monetary_plan_preserves_exact_boundary_checks_and_closed_json() {
    let mut plan = plan();
    assert!(plan.has_canonical_shape());
    assert_eq!(plan.valid_until_height, 1);
    assert_eq!(plan.precondition, preconditions(1)[0]);
    for height in [0, 1, u64::MAX] {
        for condition in preconditions(height) {
            plan.precondition = condition;
            assert_eq!(plan.has_canonical_shape(), height > 0);
            let bytes = norito::encode_canonical(&plan).unwrap();
            assert_eq!(
                norito::decode_canonical::<PublicLaneMonetaryPlanV1>(&bytes).unwrap(),
                plan
            );
            let text = norito::json::to_json(&plan).unwrap();
            assert_eq!(
                norito::json::from_str::<PublicLaneMonetaryPlanV1>(&text).unwrap(),
                plan
            );
        }
    }
    plan.precondition = PublicLaneMonetaryPreconditionV1::Slash(PublicLaneMonetarySlashV1 {
        activation_height: 1,
        slashable_exposure: Quantity::from(16_u64),
    });
    assert!(!plan.has_canonical_shape());
    plan.precondition = preconditions(1)[0].clone();
    plan.valid_until_height = 0;
    assert!(!plan.has_canonical_shape());
    plan.valid_until_height = 1;
    plan.amount = Quantity::zero();
    assert!(!plan.has_canonical_shape());
    let mut value = norito::json::to_value(&plan).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unreviewed".into(), Value::Null);
    assert!(norito::json::from_value::<PublicLaneMonetaryPlanV1>(value.clone()).is_err());
    assert!(
        norito::json::from_str::<PublicLaneMonetaryPlanV1>(&norito::json::to_json(&value).unwrap())
            .is_err()
    );
}
