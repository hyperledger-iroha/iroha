//! EbrCell - A concurrently readable cell with Ebr
//!
//! An [EbrCell] can be used in place of a `RwLock`. Readers are guaranteed that
//! the data will not change during the lifetime of the read. Readers do
//! not block writers, and writers do not block readers. Writers are serialised
//! same as the write in a `RwLock`.
//!
//! This is the Ebr collected implementation.
//! Ebr is the crossbeam-epoch based reclaim system for async memory
//! garbage collection. Ebr is faster than `Arc`,
//! but long transactions can cause the memory usage to grow very quickly
//! before a garbage reclaim. This is a space time trade, where you gain
//! performance at the expense of delaying garbage collection. Holding Ebr
//! reads for too long may impact garbage collection of other epoch structures
//! or crossbeam library components.
//! If you need accurate memory reclaim, use the Arc (`CowCell`) implementation.

use crossbeam_epoch as epoch;
use crossbeam_epoch::{Atomic, Guard, Owned, Shared};
use std::alloc::Layout;
use std::sync::atomic::Ordering::{AcqRel, Acquire};

use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};

/// Explicitly unaccounted mode for callers that do not attach allocation custody.
///
/// This mode makes no resource-admission guarantee. A cell using another charge
/// type cannot use the unaccounted `write` or `try_write` convenience methods.
#[derive(Debug)]
pub struct Untracked;

#[derive(Debug)]
#[repr(C)]
struct Allocation<T, Charge> {
    value: T,
    // Never release custody through automatic field drop: the enclosing Box has
    // not yet been deallocated then, and a payload destructor may unwind.
    charge: ManuallyDrop<Charge>,
}

fn reclaim<T, Charge>(mut allocation: Owned<Allocation<T, Charge>>) {
    // SAFETY: this allocation is uniquely owned, either by an abandoned writer
    // or by the epoch collector after every reader of this generation is gone.
    // Its charge is extracted exactly once and never automatically dropped.
    let charge = unsafe { ManuallyDrop::take(&mut allocation.charge) };
    let charge = ManuallyDrop::new(charge);
    drop(allocation);
    // Releasing only after drop returns also covers the outer allocation free.
    // If T::drop panics, retain the charge rather than claiming reclamation.
    drop(ManuallyDrop::into_inner(charge));
}

unsafe fn defer_reclaim<T: Send + 'static, Charge: Send + 'static>(
    guard: &Guard,
    allocation: Shared<'_, Allocation<T, Charge>>,
) {
    // SAFETY: the caller has unlinked this exact allocation. The epoch guard
    // protects its readers, and both payload and custody may move to the thread
    // that eventually performs collection. Conversion to Owned happens only
    // inside the deferred callback, after the original EBR grace period.
    unsafe {
        guard.defer_unchecked(move || reclaim(allocation.into_owned()));
    }
}

/// An `EbrCell` Write Transaction handle.
///
/// This allows mutation of the content of the `EbrCell` without blocking or
/// affecting current readers.
///
/// Changes are only stored in the structure until you call commit: to
/// abort a change, don't call commit and allow the write transaction to
/// go out of scope. This causes the `EbrCell` to unlock allowing other
/// writes to proceed.
pub struct EbrCellWriteTxn<
    'a,
    T: 'static + Clone + Send + Sync,
    Charge: Send + Sync + 'static = Untracked,
> {
    data: Option<Owned<Allocation<T, Charge>>>,
    // This way we know who to contact for updating our data ....
    caller: &'a EbrCell<T, Charge>,
    _guard: MutexGuard<'a, ()>,
}

/// An unpublished generation detached from its writer without cloning payloads.
///
/// This owner retains the exact original allocation and charge, but no cell
/// reference, epoch pin or writer lock. It grants no publication or predecessor
/// authority. An enclosing MV owner must retain and authenticate that identity
/// before installing this generation through [`EbrCell::try_write_owned`].
pub struct EbrCellOwned<T: Clone + Send + Sync + 'static, Charge: Send + Sync + 'static = Untracked>
{
    data: Option<Owned<Allocation<T, Charge>>>,
}

