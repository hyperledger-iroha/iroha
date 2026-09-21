//! Canonical original-session admission for native MKHE named resources.
//!
//! The original source session retains this single ledger. Its existing work
//! counter accepts explicitly accounted primitive operations; workspace is the
//! actual retained and scratch ownership requested before allocation. The qPCS
//! prototype and fixed u15 kernel share this owner without either depending on
//! the other. No caller can clone/reset a budget or substitute a child's ledger.
//! TODO: account for earlier source/control allocations and define reviewed
//! common arithmetic work units across the complete prover. Named-buffer
//! accounting is not whole-proof qualification or process RSS.

use std::sync::{Arc, Mutex, MutexGuard};

use super::rns_native_profile::{
    ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1, ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeResourceErrorV1 {
    ArithmeticOverflow,
    WorkLimit,
    WorkspaceLimit,
    LedgerPoisoned,
}

/// Original-session work and live named-buffer ledger. Failed admission is
/// atomic; work is never refunded. Each consuming owner releases only its own
/// retained bytes. Complete proof work units and prior allocations remain open.
#[derive(Debug, Default)]
pub(super) struct RnsNativeProofResourceBudgetV1 {
    usage: Arc<Mutex<RnsNativeResourceUsageV1>>,
}

#[derive(Debug, Default)]
struct RnsNativeResourceUsageV1 {
    consumed_work: u64,
    live_bytes: u64,
    peak_bytes: u64,
    #[cfg(test)]
    test_workspace_limit: Option<u64>,
}

fn release_reserved_bytes_v1(usage: &Mutex<RnsNativeResourceUsageV1>, bytes: u64) {
    // Recovery is restricted to releasing a reservation already owned before
    // poison. Never clear poison or use a recovered guard for new admission.
    let mut usage = usage
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    usage.live_bytes -= bytes;
}

impl RnsNativeProofResourceBudgetV1 {
    #[cfg(test)]
    pub(super) fn with_test_workspace_limit_v1(limit: u64) -> Self {
        assert!(limit <= ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1);
        let budget = Self::default();
        budget.set_test_workspace_limit_v1(limit);
        budget
    }

    #[cfg(test)]
    pub(super) fn set_test_workspace_limit_v1(&self, limit: u64) {
        assert!(limit <= ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1);
        let mut usage = self.usage.lock().unwrap();
        assert_eq!(usage.live_bytes, 0);
        usage.test_workspace_limit = Some(limit);
    }

    fn usage(&self) -> Result<MutexGuard<'_, RnsNativeResourceUsageV1>, RnsNativeResourceErrorV1> {
        self.usage
            .lock()
            .map_err(|_| RnsNativeResourceErrorV1::LedgerPoisoned)
    }

    #[cfg(test)]
    pub(super) fn consumed(&self) -> Result<u64, RnsNativeResourceErrorV1> {
        Ok(self.usage()?.consumed_work)
    }

    #[cfg(test)]
    pub(super) fn live_bytes(&self) -> Result<u64, RnsNativeResourceErrorV1> {
        Ok(self.usage()?.live_bytes)
    }

    #[cfg(test)]
    pub(super) fn peak_bytes(&self) -> Result<u64, RnsNativeResourceErrorV1> {
        Ok(self.usage()?.peak_bytes)
    }

    /// Debit explicit primitive operations without a retained buffer.
    #[cfg(test)]
    pub(super) fn charge(&mut self, operations: u64) -> Result<(), RnsNativeResourceErrorV1> {
        let _reservation = self.admit(operations, 0, 0)?;
        Ok(())
    }

    /// Reserve actual named workspace under this same ledger. This does not
    /// invent a conversion between T256 work and the tree's hash work units.
    pub(super) fn reserve_workspace_v1(
        &mut self,
        retained_bytes: u64,
        scratch_bytes: u64,
    ) -> Result<RnsNativeResourceReservationV1, RnsNativeResourceErrorV1> {
        self.admit(0, retained_bytes, scratch_bytes)
    }

    pub(super) fn admit(
        &mut self,
        operations: u64,
        retained_bytes: u64,
        scratch_bytes: u64,
    ) -> Result<RnsNativeResourceReservationV1, RnsNativeResourceErrorV1> {
        let bytes = retained_bytes
            .checked_add(scratch_bytes)
            .ok_or(RnsNativeResourceErrorV1::ArithmeticOverflow)?;
        let mut usage = self.usage()?;
        let next_work = usage
            .consumed_work
            .checked_add(operations)
            .ok_or(RnsNativeResourceErrorV1::ArithmeticOverflow)?;
        let next_bytes = usage
            .live_bytes
            .checked_add(bytes)
            .ok_or(RnsNativeResourceErrorV1::ArithmeticOverflow)?;
        if next_work > ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1 {
            return Err(RnsNativeResourceErrorV1::WorkLimit);
        }
        #[cfg(test)]
        let workspace_limit = usage
            .test_workspace_limit
            .unwrap_or(ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1);
        #[cfg(not(test))]
        let workspace_limit = ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1;
        if next_bytes > workspace_limit {
            return Err(RnsNativeResourceErrorV1::WorkspaceLimit);
        }
        usage.consumed_work = next_work;
        usage.live_bytes = next_bytes;
        usage.peak_bytes = usage.peak_bytes.max(next_bytes);
        Ok(RnsNativeResourceReservationV1 {
            usage: Arc::clone(&self.usage),
            retained_bytes,
            scratch_bytes,
        })
    }
}

/// Move-only reservation stays with the named allocation owner it accounts.
#[derive(Debug)]
pub(super) struct RnsNativeResourceReservationV1 {
    usage: Arc<Mutex<RnsNativeResourceUsageV1>>,
    retained_bytes: u64,
    scratch_bytes: u64,
}

impl RnsNativeResourceReservationV1 {
    pub(super) fn belongs_to_v1(&self, budget: &RnsNativeProofResourceBudgetV1) -> bool {
        Arc::ptr_eq(&self.usage, &budget.usage)
    }

    /// A retained owner may reserve a child only from its existing ledger.
    /// This private temporary issuer shares counters; it never creates/reset them.
    pub(super) fn reserve_child_workspace_v1(
        &self,
        retained_bytes: u64,
        scratch_bytes: u64,
    ) -> Result<Self, RnsNativeResourceErrorV1> {
        let mut original = RnsNativeProofResourceBudgetV1 {
            usage: Arc::clone(&self.usage),
        };
        original.reserve_workspace_v1(retained_bytes, scratch_bytes)
    }

    pub(super) fn release_scratch(&mut self) {
        let scratch = std::mem::take(&mut self.scratch_bytes);
        // Called only after the exact construction scratch has been destroyed.
        release_reserved_bytes_v1(&self.usage, scratch);
    }
}

impl Drop for RnsNativeResourceReservationV1 {
    fn drop(&mut self) {
        release_reserved_bytes_v1(&self.usage, self.retained_bytes + self.scratch_bytes);
    }
}

#[cfg(test)]
#[path = "rns_native_resource_budget_tests.rs"]
mod tests;
