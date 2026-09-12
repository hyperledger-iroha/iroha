//! Canonical binary, tagged JSON, and schema coverage for native custody DTOs.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_schema::Metadata;

#[test]
fn custody_actions_roundtrip_canonical_frames_and_tagged_json() {
    let actions = [
        (
            "configure",
            SorafsStreamTokenCustodyActionV1::Configure(vec![1, 2, 3]),
        ),
        (
            "enroll",
            SorafsStreamTokenCustodyActionV1::Enroll(vec![4, 5, 6]),
        ),
        (
            "revoke",
            SorafsStreamTokenCustodyActionV1::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: true,
                attester: false,
            }),
        ),
    ];
    for (tag, action) in actions {
        let frame = norito::encode_canonical(&action).expect("canonical action frame");
        assert_eq!(
            norito::decode_canonical::<SorafsStreamTokenCustodyActionV1>(&frame)
                .expect("decode canonical action"),
            action
        );
        let json = norito::json::to_json(&action).expect("tagged action JSON");
        assert_eq!(
            norito::json::from_str::<SorafsStreamTokenCustodyActionV1>(&json)
                .expect("decode tagged action JSON"),
            action
        );
        let value = norito::json::to_value(&action).expect("action JSON object");
        assert_eq!(
            value.get("action").and_then(|value| value.as_str()),
            Some(tag)
        );
        assert!(value.get("value").is_some());
        assert!(
            norito::decode_canonical::<SorafsStreamTokenCustodyActionV1>(&frame[..frame.len() - 1])
                .is_err()
        );
    }
    for invalid in [
        r#"{"signer":true,"attester":false}"#,
        r#"{"action":"Revoke","value":{"signer":true,"attester":false}}"#,
        r#"{"action":"revoke","value":{"signer":true}}"#,
    ] {
        assert!(norito::json::from_str::<SorafsStreamTokenCustodyActionV1>(invalid).is_err());
    }
}

#[test]
fn custody_revocation_payload_roundtrips_each_flag_combination() {
    for signer in [false, true] {
        for attester in [false, true] {
            let revocation = SorafsStreamTokenCustodyRevocationV1 { signer, attester };
            let frame = norito::encode_canonical(&revocation).expect("canonical revocation");
            assert_eq!(
                norito::decode_canonical::<SorafsStreamTokenCustodyRevocationV1>(&frame)
                    .expect("decode revocation"),
                revocation
            );
            let action = SorafsStreamTokenCustodyActionV1::Revoke(revocation);
            assert_eq!(
                norito::json::to_value(&action).expect("revocation JSON"),
                norito::json!({
                    "action": "revoke",
                    "value": { "signer": signer, "attester": attester }
                })
            );
        }
    }
}

#[test]
fn custody_action_schema_references_the_typed_revocation_payload() {
    let schema = SorafsStreamTokenCustodyActionV1::schema();
    let Metadata::Enum(metadata) = schema
        .get::<SorafsStreamTokenCustodyActionV1>()
        .expect("action schema")
    else {
        panic!("custody action must have an enum schema");
    };
    assert_eq!(
        metadata
            .variants
            .iter()
            .map(|variant| (variant.tag.as_str(), variant.discriminant))
            .collect::<Vec<_>>(),
        [("configure", 0), ("enroll", 1), ("revoke", 2)]
    );
    assert_eq!(
        metadata.variants[2].ty,
        Some(core::any::TypeId::of::<SorafsStreamTokenCustodyRevocationV1>())
    );
    assert!(matches!(
        schema.get::<SorafsStreamTokenCustodyRevocationV1>(),
        Some(Metadata::Struct(_))
    ));
}

#[test]
fn custody_control_record_roundtrips_binary_and_json() {
    let key = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519)
        .expect("checked custody authority fixture");
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: ProviderId::new([2; 32]),
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [3; 32],
        execution_height: 4,
        ordinal: 0,
        recorded_at_unix_ms: 5,
        authority: AccountId::new(key.public_key().clone()),
        control_state: vec![6, 7, 8],
    };
    let frame = norito::encode_canonical(&record).expect("canonical control record");
    assert_eq!(
        norito::decode_canonical::<StreamTokenCustodyControlRecordV1>(&frame)
            .expect("decode control record"),
        record
    );
    let json = norito::json::to_json(&record).expect("control record JSON");
    assert_eq!(
        norito::json::from_str::<StreamTokenCustodyControlRecordV1>(&json)
            .expect("decode control record JSON"),
        record
    );
}
