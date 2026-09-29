//! Snapshot startup classification preserves local failures and their retry owner.

use super::TryReadSnapshotError;

pub(super) fn snapshot_read_error_is_recoverable(error: &TryReadSnapshotError) -> bool {
    match error {
        TryReadSnapshotError::IO(_, _)
        | TryReadSnapshotError::PayloadAllocation(_)
        | TryReadSnapshotError::PayloadAllocatorFailure { .. }
        | TryReadSnapshotError::StateVmInitialization(_)
        | TryReadSnapshotError::StateAdmission(_)
        | TryReadSnapshotError::StateExecutionDeferred(_)
        | TryReadSnapshotError::StateNativeSchedule(_)
        | TryReadSnapshotError::ChainIdMismatch { .. }
        | TryReadSnapshotError::NetworkIdMismatch { .. }
        | TryReadSnapshotError::ZkConfigInstall(_) => false,
        // The caller additionally requires Strict mode and a complete genesis-backed
        // prefix, then node::prepare executes original signed genesis and every certified block.
        TryReadSnapshotError::NativeExecutionReplayRequired => true,
        TryReadSnapshotError::MismatchedHeight { .. } => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot_failure_allows_empty_state_fallback;
    use mv::allocation::{AllocationBudget, AllocationRefusal};
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    #[test]
    fn native_identity_mismatch_halts_but_execution_replay_requires_strict_mode() {
        let wrong_chain = TryReadSnapshotError::ChainIdMismatch {
            expected: "configured-native-chain".parse().unwrap(),
            actual: "foreign-native-chain".parse().unwrap(),
        };
        assert!(!snapshot_failure_allows_empty_state_fallback(
            &wrong_chain,
            false
        ));
        let replay = TryReadSnapshotError::NativeExecutionReplayRequired;
        assert!(snapshot_failure_allows_empty_state_fallback(&replay, false));
        assert!(!snapshot_failure_allows_empty_state_fallback(&replay, true));
    }

    #[test]
    fn snapshot_local_refusal_never_authorizes_empty_state_fallback() {
        let budget = AllocationBudget::new(1);
        let _occupied = budget.try_reserve_bytes(1).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        for error in [
            TryReadSnapshotError::PayloadAllocation(refusal.clone()),
            TryReadSnapshotError::PayloadAllocation(AllocationRefusal::DemandOverflow),
            TryReadSnapshotError::PayloadAllocatorFailure { requested_bytes: 1 },
            TryReadSnapshotError::StateVmInitialization(ivm::VMError::AllocationDeferred(
                refusal.clone(),
            )),
            TryReadSnapshotError::StateVmInitialization(ivm::VMError::ExecutionDeferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable,
            )),
            TryReadSnapshotError::StateAdmission(iroha_core::state::StateAdmissionError::History(
                iroha_core::state::BlockHashAdmissionError::Capacity(refusal.clone()),
            )),
            TryReadSnapshotError::StateNativeSchedule(
                iroha_core::sumeragi::schedule::ScheduleError::Admission(refusal.clone()),
            ),
            TryReadSnapshotError::StateNativeSchedule(
                iroha_core::sumeragi::schedule::ScheduleError::Allocator { requested_bytes: 1 },
            ),
            TryReadSnapshotError::StateExecutionDeferred(refusal.into()),
            TryReadSnapshotError::StateExecutionDeferred(
                ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
            ),
        ] {
            assert!(!snapshot_read_error_is_recoverable(&error));
            for emergency_fast in [false, true] {
                assert!(!snapshot_failure_allows_empty_state_fallback(
                    &error,
                    emergency_fast
                ));
            }
        }
    }

    #[test]
    fn classification_borrows_raw_failure_and_preserves_original_release_observation() {
        for kind in 0..5 {
            let budget = AllocationBudget::new(1);
            let occupied = budget.try_reserve_bytes(1).unwrap();
            let refusal = budget.try_reserve_bytes(1).unwrap_err();
            let error = match kind {
                0 => TryReadSnapshotError::PayloadAllocation(refusal),
                1 => TryReadSnapshotError::StateVmInitialization(ivm::VMError::AllocationDeferred(
                    refusal,
                )),
                2 => TryReadSnapshotError::StateAdmission(
                    iroha_core::state::StateAdmissionError::History(
                        iroha_core::state::BlockHashAdmissionError::Capacity(refusal),
                    ),
                ),
                3 => TryReadSnapshotError::StateExecutionDeferred(refusal.into()),
                _ => TryReadSnapshotError::StateNativeSchedule(
                    iroha_core::sumeragi::schedule::ScheduleError::Admission(refusal),
                ),
            };
            assert!(!snapshot_failure_allows_empty_state_fallback(&error, false));
            let refusal = match &error {
                TryReadSnapshotError::PayloadAllocation(refusal)
                | TryReadSnapshotError::StateNativeSchedule(
                    iroha_core::sumeragi::schedule::ScheduleError::Admission(refusal),
                )
                | TryReadSnapshotError::StateVmInitialization(ivm::VMError::AllocationDeferred(
                    refusal,
                ))
                | TryReadSnapshotError::StateAdmission(
                    iroha_core::state::StateAdmissionError::History(
                        iroha_core::state::BlockHashAdmissionError::Capacity(refusal),
                    ),
                ) => refusal,
                TryReadSnapshotError::StateExecutionDeferred(reason) => {
                    reason.allocation_refusal().unwrap()
                }
                _ => unreachable!("fixture retains a typed capacity failure"),
            };
            let AllocationRefusal::Capacity {
                requested_bytes,
                reserved_bytes,
                limit_bytes,
                release,
            } = refusal
            else {
                panic!("classification must preserve the raw original capacity refusal");
            };
            assert_eq!((*requested_bytes, *reserved_bytes, *limit_bytes), (1, 1, 1));
            let mut wait = pin!(release.clone().wait_for_release());
            let mut context = Context::from_waker(Waker::noop());
            assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
            let unrelated = AllocationBudget::new(1);
            drop(unrelated.try_reserve_bytes(1).unwrap());
            assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
            drop(occupied);
            assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
        }
    }
}
