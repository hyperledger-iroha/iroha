mod native_preparation_errors {
    use super::*;
    use crate::{
        block::valid::NativeCandidatePreparationError,
        state::{BlockHashAdmissionError, MergeLedgerCommitError},
        sumeragi::v2_body_store::{BodyValidationRejectionIdentity, LocalValidationRefusal},
    };
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    #[test]
    fn hash_admission_retains_original_release_and_runner_through_all_native_origins() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        fixture.service.queue.set_sumeragi_wake(sender);
        for origin in 0..3 {
            for dependency in 0..3 {
                let release = concread::release::ReleaseNotification::default();
                let foreign = concread::release::ReleaseNotification::default();
                let pool = mv::allocation::AllocationBudget::new(1);
                let occupied = pool.try_reserve_bytes(1).unwrap();
                let refusal = match dependency {
                    0 => BlockHashAdmissionError::Busy(release.observe()),
                    1 => BlockHashAdmissionError::Changed(release.observe()),
                    _ => {
                        BlockHashAdmissionError::Capacity(pool.try_reserve_bytes(1).err().unwrap())
                    }
                };
                let expected = refusal.release_wait().unwrap().clone();
                let error = match origin {
                    0 => NativeCandidatePreparationError::Preflight(Box::new(
                        BlockValidationError::BlockHashAdmission(refusal),
                    )),
                    1 => NativeCandidatePreparationError::Execution(
                        MergeLedgerCommitError::BlockHashAdmission(refusal),
                    ),
                    _ => NativeCandidatePreparationError::Execution(
                        MergeLedgerCommitError::NativeControlValidation(Box::new(
                            BlockValidationError::BlockHashAdmission(refusal),
                        )),
                    ),
                };
                let classified = fixture
                    .service
                    .classify_native_preparation_failure(&fixture.body, error);
                assert!(classified.rejection_identity().is_none());
                assert!(!classified.requires_restart_recovery());
                let Some(LocalValidationRefusal::PhysicalBusy(busy)) = classified.local_refusal()
                else {
                    panic!("Native origin {origin} lost its exact hash dependency: {classified:?}");
                };
                assert_eq!(busy.resource, "block_hash_history");
                assert_eq!(busy.wait, expected);
                let mut wait = busy.wait.clone().wait_for_release();
                assert_eq!(
                    Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                    Poll::Pending
                );
                drop(foreign.guard(()));
                assert_eq!(
                    Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                    Poll::Pending
                );
                assert!(receiver.try_recv().is_err());
                if dependency == 2 {
                    drop(release.guard(()));
                    assert_eq!(
                        Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                        Poll::Pending
                    );
                    assert!(receiver.try_recv().is_err());
                    drop(occupied);
                } else {
                    drop(occupied);
                    assert_eq!(
                        Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                        Poll::Pending
                    );
                    assert!(receiver.try_recv().is_err());
                    drop(release.guard(()));
                }
                receiver
                    .try_recv()
                    .expect("original release wakes the original runner");
                assert_eq!(
                    Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                    Poll::Ready(())
                );
                assert!(receiver.try_recv().is_err());
            }
        }
    }

    #[test]
    fn native_controls_preserve_local_storage_failure_and_semantic_rejection() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        for native_control in [false, true] {
            for local in [false, true] {
                let error = if local {
                    BlockValidationError::LocalStorageRecoveryRequired {
                        reason: "original Native control storage requires recovery".to_owned(),
                    }
                } else {
                    BlockValidationError::EmptyBlock
                };
                let wrapped = if native_control {
                    NativeCandidatePreparationError::Execution(
                        MergeLedgerCommitError::NativeControlValidation(Box::new(error)),
                    )
                } else {
                    NativeCandidatePreparationError::Preflight(Box::new(error))
                };
                let classified = fixture
                    .service
                    .classify_native_preparation_failure(&fixture.body, wrapped);
                if local {
                    assert!(classified.rejection_identity().is_none());
                    assert!(classified.requires_restart_recovery());
                    assert!(matches!(classified.local_refusal(),
                        Some(LocalValidationRefusal::RecoveryRequired(reason))
                        if reason == "original Native control storage requires recovery"));
                } else {
                    assert_eq!(
                        classified.rejection_identity(),
                        Some(BodyValidationRejectionIdentity::Rejected)
                    );
                    assert!(!classified.requires_restart_recovery());
                    assert!(classified.local_refusal().is_none());
                }
            }
        }
    }

    #[test]
    fn metadata_and_recorder_diagnostics_cannot_authorize_negative_markers() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        for error in [
            NativeCandidatePreparationError::Preparation(
                "empty block: local metadata capture".to_owned(),
            ),
            NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::ExecutionRecorderConflict(
                    "original recorder is already owned".to_owned(),
                ),
            ),
            NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::ExecutionBatchInvalid(
                    "local source preparation has no typed semantic verdict".to_owned(),
                ),
            ),
            NativeCandidatePreparationError::Execution(MergeLedgerCommitError::BlockHashAdmission(
                BlockHashAdmissionError::Poisoned,
            )),
        ] {
            let classified = fixture
                .service
                .classify_native_preparation_failure(&fixture.body, error);
            assert!(classified.rejection_identity().is_none());
            assert!(classified.requires_restart_recovery());
            assert!(matches!(
                classified.local_refusal(),
                Some(LocalValidationRefusal::RecoveryRequired(_))
            ));
        }
    }

    #[test]
    fn governed_native_batch_limit_remains_a_semantic_body_verdict() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let error = NativeCandidatePreparationError::Execution(
            MergeLedgerCommitError::ExecutionBatchFull {
                fitting_prefix: 1,
                gas_limit: 100,
                gas_used: 80,
            },
        );
        let classified = fixture
            .service
            .classify_native_preparation_failure(&fixture.body, error);
        assert_eq!(
            classified.rejection_identity(),
            Some(BodyValidationRejectionIdentity::Rejected)
        );
        assert!(classified.local_refusal().is_none());
        assert!(!classified.requires_restart_recovery());
        assert!(
            classified
                .to_string()
                .contains("full after 1 inputs (limit=100, used=80)")
        );
    }
}
