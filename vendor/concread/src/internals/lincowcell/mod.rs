//! A CowCell with linear drop behaviour
//!
//! YOU SHOULD NOT USE THIS TYPE! Normally concurrent cells do NOT require the linear dropping
//! behaviour that this implements, and it will only make your application
//! worse for it. Consider `CowCell` and `EbrCell` instead.

/*
 * The reason this exists is for protecting the major concurrently readable structures
 * that can corrupt if intermediate transactions are removed early. Effectively what we
 * need to create is:
 *
 * [ A ] -> [ B ] -> [ C ] -> [ Write Head ]
 *   ^        ^        ^
 *   read     read     read
 *
 * This way if we drop the reader on B:
 *
 * [ A ] -> [ B ] -> [ C ] -> [ Write Head ]
 *   ^                 ^
 *   read              read
 *
 * Notice that A is not dropped. It's only when A is dropped:
 *
 * [ A ] -> [ B ] -> [ C ] -> [ Write Head ]
 *                     ^
 *                     read
 *
 * [ X ] -> [ B ] -> [ C ] -> [ Write Head ]
 *                     ^
 *                     read
 * [ X ] -> [ X ] -> [ C ] -> [ Write Head ]
 *                     ^
 *                     read
 *
 *                   [ C ] -> [ Write Head ]
 *                     ^
 *                     read
 *
 * At this point we drop A and B. To achieve this we need to consider that:
 * - If WriteHead is dropped, C continues to live.
 * - If A/B are dropped, we don't affect C.
 * - Everything is dropped in order until a read txn exists.
 * - When we drop the main structure, no readers can exist.
 * - A writer must be able to commit to a stable location.
 *
 *
 *   T        T        T
 * [ A ] -> [ B ] -> [ C ] -> [ Write Head ]
 *   ^        ^        ^
 *   RRR      RR       R
 *
 *
 * As the write head proceeds, it must be able to interact with past versions to commit
 * garbage that is "last seen" in the formers generation.
 *
 */

use std::alloc::Layout;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, OnceLock, TryLockError};

mod shared_allocation;
use shared_allocation::{Reserved, Shared};

/// Explicitly unaccounted shell ownership; this provides no admission policy.
#[derive(Debug)]
pub struct Untracked;

/// Actual control-block layouts allocated before constructing a writer.
#[derive(Clone, Copy, Debug)]
pub struct WriterLayouts {
    /// Original mutable cursor and its allocation control block.
    pub cursor: Layout,
    /// Original next-reader shell and its allocation control block.
    pub reader: Layout,
}

/// Move-only prepaid custody for the original cursor and next-reader shell.
///
/// These charges do not fund the cursor's nested buffers, nodes or payloads.
/// A complete caller must reserve those before their construction as well.
#[derive(Debug)]
pub struct WriterCharges<Charge> {
    /// Custody held until the original cursor allocation is reclaimed.
    pub cursor: Charge,
    /// Custody retained by the original published or abandoned reader shell.
    pub reader: Charge,
}

/// Original shell charges and the move-only input admitted for one writer.
///
/// The input is consumed by the original writer constructor after both shells
/// have been allocated. Reattaching a detached writer does not admit or construct
/// another input. Neither this type nor the cell supplies a default input.
#[derive(Debug)]
pub struct WriterAdmission<Charge, Input> {
    /// Prepaid custody for the original cursor and next-reader shells.
    pub charges: WriterCharges<Charge>,
    /// The original constructor input admitted while the writer was locked.
    pub input: Input,
}

/// Do not implement this. You don't need this negativity in your life.
pub trait LinCowCellCapable<R, U> {
    /// Move-only input explicitly supplied by original writer admission.
    type WriterInput;

    /// Create the first reader snapshot for a new instance.
    fn create_reader(&self) -> R;

    /// Create a writer that may be rolled back.
    fn create_writer(&self, input: Self::WriterInput) -> U;

    /// Given the current active reader, and the writer to commit, update our
    /// main structure as mut self, and our previously linear generations based on
    /// what was updated.
    fn pre_commit(&mut self, new: U, prev: &R) -> R;
}

