//! Bounded ephemeral coordination for finalized native repair execution.
//!
//! This gate suppresses concurrent execution of the same finalized task and
//! caps process-local work. It carries no task state or authority: the set is
//! rebuilt empty on restart, while leases and terminal outcomes remain solely
//! chain-authoritative.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use thiserror::Error;

#[derive(Debug)]
struct NativeRepairSingleflightStateV1 {
    max_inflight: usize,
    task_ids: Mutex<BTreeSet<[u8; 32]>>,
}

/// Bounded process-local single-flight gate for native repair I/O.
#[derive(Debug, Clone)]
pub(crate) struct NativeRepairSingleflightV1 {
    state: Arc<NativeRepairSingleflightStateV1>,
}

impl NativeRepairSingleflightV1 {
    /// Construct an empty gate with a non-zero process-local concurrency bound.
    pub(crate) fn new(max_inflight: usize) -> Self {
        Self {
            state: Arc::new(NativeRepairSingleflightStateV1 {
                max_inflight: max_inflight.max(1),
                task_ids: Mutex::new(BTreeSet::new()),
            }),
        }
    }

    /// Enter one task execution until the returned guard is dropped.
    pub(crate) fn try_enter(
        &self,
        task_id: [u8; 32],
    ) -> Result<NativeRepairSingleflightGuardV1, NativeRepairSingleflightErrorV1> {
        if task_id == [0; 32] {
            return Err(NativeRepairSingleflightErrorV1::InvalidTaskId);
        }
        let mut task_ids = self
            .state
            .task_ids
            .lock()
            .map_err(|_| NativeRepairSingleflightErrorV1::Poisoned)?;
        if task_ids.contains(&task_id) {
            return Err(NativeRepairSingleflightErrorV1::AlreadyInFlight);
        }
        if task_ids.len() >= self.state.max_inflight {
            return Err(NativeRepairSingleflightErrorV1::AtCapacity);
        }
        task_ids.insert(task_id);
        drop(task_ids);
        Ok(NativeRepairSingleflightGuardV1 {
            state: Arc::clone(&self.state),
            task_id,
        })
    }

    #[cfg(test)]
    fn inflight(&self) -> usize {
        self.state
            .task_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

/// Rejection from the non-authoritative native repair coordination gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum NativeRepairSingleflightErrorV1 {
    /// The task identity is not a valid native repair identity.
    #[error("native repair task identity is invalid")]
    InvalidTaskId,
    /// The same task is already executing in this process.
    #[error("native repair task is already executing")]
    AlreadyInFlight,
    /// Distinct native repair executions reached the configured process bound.
    #[error("native repair execution concurrency is saturated")]
    AtCapacity,
    /// The ephemeral coordination lock is poisoned.
    #[error("native repair execution coordination is unavailable")]
    Poisoned,
}

/// RAII token that removes its task from the ephemeral set on every exit path.
#[derive(Debug)]
pub(crate) struct NativeRepairSingleflightGuardV1 {
    state: Arc<NativeRepairSingleflightStateV1>,
    task_id: [u8; 32],
}

impl Drop for NativeRepairSingleflightGuardV1 {
    fn drop(&mut self) {
        self.state
            .task_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.task_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_task_is_suppressed_until_guard_drop() {
        let gate = NativeRepairSingleflightV1::new(2);
        let guard = gate.try_enter([1; 32]).expect("first task enters");
        assert_eq!(
            gate.try_enter([1; 32]).unwrap_err(),
            NativeRepairSingleflightErrorV1::AlreadyInFlight
        );
        assert_eq!(gate.inflight(), 1);
        drop(guard);
        assert_eq!(gate.inflight(), 0);
        gate.try_enter([1; 32])
            .expect("task re-enters after guard drop");
    }

    #[test]
    fn distinct_tasks_are_bounded_without_becoming_authority() {
        let gate = NativeRepairSingleflightV1::new(2);
        let _first = gate.try_enter([1; 32]).expect("first task enters");
        let _second = gate.try_enter([2; 32]).expect("second task enters");
        assert_eq!(
            gate.try_enter([3; 32]).unwrap_err(),
            NativeRepairSingleflightErrorV1::AtCapacity
        );
        assert_eq!(
            gate.try_enter([0; 32]).unwrap_err(),
            NativeRepairSingleflightErrorV1::InvalidTaskId
        );
    }

    #[test]
    fn unwind_drops_ephemeral_task_membership() {
        let gate = NativeRepairSingleflightV1::new(1);
        let caught = std::panic::catch_unwind({
            let gate = gate.clone();
            move || {
                let _guard = gate.try_enter([7; 32]).expect("task enters");
                panic!("exercise RAII cleanup");
            }
        });
        assert!(caught.is_err());
        assert_eq!(gate.inflight(), 0);
        gate.try_enter([8; 32])
            .expect("capacity is released after unwind");
    }
}
