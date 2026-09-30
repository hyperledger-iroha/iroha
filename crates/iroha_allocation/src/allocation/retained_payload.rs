//! Move-only custody for canonical payload fields and their original allocation charges.

use std::{fmt, mem::ManuallyDrop};

use super::{AllocationBudget, AllocationCharge, ChargedBuffer};

/// A canonical payload retaining the original charges for its owned allocations.
///
/// Only borrowing is available. No Clone or extraction API can separate this
/// payload from its ledger. This does not fund unrelated clones obtained through
/// the borrowed value, or the enclosing owner/control allocation. Those remain
/// explicit caller obligations, as does any scratch used to inspect the payload.
/// If payload destruction unwinds, this owner conservatively retains the entire
/// ledger rather than refund possibly incomplete reclamation.
pub struct RetainedPayload<T> {
    payload: ManuallyDrop<T>,
    charges: ManuallyDrop<ChargedBuffer<AllocationCharge>>,
}

/// A canonical owner was offered allocation custody from a different finite pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedPayloadError {
    /// The ledger backing itself belongs to a different original pool.
    ForeignLedger,
    /// At least one payload allocation charge belongs to a different pool.
    ForeignAllocation,
}

impl fmt::Display for RetainedPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ForeignLedger => "canonical allocation ledger belongs to another pool",
            Self::ForeignAllocation => "canonical payload allocation belongs to another pool",
        })
    }
}
impl std::error::Error for RetainedPayloadError {}

impl<T> RetainedPayload<T> {
    /// Bind an original canonical payload and its complete existing charge ledger.
    ///
    /// Checks the ledger's backing charge and every contained charge against the
    /// actual original pool. No capacity acquisition, clone or allocation occurs.
    /// Refusal returns both exact input owners unchanged for correction or drop.
    ///
    /// # Safety
    /// The caller must prove that each ledger entry corresponds to one actual
    /// allocation already owned by the payload, with its exact original layout,
    /// and that every payload allocation requiring admission has one such entry.
    /// The payload must not expose interior mutation, extraction or sharing that
    /// lets those allocations outlive its destruction or changes their layouts.
    /// The ledger must not also charge an independently refunded allocation.
    /// Until this constructor succeeds (including every refusal and unwind), the
    /// caller must preserve the same allocation-before-charge destruction order.
    /// The pool check proves source identity, not these layout/ownership facts.
    ///
    /// # Errors
    /// Returns the original payload and ledger on either kind of foreign pool.
    #[allow(unsafe_code)]
    pub unsafe fn try_new(
        payload: T,
        charges: ChargedBuffer<AllocationCharge>,
        budget: &AllocationBudget,
    ) -> Result<Self, (T, ChargedBuffer<AllocationCharge>, RetainedPayloadError)> {
        if !charges.belongs_to(budget) {
            return Err((payload, charges, RetainedPayloadError::ForeignLedger));
        }
        if charges
            .as_slice()
            .iter()
            .any(|charge| !charge.belongs_to(budget))
        {
            return Err((payload, charges, RetainedPayloadError::ForeignAllocation));
        }
        Ok(Self {
            payload: ManuallyDrop::new(payload),
            charges: ManuallyDrop::new(charges),
        })
    }

    /// Move the same canonical allocations into their next private protocol owner.
    ///
    /// The identical charge ledger follows the result without allocation, refund
    /// or reacquisition. If the mapping unwinds, the ledger is conservatively
    /// retained; incomplete reclamation cannot become retry capacity.
    ///
    /// # Safety
    /// The mapping must only move the original allocations into the returned
    /// payload, retaining their exact layouts and ownership. It must not clone,
    /// grow, replace, drop, share or export them. The returned type must preserve
    /// the constructor's no-escape/interior-mutation contract. Any additional
    /// storage requires independently retained original-pool custody; this move
    /// funds no new allocations. This is for audited canonical field movement,
    /// never a general mapping operation over arbitrary user code.
    #[allow(unsafe_code)]
    pub unsafe fn map_payload<U>(self, map: impl FnOnce(T) -> U) -> RetainedPayload<U> {
        let mut original = ManuallyDrop::new(self);
        // SAFETY: self will no longer run Drop. Exactly one mapping owns the
        // original payload; the original ledger stays retained across unwind.
        let payload = unsafe { ManuallyDrop::take(&mut original.payload) };
        let mapped = map(payload);
        // SAFETY: the mapper returned with the same owned allocations under the
        // documented contract. Transfer the one ledger without dropping it.
        let charges = unsafe { ManuallyDrop::take(&mut original.charges) };
        RetainedPayload {
            payload: ManuallyDrop::new(mapped),
            charges: ManuallyDrop::new(charges),
        }
    }

    /// Check that the ledger backing and every retained allocation use this exact pool.
    ///
    /// This allocation-free observation lets the next original owner reject a
    /// foreign funding source before allocating additional storage. Equal limits
    /// or equal available credit do not establish source identity. It grants no
    /// new capacity and does not prove the constructor's layout/ownership contract.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.charges.belongs_to(budget)
            && self
                .charges
                .as_slice()
                .iter()
                .all(|charge| charge.belongs_to(budget))
    }

    /// Borrow the exact original canonical value without transferring its allocation custody.
    pub fn get(&self) -> &T {
        &self.payload
    }
}

impl<T> Drop for RetainedPayload<T> {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: the unsafe constructor bound every original allocation to this
        // payload and forbids any escape. A normal return establishes complete
        // destruction before refund. On unwind the next statement is not run;
        // ManuallyDrop retains all ledger storage and charges conservatively.
        unsafe { ManuallyDrop::drop(&mut self.payload) };
        // SAFETY: the payload was completely destroyed exactly once above.
        unsafe { ManuallyDrop::drop(&mut self.charges) };
    }
}

#[cfg(test)]
#[path = "retained_payload/tests.rs"]
mod tests;
