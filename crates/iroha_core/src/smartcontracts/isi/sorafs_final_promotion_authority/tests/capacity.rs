//! Counter-boundary tests over coherent adjacent suffixes, without synthesizing ledger authority.
//! Pure successor/preparation checks do not claim a full retained 65,536-operation history.
use super::*;
use crate::query::final_promotion_authority::operation::{operation_digest, successor_head};
use iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompletedV1;
use iroha_data_model::sorafs::final_promotion_authority::{
    FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1, FINAL_PROMOTION_CUSTODY_NORMAL_REVISIONS_V1,
    FinalPromotionCustodyRecordV1,
};
use sorafs_manifest::signer::custody_control::SignerCustodyControlStateV1;

fn final_admission_suffix() -> (
    FinalPromotionOperationRecordV1,
    FinalPromotionOperationHeadV1,
    FinalPromotionOperationRecordV1,
) {
    let f = fixture();
    let audit = SignerOperationAuditHeadV1 {
        sequence: 0,
        digest: [0; 32],
    };
    let old_execution = FinalPromotionExecutionV1 {
        height: 40,
        ordinal: 0,
        recorded_at_unix_ms: 1_000,
        authority: f.operator.clone(),
    };
    let old = FinalPromotionOperationRecordV1 {
        deployment_id: DEPLOYMENT.into(),
        revision: 2 * (FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1),
        predecessor_digest: [11; 32],
        request_digest: [12; 32],
        execution: old_execution.clone(),
        execution_origin: None,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: [30; 32],
            request_digest: [13; 32],
            previous_audit: audit,
        },
        custody: SignerOperationCustodyV1 {
            record_digest: [14; 32],
            control_state_digest: [15; 32],
        },
        reservation: SignerOperationReservationV1 {
            reservation_id: [16; 32],
            fence: FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1,
            expires_at_unix_ms: 1_000,
        },
        reserved: FinalPromotionExecutionV1 {
            height: 39,
            ordinal: 0,
            recorded_at_unix_ms: 500,
            authority: f.operator.clone(),
        },
        reserved_origin: FinalPromotionOperationOriginV1 {
            entry_hash: [24; 32],
            entry_index: 0,
        },
        outcome: FinalPromotionOperationOutcomeV1::Expired,
    };
    let head = FinalPromotionOperationHeadV1 {
        revision: old.revision,
        digest: operation_digest(&old).unwrap(),
        fence: old.reservation.fence,
        audit,
        active_operation: None,
        total_admissions: FINAL_PROMOTION_MAX_OPERATIONS_V1 - 1,
    };
    let execution = FinalPromotionExecutionV1 {
        height: 41,
        ordinal: 0,
        recorded_at_unix_ms: 2_000,
        authority: f.operator,
    };
    let mut reserved = old.clone();
    reserved.revision = head.revision + 1;
    reserved.predecessor_digest = head.digest;
    reserved.request_digest = [17; 32];
    reserved.execution = execution.clone();
    reserved.intent.operation_id = [31; 32];
    reserved.intent.request_digest = [18; 32];
    reserved.reservation = SignerOperationReservationV1 {
        reservation_id: [19; 32],
        fence: head.fence + 1,
        expires_at_unix_ms: 2_000 + FINAL_PROMOTION_RESERVATION_MS_V1,
    };
    reserved.reserved = execution;
    reserved.reserved_origin = FinalPromotionOperationOriginV1 {
        entry_hash: [25; 32],
        entry_index: 0,
    };
    reserved.execution_origin = Some(reserved.reserved_origin);
    reserved.outcome = FinalPromotionOperationOutcomeV1::Reserved;
    (old, head, reserved)
}

