//! Canonical wire, JSON, schema and signature-free native authority records.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_schema::Metadata;
use sorafs_manifest::signer::protocol::{SignerOperationActionV1, SignerOperationAuditHeadV1};

mod check;

fn intent() -> SignerOperationIntentV1 {
    SignerOperationIntentV1 {
        action: SignerOperationActionV1::Sign,
        operation_id: [1; 32],
        request_digest: [2; 32],
        previous_audit: SignerOperationAuditHeadV1 {
            sequence: 0,
            digest: [0; 32],
        },
    }
}
fn custody() -> SignerOperationCustodyV1 {
    SignerOperationCustodyV1 {
        record_digest: [3; 32],
        control_state_digest: [4; 32],
    }
}
fn reservation() -> SignerOperationReservationV1 {
    SignerOperationReservationV1 {
        reservation_id: [5; 32],
        fence: 1,
        expires_at_unix_ms: 60_001,
    }
}
fn commitment() -> SignerOperationCommitmentV1 {
    SignerOperationCommitmentV1 {
        audit: SignerOperationAuditHeadV1 {
            sequence: 1,
            digest: [6; 32],
        },
        response_digest: [7; 32],
    }
}
fn execution() -> FinalPromotionExecutionV1 {
    let key = KeyPair::try_from_seed(vec![9; 32], Algorithm::Ed25519).expect("authority fixture");
    FinalPromotionExecutionV1 {
        height: 2,
        ordinal: 0,
        recorded_at_unix_ms: 10,
        authority: AccountId::new(key.public_key().clone()),
    }
}

#[test]
fn final_promotion_authority_actions_have_one_typed_binary_and_json_surface() {
    let actions = [
        (
            "configure",
            FinalPromotionAuthorityActionV1::Configure(vec![1, 2, 3]),
        ),
        (
            "enroll",
            FinalPromotionAuthorityActionV1::Enroll(vec![4, 5, 6]),
        ),
        (
            "revoke",
            FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
                signer: true,
                attester: false,
            }),
        ),
        (
            "reserve",
            FinalPromotionAuthorityActionV1::Reserve(FinalPromotionReserveV1 {
                intent: intent(),
                custody: custody(),
            }),
        ),
        (
            "complete",
            FinalPromotionAuthorityActionV1::Complete(FinalPromotionCompleteV1 {
                intent: intent(),
                custody: custody(),
                reservation: reservation(),
                commitment: commitment(),
                signatures_digest: [8; 32],
            }),
        ),
        (
            "expire",
            FinalPromotionAuthorityActionV1::Expire(FinalPromotionExpireV1 {
                operation_id: [1; 32],
                reservation: reservation(),
            }),
        ),
        (
            "check",
            FinalPromotionAuthorityActionV1::Check(check::current_check()),
        ),
    ];
    for (tag, action) in actions {
        let frame = norito::encode_canonical(&action).expect("native action frame");
        assert_eq!(
            norito::decode_canonical::<FinalPromotionAuthorityActionV1>(&frame)
                .expect("native action decode"),
            action
        );
        assert!(
            norito::decode_canonical::<FinalPromotionAuthorityActionV1>(&frame[..frame.len() - 1])
                .is_err()
        );
        let json = norito::json::to_json(&action).expect("native action JSON");
        assert_eq!(
            norito::json::from_str::<FinalPromotionAuthorityActionV1>(&json)
                .expect("native action JSON decode"),
            action
        );
        assert_eq!(
            norito::json::to_value(&action)
                .expect("native JSON value")
                .get("action")
                .and_then(|v| v.as_str()),
            Some(tag)
        );
    }
}

#[test]
fn final_promotion_authority_schema_reuses_manifest_operation_types() {
    let schema = FinalPromotionAuthorityActionV1::schema();
    let Metadata::Enum(metadata) = schema
        .get::<FinalPromotionAuthorityActionV1>()
        .expect("action schema")
    else {
        panic!("enum schema required")
    };
    assert_eq!(
        metadata
            .variants
            .iter()
            .map(|v| (v.tag.as_str(), v.discriminant))
            .collect::<Vec<_>>(),
        [
            ("configure", 0),
            ("enroll", 1),
            ("revoke", 2),
            ("reserve", 3),
            ("complete", 4),
            ("expire", 5),
            ("check", 6)
        ]
    );
    assert!(matches!(
        schema.get::<SignerOperationIntentV1>(),
        Some(Metadata::Struct(_))
    ));
    assert!(matches!(
        schema.get::<SignerOperationReservationV1>(),
        Some(Metadata::Struct(_))
    ));
    assert!(matches!(
        schema.get::<SignerOperationCustodyV1>(),
        Some(Metadata::Struct(_))
    ));
    assert!(matches!(
        schema.get::<SignerOperationCommitmentV1>(),
        Some(Metadata::Struct(_))
    ));
}

