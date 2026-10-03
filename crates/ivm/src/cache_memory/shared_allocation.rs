//! Shared immutable slices with original allocation and retention custody.

#[cfg(test)]
use super::REFUSE_NEXT_SHARED_ALLOCATION;
use super::{MemoryBudget, MemoryReservation, global_budget, strong_owner::StrongOwner};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedShared};
use std::ops::Deref;

#[derive(Debug)]
pub(super) struct Allocation<T> {
    values: Storage<T>,
    reservation: MemoryReservation,
}

enum Storage<T> {
    Local(Box<[T]>),
    Funded(ChargedBuffer<T>),
}

impl<T> Storage<T> {
    fn as_slice(&self) -> &[T] {
        match self {
            Self::Local(values) => values,
            Self::Funded(values) => values.as_slice(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Storage<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(formatter)
    }
}

#[derive(Debug)]
enum Owner<T> {
    Local(StrongOwner<Allocation<T>>),
    Funded(ChargedShared<Allocation<T>>),
}

impl<T> Owner<T> {
    fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (left, right) {
            (Self::Local(left), Self::Local(right)) => StrongOwner::ptr_eq(left, right),
            (Self::Funded(left), Self::Funded(right)) => ChargedShared::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl<T> Clone for Owner<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Local(owner) => Self::Local(owner.clone()),
            Self::Funded(owner) => Self::Funded(owner.clone()),
        }
    }
}

impl<T> Deref for Owner<T> {
    type Target = Allocation<T>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(owner) => owner,
            Self::Funded(owner) => owner,
        }
    }
}

/// An immutable shared slice whose memory charge follows its final owner.
///
/// Clones retain the same reservation. This deliberately does not expose a raw
/// `Arc` to the slice, which could otherwise outlive its accounting owner.
/// Elements are inline `Copy` values. Metadata with owned nested allocations
/// uses [`SharedValue`](super::SharedValue) and supplies its complete dynamic footprint.
#[derive(Debug)]
pub struct SharedAllocation<T: Copy>(Owner<T>, bool);

impl<T: Copy> SharedAllocation<T> {
    /// Copy immutable values after admitting their exact backing and shared control layouts.
    ///
    /// The supplied execution pool remains the original owner through every clone,
    /// cache eviction and final release. Retention accounting covers both allocations
    /// once; changing retention never refunds their execution credit.
    ///
    /// # Errors
    /// Returns the original allocation refusal before allocating either owner, or
    /// an operational allocation deferral if the admitted physical allocation fails.
    pub fn try_copy_from_slice_with_memory_budget(
        values: &[T],
        budget: &AllocationBudget,
    ) -> Result<Self, crate::VMError> {
        Self::try_copy_with_budgets(values, budget, global_budget())
    }

    fn try_copy_with_budgets(
        values: &[T],
        budget: &AllocationBudget,
        retention: &MemoryBudget,
    ) -> Result<Self, crate::VMError> {
        Self::try_from_iter_with_budgets(values.iter().copied().map(Ok), budget, retention)
    }

