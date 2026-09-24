//! Bounded role-11 claims cannot substitute for independent native operation authority.
use super::*;
use sorafs_manifest::signer::protocol::SignerOperationCustodyV1;

fn request() -> SignerStreamTokenRequestV1 {
    SignerStreamTokenRequestV1 {
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
    }
}

fn audit() -> SignerOperationAuditHeadV1 {
    SignerOperationAuditHeadV1 {
        sequence: 7,
        digest: [6; 32],
    }
}

fn reviewed() -> StreamTokenReviewedV1 {
    let request = request();
    StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: audit(),
        },
    }
}

fn reservation() -> SignerOperationReservationV1 {
    SignerOperationReservationV1 {
        reservation_id: [7; 32],
        fence: 11,
        expires_at_unix_ms: 1_100_000,
    }
}

fn original() -> StreamTokenOperationV1 {
    StreamTokenOperationV1 {
        reviewed: reviewed(),
        reservation: reservation(),
        outcome: StreamTokenOutcomeV1::Reserved,
    }
}

fn completion() -> StreamTokenCompleteV1 {
    StreamTokenCompleteV1 {
        reviewed: reviewed(),
        reservation: reservation(),
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

#[test]
fn stream_token_operation_claim_has_one_bounded_canonical_norito_and_json_shape() {
    let reserved = original();
    let completed = StreamTokenOperationV1 {
        outcome: StreamTokenOutcomeV1::Completed(completion()),
        ..reserved
    };
    let expired = StreamTokenOperationV1 {
        outcome: StreamTokenOutcomeV1::Expired,
        ..reserved
    };
    for claim in [reserved, completed, expired] {
        let frame = norito::encode_canonical(&claim).unwrap();
        assert!(frame.len() <= STREAM_TOKEN_OPERATION_CLAIM_MAX_BYTES_V1);
        assert_eq!(decode_stream_token_operation_claim_v1(&frame), Ok(claim));
        let json = norito::json::to_json(&claim).unwrap();
        assert_eq!(
            norito::json::from_str::<StreamTokenOperationV1>(&json).unwrap(),
            claim
        );
        let foreign = json.replacen("\"reviewed\":", "\"unknown\":1,\"reviewed\":", 1);
        assert_ne!(foreign, json);
        assert!(norito::json::from_str::<StreamTokenOperationV1>(&foreign).is_err());
        assert_eq!(
            decode_stream_token_operation_claim_v1(&frame[..frame.len() - 1]),
            Err(StreamTokenClaimErrorV1::Encoding)
        );
        let mut trailed = frame;
        trailed.push(0);
        assert_eq!(
            decode_stream_token_operation_claim_v1(&trailed),
            Err(StreamTokenClaimErrorV1::Encoding)
        );
    }
    assert_eq!(
        decode_stream_token_operation_claim_v1(&[]),
        Err(StreamTokenClaimErrorV1::Encoding)
    );
    assert_eq!(
        decode_stream_token_operation_claim_v1(&vec![
            0;
            STREAM_TOKEN_OPERATION_CLAIM_MAX_BYTES_V1 + 1
        ]),
        Err(StreamTokenClaimErrorV1::Encoding)
    );
}

#[test]
fn stream_token_review_requires_independently_pinned_body_binding_custody_and_audit() {
    let valid = reviewed();
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&valid, &request(), audit()),
        Ok(())
    );
    let mut altered = valid;
    altered.request.binding_digest = [11; 32];
    altered.request.original_custody.record_digest = [12; 32];
    altered.intent.request_digest = altered.request.digest().unwrap();
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&altered, &request(), audit()),
        Err(StreamTokenClaimErrorV1::Review)
    );
    altered = valid;
    altered.intent.action = SignerOperationActionV1::Qualify;
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&altered, &request(), audit()),
        Err(StreamTokenClaimErrorV1::Review)
    );
    altered = valid;
    altered.intent.previous_audit.sequence += 1;
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&altered, &request(), audit()),
        Err(StreamTokenClaimErrorV1::Review)
    );
    altered = valid;
    altered.request.signing_payload_size = SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1 as u64 + 1;
    altered.intent.request_digest = altered.request.digest().unwrap();
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&altered, &altered.request, audit()),
        Err(StreamTokenClaimErrorV1::Review)
    );
    altered = valid;
    altered.request.expires_at_unix_ms = altered.request.issued_at_unix_ms;
    altered.intent.request_digest = altered.request.digest().unwrap();
    assert_eq!(
        validate_stream_token_reviewed_claim_v1(&altered, &altered.request, audit()),
        Err(StreamTokenClaimErrorV1::Review)
    );
}

#[test]
fn stream_token_completion_rejects_foreign_slot_audit_and_claimed_time() {
    let original = original();
    let valid = completion();
    assert_eq!(
        validate_stream_token_complete_claim_v1(&valid, &original),
        Ok(())
    );
    let mut altered = valid;
    altered.reservation.fence += 1;
    assert_eq!(
        validate_stream_token_complete_claim_v1(&altered, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    altered = valid;
    altered.commitment.audit.sequence += 1;
    assert_eq!(
        validate_stream_token_complete_claim_v1(&altered, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    altered = valid;
    altered.signatures_digest = [0; 32];
    assert_eq!(
        validate_stream_token_complete_claim_v1(&altered, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    for bad_time in [
        valid.reviewed.request.issued_at_unix_ms - 1,
        original.reservation.expires_at_unix_ms,
        valid.reviewed.request.expires_at_unix_ms,
    ] {
        altered = valid;
        altered.completed_at_unix_ms = bad_time;
        assert_eq!(
            validate_stream_token_complete_claim_v1(&altered, &original),
            Err(StreamTokenClaimErrorV1::Operation)
        );
    }
    let terminal = StreamTokenOperationV1 {
        outcome: StreamTokenOutcomeV1::Expired,
        ..original
    };
    assert_eq!(
        validate_stream_token_complete_claim_v1(&valid, &terminal),
        Err(StreamTokenClaimErrorV1::Operation)
    );
}

#[test]
fn stream_token_expiry_keeps_exact_original_id_and_reservation() {
    let original = original();
    let valid = StreamTokenExpireV1 {
        operation_id: request().operation_id,
        reservation: reservation(),
    };
    assert_eq!(
        validate_stream_token_expire_claim_v1(&valid, &original),
        Ok(())
    );
    let mut altered = valid;
    altered.operation_id = [13; 32];
    assert_eq!(
        validate_stream_token_expire_claim_v1(&altered, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    altered = valid;
    altered.reservation.reservation_id = [14; 32];
    assert_eq!(
        validate_stream_token_expire_claim_v1(&altered, &original),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    altered = valid;
    altered.reservation.fence = 0;
    let malformed = StreamTokenOperationV1 {
        reservation: altered.reservation,
        ..original
    };
    assert_eq!(
        validate_stream_token_expire_claim_v1(&altered, &malformed),
        Err(StreamTokenClaimErrorV1::Operation)
    );
    let terminal = StreamTokenOperationV1 {
        outcome: StreamTokenOutcomeV1::Completed(completion()),
        ..original
    };
    assert_eq!(
        validate_stream_token_expire_claim_v1(&valid, &terminal),
        Err(StreamTokenClaimErrorV1::Operation)
    );
}
