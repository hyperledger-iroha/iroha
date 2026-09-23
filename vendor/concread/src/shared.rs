//! Shared allocation custody without weak references or guessed Arc layouts.
//!
//! The concrete control block owns a prepaid charge. The last reference frees
//! that exact block, then destroys its moved payload, then releases its charge.
//! No public operation exposes weak references, raw ownership or the counter.

use std::alloc::{alloc, handle_alloc_error, Layout};
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

/// One originally allocated shell that has not yet received its payload.
///
/// Reserve it before entering a phase that cannot allocate, then consume it with
/// [`Self::initialize`]. The opaque charge must already cover [`Self::layout`];
/// this owner neither grants credit nor funds allocations nested in the payload
/// or charge. Charge is caller-owned custody; this generic owner does not
/// validate it as credit authority. Dropping an unused shell frees it before
/// dropping its charge.
///
/// The shell cannot be cloned or initialized twice:
/// ```compile_fail
/// use concread::shared::Reserved;
/// fn initialize_twice(shell: Reserved<u64, ()>) {
///     let first = shell.initialize(1);
///     let second = shell.initialize(2);
/// }
/// ```
pub struct Reserved<T, Charge> {
    pointer: NonNull<Allocation<T, Charge>>,
}

/// The allocator refused one exact shared control-block layout.
///
/// This describes an allocation failure, not a pool admission decision. The
/// failed constructor returns the original charge separately for retry or abort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservationError {
    layout: Layout,
}

impl ReservationError {
    /// Exact layout refused before any shell or payload was initialized.
    pub fn layout(self) -> Layout {
        self.layout
    }
}

impl fmt::Display for ReservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shared allocation refused ({} bytes, alignment {})",
            self.layout.size(),
            self.layout.align()
        )
    }
}

impl std::error::Error for ReservationError {}

/// Strong references to one original initialized allocation and its charge.
pub struct Shared<T, Charge> {
    pointer: NonNull<Allocation<T, Charge>>,
}

/// The uniquely owned payload and charge after their control block was freed.
pub(crate) struct Reclaimed<T, Charge> {
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
    /// Exact original allocation layout, including the charge and all padding.
    ///
    /// Rust bounds this concrete sized layout before the allocator is called;
    /// the reference count makes it nonzero even for zero-sized payload/charge.
    pub fn layout() -> Layout {
        Layout::new::<Allocation<T, Charge>>()
    }

    /// Allocate one uninitialized shell with its already admitted charge.
    ///
    /// On allocator refusal, return that same charge without destroying it or
    /// constructing any payload. The caller can retry with the returned owner;
    /// no allocation size estimate or replacement capacity grant is used.
    /// Success retains the charge until the original allocation is freed.
    pub fn try_new(charge: Charge) -> Result<Self, (Charge, ReservationError)> {
        let layout = Self::layout();
        // SAFETY: the concrete sized layout is valid and nonzero. alloc uses the
        // same global allocator/layout that the existing Box reclamation owns.
        let Some(pointer) = NonNull::new(unsafe { alloc(layout) }.cast::<Allocation<T, Charge>>())
        else {
            return Err((charge, ReservationError { layout }));
        };
        // SAFETY: this unique, correctly aligned allocation has room for the
        // exact header. Initialize only its live fields; UnsafeCell<MaybeUninit<T>>
        // permits uninitialized bytes. No payload-sized stack temporary or
        // initialized T exists until the consuming initialize operation. These
        // field moves cannot invoke user code or unwind.
        unsafe {
            std::ptr::addr_of_mut!((*pointer.as_ptr()).references).write(AtomicUsize::new(1));
            std::ptr::addr_of_mut!((*pointer.as_ptr()).charge).write(ManuallyDrop::new(charge));
        }
        Ok(Self { pointer })
    }

    pub(crate) fn new(charge: Charge) -> Self {
        match Self::try_new(charge) {
            Ok(shell) => shell,
            Err((_charge, error)) => handle_alloc_error(error.layout()),
        }
    }

    /// Consume this shell and move the payload into its original allocation.
    ///
    /// This does not allocate, clone, invoke a callback, or drop either owner.
    /// The returned strong owner retains exactly the shell's original charge.
    pub fn initialize(self, value: T) -> Shared<T, Charge> {
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
    /// Exact layout of the one allocation retained by this shared owner.
    /// Nested allocations in `T` or `Charge` require their own funding.
    pub fn layout() -> Layout {
        Reserved::<T, Charge>::layout()
    }

    /// Allocate one initialized owner, retaining its original charge until the
    /// final reference has freed the allocation and destroyed its payload.
    pub fn new(value: T, charge: Charge) -> Self {
        Reserved::new(charge).initialize(value)
    }

    /// Whether both references retain the same original allocation.
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        left.pointer == right.pointer
    }

    pub(crate) fn get_mut(&mut self) -> Option<&mut T> {
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

    pub(crate) fn into_inner(self) -> Option<Reclaimed<T, Charge>> {
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
    pub(crate) fn consume<R>(self, consume: impl FnOnce(T) -> R) -> (R, Charge) {
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

#[cfg(all(test, feature = "maps", not(feature = "dhat-heap"), not(miri)))]
#[path = "shared_reservation_tests.rs"]
mod reservation_tests;
