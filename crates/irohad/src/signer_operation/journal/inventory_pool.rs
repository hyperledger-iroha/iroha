//! Process-shared, finite admission for private signer-journal inventory I/O.
//!
//! One original pool is injected into receipt purposes and pending Reserve. Credits are local
//! resources: refusal is retryable and never makes a signed operation consensus-invalid.

use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use iroha_config::parameters::actual::SorafsSignerJournalInventory;
use mv::allocation::{AllocationBudget, AllocationReservation};

use super::{
    Directory, MAX_JOURNAL_PATH_BYTES, MAX_JOURNAL_PATH_COMPONENTS, SignerReceiptJournalErrorV1,
};

const MIN_RESIDENT_BYTES: u64 = 1024 * 1024;
const MIN_METADATA_PROBES: u64 = 65_537 + 4 * 65 + 4 + 4 * 65 + 1;
const MIN_OPEN_HANDLES: u32 = 67;
const DIRECTORY_BUFFER_ALLOWANCE: usize = 256 * 1024;
const PENDING_ID_SLOT_ALLOWANCE: usize = 128;

/// One original finite pool injected into all receipt families and pending Reserve.
#[derive(Clone)]
pub struct SignerJournalInventoryPoolV1 {
    resident: AllocationBudget,
    metadata: Arc<Counter>,
    handles: Arc<Counter>,
}
impl SignerJournalInventoryPoolV1 {
    /// Construct the process pool from validated non-secret node configuration.
    ///
    /// # Errors
    /// Rejects any limit unable to fund one full receipt scan and pinned maximum path.
    pub fn new(policy: SorafsSignerJournalInventory) -> Result<Self, SignerReceiptJournalErrorV1> {
        if policy.resident_bytes.0 < MIN_RESIDENT_BYTES
            || policy.metadata_probes < MIN_METADATA_PROBES
            || policy.open_handles < MIN_OPEN_HANDLES
        {
            return Err(SignerReceiptJournalErrorV1::Unavailable);
        }
        let resident = usize::try_from(policy.resident_bytes.0)
            .map_err(|_| SignerReceiptJournalErrorV1::Unavailable)?;
        Ok(Self::from_limits(
            resident,
            policy.metadata_probes,
            u64::from(policy.open_handles),
        ))
    }

    fn from_limits(resident_bytes: usize, metadata_probes: u64, open_handles: u64) -> Self {
        Self {
            resident: AllocationBudget::new(resident_bytes),
            metadata: Arc::new(Counter::new(metadata_probes)),
            handles: Arc::new(Counter::new(open_handles)),
        }
    }

    pub(super) fn admit_open(
        &self,
        path_bytes: usize,
        components: usize,
    ) -> Result<(OpenLease, CounterPermit), SignerReceiptJournalErrorV1> {
        if path_bytes > MAX_JOURNAL_PATH_BYTES
            || components == 0
            || components > MAX_JOURNAL_PATH_COMPONENTS
        {
            return Err(SignerReceiptJournalErrorV1::Unavailable);
        }
        let resident_bytes = path_bytes
            .checked_mul(3)
            .and_then(|bytes| {
                components
                    .checked_mul(
                        mem::size_of::<Directory>() + mem::size_of::<std::ffi::OsString>() + 64,
                    )
                    .and_then(|lineage| bytes.checked_add(lineage))
            })
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or(SignerReceiptJournalErrorV1::Unavailable)?;
        let resident = self
            .resident
            .try_reserve_bytes(resident_bytes)
            .map_err(|_| SignerReceiptJournalErrorV1::Capacity)?;
        let path_probes = self.metadata.try_acquire(
            u64::try_from(components)
                .map_err(|_| SignerReceiptJournalErrorV1::Unavailable)?
                .checked_add(1)
                .and_then(|count| count.checked_mul(4))
                .and_then(|count| count.checked_add(1))
                .ok_or(SignerReceiptJournalErrorV1::Unavailable)?,
        )?;
        let handles = self.handles.try_acquire(
            u64::try_from(components + 1).map_err(|_| SignerReceiptJournalErrorV1::Unavailable)?,
        )?;
        Ok((
            OpenLease {
                _resident: resident,
                _handles: handles,
            },
            path_probes,
        ))
    }