#[test]
fn last_operation_admission_retains_both_completion_and_expiration_capacity() {
    let (old, head, reserved) = final_admission_suffix();
    let admitted = successor_head(head, Some(&old), &reserved).unwrap();
    assert_eq!(admitted.total_admissions, FINAL_PROMOTION_MAX_OPERATIONS_V1);
    assert_eq!(admitted.revision + 1, 2 * FINAL_PROMOTION_MAX_OPERATIONS_V1);
    for outcome in [
        FinalPromotionOperationOutcomeV1::Completed(FinalPromotionCompletedV1 {
            commitment: SignerOperationCommitmentV1 {
                audit: SignerOperationAuditHeadV1 {
                    sequence: 1,
                    digest: [20; 32],
                },
                response_digest: [21; 32],
            },
            signatures_digest: [22; 32],
        }),
        FinalPromotionOperationOutcomeV1::Expired,
    ] {
        let mut terminal = reserved.clone();
        terminal.revision = admitted.revision + 1;
        terminal.predecessor_digest = admitted.digest;
        terminal.request_digest = [23; 32];
        terminal.execution.height += 1;
        terminal.execution.recorded_at_unix_ms =
            if outcome == FinalPromotionOperationOutcomeV1::Expired {
                reserved.reservation.expires_at_unix_ms
            } else {
                3_000
            };
        terminal.outcome = outcome;
        terminal.execution_origin =
            matches!(outcome, FinalPromotionOperationOutcomeV1::Completed(_)).then_some(
                FinalPromotionOperationOriginV1 {
                    entry_hash: [26; 32],
                    entry_index: 0,
                },
            );
        let completed = successor_head(admitted, Some(&reserved), &terminal).unwrap();
        assert_eq!(completed.revision, 2 * FINAL_PROMOTION_MAX_OPERATIONS_V1);
        assert_eq!(completed.active_operation, None);
        assert_eq!(completed.total_admissions, admitted.total_admissions);
        assert_eq!(completed.fence, admitted.fence);
        assert_eq!(
            completed.audit.sequence,
            u64::from(matches!(
                outcome,
                FinalPromotionOperationOutcomeV1::Completed(_)
            ))
        );
        let mut next = reserved.clone();
        next.revision = completed.revision + 1;
        next.predecessor_digest = completed.digest;
        next.intent.operation_id = [32; 32];
        next.intent.previous_audit = completed.audit;
        next.execution.height = terminal.execution.height + 1;
        next.execution.recorded_at_unix_ms = terminal.execution.recorded_at_unix_ms + 1;
        next.reserved = next.execution.clone();
        next.reserved_origin = FinalPromotionOperationOriginV1 {
            entry_hash: [27; 32],
            entry_index: 0,
        };
        next.execution_origin = Some(next.reserved_origin);
        next.reservation.fence = completed.fence + 1;
        next.reservation.expires_at_unix_ms =
            next.execution.recorded_at_unix_ms + FINAL_PROMOTION_RESERVATION_MS_V1;
        assert_eq!(
            successor_head(completed, Some(&terminal), &next),
            Err(Error::Capacity)
        );
    }
}

#[test]
fn operation_revision_fence_admission_and_audit_overflow_cannot_wrap() {
    let (old, head, reserved) = final_admission_suffix();
    let mut revision_overflow = head;
    revision_overflow.revision = u64::MAX;
    let mut wrapped = reserved.clone();
    wrapped.revision = 0;
    assert_eq!(
        successor_head(revision_overflow, Some(&old), &wrapped),
        Err(Error::CorruptHistory)
    );
    let mut fence_overflow = head;
    fence_overflow.fence = u64::MAX;
    fence_overflow.total_admissions = u64::MAX;
    let mut wrapped = reserved.clone();
    wrapped.reservation.fence = 0;
    assert_eq!(
        successor_head(fence_overflow, Some(&old), &wrapped),
        Err(Error::CorruptHistory)
    );
    let mut admission_overflow = head;
    admission_overflow.total_admissions = u64::MAX;
    assert_eq!(
        successor_head(admission_overflow, Some(&old), &reserved),
        Err(Error::Capacity)
    );
    let mut audit_overflow = successor_head(head, Some(&old), &reserved).unwrap();
    audit_overflow.audit = SignerOperationAuditHeadV1 {
        sequence: u64::MAX,
        digest: [40; 32],
    };
    let mut terminal = reserved.clone();
    terminal.revision += 1;
    terminal.predecessor_digest = audit_overflow.digest;
    terminal.execution.height += 1;
    terminal.execution.recorded_at_unix_ms += 1;
    terminal.outcome = FinalPromotionOperationOutcomeV1::Completed(FinalPromotionCompletedV1 {
        commitment: SignerOperationCommitmentV1 {
            audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [41; 32],
            },
            response_digest: [42; 32],
        },
        signatures_digest: [43; 32],
    });
    terminal.execution_origin = Some(FinalPromotionOperationOriginV1 {
        entry_hash: [28; 32],
        entry_index: 0,
    });
    assert_eq!(
        successor_head(audit_overflow, Some(&reserved), &terminal),
        Err(Error::CorruptHistory)
    );
}