#[derive(Debug)]
/// A concurrently readable cell with linearised drop behaviour.
pub struct LinCowCell<T, R, U, Charge = Untracked> {
    updater: PhantomData<U>,
    write: Arc<Mutex<WriteState<T, R, Charge>>>,
    active: Mutex<Shared<LinCowCellInner<R, Charge>, Charge>>,
}

#[derive(Debug)]
struct WriteState<T, R, Charge> {
    data: T,
    // The exact active generation is also available under the writer lock.
    // Adopting an owned writer never contends with the short-lived reader lock.
    current: Shared<LinCowCellInner<R, Charge>, Charge>,
}

#[derive(Debug)]
/// A write txn over a linear cell.
pub struct LinCowCellWriteTxn<'a, T, R, U, Charge = Untracked> {
    caller: &'a LinCowCell<T, R, U, Charge>,
    // Unlock before abort destroys a charge: refunds can synchronously wake a
    // retry that reenters the cell. The retained base protects the private work
    // just as it does after detachment, until that work has been destroyed.
    guard: MutexGuard<'a, WriteState<T, R, Charge>>,
    // Allocate the cursor only during original acquisition. Every handoff moves
    // this same allocation; abort destroys it before releasing its base.
    work: Shared<U, Charge>,
    next: Reserved<LinCowCellInner<R, Charge>, Charge>,
    base: Shared<LinCowCellInner<R, Charge>, Charge>,
}

#[derive(Debug)]
/// An unpublished writer retaining its original root and base generation.
///
/// The original cursor allocation moves intact through handoff and retry.
/// Field order keeps shared nodes alive until that cursor and its allocation
/// are destroyed, including when the original cell has already been dropped.
pub struct LinCowCellOwned<T, R, U, Charge = Untracked> {
    work: Shared<U, Charge>,
    next: Reserved<LinCowCellInner<R, Charge>, Charge>,
    base: Shared<LinCowCellInner<R, Charge>, Charge>,
    root: Arc<Mutex<WriteState<T, R, Charge>>>,
}

/// Why an original unpublished writer could not be reacquired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedWriteError {
    /// The original writer lock is currently held.
    Busy,
    /// The original writer lock was poisoned by an unwind.
    Poisoned,
    /// The target is foreign or its original reader generation has changed.
    Changed,
}

#[derive(Debug)]
struct LinCowCellInner<R, Charge> {
    // The original writer installs exactly one successor. A once-set link
    // avoids a lazily allocated OS mutex at publication on pthread platforms.
    pin: OnceLock<Shared<LinCowCellInner<R, Charge>, Charge>>,
    data: R,
}

#[derive(Debug)]
/// A read txn over a linear cell.
pub struct LinCowCellReadTxn<'a, T, R, U, Charge = Untracked> {
    // We must outlive the root
    _caller: &'a LinCowCell<T, R, U, Charge>,
    // We pin the current version.
    work: Shared<LinCowCellInner<R, Charge>, Charge>,
}

impl<R, Charge> LinCowCellInner<R, Charge> {
    pub fn new(data: R) -> Self {
        LinCowCellInner {
            pin: OnceLock::new(),
            data,
        }
    }
}

impl<R, Charge> Drop for LinCowCellInner<R, Charge> {
    fn drop(&mut self) {
        // Ensure the default drop won't recursively drop the chain
        // Consuming the last strong reference also reclaims its exact allocation.
        let mut current = self.pin.take();

        // Drop the chain iteratively to avoid stack overflow
        while let Some(arc) = current {
            // Try to get exclusive ownership of the next link
            match arc.into_inner() {
                Some(mut inner) => {
                    // Continue with the next link.
                    current = inner.pin.take();
                }
                None => {
                    // Another strong reference exists; stop without breaking its chain
                    break;
                }
            }
        }
    }
}