#[test]
fn final_promotion_custody_records_roundtrip_optional_enrollment() {
    for enrollment in [None, Some(vec![5, 6, 7])] {
        let record = FinalPromotionCustodyRecordV1 {
            deployment_id: "sora-main".to_owned(),
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [9; 32],
            execution: execution(),
            control_state: vec![1, 2, 3],
            enrollment,
        };
        let frame = norito::encode_canonical(&record).expect("custody record frame");
        assert_eq!(
            norito::decode_canonical::<FinalPromotionCustodyRecordV1>(&frame)
                .expect("custody record decode"),
            record
        );
        let json = norito::json::to_json(&record).expect("custody record JSON");
        assert_eq!(
            norito::json::from_str::<FinalPromotionCustodyRecordV1>(&json)
                .expect("custody record JSON decode"),
            record
        );
    }
}

#[test]
fn final_promotion_operation_records_retain_original_reservation_for_every_outcome() {
    for outcome in [
        FinalPromotionOperationOutcomeV1::Reserved,
        FinalPromotionOperationOutcomeV1::Completed(FinalPromotionCompletedV1 {
            commitment: commitment(),
            signatures_digest: [8; 32],
        }),
        FinalPromotionOperationOutcomeV1::Expired,
        FinalPromotionOperationOutcomeV1::Invalidated,
    ] {
        let record = FinalPromotionOperationRecordV1 {
            deployment_id: "sora-main".to_owned(),
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [9; 32],
            execution: execution(),
            intent: intent(),
            custody: custody(),
            reservation: reservation(),
            reserved: execution(),
            outcome,
        };
        let frame = norito::encode_canonical(&record).expect("operation record frame");
        assert_eq!(
            norito::decode_canonical::<FinalPromotionOperationRecordV1>(&frame)
                .expect("operation record decode"),
            record
        );
        let json = norito::json::to_json(&record).expect("operation record JSON");
        assert_eq!(
            norito::json::from_str::<FinalPromotionOperationRecordV1>(&json)
                .expect("operation record JSON decode"),
            record
        );
        assert!(
            !json.contains("block_hash"),
            "execution record cannot include its own block hash"
        );
        assert!(
            !json.contains("\"signature\""),
            "unreleased signatures cannot enter a transaction"
        );
    }
}

#[test]
fn final_promotion_authority_rejects_missing_or_obsolete_json_actions() {
    for json in [
        r#"{"action":"Reserve","value":{}}"#,
        r#"{"action":"complete","value":{}}"#,
        r#"{"action":"revoke","value":{"signer":true}}"#,
        r#"{"action":"migrate","value":null}"#,
        r#"{"action":"revoke","value":{"signer":true,"attester":false,"force":true}}"#,
        r#"{"action":"revoke","value":{"signer":true,"attester":false},"compatibility":true}"#,
    ] {
        assert!(norito::json::from_str::<FinalPromotionAuthorityActionV1>(json).is_err());
    }
}

#[test]
fn final_promotion_authority_nested_operation_json_rejects_unknown_fields() {
    let json = norito::json::to_json(&intent()).expect("intent JSON");
    let extra = json.replacen('{', "{\"compatibility\":true,", 1);
    assert!(norito::json::from_str::<SignerOperationIntentV1>(&extra).is_err());
    let json = norito::json::to_json(&SignerOperationActionV1::Sign).expect("action JSON");
    let extra = json.replacen('{', "{\"compatibility\":true,", 1);
    assert!(norito::json::from_str::<SignerOperationActionV1>(&extra).is_err());
}

#[test]
fn final_promotion_authority_shared_action_tags_roundtrip_all_declared_variants() {
    for action in [
        SignerOperationActionV1::Sign,
        SignerOperationActionV1::Qualify,
        SignerOperationActionV1::Status,
        SignerOperationActionV1::ActivateCustody,
        SignerOperationActionV1::RevokeCustody,
    ] {
        let json = norito::json::to_json(&action).expect("shared action JSON");
        assert_eq!(
            norito::json::from_str::<SignerOperationActionV1>(&json).expect("shared action decode"),
            action
        );
    }
    // Declaring generic actions in the shared protocol does not admit them as native role-14 work.
    // The native executor tests require a Sign intent for every final-promotion reservation.
}