impl<T, Charge> Deref for EbrCellOwned<T, Charge>
where
    T: Clone + Send + Sync + 'static,
    Charge: Send + Sync + 'static,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.data.as_ref().unwrap().value
    }
}

impl<T, Charge> Drop for EbrCellOwned<T, Charge>
where
    T: Clone + Send + Sync + 'static,
    Charge: Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(allocation) = self.data.take() {
            reclaim(allocation);
        }
    }
}

impl<T, Charge> EbrCellWriteTxn<'_, T, Charge>
where
    T: Clone + Sync + Send + 'static,
    Charge: Send + Sync + 'static,
{
    /// Access a mutable pointer of the data in the `EbrCell`. This data is only
    /// visible to this write transaction object in this thread until you call
    /// 'commit'.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data.as_mut().unwrap().value
    }

    /// Commit the changes in this write transaction to the `EbrCell`. This will
    /// consume the transaction so that further changes can not be made to it
    /// after this function is called.
    pub fn commit(mut self) {
        self.caller.commit(self.data.take().unwrap());
    }

    /// Release this writer while retaining its exact unpublished allocation.
    /// No clone, new payload allocation or publication occurs. The returned
    /// owner can cross threads; it does not retain this cell or its lock.
    pub fn detach(mut self) -> EbrCellOwned<T, Charge> {
        EbrCellOwned {
            data: self.data.take(),
        }
    }
}

impl<T, Charge> Drop for EbrCellWriteTxn<'_, T, Charge>
where
    T: Clone + Sync + Send + 'static,
    Charge: Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(allocation) = self.data.take() {
            reclaim(allocation);
        }
    }
}

impl<T, Charge> Deref for EbrCellWriteTxn<'_, T, Charge>
where
    T: Clone + Sync + Send,
    Charge: Send + Sync + 'static,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.data.as_ref().unwrap().value
    }
}

impl<T, Charge> DerefMut for EbrCellWriteTxn<'_, T, Charge>
where
    T: Clone + Sync + Send,
    Charge: Send + Sync + 'static,
{
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

/// A concurrently readable cell.
///
/// This structure behaves in a similar manner to a `RwLock<Box<T>>`. However
/// unlike a read-write lock, writes and parallel reads can be performed
/// simultaneously. This means writes do not block reads or reads do not
/// block writes.
///
/// To achieve this a form of "copy-on-write" (or for Rust, clone on write) is
/// used. As a write transaction begins, we clone the existing data to a new
/// location that is capable of being mutated.
///
/// Readers are guaranteed that the content of the `EbrCell` will live as long
/// as the read transaction is open, and will be consistent for the duration
/// of the transaction. There can be an "unlimited" number of readers in parallel
/// accessing different generations of data of the `EbrCell`.
///
/// Data that is copied is garbage collected using the crossbeam-epoch library.
///
/// Writers are serialised and are guaranteed they have exclusive write access
/// to the structure.
///
/// A cell with a `Charge` type other than [`Untracked`] retains each move-only
/// charge in its actual allocated generation until both the payload and the
/// allocation have been destroyed. Admission precedes cloning, and commit moves
/// the writer's already allocated generation. This is an ownership hook, not a
/// complete heap budget: callers must separately fund nested payload allocations,
/// mutation growth, allocator overhead and epoch collector bookkeeping.
///
/// Charged cells have no unaccounted writer convenience:
/// ```compile_fail
/// use concread::ebrcell::EbrCell;
/// struct Charge;
/// let cell = EbrCell::new_charged(0_u64, Charge);
/// let _writer = cell.write();
/// ```
///
/// # Examples
/// ```
/// use concread::ebrcell::EbrCell;
///
/// let data: i64 = 0;
/// let ebrcell = EbrCell::new(data);
///
/// // Begin a read transaction
/// let read_txn = ebrcell.read();
/// assert_eq!(*read_txn, 0);
/// {
///     // Now create a write, and commit it.
///     let mut write_txn = ebrcell.write();
///     *write_txn = 1;
///     // Commit the change
///     write_txn.commit();
/// }
/// // Show the previous generation still reads '0'
/// assert_eq!(*read_txn, 0);
/// let new_read_txn = ebrcell.read();
/// // And a new read transaction has '1'
/// assert_eq!(*new_read_txn, 1);
/// ```
#[derive(Debug)]
pub struct EbrCell<T: Clone + Sync + Send + 'static, Charge: Send + Sync + 'static = Untracked> {
    write: Mutex<()>,
    active: Atomic<Allocation<T, Charge>>,
}

impl<T> Default for EbrCell<T>
where
    T: Default + Clone + Sync + Send + 'static,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> EbrCell<T>
where
    T: Clone + Sync + Send + 'static,
{
    /// Create a new `EbrCell` storing type `T`. `T` must implement `Clone`.
    /// This convenience constructor does not account for resource ownership.
    pub fn new(data: T) -> Self {
        Self::new_charged(data, Untracked)
    }

    /// Begin a write transaction, returning a write guard.
    /// This convenience method is available only in unaccounted mode.
    pub fn write(&self) -> EbrCellWriteTxn<'_, T> {
        match self.write_charged(|_, _| Ok::<_, std::convert::Infallible>(Untracked)) {
            Ok(writer) => writer,
            Err(never) => match never {},
        }
    }

    /// Attempt to begin a write transaction. If it's already held,
    /// `None` is returned.
    /// This convenience method is available only in unaccounted mode.
    pub fn try_write(&self) -> Option<EbrCellWriteTxn<'_, T>> {
        match self.try_write_charged(|_, _| Ok::<_, std::convert::Infallible>(Untracked)) {
            Ok(writer) => writer,
            Err(never) => match never {},
        }
    }
}

