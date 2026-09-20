//! Original pointer-bookkeeping buffers with explicit allocation custody.
//!
//! Fixed buffers consume their already admitted charge and never grow. Entries
//! are Copy bookkeeping values, so clearing or freeing a buffer does not destroy
//! the tree nodes it names; the cursor or reader retains that separate obligation.

use std::alloc::{Layout, LayoutError};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::slice;

use crate::internals::lincowcell::Untracked;

mod sealed {
    /// Only the two concrete bookkeeping owners have audited cursor bounds.
    pub trait Sealed {}
}

/// Pointer-bookkeeping operations shared by ordinary and admitted cursors.
/// Implementations are sealed so cursor thread-safety bounds can account for
/// every owned charge as well as the nodes represented by its raw pointers.
pub trait TrackingBuffer<T: Copy>: sealed::Sealed {
    /// The sole allocation charge retained by this buffer.
    type Charge;

    /// Append one initialized entry.
    ///
    /// A fixed buffer rejects exhausted admission before writing any entry.
    fn push(&mut self, value: T);

    /// Remove the last initialized entry before returning its bookkeeping value.
    ///
    /// This never allocates or releases the backing allocation or its charge.
    fn pop(&mut self) -> Option<T>;

    /// Shorten the initialized prefix without growing it or allocating.
    ///
    /// A length at least as large as the current length leaves it unchanged.
    /// Copy bookkeeping entries have no destructors; their targets remain owned
    /// by the cursor or reader, including when their entries are forgotten.
    fn truncate(&mut self, len: usize);

    /// Borrow only the initialized entries in their original order.
    fn as_slice(&self) -> &[T];

    /// Remaining fixed slots; None is the explicitly growable untracked mode.
    fn remaining_capacity(&self) -> Option<usize>;

    /// Forget initialized bookkeeping entries without releasing the allocation.
    fn clear(&mut self);
}

impl<T: Copy> sealed::Sealed for Vec<T> {}

impl<T: Copy> TrackingBuffer<T> for Vec<T> {
    type Charge = Untracked;

    fn push(&mut self, value: T) {
        Vec::push(self, value);
    }

    fn pop(&mut self) -> Option<T> {
        Vec::pop(self)
    }

    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len);
    }

    fn as_slice(&self) -> &[T] {
        Vec::as_slice(self)
    }

    fn remaining_capacity(&self) -> Option<usize> {
        None
    }

    fn clear(&mut self) {
        Vec::clear(self);
    }
}

/// One fixed original allocation and its move-only prepaid capacity owner.
///
/// No default, clone or growing operation can fabricate another admission. The
/// buffer's charge excludes the separate nodes referenced by its Copy entries.
/// Closed admitted map operations construct this storage before mutation.
pub struct FixedTrackingBuffer<T: Copy, Charge> {
    entries: ManuallyDrop<Box<[MaybeUninit<T>]>>,
    initialized: usize,
    charge: ManuallyDrop<Charge>,
}

impl<T: Copy, Charge> FixedTrackingBuffer<T, Charge> {
    /// Exact Box backing layout to admit before constructing this buffer.
    ///
    /// Zero capacity needs no allocation. Checked array layout rejects byte
    /// counts which overflow or exceed the allocator's valid layout range.
    pub(crate) fn allocation_layout(capacity: usize) -> Result<Layout, LayoutError> {
        Layout::array::<MaybeUninit<T>>(capacity)
    }

    /// Allocate the admitted fixed capacity, retaining the original charge.
    ///
    /// Invalid capacity returns that same charge before allocating. The caller
    /// must supply custody for `allocation_layout(capacity)` from its complete
    /// prepaid operation; this constructor never obtains additional pool credit.
    pub(crate) fn try_new(capacity: usize, charge: Charge) -> Result<Self, (Charge, LayoutError)> {
        if let Err(error) = Self::allocation_layout(capacity) {
            return Err((charge, error));
        }
        let entries = Box::<[T]>::new_uninit_slice(capacity);
        Ok(Self {
            entries: ManuallyDrop::new(entries),
            initialized: 0,
            charge: ManuallyDrop::new(charge),
        })
    }

    /// Original admitted entry capacity, unchanged by bookkeeping operations.
    pub(crate) fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Original backing address; only `as_slice` exposes initialized values.
    pub(crate) fn as_ptr(&self) -> *const T {
        self.entries.as_ptr().cast()
    }
}

impl<T: Copy, Charge> sealed::Sealed for FixedTrackingBuffer<T, Charge> {}

impl<T: Copy, Charge> TrackingBuffer<T> for FixedTrackingBuffer<T, Charge> {
    type Charge = Charge;

    fn push(&mut self, value: T) {
        assert!(
            self.initialized < self.capacity(),
            "B+tree tracking exceeded its admitted fixed capacity"
        );
        self.entries[self.initialized].write(value);
        self.initialized += 1;
    }

    fn pop(&mut self) -> Option<T> {
        self.initialized = self.initialized.checked_sub(1)?;
        // SAFETY: the old prefix included this initialized entry. Shortening
        // the prefix first removes its custody before returning the Copy value.
        Some(unsafe { self.entries[self.initialized].assume_init_read() })
    }

    fn truncate(&mut self, len: usize) {
        self.initialized = self.initialized.min(len);
    }

    fn as_slice(&self) -> &[T] {
        // SAFETY: push initializes exactly this prefix before increasing its
        // length. pop, truncate and clear only shorten that initialized prefix;
        // no method can expose spare slots.
        // Box supplies a non-null, aligned address even for an empty/ZST slice.
        unsafe { slice::from_raw_parts(self.as_ptr(), self.initialized) }
    }

    fn remaining_capacity(&self) -> Option<usize> {
        Some(self.capacity() - self.initialized)
    }

    fn clear(&mut self) {
        self.initialized = 0;
    }
}

impl<T: Copy, Charge> Drop for FixedTrackingBuffer<T, Charge> {
    fn drop(&mut self) {
        // SAFETY: both fields remain uniquely owned until this one destruction.
        // The uninitialized-slot Box has no payload destructors. Its original
        // backing allocation is freed before arbitrary charge refund callbacks.
        unsafe {
            ManuallyDrop::drop(&mut self.entries);
            ManuallyDrop::drop(&mut self.charge);
        }
    }
}

#[cfg(test)]
#[path = "tracking_tests.rs"]
mod tests;
