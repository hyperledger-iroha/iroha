//! Adversarial canonical DTO checks; no decoded claim is admitted as native authority.

use super::*;
use crate::sorafs::stream_token_authority::StreamTokenCompleteV1;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use sorafs_manifest::signer::{
    protocol::{SignerOperationActionV1, SignerOperationCustodyV1, SignerOperationIntentV1},
    stream_token::SignerStreamTokenRequestV1,
};

#[test]
fn stream_token_action_keeps_inline_check_and_complete_schema_payloads() {
    const _: () = assert!(core::mem::size_of::<StreamTokenAuthorityActionV1>() <= 2048);
    let schema = StreamTokenAuthorityActionV1::schema();
    let iroha_schema::Metadata::Enum(actions) = schema
        .get::<StreamTokenAuthorityActionV1>()
        .expect("stream-token action schema")
    else {
        panic!("stream-token action must have an enum schema");
    };
    assert_eq!(actions.variants.len(), 4);
    assert_eq!(actions.variants[1].discriminant, 1);
    assert_eq!(
        actions.variants[1].ty,
        Some(core::any::TypeId::of::<StreamTokenCompleteRequestV1>())
    );
    assert_eq!(actions.variants[3].discriminant, 3);
    assert_eq!(
        actions.variants[3].ty,
        Some(core::any::TypeId::of::<StreamTokenCheckV1>())
    );
}

fn operator(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
}

fn reviewed() -> StreamTokenReviewedV1 {
    let request = SignerStreamTokenRequestV1 {
        operation_id: [1; 32],
        binding_digest: [2; 32],
        original_custody: SignerOperationCustodyV1 {
            record_digest: [3; 32],
            control_state_digest: [4; 32],
        },
        signing_payload_digest: [5; 32],
        signing_payload_size: 512,
        issued_at_unix_ms: 1_000_000,
        expires_at_unix_ms: 1_200_000,
    };
    StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 7,
                digest: [6; 32],
            },
        },
    }
}

fn original() -> StreamTokenOperationV1 {
    StreamTokenOperationV1 {
        reviewed: reviewed(),
        reservation: SignerOperationReservationV1 {
            reservation_id: [7; 32],
            fence: 11,
            expires_at_unix_ms: 1_100_000,
        },
        outcome: StreamTokenOutcomeV1::Reserved,
    }
}

fn completion() -> StreamTokenCompleteV1 {
    StreamTokenCompleteV1 {
        reviewed: reviewed(),
        reservation: original().reservation,
        commitment: SignerOperationCommitmentV1 {
            audit: SignerOperationAuditHeadV1 {
                sequence: 8,
                digest: [8; 32],
            },
            response_digest: [9; 32],
        },
        signatures_digest: [10; 32],
        completed_at_unix_ms: 1_050_000,
    }
}

fn reserved() -> StreamTokenNativeOperationV1 {
    StreamTokenNativeOperationV1 {
        provider_id: ProviderId::new([0x21; 32]),
        custody_control_revision: 3,
        custody_control_digest: [0x22; 32],
        operation: original(),
        reserved_execution: StreamTokenExecutionV1 {
            height: 10,
            transaction_hash: [0x2d; 32],
            entry_index: 1,
            instruction_index: 0,
            recorded_at_unix_ms: 1_010_000,
            authority: operator(0x23),
        },
        terminal_execution: None,
    }
}

fn completed() -> StreamTokenNativeOperationV1 {
    let mut row = reserved();
    row.operation.outcome = StreamTokenOutcomeV1::Completed(completion());
    row.terminal_execution = Some(StreamTokenExecutionV1 {
        height: 11,
        transaction_hash: [0x2e; 32],
        entry_index: 0,
        instruction_index: 0,
        recorded_at_unix_ms: 1_050_000,
        authority: operator(0x23),
    });
    row
}

fn check(phase: StreamTokenCheckPhaseV1) -> StreamTokenAuthorityRequestV1 {
    StreamTokenAuthorityRequestV1 {
        network_id: [0x24; 32],
        provider_id: ProviderId::new([0x21; 32]),
        expected_control_revision: 3,
        expected_control_digest: [0x22; 32],
        action: StreamTokenAuthorityActionV1::Check(StreamTokenCheckV1 {
            challenge: [0x25; 32],
            expected_operator: operator(0x23),
            expected_observer: operator(0x2c),
            floor: StreamTokenFinalityFloorV1 {
                height: 11,
                block_hash: [0x26; 32],
                context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"role11 fixture context",
                ))),
            },
            reviewed: reviewed(),
            phase,
        }),
    }
}