impl<T, Charge> EbrCell<T, Charge>
where
    T: Clone + Sync + Send + 'static,
    Charge: Send + Sync + 'static,
{
    /// Whether an earlier writer or clone unwound while holding this cell's lock.
    /// A failed nonblocking acquisition on a poisoned cell requires recovery;
    /// waiting for a future release alone cannot make that lock usable.
    pub fn is_poisoned(&self) -> bool {
        self.write.is_poisoned()
    }

    /// The exact layout requested for one allocated payload and its charge.
    ///
    /// This excludes allocations owned by `T` or `Charge`, allocator overhead,
    /// and epoch collector bookkeeping. It is not an estimate of total heap use.
    pub fn allocation_layout() -> Layout {
        Layout::new::<Allocation<T, Charge>>()
    }

    /// Store an already owned value with its prepaid allocation custody.
    ///
    /// The caller must acquire custody before constructing any payload it needs
    /// to fund. This constructor cannot retroactively admit `data`'s allocations.
    /// A charge need not implement `Clone`; it is moved into this exact allocation.
    pub fn new_charged(data: T, charge: Charge) -> Self {
        Self {
            write: Mutex::new(()),
            active: Atomic::new(Allocation {
                value: data,
                charge: ManuallyDrop::new(charge),
            }),
        }
    }

    /// Admit and allocate a writable clone while holding the original writer lock.
    ///
    /// `admit` sees the current value and the exact outer allocation layout before
    /// the payload clone or generation allocation. Refusal leaves the cell unchanged.
    /// The returned charge must cover the intended clone and mutation lifetime;
    /// mutable payload access does not separately measure or admit later growth.
    /// A panic during cloning conservatively retains the charge because `Clone`
    /// may have leaked partially constructed payload allocations.
    pub fn write_charged<E>(
        &self,
        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,
    ) -> Result<EbrCellWriteTxn<'_, T, Charge>, E> {
        self.write_from_guard(self.write.lock().unwrap(), admit)
    }

    /// Attempt charged admission without waiting for the original writer lock.
    ///
    /// `Ok(None)` means the lock was unavailable or poisoned; in that case
    /// `admit` is never called and no value is cloned. Otherwise this has the same
    /// custody and panic behavior as [`Self::write_charged`].
    pub fn try_write_charged<E>(
        &self,
        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,
    ) -> Result<Option<EbrCellWriteTxn<'_, T, Charge>>, E> {
        let Ok(mguard) = self.write.try_lock() else {
            return Ok(None);
        };
        self.write_from_guard(mguard, admit).map(Some)
    }

    /// Acquire a writer for an already owned unpublished generation.
    ///
    /// This operation clones no payload and allocates no successor. Contention
    /// or poison returns the exact input owner. It does not compare an original
    /// cell or predecessor: an enclosing MV owner must check its original
    /// publication identity while all required writers are held. As with any
    /// ordinary mutable writer, this method alone grants no application-level
    /// permission to publish the supplied value.
    pub fn try_write_owned(
        &self,
        mut owner: EbrCellOwned<T, Charge>,
    ) -> Result<EbrCellWriteTxn<'_, T, Charge>, EbrCellOwned<T, Charge>> {
        let Ok(mguard) = self.write.try_lock() else {
            return Err(owner);
        };
        Ok(EbrCellWriteTxn {
            data: owner.data.take(),
            caller: self,
            _guard: mguard,
        })
    }

    fn write_from_guard<'a, E>(
        &'a self,
        mguard: MutexGuard<'a, ()>,
        admit: impl FnOnce(&T, Layout) -> Result<Charge, E>,
    ) -> Result<EbrCellWriteTxn<'a, T, Charge>, E> {
        let guard = epoch::pin();
        let current = self.active.load(Acquire, &guard);
        // SAFETY: this initialized allocation remains protected by our pin.
        let current = unsafe { current.deref() };
        // Do not refund if Clone panics: it may have leaked partially built data.
        let charge = ManuallyDrop::new(admit(&current.value, Self::allocation_layout())?);
        let allocation = Owned::new(Allocation {
            value: current.value.clone(),
            charge,
        });
        Ok(EbrCellWriteTxn {
            data: Some(allocation),
            caller: self,
            _guard: mguard,
        })
    }

    /// This is an internal component of the commit cycle. It takes ownership
    /// of the value stored in the writetxn, and commits it to the main EbrCell
    /// safely.
    ///
    /// In theory you could use this as a "lock free" version, but you don't
    /// know if you are trampling a previous change, so it's private and we
    /// let the writetxn struct serialise and protect this interface.
    fn commit(&self, element: Owned<Allocation<T, Charge>>) {
        // Yield a read txn?
        let guard = epoch::pin();

        // The writer already owns the exact allocation. Commit allocates no new
        // payload; its exclusive lock makes this the only possible replacement.
        let prev_data = self.active.swap(element, AcqRel, &guard);
        // SAFETY: the swap unlinked this original allocation, and the callback
        // retains its charge through the collector's actual destruction/free.
        unsafe { defer_reclaim(&guard, prev_data) };
        // Then return the current data with a readtxn. Do we need a new guard scope?
    }

    /// Begin a read transaction. The returned [`EbrCellReadTxn`] guarantees
    /// the data lives long enough via crossbeam's Epoch type. When this is
    /// dropped the data *may* be freed at some point in the future.
    pub fn read(&self) -> EbrCellReadTxn<T> {
        let guard = epoch::pin();

        // This option returns None on null pointer, but we can never be null
        // as we have to init with data, and all replacement ALWAYS gives us
        // a ptr, so unwrap?
        let cur = {
            let c = self.active.load(Acquire, &guard);
            // SAFETY: the guard protects this initialized allocation; the read
            // transaction retains that same guard for the payload reference.
            unsafe { &c.deref().value as *const T }
        };

        EbrCellReadTxn {
            _guard: guard,
            data: cur,
        }
    }
}

