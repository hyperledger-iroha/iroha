//! Shared allocation custody without weak references or guessed Arc layouts.
//!
//! The concrete control block owns a prepaid charge. The last reference frees
//! that exact block, then destroys its moved payload, then releases its charge.
//! No public operation exposes weak references, raw ownership or the counter.

use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::fmt;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::{fence, AtomicUsize, Ordering};

#[repr(C)]
struct Allocation<T, Charge> {
    references: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
    charge: ManuallyDrop<Charge>,
}

/// An originally allocated shell that has not yet received its payload.
pub(super) struct Reserved<T, Charge> {
    pointer: NonNull<Allocation<T, Charge>>,
}

/// Strong references to one original initialized allocation and its charge.
pub(super) struct Shared<T, Charge> {
    pointer: NonNull<Allocation<T, Charge>>,
}

/// The uniquely owned payload and charge after their control block was freed.
pub(super) struct Reclaimed<T, Charge> {
    value: ManuallyDrop<T>,
    charge: ManuallyDrop<Charge>,
}

// Only the last strong owner mutates or destroys a published payload. No weak
// references exist, and publication transfers an already initialized value.
unsafe impl<T: Send + Sync, Charge: Send + Sync> Send for Shared<T, Charge> {}
unsafe impl<T: Send + Sync, Charge: Send + Sync> Sync for Shared<T, Charge> {}
// Reserved shells are unique and expose no payload reference before initialization.
unsafe impl<T: Send, Charge: Send> Send for Reserved<T, Charge> {}
unsafe impl<T: Sync, Charge: Sync> Sync for Reserved<T, Charge> {}

impl<T, Charge> Reserved<T, Charge> {
    pub(super) fn layout() -> Layout {
        Layout::new::<Allocation<T, Charge>>()
    }

    pub(super) fn new(charge: Charge) -> Self {
        let allocation = Box::new(Allocation {
            references: AtomicUsize::new(1),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            charge: ManuallyDrop::new(charge),
        });
        Self {
            pointer: NonNull::from(Box::leak(allocation)),
        }
    }

    pub(super) fn initialize(self, value: T) -> Shared<T, Charge> {
        let this = ManuallyDrop::new(self);
        // SAFETY: Reserved is unique, cannot be cloned and has no published
        // payload. Initialization happens exactly once before the type changes.
        unsafe { (*this.pointer.as_ref().value.get()).write(value) };
        Shared {
            pointer: this.pointer,
        }
    }
}

impl<T, Charge> Drop for Reserved<T, Charge> {
    fn drop(&mut self) {
        // SAFETY: this shell never escaped as a shared initialized owner.
        let mut allocation = unsafe { Box::from_raw(self.pointer.as_ptr()) };
        let charge = ManuallyDrop::new(unsafe { ManuallyDrop::take(&mut allocation.charge) });
        drop(allocation);
        // The uninitialized shell and its exact control block are now gone.
        drop(ManuallyDrop::into_inner(charge));
    }
}

impl<T, Charge> Shared<T, Charge> {
    pub(super) fn ptr_eq(left: &Self, right: &Self) -> bool {
        left.pointer == right.pointer
    }

    pub(super) fn get_mut(&mut self) -> Option<&mut T> {
        // No weak owner can race an upgrade. With one strong reference and an
        // exclusive borrow of it, no other owner can create a competing clone.
        if unsafe { self.pointer.as_ref() }
            .references
            .load(Ordering::Acquire)
            != 1
        {
            return None;
        }
        Some(unsafe { (&mut *self.pointer.as_ref().value.get()).assume_init_mut() })
    }

    pub(super) fn into_inner(self) -> Option<Reclaimed<T, Charge>> {
        let this = ManuallyDrop::new(self);
        // SAFETY: this consumes exactly one strong reference. Suppressing its
        // destructor prevents a second decrement of the same ownership unit.
        unsafe { Self::release(this.pointer) }
    }