impl<T, R, U, Charge> LinCowCell<T, R, U, Charge>
where
    T: LinCowCellCapable<R, U>,
{
    /// Exact layout of each reader generation, including its reference counter.
    pub fn reader_allocation_layout() -> Layout {
        Reserved::<LinCowCellInner<R, Charge>, Charge>::layout()
    }

    /// Exact original cursor and next-reader layouts; nested storage is separate.
    pub fn writer_allocation_layouts() -> WriterLayouts {
        WriterLayouts {
            cursor: Reserved::<U, Charge>::layout(),
            reader: Self::reader_allocation_layout(),
        }
    }

    /// Construct the initial reader under its already prepaid allocation charge.
    ///
    /// Its shell is allocated before `create_reader`. The permanent root Arc,
    /// reader mutex, input T and all nested storage require separate admission.
    pub fn new_charged(data: T, reader_charge: Charge) -> Self {
        let shell = Reserved::new(reader_charge);
        let current = shell.initialize(LinCowCellInner::new(data.create_reader()));
        let active = Mutex::new(current.clone());
        // Initialize both permanent native mutexes during construction. A first
        // refused writer must not allocate a lazy platform mutex at admission.
        drop(active.lock().unwrap());
        let write = Arc::new(Mutex::new(WriteState { data, current }));
        drop(write.lock().unwrap());
        LinCowCell {
            updater: PhantomData,
            write,
            active,
        }
    }

    /// Begin a read transaction retaining the original generation and its charge.
    pub fn read(&self) -> LinCowCellReadTxn<'_, T, R, U, Charge> {
        let rwguard = self.active.lock().unwrap();
        LinCowCellReadTxn {
            _caller: self,
            work: rwguard.clone(),
        }
    }

    /// Admit both original shells before allocating either or creating a cursor.
    ///
    /// This callback must draw from one complete prepaid operation. It does not
    /// retroactively fund input T, nested cursor buffers, nodes or payloads.
    /// The input constructor can destroy nested ownership before this call
    /// unwinds. Callback-bearing refunds in that input must use the caller's
    /// original notification-deferral scope around acquisition and construction,
    /// so callbacks run only after its physical guards have been released.
    pub fn write_charged<E>(
        &self,
        admit: impl FnOnce(&T, WriterLayouts) -> Result<WriterAdmission<Charge, T::WriterInput>, E>,
    ) -> Result<LinCowCellWriteTxn<'_, T, R, U, Charge>, E> {
        let guard = self.write.lock().unwrap();
        let admission = admit(&guard.data, Self::writer_allocation_layouts())?;
        Ok(self.create_writer(guard, admission))
    }

    /// Admit original shells only after acquiring the writer without waiting.
    ///
    /// Contention or poison returns None without invoking admission or allocating.
    /// Use `is_poisoned` to distinguish poison from physical contention.
    pub fn try_write_charged<E>(
        &self,
        admit: impl FnOnce(&T, WriterLayouts) -> Result<WriterAdmission<Charge, T::WriterInput>, E>,
    ) -> Result<Option<LinCowCellWriteTxn<'_, T, R, U, Charge>>, E> {
        let Ok(guard) = self.write.try_lock() else {
            return Ok(None);
        };
        let admission = admit(&guard.data, Self::writer_allocation_layouts())?;
        Ok(Some(self.create_writer(guard, admission)))
    }

    fn create_writer<'a>(
        &'a self,
        guard: MutexGuard<'a, WriteState<T, R, Charge>>,
        admission: WriterAdmission<Charge, T::WriterInput>,
    ) -> LinCowCellWriteTxn<'a, T, R, U, Charge> {
        // Field order also releases the writer lock before shell refunds when
        // create_writer unwinds. A refund may invoke arbitrary Waker::wake code.
        let construction = (
            guard,
            Reserved::new(admission.charges.cursor),
            Reserved::new(admission.charges.reader),
        );
        let value = construction.0.data.create_writer(admission.input);
        let (guard, work, next) = construction;
        let work = work.initialize(value);
        LinCowCellWriteTxn {
            caller: self,
            work,
            next,
            base: guard.current.clone(),
            guard,
        }
    }

    /// Reacquire only the original writer lock without copying or allocating.
    ///
    /// Unlike a fully owned EBR value, a linear COW writer contains pointers to
    /// its base tree. Both the physical root and exact base must still match.
    pub fn try_write_owned(
        &self,
        owned: LinCowCellOwned<T, R, U, Charge>,
    ) -> Result<
        LinCowCellWriteTxn<'_, T, R, U, Charge>,
        (LinCowCellOwned<T, R, U, Charge>, OwnedWriteError),
    > {
        if !Arc::ptr_eq(&self.write, &owned.root) {
            return Err((owned, OwnedWriteError::Changed));
        }
        let guard = match self.write.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Err((owned, OwnedWriteError::Busy)),
            Err(TryLockError::Poisoned(_)) => return Err((owned, OwnedWriteError::Poisoned)),
        };
        if !Shared::ptr_eq(&guard.current, &owned.base) {
            return Err((owned, OwnedWriteError::Changed));
        }
        let LinCowCellOwned {
            work,
            next,
            base,
            root,
        } = owned;
        // The returned transaction borrows this same root through its caller.
        drop(root);
        Ok(LinCowCellWriteTxn {
            caller: self,
            work,
            next,
            base,
            guard,
        })
    }

    /// Whether the original writer mutex is poisoned.
    pub fn is_poisoned(&self) -> bool {
        self.write.is_poisoned()
    }

    fn commit(&self, write: LinCowCellWriteTxn<T, R, U, Charge>) {
        let LinCowCellWriteTxn {
            caller: _caller,
            work,
            next,
            base,
            mut guard,
        } = write;

        // Perform every lock and ownership check before pre_commit transfers
        // node ownership. The shell stays private until it is initialized.
        let mut rwguard = self.active.lock().unwrap();
        assert!(Shared::ptr_eq(&base, &guard.current));
        assert!(Shared::ptr_eq(&base, &rwguard));
        assert!(base.pin.get().is_none());

        // Reclaim the original cursor block, retain its charge through the
        // consuming callback, then initialize the already allocated reader shell.
        let (newdata, cursor_charge) = work
            .into_inner()
            .expect("original cursor must be uniquely owned")
            .consume(|work| guard.data.pre_commit(work, &base.data));
        let new_inner = next.initialize(LinCowCellInner::new(newdata));
        // Only the original writer can reach this link, and the retained base
        // owner prevents destruction while it is set. No reader sets the link.
        base.pin
            .set(new_inner.clone())
            .unwrap_or_else(|_| unreachable!("original generation already has a successor"));
        guard.current = new_inner.clone();
        *rwguard = new_inner;
        // No user charge destructor runs between ownership transfer and reader
        // publication, or under either physical lock. Its shell was already
        // freed before pre_commit.
        drop(rwguard);
        drop(guard);
        drop(base);
        drop(cursor_charge);
    }
}

