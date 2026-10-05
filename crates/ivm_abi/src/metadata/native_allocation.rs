//! Original-pool backing primitives for the pending owned metadata decoder.
//!
//! These primitives admit only their concrete Vec/Box backing. Nested values,
//! the codec's cumulative counters, source bytes and shared child controls need
//! their own original owners. No plain decoded value or replacement pool can
//! establish this custody. Production registration remains held until the
//! complete decoder can retain every original child owner in its returned graph.

// TODO: Register the required original-pool path only with complete decoder
// graph/child-control custody; plain Vec/Box parser results cannot retain it.

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationReservation, ChargedBuffer, PrepaidBufferError,
};
use std::{fmt, mem::ManuallyDrop};

/// Refusal before a metadata backing owner is constructed or replaced.
#[derive(Debug)]
pub(super) struct NativeAllocationError {
    kind: Failure,
    original: Option<PrepaidBufferError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    ForeignPool,
    DemandOverflow,
    Backing,
}

impl NativeAllocationError {
    fn foreign_pool() -> Self {
        Self {
            kind: Failure::ForeignPool,
            original: None,
        }
    }
    fn overflow() -> Self {
        Self {
            kind: Failure::DemandOverflow,
            original: None,
        }
    }
    fn backing(error: PrepaidBufferError) -> Self {
        Self {
            kind: Failure::Backing,
            original: Some(error),
        }
    }
}

impl fmt::Display for NativeAllocationError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.original {
            Some(error) => fmt::Display::fmt(error, output),
            None => output.write_str(match self.kind {
                Failure::ForeignPool => "metadata backing belongs to a different original pool",
                Failure::DemandOverflow => "metadata backing demand overflows",
                Failure::Backing => unreachable!("backing refusal retains its original error"),
            }),
        }
    }
}
impl std::error::Error for NativeAllocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.original {
            Some(error) => Some(error),
            None => None,
        }
    }
}

fn require_original(
    budget: &AllocationBudget,
    reservation: &AllocationReservation,
) -> Result<(), NativeAllocationError> {
    if !reservation.belongs_to(budget) {
        return Err(NativeAllocationError::foreign_pool());
    }
    Ok(())
}

/// A metadata vector whose exact native backing retains its original charge.
pub(super) struct NativeDecodeVec<T> {
    backing: ChargedBuffer<T>,
}
impl<T> NativeDecodeVec<T> {
    /// Construct only from the explicitly named pool's already prepaid remainder.
    pub(super) fn new(
        capacity: usize,
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, NativeAllocationError> {
        require_original(budget, reservation)?;
        Ok(Self {
            backing: ChargedBuffer::from_reservation(capacity, reservation)
                .map_err(NativeAllocationError::backing)?,
        })
    }

    /// Prepay a complete exact replacement before moving any original element.
    /// Failure leaves the original pointer, elements and charge unchanged.
    pub(super) fn reserve_additional(
        &mut self,
        additional: usize,
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<(), NativeAllocationError> {
        require_original(budget, reservation)?;
        if !self.backing.belongs_to(budget) {
            return Err(NativeAllocationError::foreign_pool());
        }
        let required = self
            .backing
            .as_slice()
            .len()
            .checked_add(additional)
            .ok_or_else(NativeAllocationError::overflow)?;
        if required <= self.backing.capacity() {
            return Ok(());
        }
        let capacity = required
            .max(self.backing.capacity().checked_mul(2).unwrap_or(required))
            .max(4);
        let mut replacement = ChargedBuffer::from_reservation(capacity, reservation)
            .map_err(NativeAllocationError::backing)?;
        // The complete replacement is already allocated. Drain moves original
        // values without Clone, user callbacks, a growth request or new storage.
        for value in self.backing.drain_all() {
            replacement.push_reserved(value);
        }
        let original = std::mem::replace(&mut self.backing, replacement);
        drop(original);
        Ok(())
    }

    /// Move a value into an admitted slot; a full buffer returns that same value.
    pub(super) fn push(&mut self, value: T) -> Result<(), T> {
        self.backing.try_push(value)
    }

    /// Borrow the initialized original values without separating their backing.
    pub(super) fn as_slice(&self) -> &[T] {
        self.backing.as_slice()
    }

    /// Observe the exact admitted logical capacity, including zero-sized values.
    pub(super) fn capacity(&self) -> usize {
        self.backing.capacity()
    }
}

/// A metadata Box whose original charge outlives physical backing reclamation.
pub(super) struct NativeDecodeBox<T> {
    value: ManuallyDrop<Box<T>>,
    charge: ManuallyDrop<AllocationCharge>,
}
impl<T> NativeDecodeBox<T> {
    /// Prepay and allocate one exact native slot before moving the original value.
    /// On refusal the caller receives that value unchanged.
    #[allow(unsafe_code)]
    pub(super) fn new(
        value: T,
        budget: &AllocationBudget,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, (T, NativeAllocationError)> {
        if let Err(error) = require_original(budget, reservation) {
            return Err((value, error));
        }
        let mut backing = match ChargedBuffer::from_reservation(1, reservation) {
            Ok(backing) => backing,
            Err(error) => return Err((value, NativeAllocationError::backing(error))),
        };
        backing.push_reserved(value);
        // SAFETY: one initialized T has the identical requested native layout
        // as Box<T>. Both owners move immediately into this private guard. The
        // Vec shell is suppressed; the Box alone will free this same allocation.
        let (values, charge) = unsafe { backing.into_allocation_parts() };
        let mut values = ManuallyDrop::new(values);
        // SAFETY: ChargedBuffer allocated exactly Layout::array::<T>(1), equal
        // to Layout::new::<T>(), or the canonical aligned dangling ZST pointer.
        let value = unsafe { Box::from_raw(values.as_mut_ptr()) };
        Ok(Self {
            value: ManuallyDrop::new(value),
            charge: ManuallyDrop::new(charge),
        })
    }

    /// Borrow the original value; this does not fund an owned child clone.
    pub(super) fn as_ref(&self) -> &T {
        &self.value
    }
}
impl<T> Drop for NativeDecodeBox<T> {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: exactly one stack guard now holds the original charge. Box
        // destroys T and physically reclaims its layout before the stack guard
        // can refund it, including unwind from T::drop. No refundable owner is
        // stored inside the allocation it pays for.
        let charge = unsafe { ManuallyDrop::take(&mut self.charge) };
        unsafe { ManuallyDrop::drop(&mut self.value) };
        drop(charge);
    }
}