    /// Whether this slice's original backing and shared control belong to `budget`.
    ///
    /// Equal limits do not establish ownership. Diagnostic allocations never
    /// belong to an execution pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        matches!(&self.0, Owner::Funded(owner) if owner.belongs_to(budget))
    }

    pub(crate) fn try_from_iter_with_memory_budget<E: From<crate::VMError>>(
        values: impl ExactSizeIterator<Item = Result<T, E>>,
        budget: &AllocationBudget,
    ) -> Result<Self, E> {
        Self::try_from_iter_with_budgets(values, budget, global_budget())
    }

    fn try_from_iter_with_budgets<E: From<crate::VMError>>(
        values: impl ExactSizeIterator<Item = Result<T, E>>,
        budget: &AllocationBudget,
        retention: &MemoryBudget,
    ) -> Result<Self, E> {
        use crate::error::{ExecutionDeferral, VMError};
        use iroha_allocation::AllocationRefusal;

        let len = values.len();
        let backing = std::alloc::Layout::array::<T>(len)
            .map_err(|_| VMError::AllocationDeferred(AllocationRefusal::DemandOverflow))?;
        let bytes = backing
            .size()
            .checked_add(ChargedShared::<Allocation<T>>::allocation_layout().size())
            .ok_or(VMError::AllocationDeferred(
                AllocationRefusal::DemandOverflow,
            ))?;
        let mut admission = budget
            .try_reserve_bytes(bytes)
            .map_err(VMError::AllocationDeferred)?;
        let reservation = retention.reserve(bytes);
        let allocation_failure =
            || VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable);
        #[cfg(test)]
        if REFUSE_NEXT_SHARED_ALLOCATION.replace(false) {
            return Err(allocation_failure().into());
        }
        let owner = ChargedShared::<Allocation<T>>::reserve_from(&mut admission)
            .map_err(|_| allocation_failure())?;
        #[cfg(test)]
        if REFUSE_FUNDED_BUFFER.replace(false) {
            return Err(allocation_failure().into());
        }
        let mut storage = ChargedBuffer::from_reservation(len, &mut admission)
            .map_err(|_| allocation_failure())?;
        for value in values {
            if storage.as_slice().len() == len {
                return Err(VMError::DecodeError.into());
            }
            storage.push_reserved(value?);
        }
        if storage.as_slice().len() != len {
            return Err(VMError::DecodeError.into());
        }
        debug_assert_eq!(admission.remaining_bytes(), 0);
        let owner = Self(
            Owner::Funded(owner.initialize(Allocation {
                values: Storage::Funded(storage),
                reservation,
            })),
            false,
        );
        owner.0.reservation.register_shared_initial();
        Ok(owner)
    }

    pub(crate) fn try_from_iter<E: From<crate::error::VMError>>(
        values: impl ExactSizeIterator<Item = Result<T, E>>,
    ) -> Result<Self, E> {
        Self::try_from_iter_with_budget(values, global_budget())
    }

    pub(super) fn try_from_iter_with_budget<E: From<crate::error::VMError>>(
        values: impl ExactSizeIterator<Item = Result<T, E>>,
        budget: &MemoryBudget,
    ) -> Result<Self, E> {
        use crate::error::{ExecutionDeferral, VMError};

        let allocation_refusal =
            || VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable);
        #[cfg(test)]
        if REFUSE_NEXT_SHARED_ALLOCATION.replace(false) {
            return Err(allocation_refusal().into());
        }
        let len = values.len();
        let slice_bytes = std::alloc::Layout::array::<T>(len)
            .map(|layout| layout.size())
            .map_err(|_| allocation_refusal())?;
        let owner_bytes = std::mem::size_of::<Allocation<T>>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .ok_or_else(allocation_refusal)?;
        let bytes = slice_bytes
            .checked_add(owner_bytes)
            .ok_or_else(allocation_refusal)?;
        let mut reservation = budget.reserve(bytes);
        let mut output = Vec::new();
        output
            .try_reserve_exact(len)
            .map_err(|_| allocation_refusal())?;
        let conversion_peak_bytes = std::alloc::Layout::array::<T>(output.capacity())
            .ok()
            .and_then(|layout| layout.size().checked_add(slice_bytes))
            .and_then(|bytes| bytes.checked_add(owner_bytes))
            .ok_or_else(allocation_refusal)?;
        // Shrinking the Vec into a boxed slice may allocate the exact-sized
        // destination while the temporary Vec allocation is still alive.
        reservation.set_known_bytes(conversion_peak_bytes);
        for value in values {
            if output.len() == len {
                return Err(VMError::DecodeError.into());
            }
            output.push(value?);
        }
        if output.len() != len {
            return Err(VMError::DecodeError.into());
        }
        let values = output.into_boxed_slice();
        reservation.set_known_bytes(Self::bytes_for_len(values.len()));
        let owner = Self(
            Owner::Local(StrongOwner::new(Allocation {
                values: Storage::Local(values),
                reservation,
            })),
            false,
        );
        owner.0.reservation.register_shared_initial();
        Ok(owner)
    }

    /// Take exclusive ownership of a slice and begin tracking its allocation.
    pub fn from_boxed(values: Box<[T]>) -> Self {
        Self::with_budget(values, global_budget())
    }

    pub(super) fn with_budget(values: Box<[T]>, budget: &MemoryBudget) -> Self {
        let bytes = Self::bytes_for_len(values.len());
        let owner = Self(
            Owner::Local(StrongOwner::new(Allocation {
                values: Storage::Local(values),
                reservation: budget.reserve(bytes),
            })),
            false,
        );
        owner.0.reservation.register_shared_initial();
        owner
    }

    fn bytes_for_len(len: usize) -> usize {
        // The owner allocation contains the Box and reservation, plus Arc's two
        // reference counters. The slice is its own exact-sized allocation.
        len * std::mem::size_of::<T>()
            + std::mem::size_of::<Allocation<T>>()
            + 2 * std::mem::size_of::<usize>()
    }

    /// Admit this allocation for cache retention without waiting for other owners.
    pub fn try_retain(&self) -> bool {
        self.0.reservation.try_retain()
    }

    /// Clone one cache-held reference; ordinary clones remain borrower references.
    pub fn cache_clone(&self) -> Self {
        self.0.reservation.add_shared_handle(true);
        Self(self.0.clone(), true)
    }

    /// Transfer this reference into a cache without copying the allocation.
    pub fn into_cache_owner(mut self) -> Self {
        if !self.1 {
            self.0.reservation.promote_shared_cache_handle();
            self.1 = true;
        }
        self
    }

    pub(crate) fn allocation_bytes(&self) -> usize {
        self.0.reservation.bytes()
    }

    /// Whether two handles refer to the same allocation.
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Owner::ptr_eq(&this.0, &other.0)
    }
}

impl<T: Copy> From<Vec<T>> for SharedAllocation<T> {
    fn from(values: Vec<T>) -> Self {
        Self::from_boxed(values.into_boxed_slice())
    }
}

impl<T: Copy> Clone for SharedAllocation<T> {
    fn clone(&self) -> Self {
        self.0.reservation.add_shared_handle(false);
        Self(self.0.clone(), false)
    }
}

impl<T: Copy> Drop for SharedAllocation<T> {
    fn drop(&mut self) {
        self.0.reservation.remove_shared_handle(self.1);
    }
}

impl<T: Copy> Deref for SharedAllocation<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.0.values.as_slice()
    }
}

#[cfg(test)]
std::thread_local! {
    static REFUSE_FUNDED_BUFFER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
mod tests;

impl<T: Copy> AsRef<[T]> for SharedAllocation<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}
