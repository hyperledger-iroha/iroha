//! Finalized Current Check handoff tests use the same native State/Kura fixture as the signer.

use super::*;
use crate::signer_operation::final_promotion::current_observation::{
    FinalPromotionCurrentCheckPayloadV1, FinalPromotionCurrentCheckRuntimeV1,
    FinalPromotionCurrentCheckSignerV1, FinalPromotionCurrentCheckSubmissionV1,
    FinalPromotionCurrentCheckSubmitOutcomeV1,
    FinalPromotionCurrentObservationErrorV1 as CurrentError, FinalPromotionQualifiedUtcV1,
    FinalPromotionRetainedFloorV1,
};
use crate::signer_operation::final_promotion::observer_transaction::FinalPromotionObserverKeyRequestV1;
use iroha_core::{
    query::final_promotion_authority::observation::PendingFinalPromotionCheckV1, state::State,
};
use iroha_data_model::{
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{FinalPromotionCompleteV1, FinalPromotionRevocationV1},
    transaction::TransactionPayload,
};
use sorafs_manifest::signer::{
    custody::SignerCustodyBindingV1, protocol::SignerOperationAuditHeadV1,
};
use std::collections::VecDeque;

struct QualifiedClock {
    intervals: VecDeque<Result<FinalPromotionEligibilityTimeIntervalV1, CurrentError>>,
    calls: usize,
}
impl QualifiedClock {
    fn fixed() -> Self {
        Self {
            intervals: VecDeque::from([Ok(times().0), Ok(times().0)]),
            calls: 0,
        }
    }
}
impl FinalPromotionQualifiedUtcV1 for QualifiedClock {
    fn sample(&mut self) -> Result<FinalPromotionEligibilityTimeIntervalV1, CurrentError> {
        self.calls += 1;
        self.intervals
            .pop_front()
            .unwrap_or(Err(CurrentError::Clock))
    }
}

struct RetainedFloor {
    current: FinalPromotionCheckFloorV1,
    advances: usize,
    fail_advance: bool,
    mismatched_readback: bool,
}
impl RetainedFloor {
    fn new(f: &Fixture) -> Self {
        let (height, block_hash, context_id) = f.native.finalized_floor().unwrap();
        Self {
            current: FinalPromotionCheckFloorV1 {
                height,
                block_hash,
                context_id,
            },
            advances: 0,
            fail_advance: false,
            mismatched_readback: false,
        }
    }
}
impl FinalPromotionRetainedFloorV1 for RetainedFloor {
    fn read(&mut self) -> Result<FinalPromotionCheckFloorV1, CurrentError> {
        Ok(self.current)
    }

    fn advance_and_readback(
        &mut self,
        previous: FinalPromotionCheckFloorV1,
        applied: FinalPromotionCheckFloorV1,
    ) -> Result<FinalPromotionCheckFloorV1, CurrentError> {
        self.advances += 1;
        if self.fail_advance || self.current != previous {
            return Err(CurrentError::Floor);
        }
        self.current = applied;
        if self.mismatched_readback {
            Ok(previous)
        } else {
            Ok(self.current)
        }
    }
}

fn pending_current(f: &Fixture) -> PendingFinalPromotionCheckV1 {
    let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
    let payload = f.observer_payload(prepared.instruction().clone().into());
    f.observer_transactions()
        .sign_receipt_with(prepared, payload, |request| {
            Signature::try_new(key(3).private_key(), request.signing_message())
                .map_err(|_| ObserverError::Provider)
        })
        .unwrap()
}

