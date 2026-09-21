//! Account-custody wire ownership, closed JSON and separation from receipt authority.
use super::*;
use crate::isi::sorafs::MutateSorafsFinalPromotionAccountCustody;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_schema::Metadata;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAuthorityV1, SignerCustodyBindingV1, SignerCustodyErrorV1},
    custody_control::{SignerCustodyControlStateV1, SignerCustodyPolicyV1},
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};

fn account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
    AccountId::new(key.public_key().clone())
}

fn policy() -> SignerCustodyPolicyV1 {
    let key = |seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
    SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: "promotion-account-chain".into(),
            network_id: [1; 32],
            runtime_handle: "software://sorafs/final-promotion-account-transaction/primary".into(),
            key_handle: "software://sorafs/final-promotion-account-transaction/key-1".into(),
            service_id: "promotion-account-service".into(),
            administrator_id: "promotion-account-admin".into(),
            role: SignerRoleV1::FinalPromotionAccountTransaction,
            purpose: SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                deployment_id: "production-primary".into(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key(2).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [3; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "account-custody-evidence".into(),
            administrator_id: "account-custody-security".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [4; 32],
        },
        attester_public_key: key(5).public_key().clone(),
        active_from_unix_ms: 100,
        active_until_unix_ms: 5_000,
        max_validity_ms: 2_000,
        max_anchor_age_ms: 1_000,
    }
}

fn current_check() -> FinalPromotionAccountCustodyCheckV1 {
    FinalPromotionAccountCustodyCheckV1 {
        challenge: [6; 32],
        network_id: [1; 32],
        minimum_height: 1,
        minimum_block_hash: [7; 32],
        expected_account: account(2),
        transaction_payload_digest: [8; 32],
    }
}

fn record() -> FinalPromotionAccountCustodyRecordV1 {
    let control = SignerCustodyControlStateV1 {
        policy: policy(),
        next_sequence: 1,
        predecessor_digest: [0; 32],
        active_head: None,
        signer_revoked: false,
        attester_revoked: false,
    };
    control.validate().unwrap();
    FinalPromotionAccountCustodyRecordV1 {
        deployment_id: "production-primary".into(),
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [9; 32],
        execution: FinalPromotionAccountCustodyExecutionV1 {
            height: 2,
            ordinal: 0,
            recorded_at_unix_ms: 101,
            authority: account(10),
        },
        control_state: norito::encode_canonical(&control).unwrap(),
        enrollment: None,
    }
}

#[test]
fn account_custody_actions_roundtrip_the_single_four_action_inventory() {
    let policy = policy();
    policy.validate().unwrap();
    assert_ne!(policy.binding.public_key, policy.attester_public_key);
    // Explicit testing namespaces must not serve as governed authority identities.
    for administrator in [false, true] {
        let mut invalid = policy.clone();
        if administrator {
            invalid.attester_authority.administrator_id = "test-account-custody-security".into();
        } else {
            invalid.attester_authority.service_id = "test-account-custody-evidence".into();
        }
        assert_eq!(
            invalid.validate(),
            Err(SignerCustodyErrorV1::UntrustedAuthority)
        );
    }
    let frame = norito::encode_canonical(&policy).unwrap();
    // DTO encoding retains opaque bytes; native enrollment authentication is a separate owner.
    let actions = [
        (
            "configure",
            FinalPromotionAccountCustodyActionV1::Configure(frame.clone()),
        ),
        (
            "enroll",
            FinalPromotionAccountCustodyActionV1::Enroll(vec![1, 2, 3]),
        ),
        (
            "revoke",
            FinalPromotionAccountCustodyActionV1::Revoke(
                FinalPromotionAccountCustodyRevocationV1 {
                    signer: true,
                    attester: false,
                },
            ),
        ),
        (
            "check",
            FinalPromotionAccountCustodyActionV1::Check(current_check()),
        ),
    ];
    for (tag, action) in actions {
        let encoded = norito::encode_canonical(&action).unwrap();
        assert_eq!(
            norito::decode_canonical::<FinalPromotionAccountCustodyActionV1>(&encoded).unwrap(),
            action
        );
        let json = norito::json::to_json(&action).unwrap();
        assert_eq!(
            norito::json::from_str::<FinalPromotionAccountCustodyActionV1>(&json).unwrap(),
            action
        );
        assert_eq!(
            norito::json::to_value(&action)
                .unwrap()
                .get("action")
                .and_then(|value| value.as_str()),
            Some(tag)
        );
        assert!(
            norito::decode_canonical::<FinalPromotionAccountCustodyActionV1>(
                &encoded[..encoded.len() - 1]
            )
            .is_err()
        );
    }
    let action = FinalPromotionAccountCustodyActionV1::Configure(frame.clone());
    let decoded: FinalPromotionAccountCustodyActionV1 =
        norito::decode_canonical(&norito::encode_canonical(&action).unwrap()).unwrap();
    let FinalPromotionAccountCustodyActionV1::Configure(retained) = decoded else {
        panic!("configure frame")
    };
    assert_eq!(retained, frame);
    assert_eq!(
        norito::decode_canonical::<SignerCustodyPolicyV1>(&retained).unwrap(),
        policy
    );
}

