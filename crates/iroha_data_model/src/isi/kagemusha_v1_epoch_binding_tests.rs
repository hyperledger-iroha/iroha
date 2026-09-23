//! Canonical codec and schema coverage for scheduling epoch bindings.

use super::*;
use iroha_crypto::HashOf;
use norito::codec::DecodeAll as _;
use std::any::TypeId;

#[test]
fn beacon_epoch_binding_roundtrips_both_variants_and_registers_payload_schema() {
    let installed = InstalledBeaconEpochBindingV1 {
        session_id: [7; 32],
        transcript_hash: [8; 32],
    };
    for (binding, tag) in [
        (BeaconEpochBindingV1::Bootstrap, "bootstrap"),
        (BeaconEpochBindingV1::Installed(installed), "installed"),
    ] {
        let bytes = binding.encode();
        let decoded: BeaconEpochBindingV1 =
            BeaconEpochBindingV1::decode_all(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, binding);
        let json = norito::json::to_json(&binding).unwrap();
        let decoded: BeaconEpochBindingV1 = norito::json::from_json(&json).unwrap();
        assert_eq!(decoded, binding);
        let mut value = norito::json::to_value(&binding).unwrap();
        assert_eq!(value.as_object().unwrap()["kind"].as_str(), Some(tag));
        let decoded: BeaconEpochBindingV1 = norito::json::from_value(value.clone()).unwrap();
        assert_eq!(decoded, binding);
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), norito::json::Value::Null);
        assert!(norito::json::from_value::<BeaconEpochBindingV1>(value.clone()).is_err());
        assert!(
            norito::json::from_json::<BeaconEpochBindingV1>(
                &norito::json::to_json(&value).unwrap()
            )
            .is_err()
        );
    }
    let mut value = norito::json::to_value(&BeaconEpochBindingV1::Installed(installed)).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .get_mut("value")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("unknown".into(), norito::json::Value::Null);
    assert!(norito::json::from_value::<BeaconEpochBindingV1>(value.clone()).is_err());
    assert!(
        norito::json::from_json::<BeaconEpochBindingV1>(&norito::json::to_json(&value).unwrap())
            .is_err()
    );
    let schema = BeaconEpochBindingV1::schema();
    let Some(iroha_schema::Metadata::Enum(metadata)) = schema.get::<BeaconEpochBindingV1>() else {
        panic!("beacon enum schema");
    };
    assert_eq!(metadata.variants.len(), 2);
    assert_eq!(metadata.variants[0].discriminant, 0);
    assert_eq!(metadata.variants[0].ty, None);
    assert_eq!(metadata.variants[1].discriminant, 1);
    assert_eq!(
        metadata.variants[1].ty,
        Some(TypeId::of::<InstalledBeaconEpochBindingV1>())
    );
    let Some(iroha_schema::Metadata::Struct(payload)) =
        schema.get::<InstalledBeaconEpochBindingV1>()
    else {
        panic!("installed payload schema");
    };
    assert_eq!(
        payload
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["session_id", "transcript_hash"]
    );
}

#[test]
fn epoch_decisions_roundtrip_all_discriminants_and_reject_untagged_json() {
    for (decision, discriminant, tag) in [
        (KagemushaMintFinalityEpochDecisionV1::Genesis, 0, "genesis"),
        (
            KagemushaMintFinalityEpochDecisionV1::Activate,
            1,
            "activate",
        ),
        (KagemushaMintFinalityEpochDecisionV1::Retain, 2, "retain"),
        (
            KagemushaMintFinalityEpochDecisionV1::RetainAndCancel,
            3,
            "retain_and_cancel",
        ),
    ] {
        assert_eq!(decision as u8, discriminant);
        let bytes = decision.encode();
        let decoded: KagemushaMintFinalityEpochDecisionV1 =
            KagemushaMintFinalityEpochDecisionV1::decode_all(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, decision);
        let json = norito::json::to_json(&decision).unwrap();
        assert_eq!(json, format!(r#"{{"kind":"{tag}","value":null}}"#));
        let decoded: KagemushaMintFinalityEpochDecisionV1 = norito::json::from_json(&json).unwrap();
        assert_eq!(decoded, decision);
        let decoded: KagemushaMintFinalityEpochDecisionV1 =
            norito::json::from_value(norito::json::to_value(&decision).unwrap()).unwrap();
        assert_eq!(decoded, decision);
    }
    for json in [
        r#""retain""#,
        r#"{"value":null}"#,
        r#"{"kind":"retain","value":null,"future":1}"#,
    ] {
        assert!(norito::json::from_json::<KagemushaMintFinalityEpochDecisionV1>(json).is_err());
        let value: norito::json::Value = norito::json::from_json(json).unwrap();
        assert!(norito::json::from_value::<KagemushaMintFinalityEpochDecisionV1>(value).is_err());
    }
}

#[test]
fn epoch_authorization_binding_keeps_fixed_width_identity() {
    let genesis = KagemushaMintFinalityEpochAuthorizationV1 {
        version: 1,
        network_id: NetworkId::from_genesis_hash(
            HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed([1; 32])),
        ),
        epoch: 0,
        first_height: 1,
        last_height: 10,
        authority_generation: 0,
        authority_id: [2; 32],
        beacon: BeaconEpochBindingV1::Bootstrap,
        previous_authorization_id: [0; 32],
        transition_id: [0; 32],
        decision: KagemushaMintFinalityEpochDecisionV1::Genesis,
    };
    let installed = KagemushaMintFinalityEpochAuthorizationV1 {
        epoch: 1,
        first_height: 11,
        last_height: 20,
        beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
            session_id: [7; 32],
            transcript_hash: [8; 32],
        }),
        previous_authorization_id: [3; 32],
        decision: KagemushaMintFinalityEpochDecisionV1::Retain,
        ..genesis
    };
    // Golden values independently computed from the documented fixed-width SHA-256 transcript.
    for (authorization, expected) in [
        (
            genesis,
            "b91c4a12f6d54b0124cb54865be202cdacbd9be78e8b75a6cefb346f90f5e918",
        ),
        (
            installed,
            "08dc4e2662ec63f7d6d18a185a8027120f1a1f6e7d98f9625241a41a9ecfa755",
        ),
    ] {
        let digest = authorization.authorization_id().unwrap();
        let actual: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(actual, expected);
        let bytes = authorization.encode();
        let decoded: KagemushaMintFinalityEpochAuthorizationV1 =
            KagemushaMintFinalityEpochAuthorizationV1::decode_all(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, authorization);
        assert_eq!(decoded.authorization_id().unwrap(), digest);
    }
}