fn validate_check(request: &StreamTokenAuthorityRequestV1) -> Result<(), StreamTokenClaimErrorV1> {
    let fallback = StreamTokenCheckPhaseV1::BeforeProvider(reserved());
    let expected_phase = match &request.action {
        StreamTokenAuthorityActionV1::Check(inner) => &inner.phase,
        _ => &fallback,
    };
    validate_stream_token_check_claim_v1(
        request,
        [0x24; 32],
        ProviderId::new([0x21; 32]),
        3,
        [0x22; 32],
        &operator(0x23),
        &operator(0x2c),
        [0x25; 32],
        StreamTokenFinalityFloorV1 {
            height: 11,
            block_hash: [0x26; 32],
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"role11 fixture context",
            ))),
        },
        &reviewed(),
        expected_phase,
    )
}

#[test]
fn native_request_actions_have_one_strict_bounded_canonical_frame() {
    let completed_row = completed();
    let StreamTokenOutcomeV1::Completed(completion) = completed_row.operation.outcome else {
        panic!("completed fixture");
    };
    let complete = StreamTokenCompleteRequestV1 {
        reviewed: completion.reviewed,
        reservation: completion.reservation,
        commitment: completion.commitment,
        signatures_digest: completion.signatures_digest,
    };
    let expire = StreamTokenExpireV1 {
        operation_id: reviewed().request.operation_id,
        reservation: reserved().operation.reservation,
    };
    for action in [
        StreamTokenAuthorityActionV1::Reserve(reviewed()),
        StreamTokenAuthorityActionV1::Complete(complete),
        StreamTokenAuthorityActionV1::Expire(expire),
        check(StreamTokenCheckPhaseV1::Current(
            reviewed().intent.previous_audit,
        ))
        .action,
    ] {
        let request = StreamTokenAuthorityRequestV1 {
            network_id: [0x24; 32],
            provider_id: ProviderId::new([0x21; 32]),
            expected_control_revision: 3,
            expected_control_digest: [0x22; 32],
            action,
        };
        let frame = norito::encode_canonical(&request).unwrap();
        assert!(frame.len() <= STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1);
        assert_eq!(
            decode_stream_token_authority_request_claim_v1(&frame),
            Ok(request.clone())
        );
        let json = norito::json::to_json(&request).unwrap();
        assert_eq!(
            norito::json::from_str::<StreamTokenAuthorityRequestV1>(&json).unwrap(),
            request
        );
        let extra = json.replacen("\"network_id\":", "\"extra\":1,\"network_id\":", 1);
        assert_ne!(extra, json);
        assert!(norito::json::from_str::<StreamTokenAuthorityRequestV1>(&extra).is_err());
        assert_eq!(
            decode_stream_token_authority_request_claim_v1(&frame[..frame.len() - 1]),
            Err(StreamTokenClaimErrorV1::Encoding)
        );
        let mut trailing = frame;
        trailing.push(0);
        assert_eq!(
            decode_stream_token_authority_request_claim_v1(&trailing),
            Err(StreamTokenClaimErrorV1::Encoding)
        );
    }
    assert_eq!(
        decode_stream_token_authority_request_claim_v1(&[]),
        Err(StreamTokenClaimErrorV1::Encoding)
    );
    assert_eq!(
        decode_stream_token_authority_request_claim_v1(&vec![
            0;
            STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1
                + 1
        ]),
        Err(StreamTokenClaimErrorV1::Encoding)
    );
}