impl<T, R, U, Charge> Deref for LinCowCellReadTxn<'_, T, R, U, Charge> {
    type Target = R;

    #[inline]
    fn deref(&self) -> &R {
        &self.work.data
    }
}

impl<T, R, U, Charge> AsRef<R> for LinCowCellReadTxn<'_, T, R, U, Charge> {
    #[inline]
    fn as_ref(&self) -> &R {
        &self.work.data
    }
}

impl<T, R, U, Charge> LinCowCellWriteTxn<'_, T, R, U, Charge>
where
    T: LinCowCellCapable<R, U>,
{
    #[inline]
    /// Get the mutable inner of this type
    pub fn get_mut(&mut self) -> &mut U {
        Shared::get_mut(&mut self.work).expect("original cursor must be uniquely owned")
    }

    /// Commit the active changes.
    pub fn commit(self) {
        /* Write our data back to the LinCowCell */
        self.caller.commit(self);
    }

    /// Retain the original unpublished work and release its writer lock.
    /// No publication or reconstruction takes place.
    pub fn detach(self) -> LinCowCellOwned<T, R, U, Charge> {
        let Self {
            caller,
            work,
            next,
            base,
            guard,
        } = self;
        let root = Arc::clone(&caller.write);
        drop(guard);
        LinCowCellOwned {
            work,
            next,
            base,
            root,
        }
    }
}

impl<T, R, U, Charge> AsRef<U> for LinCowCellOwned<T, R, U, Charge> {
    fn as_ref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U, Charge> Deref for LinCowCellWriteTxn<'_, T, R, U, Charge> {
    type Target = U;

    #[inline]
    fn deref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U, Charge> DerefMut for LinCowCellWriteTxn<'_, T, R, U, Charge> {
    #[inline]
    fn deref_mut(&mut self) -> &mut U {
        Shared::get_mut(&mut self.work).expect("original cursor must be uniquely owned")
    }
}