#[test]
fn current_handoff_uses_one_finalized_snapshot_and_retains_floor_before_returning_state() {
    let mut f = Fixture::new();
    let mut floor = RetainedFloor::new(&f);
    let pending = pending_current(&f);
    assert_eq!(
        f.native
            .commit(NOW, vec![pending.signed_transaction().clone()]),
        [true]
    );
    let view = f.native.state().view();
    let expected = read_final_promotion_authority_at_v1(
        &view,
        &f.receipt_policy.binding,
        view.height() as u64,
        Some(f.expected.operation_id),
    )
    .unwrap()
    .unwrap();
    drop(view);
    let mut clock = QualifiedClock {
        intervals: VecDeque::from([
            Ok(times().0),
            Ok(FinalPromotionEligibilityTimeIntervalV1 {
                earliest_unix_ms: NOW + 1,
                latest_unix_ms: NOW + 2,
            }),
        ]),
        calls: 0,
    };
    let observation = f
        .observer_transactions()
        .finish_current_with(pending, &mut clock, &mut floor)
        .unwrap();
    assert_eq!(clock.calls, 2);
    assert_eq!(floor.advances, 1);
    assert_eq!(observation.applied_floor(), floor.current);
    let signing = observation.into_signing_state().unwrap();
    assert_eq!(signing.audit_head, expected.operations.audit);
    assert_eq!(signing.custody.current_anchor, expected.custody_anchor);
    assert_eq!(
        signing.custody.active_head,
        expected.control.active_head.unwrap()
    );
    assert_eq!(signing.custody.now_unix_ms, NOW + 2);
    assert_eq!(signing.custody.anchor_observed_at_unix_ms, NOW);
}

#[test]
fn pending_or_acknowledged_check_without_application_never_produces_signing_state() {
    let f = Fixture::new();
    let mut floor = RetainedFloor::new(&f);
    let pending = pending_current(&f);
    // A signed envelope or transport acknowledgement has no State/Kura execution proof.
    assert!(pending.signed_transaction().verify_signature().is_ok());
    let mut clock = QualifiedClock::fixed();
    assert!(matches!(
        f.observer_transactions()
            .finish_current_with(pending, &mut clock, &mut floor),
        Err(CurrentError::Check)
    ));
    assert_eq!(clock.calls, 0);
    assert_eq!(floor.advances, 0);
}

#[test]
fn current_handoff_rejects_another_subject_and_changed_custody() {
    let mut f = Fixture::new();
    let current = f.receipt_check(None);
    let prepared = f.prepare(current);
    let Executable::Instructions(instructions) = &prepared.payload.instructions else {
        panic!("native Reserve");
    };
    let reserve = f.signed(instructions[0].clone(), 2, NOW);
    assert_eq!(f.native.commit(NOW, vec![reserve]), [true]);
    let view = f.native.state().view();
    let reserved = read_final_promotion_authority_at_v1(
        &view,
        &f.receipt_policy.binding,
        view.height() as u64,
        Some(f.expected.operation_id),
    )
    .unwrap()
    .unwrap()
    .operation
    .unwrap();
    drop(view);
    let mut floor = RetainedFloor::new(&f);
    let prepared = f.prepare_receipt_check(
        Some(FinalPromotionCheckSubjectV1::BeforeProvider(reserved)),
        Duration::from_secs(60),
    );
    let payload = f.observer_payload(prepared.instruction().clone().into());
    let pending = f
        .observer_transactions()
        .sign_receipt_with(prepared, payload, |request| {
            Signature::try_new(key(3).private_key(), request.signing_message())
                .map_err(|_| ObserverError::Provider)
        })
        .unwrap();
    assert_eq!(
        f.native
            .commit(NOW, vec![pending.signed_transaction().clone()]),
        [true]
    );
    let mut clock = QualifiedClock::fixed();
    assert!(matches!(
        f.observer_transactions()
            .finish_current_with(pending, &mut clock, &mut floor),
        Err(CurrentError::Binding)
    ));
    assert_eq!(floor.advances, 0);

    let mut f = Fixture::new();
    let mut floor = RetainedFloor::new(&f);
    let pending = pending_current(&f);
    let view = f.native.state().view();
    let snapshot = read_final_promotion_authority_at_v1(
        &view,
        &f.receipt_policy.binding,
        view.height() as u64,
        None,
    )
    .unwrap()
    .unwrap();
    drop(view);
    let revoke = MutateSorafsFinalPromotionAuthority {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: snapshot.control_record.revision,
        expected_control_digest: snapshot.custody_anchor.state_digest,
        action: FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        }),
    };
    let revoke = f.signed(revoke.into(), 1, NOW);
    assert_eq!(
        f.native
            .commit(NOW, vec![pending.signed_transaction().clone(), revoke]),
        [true, true]
    );
    let mut clock = QualifiedClock::fixed();
    assert!(matches!(
        f.observer_transactions()
            .finish_current_with(pending, &mut clock, &mut floor),
        Err(CurrentError::Check)
    ));
    assert_eq!(floor.advances, 0);
}

