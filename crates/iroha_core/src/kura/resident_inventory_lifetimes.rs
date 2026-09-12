//! Association ownership tied to the last shared startup-inventory allocation.

use super::{
    resident_inventory::{AssociationCount, ResidentMutex, ResidentOwner},
    resource_inventory::{Family, Unavailable},
};
use std::{ops::Deref, sync::Arc};

/// Fixed-size aggregate of live allocations, including escaped replay readers.
#[derive(Debug, Default)]
pub(super) struct VerificationAllocations {
    count: AssociationCount,
}
impl ResidentOwner for VerificationAllocations {
    const FAMILY: Family = Family::ResidentVerification;
    fn resident_associations(&self) -> Result<u64, Unavailable> {
        self.count.get()
    }
    fn resident_complete(&self) -> bool {
        self.count.get().is_ok()
    }
}

/// Actual allocation owner; cloning an enclosing Arc does not duplicate indexes.
#[derive(Debug)]
pub(super) struct VerificationLease<T: ResidentOwner> {
    value: Option<T>,
    ledger: Arc<ResidentMutex<VerificationAllocations>>,
}
impl<T: ResidentOwner> VerificationLease<T> {
    /// Attach one completed authenticated allocation before publishing it to readers.
    pub(super) fn new(value: T, ledger: &Arc<ResidentMutex<VerificationAllocations>>) -> Self {
        {
            let mut totals = ledger.lock();
            let totals = &mut *totals;
            totals
                .count
                .replace(Some(0), value.resident_associations().ok());
            if !value.resident_complete() {
                totals.count.replace(None, None);
            }
        }
        Self {
            value: Some(value),
            ledger: Arc::clone(ledger),
        }
    }
    /// Mutate the unique allocation under its existing external ownership guard.
    ///
    /// Busy spans all indexed changes; unwind invalidates instead of publishing.
    pub(super) fn with_mut<R>(&mut self, mutate: impl FnOnce(&mut T) -> R) -> R {
        let mut totals = self.ledger.lock();
        let totals = &mut *totals;
        let value = self
            .value
            .as_mut()
            .expect("live inventory lease owns its allocation");
        let before = value.resident_associations().ok();
        let result = mutate(value);
        totals
            .count
            .replace(before, value.resident_associations().ok());
        if !value.resident_complete() {
            totals.count.replace(None, None);
        }
        result
    }
}
impl<T: ResidentOwner> Deref for VerificationLease<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
            .as_ref()
            .expect("live inventory lease owns its allocation")
    }
}
impl<T: ResidentOwner> Drop for VerificationLease<T> {
    fn drop(&mut self) {
        let mut totals = self.ledger.lock();
        let totals = &mut *totals;
        let before = self
            .value
            .as_ref()
            .and_then(|value| value.resident_associations().ok());
        // Destruction can walk large maps. Keep publication busy until those
        // actual allocations are gone; clearing Kura's Arc alone cannot subtract.
        drop(self.value.take());
        totals.count.removed(before);
    }
}
