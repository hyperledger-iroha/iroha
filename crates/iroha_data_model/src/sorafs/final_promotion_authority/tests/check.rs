//! Closed challenged phase encoding retains existing public request and native record owners.
use super::*;
use crate::isi::sorafs::MutateSorafsFinalPromotionAuthority;
use std::any::TypeId;

pub(super) fn current_check() -> FinalPromotionCheckV1 {
    FinalPromotionCheckV1 {
        challenge: [10; 32],
        network_id: [11; 32],
        expected_operator: execution().authority,
        minimum_height: 1,
        minimum_block_hash: [12; 32],
        request: SignerFinalPromotionRequestV1 {
            operation_id: intent().operation_id,
            binding_digest: [13; 32],
            original_custody: custody(),
            statement_digest: [14; 32],
            statement_size: 3_380,
        },
        subject: FinalPromotionCheckSubjectV1::Current(intent().previous_audit),
    }
}

fn operation(completed: bool) -> FinalPromotionOperationRecordV1 {
    let mut intent = intent();
    intent.request_digest = current_check().request.digest().unwrap();
    let mut executed = execution();
    if completed {
        executed.height += 1;
        executed.recorded_at_unix_ms += 1;
    }
    FinalPromotionOperationRecordV1 {
        deployment_id: "sora-main".into(),
        revision: if completed { 2 } else { 1 },
        predecessor_digest: if completed { [15; 32] } else { [0; 32] },
        request_digest: [16; 32],
        execution: executed,
        intent,
        custody: custody(),
        reservation: reservation(),
        reserved: execution(),
        outcome: if completed {
            FinalPromotionOperationOutcomeV1::Completed(FinalPromotionCompletedV1 {
                commitment: commitment(),
                signatures_digest: [17; 32],
            })
        } else {
            FinalPromotionOperationOutcomeV1::Reserved
        },
    }
}

#[test]
fn final_promotion_checks_roundtrip_every_exact_phase_with_bounded_instruction_frames() {
    let reserved = operation(false);
    let completed = operation(true);
    for (tag, subject) in [
        (
            "current",
            FinalPromotionCheckSubjectV1::Current(intent().previous_audit),
        ),
        (
            "before_provider",
            FinalPromotionCheckSubjectV1::BeforeProvider(reserved.clone()),
        ),
        (
            "after_provider",
            FinalPromotionCheckSubjectV1::AfterProvider(reserved.clone()),
        ),
        (
            "before_commit",
            FinalPromotionCheckSubjectV1::BeforeCommit(reserved),
        ),
        (
            "after_commit",
            FinalPromotionCheckSubjectV1::AfterCommit(completed.clone()),
        ),
        (
            "before_release",
            FinalPromotionCheckSubjectV1::BeforeRelease(completed),
        ),
    ] {
        let check = FinalPromotionCheckV1 {
            subject,
            ..current_check()
        };
        let instruction = MutateSorafsFinalPromotionAuthority {
            deployment_id: "sora-main".into(),
            expected_control_revision: 2,
            expected_control_digest: check.request.original_custody.control_state_digest,
            action: FinalPromotionAuthorityActionV1::Check(check.clone()),
        };
        let instruction_frame = norito::encode_canonical(&instruction).unwrap();
        assert!(instruction_frame.len() <= FINAL_PROMOTION_MAX_RECORD_BYTES_V1);
        assert_eq!(
            norito::decode_canonical::<MutateSorafsFinalPromotionAuthority>(&instruction_frame)
                .unwrap(),
            instruction
        );
        let frame = norito::encode_canonical(&check).unwrap();
        assert_eq!(
            norito::decode_canonical::<FinalPromotionCheckV1>(&frame).unwrap(),
            check
        );
        assert!(
            norito::decode_canonical::<FinalPromotionCheckV1>(&frame[..frame.len() - 1]).is_err()
        );
        assert!(norito::decode_canonical::<FinalPromotionCheckV1>(&instruction_frame).is_err());
        let json = norito::json::to_json(&check).unwrap();
        assert_eq!(
            norito::json::from_str::<FinalPromotionCheckV1>(&json).unwrap(),
            check
        );
        assert_eq!(
            norito::json::to_value(&check.subject)
                .unwrap()
                .get("phase")
                .and_then(|v| v.as_str()),
            Some(tag)
        );
    }
}