#[test]
fn current_handoff_rejects_floor_regression_persistence_failure_and_false_readback() {
    for fault in 0..3 {
        let mut f = Fixture::new();
        let mut floor = RetainedFloor::new(&f);
        let pending = pending_current(&f);
        assert_eq!(
            f.native
                .commit(NOW, vec![pending.signed_transaction().clone()]),
            [true]
        );
        match fault {
            0 => floor.current.block_hash = [0xA5; 32],
            1 => floor.fail_advance = true,
            2 => floor.mismatched_readback = true,
            _ => unreachable!(),
        }
        let mut clock = QualifiedClock::fixed();
        assert!(matches!(
            f.observer_transactions()
                .finish_current_with(pending, &mut clock, &mut floor),
            Err(CurrentError::Floor)
        ));
        assert_eq!(clock.calls, 1, "fault {fault} must fail before reuse time");
        assert_eq!(floor.advances, usize::from(fault != 0));
    }
}

#[test]
fn current_handoff_rejects_backward_widened_and_unavailable_qualified_time() {
    for fault in 0..3 {
        let mut f = Fixture::new();
        let mut floor = RetainedFloor::new(&f);
        let pending = pending_current(&f);
        assert_eq!(
            f.native
                .commit(NOW, vec![pending.signed_transaction().clone()]),
            [true]
        );
        let later = match fault {
            0 => Ok(FinalPromotionEligibilityTimeIntervalV1 {
                earliest_unix_ms: NOW - 1,
                latest_unix_ms: NOW,
            }),
            1 => Ok(FinalPromotionEligibilityTimeIntervalV1 {
                earliest_unix_ms: NOW,
                latest_unix_ms: 100_000,
            }),
            2 => Err(CurrentError::Clock),
            _ => unreachable!(),
        };
        let mut clock = QualifiedClock {
            intervals: VecDeque::from([Ok(times().0), later]),
            calls: 0,
        };
        assert!(matches!(
            f.observer_transactions()
                .finish_current_with(pending, &mut clock, &mut floor),
            Err(CurrentError::Clock)
        ));
        assert_eq!(clock.calls, 2);
        assert_eq!(floor.advances, 1, "durable floor only moves forward");
    }
}

fn current_runtime(f: &Fixture) -> FinalPromotionCurrentCheckRuntimeV1 {
    let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
    let FinalPromotionAuthorityActionV1::Check(check) = &prepared.instruction().action else {
        panic!("native Current Check");
    };
    FinalPromotionCurrentCheckRuntimeV1::new(
        Arc::clone(f.native.state()),
        f.observer_transactions(),
        check.request,
        Duration::from_secs(60),
    )
    .unwrap()
}

struct RuntimePayload {
    state: Arc<State>,
    calls: usize,
    substitute_instruction: bool,
}
impl RuntimePayload {
    fn new(f: &Fixture) -> Self {
        Self {
            state: Arc::clone(f.native.state()),
            calls: 0,
            substitute_instruction: false,
        }
    }
}
impl FinalPromotionCurrentCheckPayloadV1 for RuntimePayload {
    fn prepare(
        &mut self,
        instruction: &MutateSorafsFinalPromotionAuthority,
    ) -> Result<TransactionPayload, CurrentError> {
        self.calls += 1;
        let mut instruction = instruction.clone();
        if self.substitute_instruction {
            let FinalPromotionAuthorityActionV1::Check(check) = &mut instruction.action else {
                panic!("native Check");
            };
            check.challenge = [0xA5; 32];
        }
        let mut builder = TransactionBuilder::new(
            *self.state.network_id_ref(),
            account(3),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(NOW));
        builder
            .with_instructions([instruction])
            .into_payload()
            .map_err(|_| CurrentError::Payload)
    }
}

struct RuntimeSigner {
    calls: usize,
    seed: u8,
}
impl FinalPromotionCurrentCheckSignerV1 for RuntimeSigner {
    fn sign(
        &mut self,
        request: &FinalPromotionObserverKeyRequestV1<'_>,
    ) -> Result<Signature, ObserverError> {
        self.calls += 1;
        Signature::try_new(key(self.seed).private_key(), request.signing_message())
            .map_err(|_| ObserverError::Provider)
    }
}

