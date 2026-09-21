//! Shared session reservations enforced by the actual ordered-spool owners.
//!
//! Spool usage is the sum of live detached file lengths, including tags. I/O
//! counts authenticated record bytes authorized before leaf calls; failed or
//! partial operations retain their full conservative charge. Kernel metadata,
//! caching, retries below the file API, allocator overhead and this Arc/Mutex
//! control allocation are outside these counters. This does not qualify RSS.
//! TODO: carry this same session ledger through every production source/qPCS
//! spool owner. The ordered-pair integration alone is not whole-proof evidence.
use std::sync::{Arc, Mutex, MutexGuard};

use crate::vega::zk_ams::mkhe::rns_native_profile::{
    ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum StorageBudgetErrorV1 {
    Overflow,
    SpoolLimit,
    IoLimit,
    Poisoned,
    Order,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct StorageUsageV1 {
    pub(in super::super) live_spool_bytes: u64,
    pub(in super::super) peak_spool_bytes: u64,
    pub(in super::super) reserved_io_bytes: u64,
    pub(in super::super) attempted_io_bytes: u64,
}

struct StorageLedgerV1 {
    usage: Mutex<StorageUsageV1>,
    spool_limit: u64,
    io_limit: u64,
}

impl StorageLedgerV1 {
    fn lock_v1(&self) -> Result<MutexGuard<'_, StorageUsageV1>, StorageBudgetErrorV1> {
        self.usage
            .lock()
            .map_err(|_| StorageBudgetErrorV1::Poisoned)
    }
}

/// Sole admission issuer for all ordered spools of one caller-owned session.
/// Reservations retain its ledger; the issuer itself has no Clone or reset API.
pub(in crate::vega::zk_ams::mkhe) struct OrderedStorageSessionBudgetV1 {
    ledger: Arc<StorageLedgerV1>,
}

impl OrderedStorageSessionBudgetV1 {
    pub(in crate::vega::zk_ams::mkhe) fn new_v1() -> Self {
        Self {
            ledger: Arc::new(StorageLedgerV1 {
                usage: Mutex::new(StorageUsageV1::default()),
                spool_limit: ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1,
                io_limit: ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1,
            }),
        }
    }

    pub(in super::super) fn usage_v1(&self) -> Result<StorageUsageV1, StorageBudgetErrorV1> {
        Ok(*self.ledger.lock_v1()?)
    }

    #[cfg(test)]
    pub(in crate::vega::zk_ams::mkhe) fn test_usage_words_v1(&self) -> [u64; 4] {
        let usage = self.usage_v1().unwrap();
        [
            usage.live_spool_bytes,
            usage.peak_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes,
        ]
    }

    // Admission completes before any admitted file is created/sized. The caller
    // retains this move-only reservation after all leaves for field drop order.
    pub(super) fn reserve_files_v1(
        &mut self,
        spool_bytes: u64,
    ) -> Result<OrderedStorageReservationV1, StorageBudgetErrorV1> {
        if spool_bytes == 0 {
            return Err(StorageBudgetErrorV1::Order);
        }
        let io_bytes = spool_bytes
            .checked_mul(2)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        let mut usage = self.ledger.lock_v1()?;
        let live = usage
            .live_spool_bytes
            .checked_add(spool_bytes)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        let reserved = usage
            .reserved_io_bytes
            .checked_add(io_bytes)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        let total_io = usage
            .attempted_io_bytes
            .checked_add(reserved)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        if live > self.ledger.spool_limit {
            return Err(StorageBudgetErrorV1::SpoolLimit);
        }
        if total_io > self.ledger.io_limit {
            return Err(StorageBudgetErrorV1::IoLimit);
        }
        usage.live_spool_bytes = live;
        usage.peak_spool_bytes = usage.peak_spool_bytes.max(live);
        usage.reserved_io_bytes = reserved;
        Ok(OrderedStorageReservationV1 {
            ledger: Arc::clone(&self.ledger),
            spool_bytes,
            reserved_io_bytes: io_bytes,
        })
    }

    #[cfg(test)]
    pub(in crate::vega::zk_ams::mkhe) fn with_test_limits_v1(
        spool_limit: u64,
        io_limit: u64,
    ) -> Self {
        assert!(spool_limit <= ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1);
        assert!(io_limit <= ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1);
        Self {
            ledger: Arc::new(StorageLedgerV1 {
                usage: Mutex::new(StorageUsageV1::default()),
                spool_limit,
                io_limit,
            }),
        }
    }
}

/// Move-only custody of exact file lengths and unspent write/seal I/O.
/// Drop refunds only live file lengths and unattempted reserved operations.
pub(super) struct OrderedStorageReservationV1 {
    ledger: Arc<StorageLedgerV1>,
    spool_bytes: u64,
    reserved_io_bytes: u64,
}

impl OrderedStorageReservationV1 {
    /// Reserve another exact file under this same sealed parent's ledger.
    pub(super) fn reserve_sibling_file_v1(
        &self,
        spool_bytes: u64,
    ) -> Result<Self, StorageBudgetErrorV1> {
        self.require_sealed_v1()?;
        let mut original = OrderedStorageSessionBudgetV1 {
            ledger: Arc::clone(&self.ledger),
        };
        original.reserve_files_v1(spool_bytes)
    }

    /// Consume already-reserved write/seal bytes before calling the actual leaf.
    pub(super) fn charge_reserved_io_v1(&mut self, bytes: u64) -> Result<(), StorageBudgetErrorV1> {
        if bytes == 0 || bytes > self.reserved_io_bytes {
            return Err(StorageBudgetErrorV1::Order);
        }
        let mut usage = self.ledger.lock_v1()?;
        let reserved = usage
            .reserved_io_bytes
            .checked_sub(bytes)
            .ok_or(StorageBudgetErrorV1::Order)?;
        let attempted = usage
            .attempted_io_bytes
            .checked_add(bytes)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        usage.reserved_io_bytes = reserved;
        usage.attempted_io_bytes = attempted;
        self.reserved_io_bytes -= bytes;
        Ok(())
    }

    /// A sealed pair cannot retain a skipped write/seal reservation.
    pub(super) fn require_sealed_v1(&self) -> Result<(), StorageBudgetErrorV1> {
        let _guard = self.ledger.lock_v1()?;
        if self.reserved_io_bytes != 0 {
            return Err(StorageBudgetErrorV1::Order);
        }
        Ok(())
    }

    /// Authorize one authenticated record read without borrowing another budget.
    pub(super) fn charge_read_io_v1(&mut self, bytes: u64) -> Result<(), StorageBudgetErrorV1> {
        if bytes == 0 || self.reserved_io_bytes != 0 {
            return Err(StorageBudgetErrorV1::Order);
        }
        let mut usage = self.ledger.lock_v1()?;
        let attempted = usage
            .attempted_io_bytes
            .checked_add(bytes)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        let total = attempted
            .checked_add(usage.reserved_io_bytes)
            .ok_or(StorageBudgetErrorV1::Overflow)?;
        if total > self.ledger.io_limit {
            return Err(StorageBudgetErrorV1::IoLimit);
        }
        usage.attempted_io_bytes = attempted;
        Ok(())
    }
}

impl Drop for OrderedStorageReservationV1 {
    fn drop(&mut self) {
        // Only release an already-owned reservation under poison. No subsequent
        // admission or I/O authorization may recover/clear the poisoned mutex.
        let mut usage = self
            .ledger
            .usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        usage.live_spool_bytes -= self.spool_bytes;
        usage.reserved_io_bytes -= self.reserved_io_bytes;
    }
}

#[cfg(test)]
#[path = "resource_budget_v1_tests.rs"]
mod tests;