impl<T, Charge> Drop for EbrCell<T, Charge>
where
    T: Clone + Sync + Send + 'static,
    Charge: Send + Sync + 'static,
{
    fn drop(&mut self) {
        // Right, we are dropping! Everything is okay here *except*
        // that we need to tell our active data to be unlinked, else it may
        // be dropped "unsafely".
        let guard = epoch::pin();

        let prev_data = self.active.load(Acquire, &guard);
        // SAFETY: the cell is exclusively owned during Drop. Existing read
        // transactions remain protected by the same epoch reclamation boundary.
        unsafe { defer_reclaim(&guard, prev_data) };
    }
}

/// A read transaction. This stores a reference to the data from the main
/// `EbrCell`, and guarantees it is alive for the duration of the read.
// #[derive(Debug)]
pub struct EbrCellReadTxn<T> {
    _guard: Guard,
    data: *const T,
}

impl<T> Deref for EbrCellReadTxn<T> {
    type Target = T;

    /// De-reference and access the value within the read transaction.
    fn deref(&self) -> &T {
        unsafe { &(*self.data) }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::EbrCell;
    use std::thread::scope;

    #[test]
    fn test_deref_mut() {
        let data: i64 = 0;
        let cc = EbrCell::new(data);
        {
            /* Take a write txn */
            let mut cc_wrtxn = cc.write();
            *cc_wrtxn = 1;
            cc_wrtxn.commit();
        }
        let cc_rotxn = cc.read();
        assert_eq!(*cc_rotxn, 1);
    }

    #[test]
    fn test_try_write() {
        let data: i64 = 0;
        let cc = EbrCell::new(data);
        /* Take a write txn */
        let cc_wrtxn_a = cc.try_write();
        assert!(cc_wrtxn_a.is_some());
        /* Because we already hold the writ, the second is guaranteed to fail */
        let cc_wrtxn_a = cc.try_write();
        assert!(cc_wrtxn_a.is_none());
    }

    #[test]
    fn test_simple_create() {
        let data: i64 = 0;
        let cc = EbrCell::new(data);

        let cc_rotxn_a = cc.read();
        assert_eq!(*cc_rotxn_a, 0);

        {
            /* Take a write txn */
            let mut cc_wrtxn = cc.write();
            /* Get the data ... */
            {
                let mut_ptr = cc_wrtxn.get_mut();
                /* Assert it's 0 */
                assert_eq!(*mut_ptr, 0);
                *mut_ptr = 1;
                assert_eq!(*mut_ptr, 1);
            }
            assert_eq!(*cc_rotxn_a, 0);

            let cc_rotxn_b = cc.read();
            assert_eq!(*cc_rotxn_b, 0);
            /* The write txn and it's lock is dropped here */
            cc_wrtxn.commit();
        }

        /* Start a new txn and see it's still good */
        let cc_rotxn_c = cc.read();
        assert_eq!(*cc_rotxn_c, 1);
        assert_eq!(*cc_rotxn_a, 0);
    }

    const MAX_TARGET: i64 = 2000;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_multithread_create() {
        use std::time::Instant;

        let start = Instant::now();
        // Create the new ebrcell.
        let data: i64 = 0;
        let cc = EbrCell::new(data);

        assert!(scope(|scope| {
            let cc_ref = &cc;

            let readers: Vec<_> = (0..7)
                .map(|_| {
                    scope.spawn(move || {
                        let mut last_value: i64 = 0;
                        while last_value < MAX_TARGET {
                            let cc_rotxn = cc_ref.read();
                            {
                                assert!(*cc_rotxn >= last_value);
                                last_value = *cc_rotxn;
                            }
                        }
                    })
                })
                .collect();

            let writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        let mut last_value: i64 = 0;
                        while last_value < MAX_TARGET {
                            let mut cc_wrtxn = cc_ref.write();
                            {
                                let mut_ptr = cc_wrtxn.get_mut();
                                assert!(*mut_ptr >= last_value);
                                last_value = *mut_ptr;
                                *mut_ptr += 1;
                            }
                            cc_wrtxn.commit();
                        }
                    })
                })
                .collect();

            for h in readers.into_iter() {
                h.join().unwrap();
            }
            for h in writers.into_iter() {
                h.join().unwrap();
            }
            true
        }));

        let end = Instant::now();
        print!("Ebr MT create :{:?} ", end - start);
    }

    static GC_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Clone)]
    struct TestGcWrapper<T> {
        data: T,
    }

    impl<T> Drop for TestGcWrapper<T> {
        fn drop(&mut self) {
            // Add to the atomic counter ...
            GC_COUNT.fetch_add(1, Ordering::Release);
        }
    }

    fn test_gc_operation_thread(cc: &EbrCell<TestGcWrapper<i64>>) {
        while GC_COUNT.load(Ordering::Acquire) < 50 {
            // thread::sleep(std::time::Duration::from_millis(200));
            {
                let mut cc_wrtxn = cc.write();
                {
                    let mut_ptr = cc_wrtxn.get_mut();
                    mut_ptr.data += 1;
                }
                cc_wrtxn.commit();
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_gc_operation() {
        GC_COUNT.store(0, Ordering::Release);
        let data = TestGcWrapper { data: 0 };
        let cc = EbrCell::new(data);

        assert!(scope(|scope| {
            let cc_ref = &cc;
            let writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        test_gc_operation_thread(cc_ref);
                    })
                })
                .collect();

            for h in writers.into_iter() {
                h.join().unwrap();
            }
            true
        }));

        assert!(GC_COUNT.load(Ordering::Acquire) >= 50);
    }
}