enum SubmissionMode {
    Applied,
    Rejected,
    AcceptedWithoutApplication,
    AmbiguousThenApplied,
    AmbiguousReconciledWithoutApplication,
    AmbiguousUnresolved,
    AdvanceAudit {
        reserve: SignedTransaction,
        binding: SignerCustodyBindingV1,
        operation_id: [u8; 32],
    },
}
struct RuntimeSubmission<'a> {
    native: &'a mut NativeCheckTestFixtureV1,
    mode: SubmissionMode,
    submitted: Option<SignedTransaction>,
    submits: usize,
    reconciles: usize,
}
impl<'a> RuntimeSubmission<'a> {
    fn new(native: &'a mut NativeCheckTestFixtureV1, mode: SubmissionMode) -> Self {
        Self {
            native,
            mode,
            submitted: None,
            submits: 0,
            reconciles: 0,
        }
    }
}
impl FinalPromotionCurrentCheckSubmissionV1 for RuntimeSubmission<'_> {
    fn submit_exact(
        &mut self,
        transaction: &SignedTransaction,
    ) -> Result<FinalPromotionCurrentCheckSubmitOutcomeV1, CurrentError> {
        self.submits += 1;
        self.submitted = Some(transaction.clone());
        match &self.mode {
            SubmissionMode::Applied => {
                assert_eq!(self.native.commit(NOW, vec![transaction.clone()]), [true]);
                Ok(FinalPromotionCurrentCheckSubmitOutcomeV1::Accepted)
            }
            SubmissionMode::Rejected => {
                assert_eq!(self.native.commit(NOW, vec![transaction.clone()]), [false]);
                Ok(FinalPromotionCurrentCheckSubmitOutcomeV1::Accepted)
            }
            SubmissionMode::AcceptedWithoutApplication => {
                Ok(FinalPromotionCurrentCheckSubmitOutcomeV1::Accepted)
            }
            SubmissionMode::AmbiguousThenApplied
            | SubmissionMode::AmbiguousReconciledWithoutApplication
            | SubmissionMode::AmbiguousUnresolved => {
                Ok(FinalPromotionCurrentCheckSubmitOutcomeV1::Ambiguous)
            }
            SubmissionMode::AdvanceAudit {
                reserve,
                binding,
                operation_id,
            } => {
                assert_eq!(self.native.commit(NOW, vec![reserve.clone()]), [true]);
                let view = self.native.state().view();
                let reserved = read_final_promotion_authority_at_v1(
                    &view,
                    binding,
                    view.height() as u64,
                    Some(*operation_id),
                )
                .unwrap()
                .unwrap();
                let row = reserved.operation.unwrap();
                drop(view);
                let complete = MutateSorafsFinalPromotionAuthority {
                    deployment_id: DEPLOYMENT.into(),
                    expected_control_revision: reserved.control_record.revision,
                    expected_control_digest: reserved.custody_anchor.state_digest,
                    action: FinalPromotionAuthorityActionV1::Complete(FinalPromotionCompleteV1 {
                        intent: row.intent,
                        custody: row.custody,
                        reservation: row.reservation,
                        commitment: SignerOperationCommitmentV1 {
                            audit: SignerOperationAuditHeadV1 {
                                sequence: row.intent.previous_audit.sequence + 1,
                                digest: [0xA4; 32],
                            },
                            response_digest: [0xA5; 32],
                        },
                        signatures_digest: [0xA6; 32],
                    }),
                };
                let mut builder = TransactionBuilder::new(
                    *self.native.state().network_id_ref(),
                    account(2),
                    FeePaymentIntent::authority(Vec::new(), None),
                );
                builder.set_creation_time(Duration::from_millis(NOW));
                let completed = builder
                    .with_instructions([complete])
                    .try_sign(key(2).private_key())
                    .unwrap();
                assert_eq!(self.native.commit(NOW, vec![completed]), [true]);
                assert_eq!(self.native.commit(NOW, vec![transaction.clone()]), [false]);
                Ok(FinalPromotionCurrentCheckSubmitOutcomeV1::Accepted)
            }
        }
    }

    fn reconcile_exact(&mut self, transaction: &SignedTransaction) -> Result<(), CurrentError> {
        self.reconciles += 1;
        assert_eq!(self.submitted.as_ref(), Some(transaction));
        match &self.mode {
            SubmissionMode::AmbiguousThenApplied => {
                assert_eq!(self.native.commit(NOW, vec![transaction.clone()]), [true]);
                Ok(())
            }
            SubmissionMode::AmbiguousReconciledWithoutApplication => Ok(()),
            SubmissionMode::AmbiguousUnresolved => Err(CurrentError::Submission),
            _ => panic!("only ambiguous submission may reconcile"),
        }
    }
}