impl<T, R, U, Charge> AsRef<U> for LinCowCellWriteTxn<'_, T, R, U, Charge> {
    #[inline]
    fn as_ref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U, Charge> AsMut<U> for LinCowCellWriteTxn<'_, T, R, U, Charge> {
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        Shared::get_mut(&mut self.work).expect("original cursor must be uniquely owned")
    }
}

impl<T, R, U> LinCowCell<T, R, U, Untracked>
where
    T: LinCowCellCapable<R, U>,
{
    /// Construct explicitly unaccounted generation shells.
    pub fn new(data: T) -> Self {
        Self::new_charged(data, Untracked)
    }

    /// Construct an explicitly unaccounted input under the original writer lock.
    pub fn write_with(
        &self,
        input: impl FnOnce(&T) -> T::WriterInput,
    ) -> LinCowCellWriteTxn<'_, T, R, U> {
        self.write_charged(|data, _| {
            Ok::<_, std::convert::Infallible>(WriterAdmission {
                charges: WriterCharges {
                    cursor: Untracked,
                    reader: Untracked,
                },
                input: input(data),
            })
        })
        .unwrap_or_else(|never| match never {})
    }

    /// Try an unaccounted writer, constructing its input only after locking.
    pub fn try_write_with(
        &self,
        input: impl FnOnce(&T) -> T::WriterInput,
    ) -> Option<LinCowCellWriteTxn<'_, T, R, U>> {
        self.try_write_charged(|data, _| {
            Ok::<_, std::convert::Infallible>(WriterAdmission {
                charges: WriterCharges {
                    cursor: Untracked,
                    reader: Untracked,
                },
                input: input(data),
            })
        })
        .unwrap_or_else(|never| match never {})
    }
}

