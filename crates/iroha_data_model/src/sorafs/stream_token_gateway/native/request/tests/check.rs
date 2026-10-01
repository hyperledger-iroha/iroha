//! Distinct gateway Check purposes, complete pending commitments and exact canonical framing.

use super::*;
use crate::{
    account::{MultisigMember, MultisigPolicy},
    block::consensus::HeightContextId,
    sorafs::stream_token_gateway::{
        STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1,
        StreamTokenGatewayAdmissionDeliveryStateV1 as Delivery,
        StreamTokenGatewayAdmissionReadbackV1 as Readback,
        StreamTokenGatewayAdmissionResultV1 as AdmissionResult,
        native::{
            STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1,
            StreamTokenGatewayCheckSubjectV1 as Subject, StreamTokenGatewayCheckV1,
            StreamTokenGatewayFinalityFloorV1,
            stream_token_gateway_pending_readback_digest_v1 as pending_digest,
        },
    },
};

fn result(acknowledged: bool) -> AdmissionResult {
    AdmissionResult {
        record: record(),
        delivery_state: if acknowledged {
            Delivery::AcknowledgedExactReplay {
                acknowledged_through_sequence: 1,
            }
        } else {
            Delivery::Pending {
                predecessor_sequence: 0,
            }
        },
    }
}
fn check(subject: Subject) -> StreamTokenGatewayCheckV1 {
    let policy = policy();
    StreamTokenGatewayCheckV1 {
        challenge: [0x81; 32],
        expected_operator: policy.operators.into_iter().next().unwrap(),
        expected_observer: policy.observers.into_iter().next().unwrap(),
        floor: StreamTokenGatewayFinalityFloorV1 {
            height: 1,
            block_hash: [0x82; 32],
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0x83; 32],
            ))),
        },
        subject,
    }
}
fn request(subject: Subject) -> StreamTokenGatewayRequestV1 {
    envelope(StreamTokenGatewayActionV1::Check(check(subject)))
}
fn pending(count: u32) -> Readback {
    let original = record();
    Readback {
        acknowledged_through_sequence: 0,
        high_water_sequence: u64::from(count),
        records: (1..=count)
            .map(|sequence| {
                let mut row = original;
                row.outcome.binding.gateway_sequence = u64::from(sequence);
                row.serving_attempt_id = *Hash::new(sequence.to_le_bytes()).as_ref();
                row.outcome.binding.request_context_digest =
                    *Hash::new(sequence.to_be_bytes()).as_ref();
                row.lease_id = Some(
                    *Hash::new([sequence.to_le_bytes().as_slice(), b"lease"].concat()).as_ref(),
                );
                row
            })
            .collect(),
    }
}

#[test]
fn gateway_check_shape_binds_nonzero_floor_challenge_accounts_and_policy() {
    let valid = request(Subject::Admission {
        request_digest: [0x91; 32],
        result: result(false),
    });
    valid.validate().unwrap();
    let changes: [fn(&mut StreamTokenGatewayCheckV1); 5] = [
        |check| check.challenge = [0; 32],
        |check| check.floor.height = 0,
        |check| check.floor.block_hash = [0; 32],
        |check| {
            check.floor.context_id =
                HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])))
        },
        |check| check.expected_observer = check.expected_operator.clone(),
    ];
    for change in changes {
        let mut wrong = valid.clone();
        let StreamTokenGatewayActionV1::Check(check) = &mut wrong.action else {
            unreachable!()
        };
        change(check);
        assert!(wrong.validate().is_err());
    }
    for (revision, digest) in [(0, [0; 32]), (1, [0x92; 32])] {
        let mut wrong = valid.clone();
        wrong.expected_policy_revision = revision;
        wrong.expected_policy_digest = digest;
        assert!(wrong.validate().is_err());
    }
    let mut older = valid.clone();
    older.expected_policy_revision = 2;
    older.expected_policy_digest = [0x93; 32];
    older
        .validate()
        .expect("historical policy remains structurally bound after rotation");
    let mut foreign = valid;
    foreign.gateway_id = [0x94; 32];
    assert!(foreign.validate().is_err());
    assert!(
        request(Subject::Admission {
            request_digest: [0; 32],
            result: result(false)
        })
        .validate()
        .is_err()
    );
}