    pub(super) fn admit_scan(
        &self,
        max_records: usize,
        pending_ids: bool,
        lineage_len: usize,
    ) -> Result<ScanLease, SignerReceiptJournalErrorV1> {
        let id_bytes = if pending_ids {
            max_records.checked_mul(PENDING_ID_SLOT_ALLOWANCE)
        } else {
            Some(0)
        };
        let resident_bytes = id_bytes
            .and_then(|bytes| bytes.checked_add(DIRECTORY_BUFFER_ALLOWANCE))
            .ok_or(SignerReceiptJournalErrorV1::Unavailable)?;
        let probes = u64::try_from(max_records)
            .ok()
            .and_then(|count| count.checked_add(1))
            .and_then(|count| {
                u64::try_from(lineage_len)
                    .ok()
                    .and_then(|len| len.checked_mul(4))
                    .and_then(|lineage| count.checked_add(lineage))
            })
            .and_then(|count| count.checked_add(4))
            .ok_or(SignerReceiptJournalErrorV1::Unavailable)?;
        let resident = self
            .resident
            .try_reserve_bytes(resident_bytes)
            .map_err(|_| SignerReceiptJournalErrorV1::Capacity)?;
        let metadata = self.metadata.try_acquire(probes)?;
        let directory = self.handles.try_acquire(1)?;
        Ok(ScanLease {
            _resident: resident,
            _metadata: metadata,
            _directory: directory,
        })
    }

    pub(super) fn admit_file(&self) -> Result<CounterPermit, SignerReceiptJournalErrorV1> {
        self.handles.try_acquire(1)
    }

    #[cfg(test)]
    pub(super) fn for_test(resident_bytes: usize, metadata_probes: u64, open_handles: u64) -> Self {
        Self::from_limits(resident_bytes, metadata_probes, open_handles)
    }
}

pub(super) struct OpenLease {
    _resident: AllocationReservation,
    _handles: CounterPermit,
}

pub(super) struct ScanLease {
    _resident: AllocationReservation,
    _metadata: CounterPermit,
    _directory: CounterPermit,
}

struct Counter {
    limit: u64,
    held: AtomicU64,
}
impl Counter {
    const fn new(limit: u64) -> Self {
        Self {
            limit,
            held: AtomicU64::new(0),
        }
    }
    fn try_acquire(
        self: &Arc<Self>,
        amount: u64,
    ) -> Result<CounterPermit, SignerReceiptJournalErrorV1> {
        let mut held = self.held.load(Ordering::Acquire);
        loop {
            if amount > self.limit.saturating_sub(held) {
                return Err(SignerReceiptJournalErrorV1::Capacity);
            }
            match self.held.compare_exchange_weak(
                held,
                held + amount,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(CounterPermit {
                        pool: Arc::clone(self),
                        amount,
                    });
                }
                Err(current) => held = current,
            }
        }
    }
}

pub(super) struct CounterPermit {
    pool: Arc<Counter>,
    amount: u64,
}
impl Drop for CounterPermit {
    fn drop(&mut self) {
        self.pool.held.fetch_sub(self.amount, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_refusal_refunds_partial_acquisition_and_keeps_original_pool() {
        let pool = SignerJournalInventoryPoolV1::for_test(1024 * 1024, 100, 2);
        let (_open_lease, open_probes) = pool.admit_open(10, 1).expect("first path");
        assert_eq!(pool.metadata.held.load(Ordering::Acquire), 9);
        assert_eq!(pool.handles.held.load(Ordering::Acquire), 2);
        assert!(matches!(
            pool.admit_scan(1, false, 2),
            Err(SignerReceiptJournalErrorV1::Capacity)
        ));
        assert_eq!(pool.metadata.held.load(Ordering::Acquire), 9);
        drop(open_probes);
        assert_eq!(pool.metadata.held.load(Ordering::Acquire), 0);
    }
    #[test]
    fn distinct_purpose_handles_share_one_original_pool_and_refund_on_drop() {
        let pool = SignerJournalInventoryPoolV1::for_test(1024 * 1024, 100, 3);
        let (first, probes) = pool.admit_open(10, 1).unwrap();
        drop(probes);
        let clone = pool.clone();
        assert!(matches!(
            clone.admit_open(10, 1),
            Err(SignerReceiptJournalErrorV1::Capacity)
        ));
        drop(first);
        assert!(clone.admit_open(10, 1).is_ok());
    }
    #[test]
    fn exact_scan_probe_boundary_and_one_below_are_distinct_from_invalid_content() {
        let probe_demand = 1 + 1 + 4 * 2 + 4;
        let exact = SignerJournalInventoryPoolV1::for_test(1024 * 1024, probe_demand, 1);
        assert!(exact.admit_scan(1, false, 2).is_ok());
        let below = SignerJournalInventoryPoolV1::for_test(1024 * 1024, probe_demand - 1, 1);
        assert!(matches!(
            below.admit_scan(1, false, 2),
            Err(SignerReceiptJournalErrorV1::Capacity)
        ));
    }
}