#[test]
fn account_custody_revocations_retain_each_flag_without_an_operation_action() {
    for signer in [false, true] {
        for attester in [false, true] {
            let value = FinalPromotionAccountCustodyRevocationV1 { signer, attester };
            let frame = norito::encode_canonical(&value).unwrap();
            assert_eq!(
                norito::decode_canonical::<FinalPromotionAccountCustodyRevocationV1>(&frame)
                    .unwrap(),
                value
            );
            assert_eq!(
                norito::json::to_value(&value).unwrap(),
                norito::json!({"signer": signer, "attester": attester})
            );
        }
    }
    // The wire can express false/false. Native monotonic revocation rejects this no-op.
}

#[test]
fn account_custody_records_preserve_the_manifest_control_frame_and_provenance() {
    for enrollment in [None, Some(vec![11, 12, 13])] {
        let record = FinalPromotionAccountCustodyRecordV1 {
            enrollment,
            ..record()
        };
        let frame = norito::encode_canonical(&record).unwrap();
        assert!(frame.len() < FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1);
        let decoded: FinalPromotionAccountCustodyRecordV1 =
            norito::decode_canonical(&frame).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(decoded.control_state, record.control_state);
        let control: SignerCustodyControlStateV1 =
            norito::decode_canonical(&decoded.control_state).unwrap();
        assert_eq!(control.policy, policy());
        let json = norito::json::to_json(&record).unwrap();
        assert_eq!(
            norito::json::from_str::<FinalPromotionAccountCustodyRecordV1>(&json).unwrap(),
            record
        );
        assert!(!json.contains("block_hash"));
        assert!(!json.contains("operation_id"));
        assert!(
            norito::decode_canonical::<FinalPromotionAccountCustodyRecordV1>(
                &frame[..frame.len() - 1]
            )
            .is_err()
        );
    }
}

#[test]
fn account_custody_check_and_outer_cas_roundtrip_exact_independent_coordinates() {
    let check = current_check();
    let instruction = MutateSorafsFinalPromotionAccountCustody {
        deployment_id: "production-primary".into(),
        expected_control_revision: 2,
        expected_control_digest: [14; 32],
        action: FinalPromotionAccountCustodyActionV1::Check(check.clone()),
    };
    let frame = norito::encode_canonical(&instruction).unwrap();
    assert!(frame.len() < FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1);
    assert_eq!(
        norito::decode_canonical::<MutateSorafsFinalPromotionAccountCustody>(&frame).unwrap(),
        instruction
    );
    let json = norito::json::to_json(&instruction).unwrap();
    assert_eq!(
        norito::json::from_str::<MutateSorafsFinalPromotionAccountCustody>(&json).unwrap(),
        instruction
    );
    let frame = norito::encode_canonical(&check).unwrap();
    assert_eq!(
        norito::decode_canonical::<FinalPromotionAccountCustodyCheckV1>(&frame).unwrap(),
        check
    );
    assert!(
        norito::decode_canonical::<FinalPromotionAccountCustodyCheckV1>(&frame[..frame.len() - 1])
            .is_err()
    );
}

#[test]
fn account_custody_action_schema_has_only_control_and_current_check() {
    let schema = FinalPromotionAccountCustodyActionV1::schema();
    let Metadata::Enum(metadata) = schema
        .get::<FinalPromotionAccountCustodyActionV1>()
        .unwrap()
    else {
        panic!("action enum")
    };
    assert_eq!(
        metadata
            .variants
            .iter()
            .map(|v| (v.tag.as_str(), v.discriminant))
            .collect::<Vec<_>>(),
        [("configure", 0), ("enroll", 1), ("revoke", 2), ("check", 3)]
    );
    assert_eq!(
        metadata.variants[2].ty,
        Some(core::any::TypeId::of::<
            FinalPromotionAccountCustodyRevocationV1,
        >())
    );
    assert_eq!(
        metadata.variants[3].ty,
        Some(core::any::TypeId::of::<FinalPromotionAccountCustodyCheckV1>())
    );
    let Metadata::Struct(check) = schema.get::<FinalPromotionAccountCustodyCheckV1>().unwrap()
    else {
        panic!("current check")
    };
    assert_eq!(
        check
            .declarations
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        [
            "challenge",
            "network_id",
            "minimum_height",
            "minimum_block_hash",
            "expected_account",
            "transaction_payload_digest"
        ]
    );
    assert_eq!(
        check.declarations[4].ty,
        core::any::TypeId::of::<AccountId>()
    );
    for excluded in [
        "SignerFinalPromotionRequestV1",
        "SignerOperationIntentV1",
        "SignerOperationReservationV1",
        "FinalPromotionCheckSubjectV1",
    ] {
        assert!(
            schema.iter().all(|(_, entry)| entry.type_name != excluded),
            "{excluded}"
        );
    }
}