#[test]
fn gateway_finality_floor_rejects_the_marked_empty_context_after_roundtrip() {
    let mut floor = check(Subject::Qualification).floor;
    let empty = Hash::prehashed([0; 32]);
    assert_ne!(
        *empty.as_ref(),
        [0; 32],
        "prehashed preserves the hash marker"
    );
    floor.context_id = HeightContextId(HashOf::from_untyped_unchecked(empty));
    assert_eq!(floor.validate(), Err(Error::BindingMismatch));
    let frame = norito::encode_canonical(&floor).unwrap();
    let decoded = norito::decode_canonical::<StreamTokenGatewayFinalityFloorV1>(&frame).unwrap();
    assert_eq!(decoded, floor);
    assert_eq!(decoded.validate(), Err(Error::BindingMismatch));
    let json = norito::json::to_json(&floor).unwrap();
    let decoded = norito::json::from_str::<StreamTokenGatewayFinalityFloorV1>(&json).unwrap();
    assert_eq!(decoded, floor);
    assert_eq!(decoded.validate(), Err(Error::BindingMismatch));
    let mut cleared = empty;
    iroha_crypto::zeroize_value_for_confidential_discard(&mut cleared);
    assert_eq!(*cleared.as_ref(), [0; 32]);
    floor.context_id = HeightContextId(HashOf::from_untyped_unchecked(cleared));
    assert_eq!(floor.validate(), Err(Error::BindingMismatch));
}

#[test]
fn gateway_check_rejects_distinct_accounts_with_shared_controller_keys() {
    let mut candidate = check(Subject::Qualification);
    let original_key = candidate
        .expected_operator
        .expect_single_signatory()
        .clone();
    candidate.expected_observer = AccountId::new_multisig(
        MultisigPolicy::new(1, vec![MultisigMember::new(original_key, 1).unwrap()]).unwrap(),
    );
    assert_ne!(candidate.expected_operator, candidate.expected_observer);
    assert!(
        envelope(StreamTokenGatewayActionV1::Check(candidate))
            .validate()
            .is_err()
    );
    request(Subject::Qualification)
        .validate()
        .expect("independent controller keys");
}

#[test]
fn gateway_serving_requires_accepted_acknowledged_original_while_admission_is_historical() {
    let accepted = result(true);
    request(Subject::Serving {
        request_digest: [0x91; 32],
        result: accepted,
    })
    .validate()
    .unwrap();
    assert!(
        request(Subject::Serving {
            request_digest: [0x91; 32],
            result: result(false)
        })
        .validate()
        .is_err()
    );
    let mut diagnostic = accepted;
    diagnostic.record.outcome.status =
        StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::MissingToken);
    diagnostic.record.outcome.token_body_digest = None;
    diagnostic.record.outcome.token_key_version = None;
    diagnostic.record.lease_id = None;
    diagnostic.record.lease_expires_at_unix_ms = None;
    diagnostic.record.lease_token_expires_at_epoch = None;
    request(Subject::Admission {
        request_digest: [0x91; 32],
        result: diagnostic,
    })
    .validate()
    .unwrap();
    request(Subject::Acknowledged {
        record: diagnostic.record,
    })
    .validate()
    .unwrap();
    assert!(
        request(Subject::Serving {
            request_digest: [0x91; 32],
            result: diagnostic
        })
        .validate()
        .is_err()
    );
    assert!(
        request(Subject::Released {
            record: diagnostic.record
        })
        .validate()
        .is_err()
    );
    let mut substituted = accepted;
    substituted.record.serving_attempt_id = [0; 32];
    assert!(
        request(Subject::Serving {
            request_digest: [0x91; 32],
            result: substituted
        })
        .validate()
        .is_err()
    );
    for delivery_state in [
        Delivery::Pending {
            predecessor_sequence: 1,
        },
        Delivery::AcknowledgedExactReplay {
            acknowledged_through_sequence: 0,
        },
    ] {
        let mut invalid = accepted;
        invalid.delivery_state = delivery_state;
        assert!(
            request(Subject::Admission {
                request_digest: [0x91; 32],
                result: invalid
            })
            .validate()
            .is_err()
        );
    }
}

#[test]
fn gateway_pending_commitment_binds_complete_prefix_policy_limit_and_empty_state() {
    let qualification = policy().qualification;
    let prefix = pending(2);
    let original = pending_digest(qualification, 2, &prefix).unwrap();
    assert_ne!(original, [0; 32]);
    assert_ne!(pending_digest(qualification, 3, &prefix).unwrap(), original);
    let mut rotated = qualification;
    rotated.revision += 1;
    rotated.policy_digest = [0xa1; 32];
    assert_ne!(pending_digest(rotated, 2, &prefix).unwrap(), original);
    let mut substituted = prefix.clone();
    substituted.records[1].serving_attempt_id = [0xa2; 32];
    assert_ne!(
        pending_digest(qualification, 2, &substituted).unwrap(),
        original
    );
    let mut omitted = prefix.clone();
    omitted.records.pop();
    assert!(pending_digest(qualification, 2, &omitted).is_err());
    let mut reordered = prefix.clone();
    reordered.records.reverse();
    assert!(pending_digest(qualification, 2, &reordered).is_err());
    let mut gap = prefix;
    gap.records[1].outcome.binding.gateway_sequence = 3;
    assert!(pending_digest(qualification, 2, &gap).is_err());
    let empty = pending(0);
    let empty_digest = pending_digest(qualification, 2, &empty).unwrap();
    assert_ne!(empty_digest, [0; 32]);
    assert_ne!(empty_digest, original);
    let mut acknowledged_empty = empty.clone();
    acknowledged_empty.acknowledged_through_sequence = 2;
    acknowledged_empty.high_water_sequence = 2;
    assert_ne!(
        pending_digest(qualification, 2, &acknowledged_empty).unwrap(),
        empty_digest
    );
    let mut hidden = empty;
    hidden.high_water_sequence = 1;
    assert!(pending_digest(qualification, 2, &hidden).is_err());
}