    unsafe fn release(pointer: NonNull<Allocation<T, Charge>>) -> Option<Reclaimed<T, Charge>> {
        // Release publishes each previous owner's accesses. The final acquire
        // fence joins them before reading or destroying the unique payload.
        let allocation = unsafe { pointer.as_ref() };
        if allocation.references.fetch_sub(1, Ordering::Release) != 1 {
            return None;
        }
        fence(Ordering::Acquire);
        let mut allocation = unsafe { Box::from_raw(pointer.as_ptr()) };
        let value = ManuallyDrop::new(unsafe { allocation.value.get_mut().assume_init_read() });
        let charge = ManuallyDrop::new(unsafe { ManuallyDrop::take(&mut allocation.charge) });
        // MaybeUninit and ManuallyDrop suppress automatic payload/charge drop.
        // The exact allocation is freed before either is returned to the caller.
        drop(allocation);
        Some(Reclaimed { value, charge })
    }
}

impl<T, Charge> Clone for Shared<T, Charge> {
    fn clone(&self) -> Self {
        let previous = unsafe { self.pointer.as_ref() }
            .references
            .fetch_add(1, Ordering::Relaxed);
        // As with Arc, abort before an overflowing reference count can make a
        // live allocation appear uniquely owned. No panic may expose that state.
        if previous >= isize::MAX as usize {
            std::process::abort();
        }
        Self {
            pointer: self.pointer,
        }
    }
}

impl<T, Charge> Deref for Shared<T, Charge> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: Shared is constructed only after full initialization; its
        // strong reference retains the original allocation throughout this borrow.
        unsafe { (&*self.pointer.as_ref().value.get()).assume_init_ref() }
    }
}

impl<T, Charge> Drop for Shared<T, Charge> {
    fn drop(&mut self) {
        // SAFETY: this destructor consumes its one original strong reference.
        drop(unsafe { Self::release(self.pointer) });
    }
}

impl<T, Charge> Reclaimed<T, Charge> {
    pub(super) fn consume<R>(self, consume: impl FnOnce(T) -> R) -> (R, Charge) {
        let mut this = ManuallyDrop::new(self);
        // SAFETY: self is consumed, its automatic destructor is suppressed, and
        // these are the only reads of the original payload and charge.
        let value = unsafe { ManuallyDrop::take(&mut this.value) };
        let output = consume(value);
        // A panicking consumer retains the charge conservatively. Return the
        // original charge so the caller can finish publication before invoking
        // its destructor. This charge covers the control block, not nested T.
        let charge = unsafe { ManuallyDrop::take(&mut this.charge) };
        (output, charge)
    }
}

impl<T, Charge> Deref for Reclaimed<T, Charge> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T, Charge> DerefMut for Reclaimed<T, Charge> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T, Charge> Drop for Reclaimed<T, Charge> {
    fn drop(&mut self) {
        // If payload destruction unwinds, charge stays retained rather than
        // falsely reporting complete reclamation of an incompletely dropped T.
        unsafe { ManuallyDrop::drop(&mut self.value) };
        unsafe { ManuallyDrop::drop(&mut self.charge) };
    }
}

impl<T: fmt::Debug, Charge> fmt::Debug for Shared<T, Charge> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T, Charge> fmt::Debug for Reserved<T, Charge> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reserved")
            .field("pointer", &self.pointer)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Reserved, Shared};
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::Arc;

    struct Charge(Arc<AtomicUsize>);

    impl Drop for Charge {
        fn drop(&mut self) {
            assert_eq!(self.0.fetch_add(1, SeqCst), 0);
        }
    }

    #[test]
    fn concurrent_clones_retain_one_original_allocation_and_unique_mutation() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut original = Reserved::new(Charge(Arc::clone(&dropped))).initialize(17_u64);
        let pointer = &*original as *const u64 as usize;
        let other = original.clone();
        assert!(Shared::ptr_eq(&original, &other));
        assert!(Shared::get_mut(&mut original).is_none());
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let owner = other.clone();
                scope.spawn(move || {
                    for _ in 0..10_000 {
                        let copy = owner.clone();
                        assert_eq!(*copy, 17);
                        assert_eq!(&*copy as *const u64 as usize, pointer);
                    }
                });
            }
        });
        drop(other);
        assert_eq!(dropped.load(SeqCst), 0);
        *Shared::get_mut(&mut original).unwrap() = 23;
        assert_eq!(*original, 23);
        drop(original);
        assert_eq!(dropped.load(SeqCst), 1);
    }
}
