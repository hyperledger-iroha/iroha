//! Prepaid fixed backing storage with no escaping uncharged allocation.
//!
//! Only the fixed backing allocation is covered. Referenced objects, scratch and
//! budget control storage remain obligations of the integrating caller.

use std::alloc::Layout;

use super::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation,
    InsufficientReservation,
};

/// A fixed allocation retaining its original prepaid charge until deallocation.
///
/// Safe access cannot extract, replace or grow the backing allocation. Bounded appends
/// initialize elements before exposing them without invoking `Clone`. Field drop
/// order frees the backing Vec before returning its credits. This owner accounts
/// only its requested layout;
/// callers must separately admit any nested objects, scratch or other storage.
pub struct ChargedBuffer<T> {
    values: Vec<T>,
    capacity: usize,
    _charge: AllocationCharge,
}

/// Local admission or allocator failure before backing storage is constructed.
#[derive(Debug)]
pub enum ChargedBufferError {
    /// Preserve the original finite pool's refusal and release observation.
    Admission(AllocationRefusal),
    /// The global allocator could not supply an already admitted layout.
    Allocator {
        /// Exact requested backing allocation size.
        requested_bytes: usize,
    },
}

impl std::fmt::Display for ChargedBufferError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(refusal) => refusal.fmt(formatter),
            Self::Allocator { requested_bytes } => write!(
                formatter,
                "failed to allocate {requested_bytes} admitted buffer bytes"
            ),
        }
    }
}

impl std::error::Error for ChargedBufferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(refusal) => Some(refusal),
            Self::Allocator { .. } => None,
        }
    }
}

impl From<AllocationRefusal> for ChargedBufferError {
    fn from(refusal: AllocationRefusal) -> Self {
        Self::Admission(refusal)
    }
}

/// Refusal to construct a buffer from one original prepaid allocation charge.
///
/// The constructor returns the same charge separately on every error. These
/// checks do not acquire pool capacity or turn a different layout into funding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargedBufferFromChargeError {
    /// The requested typed array layout cannot be represented.
    DemandOverflow,
    /// The original charge covers a different size or alignment.
    LayoutMismatch {
        /// Exact backing layout computed from the element type and capacity.
        expected: Layout,
        /// Unchanged layout retained by the returned original charge.
        actual: Layout,
    },
    /// The global allocator refused the exact already admitted layout.
    Allocator {
        /// Requested backing allocation layout, including its alignment.
        layout: Layout,
    },
}

impl std::fmt::Display for ChargedBufferFromChargeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DemandOverflow => AllocationRefusal::DemandOverflow.fmt(formatter),
            Self::LayoutMismatch { expected, actual } => write!(
                formatter,
                "buffer requires layout {expected:?} but original charge covers {actual:?}"
            ),
            Self::Allocator { layout } => write!(
                formatter,
                "failed to allocate {} admitted buffer bytes with alignment {}",
                layout.size(),
                layout.align()
            ),
        }
    }
}

impl std::error::Error for ChargedBufferFromChargeError {}

/// Failure to construct one fixed allocation from an already admitted parent.
#[derive(Debug)]
pub enum PrepaidBufferError {
    /// A concrete child allocation exceeds the parent's unchanged remainder.
    Reservation(InsufficientReservation),
    /// Layout overflow or physical allocation failure; no pool reacquisition occurs.
    Allocation(ChargedBufferError),
}

impl std::fmt::Display for PrepaidBufferError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reservation(refusal) => refusal.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PrepaidBufferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reservation(refusal) => Some(refusal),
            Self::Allocation(error) => Some(error),
        }
    }
}