#[cfg(test)]
mod tests_linear {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::EbrCell;

    static GC_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Clone)]
    struct TestGcWrapper<T> {
        data: T,
    }

    impl<T> Drop for TestGcWrapper<T> {
        fn drop(&mut self) {
            // Add to the atomic counter ...
            GC_COUNT.fetch_add(1, Ordering::Release);
        }
    }

    #[test]
    fn test_gc_operation_linear() {
        /*
         * Test if epoch drops in order (or ordered enough).
         * A property required for b+tree with cow is that txn's
         * are dropped in order so that tree states are not invalidated.
         *
         * A -> B -> C
         *
         * If B is dropped, it invalidates nodes copied from A
         * causing the tree to corrupt txn A (and maybe C).
         *
         * EBR due to it's design while it won't drop in order,
         * it drops generationally, in blocks. This is probably
         * good enough. This means that:
         *
         * A -> B -> C .. -> X -> Y
         *
         * EBR will drop in blocks such as:
         *
         * |  g1   |  g2   |  live |
         * A -> B -> C .. -> X -> Y
         *
         * This test is "small" but asserts a basic sanity of drop
         * ordering, but it's not conclusive for b+tree. More testing
         * (likely multi-thread strees test) is needed, or analysis from
         * other EBR developers.
         */
        GC_COUNT.store(0, Ordering::Release);
        let data = TestGcWrapper { data: 0 };
        let cc = EbrCell::new(data);

        // Open a read A.
        let cc_rotxn_a = cc.read();
        // open a write, change and commit
        {
            let mut cc_wrtxn = cc.write();
            {
                let mut_ptr = cc_wrtxn.get_mut();
                mut_ptr.data += 1;
            }
            cc_wrtxn.commit();
        }
        // open a read B.
        let cc_rotxn_b = cc.read();
        // open a write, change and commit
        {
            let mut cc_wrtxn = cc.write();
            {
                let mut_ptr = cc_wrtxn.get_mut();
                mut_ptr.data += 1;
            }
            cc_wrtxn.commit();
        }
        // open a read C
        let cc_rotxn_c = cc.read();

        assert!(GC_COUNT.load(Ordering::Acquire) == 0);

        // drop B
        drop(cc_rotxn_b);

        // gc count should be 0.
        assert!(GC_COUNT.load(Ordering::Acquire) == 0);

        // drop C
        drop(cc_rotxn_c);

        // gc count should be 0
        assert!(GC_COUNT.load(Ordering::Acquire) == 0);

        // drop A
        drop(cc_rotxn_a);

        // gc count should be 2 (A + B, C is still live)
        assert!(GC_COUNT.load(Ordering::Acquire) <= 2);
    }

    #[test]
    fn test_default() {
        EbrCell::<()>::default();
    }
}
