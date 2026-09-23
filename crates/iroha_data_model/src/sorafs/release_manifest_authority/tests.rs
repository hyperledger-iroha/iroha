//! Focused canonical-frame and adversarial claim-shape checks for role 13.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use sorafs_manifest::signer::protocol::SignerOperationCustodyV1;

fn fixture_request() -> SignerReleaseManifestRequestV1 {
    SignerReleaseManifestRequestV1 {
        operation_id: [1; 32],
        binding_digest: [2; 32],
        original_custody: SignerOperationCustodyV1 {
            record_digest: [3; 32],
            control_state_digest: [4; 32],
        },
        manifest_digest: [5; 32],
        manifest_size: 128,
    }
}

fn fixture_audit() -> SignerOperationAuditHeadV1 {
    SignerOperationAuditHeadV1 {
        sequence: 7,
        digest: [6; 32],
    }
}

fn fixture_review() -> ReleaseManifestReserveV1 {
    let request = fixture_request();
    ReleaseManifestReserveV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().expect("request digest"),
            previous_audit: fixture_audit(),
        },
    }
}

fn fixture_reservation() -> SignerOperationReservationV1 {
    SignerOperationReservationV1 {
        reservation_id: [8; 32],
        fence: 11,
        expires_at_unix_ms: 30_000,
    }
}

fn fixture_reserved_row() -> ReleaseManifestOperationV1 {
    ReleaseManifestOperationV1 {
        reviewed: fixture_review(),
        reservation: fixture_reservation(),
        outcome: ReleaseManifestOutcomeV1::Reserved,
    }
}

fn fixture_completion() -> ReleaseManifestCompleteV1 {
    ReleaseManifestCompleteV1 {
        reviewed: fixture_review(),
        reservation: fixture_reservation(),
        commitment: SignerOperationCommitmentV1 {
            audit: SignerOperationAuditHeadV1 {
                sequence: 8,
                digest: [9; 32],
            },
            response_digest: [10; 32],
        },
        signatures_digest: [11; 32],
        completed_at_unix_ms: 29_999,
    }
}

fn fixture_operator() -> AccountId {
    let key = KeyPair::try_from_seed(vec![12; 32], Algorithm::Ed25519).expect("operator key");
    AccountId::new(key.public_key().clone())
}

fn fixture_floor() -> ReleaseManifestFloorV1 {
    ReleaseManifestFloorV1 {
        height: 21,
        block_hash: [13; 32],
    }
}

fn fixture_check(phase: ReleaseManifestCheckPhaseV1) -> ReleaseManifestCheckV1 {
    ReleaseManifestCheckV1 {
        challenge: [14; 32],
        network_id: [15; 32],
        expected_operator: fixture_operator(),
        floor: fixture_floor(),
        reviewed: fixture_review(),
        phase,
    }
}

fn check_claim(check: &ReleaseManifestCheckV1) -> Result<(), ReleaseManifestClaimErrorV1> {
    validate_release_manifest_check_claim_v1(
        check,
        [14; 32],
        [15; 32],
        &fixture_operator(),
        fixture_floor(),
        &fixture_request(),
        fixture_audit(),
    )
}

#[test]
fn release_manifest_actions_have_one_bounded_canonical_norito_surface() {
    let actions = [
        ReleaseManifestActionV1::Configure(vec![1, 2, 3]),
        ReleaseManifestActionV1::Enroll(vec![4, 5, 6]),
        ReleaseManifestActionV1::Revoke(ReleaseManifestRevokeV1 {
            signer: true,
            attester: false,
        }),
        ReleaseManifestActionV1::Reserve(fixture_review()),
        ReleaseManifestActionV1::Complete(fixture_completion()),
        ReleaseManifestActionV1::Expire(ReleaseManifestExpireV1 {
            operation_id: [1; 32],
            reservation: fixture_reservation(),
        }),
        ReleaseManifestActionV1::Check(fixture_check(ReleaseManifestCheckPhaseV1::Current(
            fixture_audit(),
        ))),
    ];
    for action in actions {
        let frame = norito::encode_canonical(&action).expect("canonical frame");
        assert!(frame.len() <= RELEASE_MANIFEST_ACTION_MAX_BYTES_V1);
        assert_eq!(
            decode_release_manifest_action_claim_v1(&frame).expect("canonical decode"),
            action
        );
        assert_eq!(
            decode_release_manifest_action_claim_v1(&frame[..frame.len() - 1]),
            Err(ReleaseManifestClaimErrorV1::Encoding)
        );
        let mut trailed = frame;
        trailed.push(0);
        assert_eq!(
            decode_release_manifest_action_claim_v1(&trailed),
            Err(ReleaseManifestClaimErrorV1::Encoding)
        );
    }
    assert_eq!(
        decode_release_manifest_action_claim_v1(&[]),
        Err(ReleaseManifestClaimErrorV1::Encoding)
    );
    let oversized = norito::encode_canonical(&ReleaseManifestActionV1::Configure(vec![
        0;
        RELEASE_MANIFEST_ACTION_MAX_BYTES_V1
    ]))
    .expect("oversized fixture");
    assert_eq!(
        decode_release_manifest_action_claim_v1(&oversized),
        Err(ReleaseManifestClaimErrorV1::Encoding)
    );
}