impl<T, R, U> LinCowCell<T, R, U, Untracked>
where
    T: LinCowCellCapable<R, U, WriterInput = ()>,
{
    /// Begin an explicitly unaccounted writer with unit constructor input.
    pub fn write(&self) -> LinCowCellWriteTxn<'_, T, R, U> {
        self.write_with(|_| ())
    }

    /// Try an unaccounted unit-input writer without waiting.
    pub fn try_write(&self) -> Option<LinCowCellWriteTxn<'_, T, R, U>> {
        self.try_write_with(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::LinCowCell;
    use super::LinCowCellCapable;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::scope;
    use std::time::Instant;

    #[derive(Debug)]
    struct TestData {
        x: i64,
    }

    #[derive(Debug)]
    struct TestDataReadTxn {
        x: i64,
    }

    #[derive(Debug)]
    struct TestDataWriteTxn {
        x: i64,
    }

    impl LinCowCellCapable<TestDataReadTxn, TestDataWriteTxn> for TestData {
        type WriterInput = ();

        fn create_reader(&self) -> TestDataReadTxn {
            TestDataReadTxn { x: self.x }
        }

        fn create_writer(&self, (): Self::WriterInput) -> TestDataWriteTxn {
            TestDataWriteTxn { x: self.x }
        }

        fn pre_commit(
            &mut self,
            new: TestDataWriteTxn,
            _prev: &TestDataReadTxn,
        ) -> TestDataReadTxn {
            // Update self if needed.
            self.x = new.x;
            // return a new reader.
            TestDataReadTxn { x: new.x }
        }
    }

    #[test]
    fn test_simple_create() {
        let data = TestData { x: 0 };
        let cc = LinCowCell::new(data);

        let cc_rotxn_a = cc.read();
        println!("cc_rotxn_a -> {:?}", cc_rotxn_a);
        assert_eq!(cc_rotxn_a.work.data.x, 0);

        {
            /* Take a write txn */
            let mut cc_wrtxn = cc.write();
            println!("cc_wrtxn -> {:?}", cc_wrtxn);
            assert_eq!(cc_wrtxn.work.x, 0);
            assert_eq!(cc_wrtxn.as_ref().x, 0);
            {
                let mut_ptr = cc_wrtxn.get_mut();
                /* Assert it's 0 */
                assert_eq!(mut_ptr.x, 0);
                mut_ptr.x = 1;
                assert_eq!(mut_ptr.x, 1);
            }
            // Check we haven't mutated the old data.
            assert_eq!(cc_rotxn_a.work.data.x, 0);
        }
        // The writer is dropped here. Assert no changes.
        assert_eq!(cc_rotxn_a.work.data.x, 0);
        {
            /* Take a new write txn */
            let mut cc_wrtxn = cc.write();
            println!("cc_wrtxn -> {:?}", cc_wrtxn);
            assert_eq!(cc_wrtxn.work.x, 0);
            assert_eq!(cc_wrtxn.as_ref().x, 0);
            {
                let mut_ptr = cc_wrtxn.get_mut();
                /* Assert it's 0 */
                assert_eq!(mut_ptr.x, 0);
                mut_ptr.x = 2;
                assert_eq!(mut_ptr.x, 2);
            }
            // Check we haven't mutated the old data.
            assert_eq!(cc_rotxn_a.work.data.x, 0);
            // Now commit
            cc_wrtxn.commit();
        }
        // Should not be perceived by the old txn.
        assert_eq!(cc_rotxn_a.work.data.x, 0);
        let cc_rotxn_c = cc.read();
        // Is visible to the new one though.
        assert_eq!(cc_rotxn_c.work.data.x, 2);
    }

    // == mt tests ==

    fn mt_writer(cc: &LinCowCell<TestData, TestDataReadTxn, TestDataWriteTxn>) {
        let mut last_value: i64 = 0;
        while last_value < 500 {
            let mut cc_wrtxn = cc.write();
            {
                let mut_ptr = cc_wrtxn.get_mut();
                assert!(mut_ptr.x >= last_value);
                last_value = mut_ptr.x;
                mut_ptr.x += 1;
            }
            cc_wrtxn.commit();
        }
    }

    fn rt_writer(cc: &LinCowCell<TestData, TestDataReadTxn, TestDataWriteTxn>) {
        let mut last_value: i64 = 0;
        while last_value < 500 {
            let cc_rotxn = cc.read();
            {
                assert!(cc_rotxn.work.data.x >= last_value);
                last_value = cc_rotxn.work.data.x;
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_multithread_create() {
        let start = Instant::now();
        // Create the new cowcell.
        let data = TestData { x: 0 };
        let cc = LinCowCell::new(data);

        assert!(scope(|scope| {
            let cc_ref = &cc;

            let readers: Vec<_> = (0..7)
                .map(|_| {
                    scope.spawn(move || {
                        rt_writer(cc_ref);
                    })
                })
                .collect();

            let writers: Vec<_> = (0..3)
                .map(|_| {
                    scope.spawn(move || {
                        mt_writer(cc_ref);
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
        print!("Arc MT create :{:?} ", end - start);
    }

    static GC_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Clone)]
    struct TestGcWrapper<T> {
        data: T,
    }

    #[derive(Debug)]
    struct TestGcWrapperReadTxn<T> {
        _data: T,
    }

    #[derive(Debug)]
    struct TestGcWrapperWriteTxn<T> {
        data: T,
    }

    impl<T: Clone> LinCowCellCapable<TestGcWrapperReadTxn<T>, TestGcWrapperWriteTxn<T>>
        for TestGcWrapper<T>
    {
        type WriterInput = ();

        fn create_reader(&self) -> TestGcWrapperReadTxn<T> {
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }

        fn create_writer(&self, (): Self::WriterInput) -> TestGcWrapperWriteTxn<T> {
            TestGcWrapperWriteTxn {
                data: self.data.clone(),
            }
        }

        fn pre_commit(
            &mut self,
            new: TestGcWrapperWriteTxn<T>,
            _prev: &TestGcWrapperReadTxn<T>,
        ) -> TestGcWrapperReadTxn<T> {
            // Update self if needed.
            self.data = new.data.clone();
            // return a new reader.
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }
    }

    impl<T> Drop for TestGcWrapperReadTxn<T> {
        fn drop(&mut self) {
            // Add to the atomic counter ...
            GC_COUNT.fetch_add(1, Ordering::Release);
        }
    }

    fn test_gc_operation_thread(
        cc: &LinCowCell<TestGcWrapper<i64>, TestGcWrapperReadTxn<i64>, TestGcWrapperWriteTxn<i64>>,
    ) {
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
        let cc = LinCowCell::new(data);

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

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_long_chain_drop_no_stack_overflow() {
        let data = TestData { x: 0 };
        let cc = LinCowCell::new(data);

        // Simulate a read txn that is not dropped.
        let initial_read = cc.read();

        // Create a long chain of versions by committing many writes.
        for i in 0..10000 {
            let mut write_txn = cc.write();
            write_txn.get_mut().x = i;
            write_txn.commit();
        }

        drop(initial_read);

        // Verify the final state is correct.
        let final_read = cc.read();
        assert_eq!(final_read.work.data.x, 9999);
    }
}

#[cfg(test)]
mod tests_linear {
    use super::LinCowCell;
    use super::LinCowCellCapable;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static GC_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Clone)]
    struct TestGcWrapper<T> {
        data: T,
    }

    #[derive(Debug)]
    struct TestGcWrapperReadTxn<T> {
        _data: T,
    }

    #[derive(Debug)]
    struct TestGcWrapperWriteTxn<T> {
        data: T,
    }

    impl<T: Clone> LinCowCellCapable<TestGcWrapperReadTxn<T>, TestGcWrapperWriteTxn<T>>
        for TestGcWrapper<T>
    {
        type WriterInput = ();

        fn create_reader(&self) -> TestGcWrapperReadTxn<T> {
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }

        fn create_writer(&self, (): Self::WriterInput) -> TestGcWrapperWriteTxn<T> {
            TestGcWrapperWriteTxn {
                data: self.data.clone(),
            }
        }

        fn pre_commit(
            &mut self,
            new: TestGcWrapperWriteTxn<T>,
            _prev: &TestGcWrapperReadTxn<T>,
        ) -> TestGcWrapperReadTxn<T> {
            // Update self if needed.
            self.data = new.data.clone();
            // return a new reader.
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }
    }

    impl<T> Drop for TestGcWrapperReadTxn<T> {
        fn drop(&mut self) {
            // Add to the atomic counter ...
            GC_COUNT.fetch_add(1, Ordering::Release);
        }
    }

    /*
     * This tests an important property of the lincowcell over the cow cell
     * that read txns are dropped *in order*.
     */
    #[test]
    fn test_gc_operation_linear() {
        GC_COUNT.store(0, Ordering::Release);
        assert!(GC_COUNT.load(Ordering::Acquire) == 0);
        let data = TestGcWrapper { data: 0 };
        let cc = LinCowCell::new(data);

        // Open a read A.
        let cc_rotxn_a = cc.read();
        let cc_rotxn_a_2 = cc.read();
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

        // Drop the second A, should not trigger yet.
        drop(cc_rotxn_a_2);
        assert!(GC_COUNT.load(Ordering::Acquire) == 0);

        // drop A
        drop(cc_rotxn_a);

        // gc count should be 2 (A + B, C is still live)
        assert!(GC_COUNT.load(Ordering::Acquire) == 2);
    }
}

#[cfg(test)]
mod writer_input_tests {
    use super::{LinCowCell, LinCowCellCapable};

    struct Data(usize);

    impl LinCowCellCapable<usize, Box<usize>> for Data {
        type WriterInput = Box<usize>;

        fn create_reader(&self) -> usize {
            self.0
        }

        fn create_writer(&self, mut input: Self::WriterInput) -> Box<usize> {
            *input += self.0;
            input
        }

        fn pre_commit(&mut self, new: Box<usize>, _previous: &usize) -> usize {
            self.0 = *new;
            self.0
        }
    }

    #[test]
    fn untracked_entry_points_consume_explicit_input_only_under_original_lock() {
        let cell = LinCowCell::new(Data(10));
        let input = Box::new(7);
        let original = input.as_ref() as *const usize;
        let writer = cell.write_with(|data| {
            assert_eq!(data.0, 10);
            assert!(cell
                .try_write_with(|_| panic!("busy constructor input was evaluated"))
                .is_none());
            input
        });
        assert_eq!(writer.as_ref().as_ref() as *const usize, original);
        assert_eq!(**writer, 17);
        writer.commit();
        assert_eq!(*cell.read(), 17);
        let writer = cell
            .try_write_with(|data| {
                assert_eq!(data.0, 17);
                Box::new(3)
            })
            .unwrap();
        assert_eq!(**writer, 20);
        drop(writer);
        assert_eq!(*cell.read(), 17);
    }
}