#[test]
fn current_runtime_uses_original_signed_envelope_through_ambiguous_reconciliation() {
    for mode in [
        SubmissionMode::Applied,
        SubmissionMode::AmbiguousThenApplied,
    ] {
        let mut f = Fixture::new();
        let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
        let FinalPromotionAuthorityActionV1::Check(reviewed) = &prepared.instruction().action
        else {
            panic!("native Current Check");
        };
        let reviewed_request = reviewed.request;
        let runtime = current_runtime(&f);
        let mut payload = RuntimePayload::new(&f);
        let mut signer = RuntimeSigner { calls: 0, seed: 3 };
        let mut floor = RetainedFloor::new(&f);
        let mut clock = QualifiedClock::fixed();
        let mut submission = RuntimeSubmission::new(&mut f.native, mode);
        let observation = runtime
            .observe_current_with(
                &mut payload,
                &mut signer,
                &mut submission,
                &mut clock,
                &mut floor,
            )
            .unwrap();
        assert_eq!(submission.submits, 1);
        assert_eq!(
            submission.reconciles,
            usize::from(matches!(
                &submission.mode,
                SubmissionMode::AmbiguousThenApplied
            ))
        );
        assert_eq!(payload.calls, 1);
        assert_eq!(signer.calls, 1);
        assert_eq!(clock.calls, 2);
        assert_eq!(floor.advances, 1);
        let signed = submission.submitted.as_ref().unwrap();
        let Executable::Instructions(instructions) = &signed.payload().instructions else {
            panic!("signed native Check");
        };
        let instruction = instructions[0]
            .as_any()
            .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
            .unwrap();
        let FinalPromotionAuthorityActionV1::Check(check) = &instruction.action else {
            panic!("signed native Current Check");
        };
        assert_eq!(check.request, reviewed_request);
        let state = observation.into_signing_state().unwrap();
        assert_eq!(state.audit_head.sequence, 0);
        assert_eq!(state.custody.current_anchor.height, floor.current.height);
    }
}

#[test]
fn current_runtime_rejects_unknown_or_unapplied_submission_without_finalized_application() {
    for mode in [
        SubmissionMode::AcceptedWithoutApplication,
        SubmissionMode::AmbiguousReconciledWithoutApplication,
        SubmissionMode::AmbiguousUnresolved,
    ] {
        let mut f = Fixture::new();
        let runtime = current_runtime(&f);
        let mut payload = RuntimePayload::new(&f);
        let mut signer = RuntimeSigner { calls: 0, seed: 3 };
        let mut floor = RetainedFloor::new(&f);
        let mut clock = QualifiedClock::fixed();
        let mut submission = RuntimeSubmission::new(&mut f.native, mode);
        let result = runtime.observe_current_with(
            &mut payload,
            &mut signer,
            &mut submission,
            &mut clock,
            &mut floor,
        );
        let expected_error = if matches!(&submission.mode, SubmissionMode::AmbiguousUnresolved) {
            CurrentError::Submission
        } else {
            CurrentError::Check
        };
        assert_eq!(result.err(), Some(expected_error));
        assert_eq!(submission.submits, 1);
        assert_eq!(
            submission.reconciles,
            usize::from(matches!(
                &submission.mode,
                SubmissionMode::AmbiguousReconciledWithoutApplication
                    | SubmissionMode::AmbiguousUnresolved
            ))
        );
        assert_eq!(floor.advances, 0);
        assert_eq!(clock.calls, 0);
    }
}

#[test]
fn current_runtime_rejects_payload_and_observer_substitution_before_submission() {
    for fault in 0..2 {
        let mut f = Fixture::new();
        let runtime = current_runtime(&f);
        let mut payload = RuntimePayload::new(&f);
        payload.substitute_instruction = fault == 0;
        let mut signer = RuntimeSigner {
            calls: 0,
            seed: if fault == 0 { 3 } else { 4 },
        };
        let mut floor = RetainedFloor::new(&f);
        let mut clock = QualifiedClock::fixed();
        let mut submission = RuntimeSubmission::new(&mut f.native, SubmissionMode::Applied);
        let result = runtime.observe_current_with(
            &mut payload,
            &mut signer,
            &mut submission,
            &mut clock,
            &mut floor,
        );
        assert!(matches!(
            result,
            Err(CurrentError::Payload | CurrentError::Provider)
        ));
        assert_eq!(signer.calls, usize::from(fault != 0));
        assert_eq!(submission.submits, 0);
        assert_eq!(floor.advances, 0);
    }
}