#[test]
fn complete_request_keeps_original_slot_and_omits_caller_completion_time() {
    let original = original();
    let completion = completion();
    let good = StreamTokenCompleteRequestV1 {
        reviewed: completion.reviewed,
        reservation: completion.reservation,
        commitment: completion.commitment,
        signatures_digest: completion.signatures_digest,
    };
    assert_eq!(
        validate_stream_token_complete_request_claim_v1(&good, &original),
        Ok(())
    );
    let json = norito::json::to_json(&good).unwrap();
    assert!(!json.contains("completed_at_unix_ms"));
    let mut changed = good;
    changed.reservation.fence += 1;
    assert_eq!(
        validate_stream_token_complete_request_claim_v1(&changed, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    changed = good;
    changed.commitment.audit.sequence += 1;
    assert_eq!(
        validate_stream_token_complete_request_claim_v1(&changed, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    changed = good;
    changed.signatures_digest = [0; 32];
    assert_eq!(
        validate_stream_token_complete_request_claim_v1(&changed, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    let terminal = StreamTokenOperationV1 {
        outcome: StreamTokenOutcomeV1::Expired,
        ..original
    };
    assert_eq!(
        validate_stream_token_complete_request_claim_v1(&good, &terminal),
        Err(StreamTokenClaimErrorV1::Operation)
    );
}

#[test]
fn native_operation_claim_rejects_foreign_authority_control_and_execution_chronology() {
    let original = reserved();
    let valid = |row: &StreamTokenNativeOperationV1| {
        validate_stream_token_native_operation_claim_v1(
            row,
            ProviderId::new([0x21; 32]),
            3,
            [0x22; 32],
            &operator(0x23),
        )
    };
    assert_eq!(valid(&original), Ok(()));
    assert_eq!(valid(&completed()), Ok(()));
    let mut revoked = original.clone();
    revoked.operation.outcome = StreamTokenOutcomeV1::Expired;
    revoked.terminal_execution = Some(StreamTokenExecutionV1 {
        height: 11,
        transaction_hash: [0x2f; 32],
        entry_index: 0,
        instruction_index: 0,
        recorded_at_unix_ms: 1_020_000,
        authority: operator(0x30),
    });
    assert_eq!(valid(&revoked), Ok(()));
    let mut changed = original.clone();
    changed.provider_id = ProviderId::new([0x27; 32]);
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = original.clone();
    changed.custody_control_digest = [0x27; 32];
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = original.clone();
    changed.reserved_execution.authority = operator(0x28);
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = original.clone();
    changed.reserved_execution.transaction_hash = [0; 32];
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = original.clone();
    changed.reserved_execution.recorded_at_unix_ms =
        changed.operation.reservation.expires_at_unix_ms;
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = completed();
    changed
        .terminal_execution
        .as_mut()
        .unwrap()
        .recorded_at_unix_ms += 1;
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = completed();
    changed
        .terminal_execution
        .as_mut()
        .unwrap()
        .transaction_hash = [0x2d; 32];
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
    changed = completed();
    changed.terminal_execution = None;
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Phase));
    changed = completed();
    let reserved_height = changed.reserved_execution.height;
    let reserved_entry_index = changed.reserved_execution.entry_index;
    let reserved_instruction_index = changed.reserved_execution.instruction_index;
    let terminal = changed.terminal_execution.as_mut().unwrap();
    terminal.height = reserved_height;
    terminal.entry_index = reserved_entry_index;
    terminal.instruction_index = reserved_instruction_index;
    assert_eq!(valid(&changed), Err(StreamTokenClaimErrorV1::Operation));
}

#[test]
fn challenged_check_binds_network_provider_operator_nonce_floor_and_phase() {
    for phase in [
        StreamTokenCheckPhaseV1::Current(reviewed().intent.previous_audit),
        StreamTokenCheckPhaseV1::BeforeProvider(reserved()),
        StreamTokenCheckPhaseV1::AfterProvider(reserved()),
        StreamTokenCheckPhaseV1::BeforeCommit(reserved()),
        StreamTokenCheckPhaseV1::AfterCommit(completed()),
        StreamTokenCheckPhaseV1::BeforeRelease(completed()),
    ] {
        assert_eq!(validate_check(&check(phase)), Ok(()));
    }
    let valid = check(StreamTokenCheckPhaseV1::BeforeProvider(reserved()));
    let mut changed = valid.clone();
    changed.network_id[0] ^= 1;
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    changed.provider_id = ProviderId::new([0x29; 32]);
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    changed.expected_control_revision += 1;
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.challenge[0] ^= 1;
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.expected_operator = operator(0x2a);
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.expected_observer = operator(0x2a);
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.expected_observer = operator(0x23);
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.floor.height += 1;
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.floor.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign context",
        )));
    }
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = check(StreamTokenCheckPhaseV1::BeforeProvider(completed()));
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Phase)
    );
    changed = check(StreamTokenCheckPhaseV1::AfterCommit(reserved()));
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Phase)
    );
    changed = check(StreamTokenCheckPhaseV1::Current(
        SignerOperationAuditHeadV1 {
            sequence: 8,
            digest: [0x2b; 32],
        },
    ));
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Phase)
    );
    changed = valid.clone();
    if let StreamTokenAuthorityActionV1::Check(inner) = &mut changed.action {
        inner.floor.height = 9;
    }
    let StreamTokenAuthorityActionV1::Check(inner) = &changed.action else {
        unreachable!();
    };
    assert_eq!(
        validate_stream_token_check_claim_v1(
            &changed,
            [0x24; 32],
            ProviderId::new([0x21; 32]),
            3,
            [0x22; 32],
            &operator(0x23),
            &operator(0x2c),
            [0x25; 32],
            inner.floor,
            &reviewed(),
            &inner.phase,
        ),
        Err(StreamTokenClaimErrorV1::Phase)
    );
    let StreamTokenAuthorityActionV1::Check(inner) = &valid.action else {
        unreachable!();
    };
    assert_eq!(
        validate_stream_token_check_claim_v1(
            &valid,
            [0x24; 32],
            ProviderId::new([0x21; 32]),
            3,
            [0x22; 32],
            &operator(0x23),
            &operator(0x2c),
            [0x25; 32],
            inner.floor,
            &reviewed(),
            &StreamTokenCheckPhaseV1::AfterProvider(reserved()),
        ),
        Err(StreamTokenClaimErrorV1::Round)
    );
    changed = valid;
    changed.action = StreamTokenAuthorityActionV1::Reserve(reviewed());
    assert_eq!(
        validate_check(&changed),
        Err(StreamTokenClaimErrorV1::Phase)
    );
}