impl<T> ChargedBuffer<T> {
    /// Admit the exact backing layout before requesting it from the allocator.
    ///
    /// Zero capacity and zero-sized elements request no backing allocation. On
    /// admission or allocator refusal, no allocation owner escapes and unused
    /// credits are returned.
    ///
    /// # Errors
    /// Returns the original pool refusal for overflow, oversize or capacity
    /// exhaustion, or an allocator failure for an admitted nonzero layout.
    pub fn new(capacity: usize, budget: &AllocationBudget) -> Result<Self, ChargedBufferError> {
        let layout = Layout::array::<T>(capacity).map_err(|_| AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve(layout)?;
        let charge = reservation
            .try_split(layout)
            .expect("the exact backing layout was reserved above");
        Self::allocate(capacity, layout, charge).map_err(|_charge| ChargedBufferError::Allocator {
            requested_bytes: layout.size(),
        })
    }

    /// Construct fixed backing from its exact original prepaid layout charge.
    ///
    /// A caller may reserve a complete operation once, split the actual backing
    /// and control-header layouts, then retain partial allocation owners on
    /// failure. This method never reacquires capacity, refunds a returned charge,
    /// or substitutes a new pool. The charge's size **and alignment** must match
    /// `Layout::array::<T>(capacity)`, even for a zero-byte layout.
    ///
    /// On success this same charge stays with the backing until deallocation.
    /// On every refusal the caller receives the original charge for retry or
    /// abandonment. Zero capacity and zero-sized elements allocate no backing,
    /// but still retain that original charge and the fixed logical capacity.
    /// The caller remains responsible for admitting every nested/control owner
    /// and deferring refunds until all enclosing physical guards are released.
    ///
    /// # Errors
    /// Returns checked layout overflow, an exact-layout mismatch before any
    /// allocation, or allocator refusal after layout validation. No input values
    /// are consumed or initialized by this constructor.
    pub fn try_from_charge(
        capacity: usize,
        charge: AllocationCharge,
    ) -> Result<Self, (AllocationCharge, ChargedBufferFromChargeError)> {
        let Ok(layout) = Layout::array::<T>(capacity) else {
            return Err((charge, ChargedBufferFromChargeError::DemandOverflow));
        };
        if charge.layout() != layout {
            let actual = charge.layout();
            return Err((
                charge,
                ChargedBufferFromChargeError::LayoutMismatch {
                    expected: layout,
                    actual,
                },
            ));
        }
        Self::allocate(capacity, layout, charge)
            .map_err(|charge| (charge, ChargedBufferFromChargeError::Allocator { layout }))
    }

    /// Construct fixed backing storage from its parent's original prepaid credit.
    ///
    /// This performs no second pool admission and never waits for a parent to
    /// release memory. It splits the exact requested layout before allocating;
    /// the resulting owner keeps that charge through actual deallocation.
    ///
    /// # Errors
    /// Returns an unchanged reservation on layout overflow or insufficient
    /// prepaid credit. A global allocator failure refunds the split allocation
    /// charge and leaves the parent's unspent remainder intact.
    pub fn from_reservation(
        capacity: usize,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, PrepaidBufferError> {
        let layout = Layout::array::<T>(capacity).map_err(|_| {
            PrepaidBufferError::Allocation(ChargedBufferError::Admission(
                AllocationRefusal::DemandOverflow,
            ))
        })?;
        let charge = reservation
            .try_split(layout)
            .map_err(PrepaidBufferError::Reservation)?;
        Self::allocate(capacity, layout, charge).map_err(|_charge| {
            PrepaidBufferError::Allocation(ChargedBufferError::Allocator {
                requested_bytes: layout.size(),
            })
        })
    }

    // All constructors validate this concrete layout and its original charge before
    // entering the sole backing-allocation kernel. Null returns the same charge;
    // only the caller decides whether that original custody is retained or freed.
    fn allocate(
        capacity: usize,
        layout: Layout,
        charge: AllocationCharge,
    ) -> Result<Self, AllocationCharge> {
        let values = if layout.size() == 0 {
            Vec::new()
        } else {
            // SAFETY: layout is nonzero and checked above. The original credit
            // already covers exactly this global allocator request. A null
            // result installs no allocation and returns the original charge.
            let pointer = unsafe { std::alloc::alloc(layout) };
            if pointer.is_null() {
                return Err(charge);
            }
            // SAFETY: pointer came from the global allocator with precisely
            // Layout::array::<T>(capacity). Length zero exposes no uninitialized
            // elements. Vec owns that same allocation and deallocates its original
            // layout. The only mutation below checks the fixed capacity first.
            unsafe { Vec::from_raw_parts(pointer.cast::<T>(), 0, capacity) }
        };
        Ok(Self {
            values,
            capacity,
            _charge: charge,
        })
    }

    /// Whether this backing allocation retains a charge from the exact original pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self._charge.belongs_to(budget)
    }

    /// Transfer the exact Vec allocation and its original charge to a canonical owner.
    ///
    /// This performs no allocation, clone, refund or capacity acquisition. Ordinary
    /// callers should retain this buffer; only an audited owning representation
    /// which must embed a concrete Vec field needs this seam.
    ///
    /// # Safety
    /// The caller must bind both returned values into one move-only owner before
    /// any fallible work. The Vec must retain this allocation and capacity without
    /// replacement or growth; it must be fully destroyed before its charge drops.
    /// On unwind the owner must not refund until physical reclamation is complete.
    /// If moved into a nested field, the enclosing owner must preserve these rules
    /// and offer no safe extraction or mutation that separates allocation/charge.
    /// Nested element allocations require their own retained charges. The charge
    /// funds only this original backing, never a later clone or replacement.
    #[allow(unsafe_code)]
    pub unsafe fn into_allocation_parts(self) -> (Vec<T>, AllocationCharge) {
        let Self {
            values,
            capacity: _,
            _charge,
        } = self;
        (values, _charge)
    }

    /// Borrow the uninitialized tail of this exact fixed backing allocation.
    /// The slice cannot grow, replace or separate the allocation from its charge.
    /// Initializing a slot does not expose it as a `T`; the owner must explicitly
    /// establish a complete initialized prefix with `set_initialized_len`.
    pub fn spare_capacity_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>] {
        let remaining = self.capacity - self.values.len();
        &mut self.values.spare_capacity_mut()[..remaining]
    }