#[test]
fn normal_control_capacity_preserves_two_emergency_revocations_and_rejects_overflow() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 2_000, |tx| {
        // Supply one coherent immutable tail to the existing preparation boundary. This tests its
        // capacity policy without pretending the omitted history is an authenticated State.
        let mut tail = read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        tail.record.revision = FINAL_PROMOTION_CUSTODY_NORMAL_REVISIONS_V1;
        tail.record.predecessor_digest = [50; 32];
        tail.index.revision = tail.record.revision;
        tail.index.digest =
            crate::query::signer_custody_history::control_digest::<ReceiptPurpose>(&tail.record)
                .unwrap();
        let base = MutateSorafsFinalPromotionAuthority {
            deployment_id: DEPLOYMENT.into(),
            expected_control_revision: tail.index.revision,
            expected_control_digest: tail.index.digest,
            action: Action::Revoke(FinalPromotionRevocationV1 {
                signer: true,
                attester: false,
            }),
        };
        for action in [
            Action::Configure(encode(&f.policy).unwrap()),
            Action::Enroll(Vec::new()),
        ] {
            let mut mutation = base.clone();
            mutation.action = action;
            let mut writes = Writes::new();
            assert_eq!(
                control::prepare(
                    &mutation,
                    &f.manager,
                    tx,
                    Some(&tail),
                    FinalPromotionOperationHeadV1::empty(),
                    [51; 32],
                    &mut writes
                ),
                Err(Error::Capacity)
            );
            assert!(writes.is_empty());
        }
        let before = retained(tx);
        let mut writes = Writes::new();
        control::prepare(
            &base,
            &f.manager,
            tx,
            Some(&tail),
            FinalPromotionOperationHeadV1::empty(),
            [52; 32],
            &mut writes,
        )
        .unwrap();
        let first_key = control_record_key::<ReceiptPurpose>(
            DEPLOYMENT,
            FINAL_PROMOTION_CUSTODY_NORMAL_REVISIONS_V1 + 1,
        )
        .unwrap();
        let first: FinalPromotionCustodyRecordV1 =
            decode(&writes.iter().find(|(key, _)| key == &first_key).unwrap().1).unwrap();
        let first_state: SignerCustodyControlStateV1 = decode(&first.control_state).unwrap();
        assert!(first_state.signer_revoked);
        assert!(!first_state.attester_revoked);
        let next = NativeControl::<ReceiptPurpose> {
            index: ControlIndexV1 {
                revision: first.revision,
                digest: crate::query::signer_custody_history::control_digest::<ReceiptPurpose>(
                    &first,
                )
                .unwrap(),
                height: first.execution.height,
                ordinal: first.execution.ordinal,
            },
            record: first,
            state: first_state,
        };
        let final_revoke = MutateSorafsFinalPromotionAuthority {
            deployment_id: DEPLOYMENT.into(),
            expected_control_revision: next.index.revision,
            expected_control_digest: next.index.digest,
            action: Action::Revoke(FinalPromotionRevocationV1 {
                signer: false,
                attester: true,
            }),
        };
        control::prepare(
            &final_revoke,
            &f.manager,
            tx,
            Some(&next),
            FinalPromotionOperationHeadV1::empty(),
            [53; 32],
            &mut writes,
        )
        .unwrap();
        let final_key = control_record_key::<ReceiptPurpose>(
            DEPLOYMENT,
            FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1,
        )
        .unwrap();
        let final_record: FinalPromotionCustodyRecordV1 =
            decode(&writes.iter().find(|(key, _)| key == &final_key).unwrap().1).unwrap();
        let final_state: SignerCustodyControlStateV1 = decode(&final_record.control_state).unwrap();
        assert!(final_state.signer_revoked && final_state.attester_revoked);
        for revision in [FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1, u64::MAX] {
            let mut exhausted = final_revoke.clone();
            exhausted.expected_control_revision = revision;
            let mut rejected_writes = Writes::new();
            assert_eq!(
                control::prepare(
                    &exhausted,
                    &f.manager,
                    tx,
                    Some(&next),
                    FinalPromotionOperationHeadV1::empty(),
                    [54; 32],
                    &mut rejected_writes
                ),
                Err(Error::Capacity)
            );
            assert!(rejected_writes.is_empty());
        }
        assert_eq!(retained(tx), before);
    });
}