#[test]
fn gateway_pending_full_limit_fits_aggregate_bound_and_rejects_extra_work() {
    let qualification = policy().qualification;
    let max = STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1;
    let full = pending(max);
    let frame_len = norito::canonical_frame_len(&(qualification, max, full.clone())).unwrap();
    assert!(frame_len <= STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1);
    pending_digest(qualification, max, &full).expect("complete canonical maximum prefix");
    for limit in [0, max + 1, u32::MAX] {
        assert!(pending_digest(qualification, limit, &full).is_err());
        assert!(
            request(Subject::Pending {
                max_items: limit,
                readback_digest: [1; 32]
            })
            .validate()
            .is_err()
        );
    }
    let too_many = pending(max + 1);
    assert_eq!(
        pending_digest(qualification, max, &too_many),
        Err(Error::InvalidRequest)
    );
    assert!(
        request(Subject::Pending {
            max_items: 1,
            readback_digest: [0; 32]
        })
        .validate()
        .is_err()
    );
    let check = request(Subject::Pending {
        max_items: max,
        readback_digest: pending_digest(qualification, max, &full).unwrap(),
    });
    assert!(
        norito::canonical_frame_len(&check).unwrap() < STREAM_TOKEN_GATEWAY_MAX_REQUEST_BYTES_V1
    );
}

#[test]
fn gateway_check_subjects_have_distinct_strict_canonical_frames_and_schemas() {
    let subjects = [
        Subject::Qualification,
        Subject::Admission {
            request_digest: [0x91; 32],
            result: result(false),
        },
        Subject::Serving {
            request_digest: [0x91; 32],
            result: result(true),
        },
        Subject::Pending {
            max_items: 1,
            readback_digest: pending_digest(policy().qualification, 1, &pending(1)).unwrap(),
        },
        Subject::Acknowledged { record: record() },
        Subject::Released { record: record() },
    ];
    let mut distinct = BTreeSet::new();
    for subject in subjects {
        let value = check(subject);
        let envelope = envelope(StreamTokenGatewayActionV1::Check(value.clone()));
        envelope.validate().unwrap();
        let frame = norito::encode_canonical(&envelope).unwrap();
        assert!(distinct.insert(frame.clone()));
        assert_eq!(
            norito::decode_canonical::<StreamTokenGatewayRequestV1>(&frame).unwrap(),
            envelope
        );
        let mut foreign = frame.clone();
        foreign[6] ^= 1;
        assert!(norito::decode_canonical::<StreamTokenGatewayRequestV1>(&foreign).is_err());
        let mut trailing = frame;
        trailing.push(0);
        assert!(norito::decode_canonical::<StreamTokenGatewayRequestV1>(&trailing).is_err());
        let check_frame = norito::encode_canonical(&value).unwrap();
        assert_eq!(
            &check_frame[6..22],
            norito::schema::identity::frame_hash::<StreamTokenGatewayCheckV1>().as_slice()
        );
        assert_eq!(
            norito::decode_canonical::<StreamTokenGatewayCheckV1>(&check_frame).unwrap(),
            value
        );
        let json = norito::json::to_json(&value).unwrap();
        assert_eq!(
            norito::json::from_str::<StreamTokenGatewayCheckV1>(&json).unwrap(),
            value
        );
        assert!(
            norito::json::from_str::<StreamTokenGatewayCheckV1>(&json.replacen(
                '{',
                "{\"unexpected\":true,",
                1
            ))
            .is_err()
        );
        let json = norito::json::to_json(&subject).unwrap();
        assert_eq!(norito::json::from_str::<Subject>(&json).unwrap(), subject);
        assert!(
            norito::json::from_str::<Subject>(&json.replacen('{', "{\"unexpected\":true,", 1))
                .is_err()
        );
    }
    let floor = check(Subject::Qualification).floor;
    let json = norito::json::to_json(&floor).unwrap();
    assert_eq!(
        norito::json::from_str::<StreamTokenGatewayFinalityFloorV1>(&json).unwrap(),
        floor
    );
    assert!(
        norito::json::from_str::<StreamTokenGatewayFinalityFloorV1>(&json.replacen(
            '{',
            "{\"unexpected\":true,",
            1
        ))
        .is_err()
    );
    let schema = StreamTokenGatewayRequestV1::schema();
    assert!(schema.contains_key::<StreamTokenGatewayCheckV1>());
    assert!(schema.contains_key::<Subject>());
    assert!(schema.contains_key::<StreamTokenGatewayFinalityFloorV1>());
    assert!(schema.contains_key::<AdmissionResult>());
}