#[test]
fn final_promotion_check_subject_schema_reuses_exact_native_records() {
    let schema = FinalPromotionCheckV1::schema();
    let Metadata::Enum(subject) = schema.get::<FinalPromotionCheckSubjectV1>().unwrap() else {
        panic!("closed Check subject enum required");
    };
    assert_eq!(
        subject
            .variants
            .iter()
            .map(|v| (v.tag.as_str(), v.discriminant))
            .collect::<Vec<_>>(),
        [
            ("current", 0),
            ("before_provider", 1),
            ("after_provider", 2),
            ("before_commit", 3),
            ("after_commit", 4),
            ("before_release", 5),
        ]
    );
    for (index, variant) in subject.variants.iter().enumerate() {
        assert_eq!(
            variant.ty,
            Some(if index == 0 {
                TypeId::of::<SignerOperationAuditHeadV1>()
            } else {
                TypeId::of::<FinalPromotionOperationRecordV1>()
            })
        );
    }
    let Metadata::Struct(check) = schema.get::<FinalPromotionCheckV1>().unwrap() else {
        panic!("closed Check record required");
    };
    assert_eq!(
        check
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "challenge",
            "network_id",
            "expected_operator",
            "minimum_height",
            "minimum_block_hash",
            "request",
            "subject"
        ]
    );
    assert_eq!(
        check
            .declarations
            .iter()
            .find(|field| field.name == "expected_operator")
            .unwrap()
            .ty,
        TypeId::of::<AccountId>()
    );
    assert_eq!(
        check
            .declarations
            .iter()
            .find(|field| field.name == "request")
            .unwrap()
            .ty,
        TypeId::of::<SignerFinalPromotionRequestV1>()
    );
}

#[test]
fn final_promotion_check_rejects_unknown_nested_fields_and_obsolete_phases() {
    let check = FinalPromotionCheckV1 {
        subject: FinalPromotionCheckSubjectV1::BeforeProvider(operation(false)),
        ..current_check()
    };
    let json = norito::json::to_json(&check).unwrap();
    for marker in [
        "{",
        "\"request\":{",
        "\"original_custody\":{",
        "\"subject\":{",
        "\"value\":{",
        "\"reserved\":{",
        "\"intent\":{",
    ] {
        let changed = json.replacen(marker, &format!("{marker}\"compatibility\":true,"), 1);
        assert_ne!(changed, json, "fixture must contain {marker}");
        assert!(
            norito::json::from_str::<FinalPromotionCheckV1>(&changed).is_err(),
            "{marker}"
        );
    }
    for phase in ["reserved", "BeforeProvider", "check_v2", ""] {
        let changed = json.replacen("before_provider", phase, 1);
        assert_ne!(changed, json);
        assert!(norito::json::from_str::<FinalPromotionCheckV1>(&changed).is_err());
    }
    let duplicated = json.replacen('{', &format!("{{\"challenge\":\"{}\",", "01".repeat(32)), 1);
    assert_ne!(duplicated, json);
    assert!(norito::json::from_str::<FinalPromotionCheckV1>(&duplicated).is_err());
}

#[test]
fn receipt_check_requires_exact_operator_in_closed_json_and_canonical_norito() {
    let check = current_check();
    let json = norito::json::to_json(&check).unwrap();
    let operator_json = norito::json::to_json(&check.expected_operator).unwrap();
    let field = format!("\"expected_operator\":{operator_json},");
    assert_eq!(json.matches(&field).count(), 1);
    let missing = json.replace(&field, "");
    assert!(norito::json::from_str::<FinalPromotionCheckV1>(&missing).is_err());
    for alias in ["operator", "expectedOperator", "authority"] {
        let renamed = json.replace("\"expected_operator\"", &format!("\"{alias}\""));
        assert_ne!(renamed, json);
        assert!(norito::json::from_str::<FinalPromotionCheckV1>(&renamed).is_err());
    }
    let duplicate = json.replacen('{', &format!("{{{field}"), 1);
    assert!(norito::json::from_str::<FinalPromotionCheckV1>(&duplicate).is_err());
    for invalid in ["null", "true", "1", "{}", "\"operator@deployment\""] {
        let substituted = json.replace(&field, &format!("\"expected_operator\":{invalid},"));
        assert_ne!(substituted, json);
        assert!(norito::json::from_str::<FinalPromotionCheckV1>(&substituted).is_err());
    }
    let other_key = KeyPair::try_from_seed(vec![42; 32], Algorithm::Ed25519).unwrap();
    let mut substituted = check.clone();
    substituted.expected_operator = AccountId::new(other_key.public_key().clone());
    assert_ne!(substituted, check);
    let original_frame = norito::encode_canonical(&check).unwrap();
    let substituted_frame = norito::encode_canonical(&substituted).unwrap();
    assert_ne!(original_frame, substituted_frame);
    assert_eq!(
        norito::decode_canonical::<FinalPromotionCheckV1>(&original_frame).unwrap(),
        check
    );
    assert_eq!(
        norito::decode_canonical::<FinalPromotionCheckV1>(&substituted_frame).unwrap(),
        substituted
    );
}