#[test]
fn release_manifest_review_must_equal_independently_expected_request_and_audit() {
    let valid = fixture_review();
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&valid, &fixture_request(), fixture_audit()),
        Ok(())
    );
    let mut altered = valid;
    altered.request.manifest_digest = [16; 32];
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    altered = valid;
    altered.request.original_custody.control_state_digest = [17; 32];
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    altered = valid;
    altered.intent.action = SignerOperationActionV1::Qualify;
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    altered = valid;
    altered.intent.request_digest = [18; 32];
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    altered = valid;
    altered.intent.previous_audit.sequence += 1;
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    altered = valid;
    altered.request.manifest_size = SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1 as u64 + 1;
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &altered.request, fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    // A coherent foreign-purpose request/intent pair cannot replace the independently pinned
    // role-13 request. The role itself is verified when the caller constructs that expectation.
    altered = valid;
    altered.request.binding_digest = [23; 32];
    altered.request.original_custody.record_digest = [24; 32];
    altered.intent.request_digest = altered.request.digest().expect("foreign request digest");
    assert_eq!(
        validate_release_manifest_reserve_claim_v1(&altered, &fixture_request(), fixture_audit()),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
}

#[test]
fn release_manifest_completion_requires_original_slot_and_next_audit() {
    let original = fixture_reserved_row();
    let valid = fixture_completion();
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&valid, &original),
        Ok(())
    );
    let mut altered = valid;
    altered.reservation.fence += 1;
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&altered, &original),
        Err(ReleaseManifestClaimErrorV1::Operation)
    );
    altered = valid;
    altered.commitment.audit.sequence += 1;
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&altered, &original),
        Err(ReleaseManifestClaimErrorV1::Operation)
    );
    altered = valid;
    altered.completed_at_unix_ms = original.reservation.expires_at_unix_ms;
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&altered, &original),
        Err(ReleaseManifestClaimErrorV1::Operation)
    );
    altered = valid;
    altered.signatures_digest = [0; 32];
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&altered, &original),
        Err(ReleaseManifestClaimErrorV1::Operation)
    );
    let mut expired = original;
    expired.outcome = ReleaseManifestOutcomeV1::Expired;
    assert_eq!(
        validate_release_manifest_complete_claim_v1(&valid, &expired),
        Err(ReleaseManifestClaimErrorV1::Operation)
    );
}

#[test]
fn release_manifest_check_rejects_round_substitution_and_forged_phase() {
    let reserved = fixture_reserved_row();
    let mut completed = reserved;
    completed.outcome = ReleaseManifestOutcomeV1::Completed(fixture_completion());
    let phases = [
        ReleaseManifestCheckPhaseV1::Current(fixture_audit()),
        ReleaseManifestCheckPhaseV1::BeforeProvider(reserved),
        ReleaseManifestCheckPhaseV1::AfterProvider(reserved),
        ReleaseManifestCheckPhaseV1::BeforeCommit(reserved),
        ReleaseManifestCheckPhaseV1::AfterCommit(completed),
        ReleaseManifestCheckPhaseV1::BeforeRelease(completed),
    ];
    for phase in phases {
        assert_eq!(check_claim(&fixture_check(phase)), Ok(()));
    }
    let current = fixture_check(ReleaseManifestCheckPhaseV1::Current(fixture_audit()));
    assert_eq!(
        validate_release_manifest_check_claim_v1(
            &current,
            [19; 32],
            [15; 32],
            &fixture_operator(),
            fixture_floor(),
            &fixture_request(),
            fixture_audit(),
        ),
        Err(ReleaseManifestClaimErrorV1::Round)
    );
    let mut wrong_floor = current.clone();
    wrong_floor.floor.block_hash = [20; 32];
    assert_eq!(
        check_claim(&wrong_floor),
        Err(ReleaseManifestClaimErrorV1::Round)
    );
    let mut wrong_review = current.clone();
    wrong_review.reviewed.request.manifest_digest = [21; 32];
    assert_eq!(
        check_claim(&wrong_review),
        Err(ReleaseManifestClaimErrorV1::Review)
    );
    let forged = fixture_check(ReleaseManifestCheckPhaseV1::AfterCommit(reserved));
    assert_eq!(
        check_claim(&forged),
        Err(ReleaseManifestClaimErrorV1::Phase)
    );
    let forged = fixture_check(ReleaseManifestCheckPhaseV1::BeforeProvider(completed));
    assert_eq!(
        check_claim(&forged),
        Err(ReleaseManifestClaimErrorV1::Phase)
    );
    let mut changed_row = reserved;
    changed_row.reviewed.intent.operation_id = [22; 32];
    let forged = fixture_check(ReleaseManifestCheckPhaseV1::BeforeProvider(changed_row));
    assert_eq!(
        check_claim(&forged),
        Err(ReleaseManifestClaimErrorV1::Phase)
    );
    let mut changed_completion = completed;
    changed_completion.outcome = ReleaseManifestOutcomeV1::Completed(ReleaseManifestCompleteV1 {
        signatures_digest: [0; 32],
        ..fixture_completion()
    });
    let forged = fixture_check(ReleaseManifestCheckPhaseV1::BeforeRelease(
        changed_completion,
    ));
    assert_eq!(
        check_claim(&forged),
        Err(ReleaseManifestClaimErrorV1::Phase)
    );
}