#[test]
fn current_runtime_rejects_changed_custody_audit_and_rollback() {
    let mut f = Fixture::new();
    let runtime = current_runtime(&f);
    let mut floor = RetainedFloor::new(&f);
    let view = f.native.state().view();
    let snapshot = read_final_promotion_authority_at_v1(
        &view,
        &f.receipt_policy.binding,
        view.height() as u64,
        None,
    )
    .unwrap()
    .unwrap();
    drop(view);
    let revoke = MutateSorafsFinalPromotionAuthority {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: snapshot.control_record.revision,
        expected_control_digest: snapshot.custody_anchor.state_digest,
        action: FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        }),
    };
    let revoke = f.signed(revoke.into(), 1, NOW);
    assert_eq!(f.native.commit(NOW, vec![revoke]), [true]);
    let mut payload = RuntimePayload::new(&f);
    let mut signer = RuntimeSigner { calls: 0, seed: 3 };
    let mut submission = RuntimeSubmission::new(&mut f.native, SubmissionMode::Applied);
    let mut clock = QualifiedClock::fixed();
    assert!(matches!(
        runtime.observe_current_with(
            &mut payload,
            &mut signer,
            &mut submission,
            &mut clock,
            &mut floor,
        ),
        Err(CurrentError::Custody)
    ));
    assert_eq!(payload.calls, 0);
    assert_eq!(submission.submits, 0);

    let mut f = Fixture::new();
    let receipt = f.receipt_check(None);
    let prepared = f.prepare(receipt);
    let Executable::Instructions(instructions) = &prepared.payload.instructions else {
        panic!("native Reserve");
    };
    let reserve = f.signed(instructions[0].clone(), 2, NOW);
    let runtime = current_runtime(&f);
    let mut payload = RuntimePayload::new(&f);
    let mut signer = RuntimeSigner { calls: 0, seed: 3 };
    let mut floor = RetainedFloor::new(&f);
    let mut clock = QualifiedClock::fixed();
    let binding = f.receipt_policy.binding.clone();
    let operation_id = f.expected.operation_id;
    let mut submission = RuntimeSubmission::new(
        &mut f.native,
        SubmissionMode::AdvanceAudit {
            reserve,
            binding,
            operation_id,
        },
    );
    assert!(matches!(
        runtime.observe_current_with(
            &mut payload,
            &mut signer,
            &mut submission,
            &mut clock,
            &mut floor,
        ),
        Err(CurrentError::Check)
    ));
    assert_eq!(floor.advances, 0);

    let mut f = Fixture::new();
    let runtime = current_runtime(&f);
    let mut payload = RuntimePayload::new(&f);
    let mut signer = RuntimeSigner { calls: 0, seed: 3 };
    let mut floor = RetainedFloor::new(&f);
    floor.current.block_hash = [0xA5; 32];
    let mut clock = QualifiedClock::fixed();
    let mut submission = RuntimeSubmission::new(&mut f.native, SubmissionMode::Rejected);
    assert!(
        runtime
            .observe_current_with(
                &mut payload,
                &mut signer,
                &mut submission,
                &mut clock,
                &mut floor,
            )
            .is_err()
    );
    assert_eq!(floor.advances, 0);
}

#[test]
fn current_runtime_rejects_invalid_reviewed_request_at_construction() {
    let f = Fixture::new();
    let prepared = f.prepare_receipt_check(None, Duration::from_secs(60));
    let FinalPromotionAuthorityActionV1::Check(check) = &prepared.instruction().action else {
        panic!("native Current Check");
    };
    let mut request = check.request;
    request.binding_digest = [0xA5; 32];
    assert!(matches!(
        FinalPromotionCurrentCheckRuntimeV1::new(
            Arc::clone(f.native.state()),
            f.observer_transactions(),
            request,
            Duration::from_secs(60),
        ),
        Err(CurrentError::Binding)
    ));
}