    /// Expose an already initialized prefix without moving any element or growing.
    ///
    /// # Safety
    /// Every element in the newly exposed prefix must be fully initialized as a
    /// valid `T` in this same original backing. All partial initialization must be
    /// guarded so panic/refusal releases enclosing physical owners before dropping
    /// initialized fields or reclaiming their backing. No element may be exposed
    /// twice, and no uninitialized field may be read or dropped. Shrinking is not
    /// accepted: remove initialized elements with the existing bounded operations.
    #[allow(unsafe_code)]
    pub unsafe fn set_initialized_len(&mut self, len: usize) {
        assert!(
            len >= self.values.len() && len <= self.capacity,
            "initialized prefix must extend within original admitted capacity"
        );
        // SAFETY: the caller establishes every newly exposed element above.
        unsafe { self.values.set_len(len) };
    }

    /// Borrow initialized elements without separating allocation and charge.
    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    /// Mutate initialized elements without growing or replacing their allocation.
    ///
    /// This permits in-place canonical ordering of a funded batch. Operations on
    /// referenced objects and caller-supplied comparison code are not funded here.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.values
    }

    /// Return the admitted element capacity, including for zero-sized elements.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Shorten the initialized prefix without releasing its backing allocation.
    /// A length greater than the current length leaves the buffer unchanged.
    /// Credits remain with the original backing storage until it is freed.
    pub fn truncate(&mut self, len: usize) {
        self.values.truncate(len);
    }

    /// Move one element into the admitted backing without cloning or growing it.
    ///
    /// A full logical buffer returns the original element to its caller. This
    /// includes zero-sized elements, whose `Vec` has an effectively unlimited
    /// physical capacity. Nested storage owned by the element is not covered by
    /// this buffer's backing charge.
    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        if self.values.len() >= self.capacity {
            return Err(value);
        }
        self.values.push(value);
        Ok(())
    }

    /// Move one value into an already reserved slot without allocating.
    /// The caller must have preflighted capacity before any visible effects.
    pub fn push_reserved(&mut self, value: T) {
        assert!(self.values.len() < self.capacity);
        self.values.push(value);
    }

    /// Remove the last initialized value while retaining this backing charge.
    pub fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }

    /// Move initialized values out of this backing without reallocating it.
    /// Unconsumed values are dropped with the drain; the backing stays charged.
    pub fn drain_all(&mut self) -> std::vec::Drain<'_, T> {
        self.values.drain(..)
    }
}

impl<T: Copy> ChargedBuffer<T> {
    /// Fill already allocated capacity; never grow or replace the backing Vec.
    ///
    /// # Errors
    /// Returns [`std::io::ErrorKind::InvalidInput`] if the append would exceed
    /// the fixed logical capacity, even for zero-sized elements. Existing values
    /// and allocation ownership are unchanged on refusal.
    pub fn append(&mut self, values: &[T]) -> std::io::Result<()> {
        if values.len() > self.capacity - self.values.len() {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        let initialized = self.values.len();
        // SAFETY: the fixed-capacity check covers the entire destination. The
        // mutable borrow excludes a source slice into this same allocation.
        // Copy elements own no destructor obligation, so a bitwise copy avoids
        // invoking even a custom Clone implementation. Zero-sized elements
        // still obey the logical capacity and both pointers remain aligned.
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr(),
                self.values.as_mut_ptr().add(initialized),
                values.len(),
            );
            self.values.set_len(initialized + values.len());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "buffer/move_only_tests.rs"]
mod move_only_tests;

#[cfg(test)]
#[path = "buffer/placement_tests.rs"]
mod placement_tests;