#[test]
fn account_custody_json_rejects_foreign_actions_and_missing_or_extra_control_fields() {
    for json in [
        r#"{"action":"reserve","value":{}}"#,
        r#"{"action":"complete","value":{}}"#,
        r#"{"action":"expire","value":{}}"#,
        r#"{"action":"current","value":{}}"#,
        r#"{"action":"Revoke","value":{"signer":true,"attester":false}}"#,
        r#"{"action":"revoke","value":{"signer":true}}"#,
        r#"{"action":"revoke","value":{"signer":true,"attester":false,"force":true}}"#,
        r#"{"action":"revoke","value":{"signer":true,"attester":false},"signature":[]}"#,
    ] {
        assert!(
            norito::json::from_str::<FinalPromotionAccountCustodyActionV1>(json).is_err(),
            "{json}"
        );
    }
    let json = norito::json::to_json(&record()).unwrap();
    for marker in ["{", "\"execution\":{"] {
        let changed = json.replacen(marker, &format!("{marker}\"verified\":true,"), 1);
        assert_ne!(changed, json);
        assert!(norito::json::from_str::<FinalPromotionAccountCustodyRecordV1>(&changed).is_err());
    }
}

#[test]
fn account_custody_check_json_requires_every_coordinate_and_exact_digest_bytes() {
    let value = norito::json::to_value(&current_check()).unwrap();
    let norito::json::Value::Object(fields) = value else {
        panic!("Check object")
    };
    for field in fields.keys() {
        let mut incomplete = fields.clone();
        incomplete.remove(field);
        let json = norito::json::to_json(&norito::json::Value::Object(incomplete)).unwrap();
        assert!(
            norito::json::from_str::<FinalPromotionAccountCustodyCheckV1>(&json).is_err(),
            "{field}"
        );
    }
    let json = norito::json::to_json(&current_check()).unwrap();
    for field in [
        "request",
        "subject",
        "reservation",
        "verified",
        "provider_id",
    ] {
        let changed = json.replacen('{', &format!("{{\"{field}\":null,"), 1);
        assert!(norito::json::from_str::<FinalPromotionAccountCustodyCheckV1>(&changed).is_err());
    }
    let duplicate = json.replacen('{', &format!("{{\"challenge\":\"{}\",", "06".repeat(32)), 1);
    assert!(norito::json::from_str::<FinalPromotionAccountCustodyCheckV1>(&duplicate).is_err());
    for field in [
        "challenge",
        "network_id",
        "minimum_block_hash",
        "transaction_payload_digest",
    ] {
        for wrong in ["00".repeat(31), "00".repeat(33), "zz".repeat(32)] {
            let mut changed = fields.clone();
            changed.insert(field.into(), norito::json::Value::String(wrong));
            let json = norito::json::to_json(&norito::json::Value::Object(changed)).unwrap();
            assert!(
                norito::json::from_str::<FinalPromotionAccountCustodyCheckV1>(&json).is_err(),
                "{field}"
            );
        }
    }
}

#[test]
fn account_custody_records_cannot_decode_as_receipt_or_stream_token_authority() {
    use crate::sorafs::{
        final_promotion_authority::FinalPromotionCustodyRecordV1,
        stream_token_custody::StreamTokenCustodyControlRecordV1,
    };
    let frame = norito::encode_canonical(&record()).unwrap();
    assert!(matches!(
        norito::decode_canonical::<FinalPromotionCustodyRecordV1>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(matches!(
        norito::decode_canonical::<StreamTokenCustodyControlRecordV1>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
    assert_ne!(
        FINAL_PROMOTION_ACCOUNT_CUSTODY_RECORD_DOMAIN_V1,
        crate::sorafs::final_promotion_authority::FINAL_PROMOTION_CUSTODY_RECORD_DOMAIN_V1
    );
    assert_ne!(
        FINAL_PROMOTION_ACCOUNT_CUSTODY_RECORD_DOMAIN_V1,
        crate::sorafs::stream_token_custody::STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1
    );
    assert_ne!(
        FINAL_PROMOTION_ACCOUNT_TRANSACTION_PAYLOAD_DOMAIN_V1,
        FINAL_PROMOTION_ACCOUNT_CUSTODY_RECORD_DOMAIN_V1
    );
    assert_eq!(
        FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1 + 2,
        FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1
    );
}
