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

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, OnceLock, TryLockError};

/// Do not implement this. You don't need this negativity in your life.
pub trait LinCowCellCapable<R, U> {
    /// Create the first reader snapshot for a new instance.
    fn create_reader(&self) -> R;

    /// Create a writer that may be rolled back.
    fn create_writer(&self) -> U;

    /// Given the current active reader, and the writer to commit, update our
    /// main structure as mut self, and our previously linear generations based on
    /// what was updated.
    fn pre_commit(&mut self, new: U, prev: &R) -> R;
}

#[derive(Debug)]
/// A concurrently readable cell with linearised drop behaviour.
pub struct LinCowCell<T, R, U> {
    updater: PhantomData<U>,
    write: Arc<Mutex<WriteState<T, R>>>,
    active: Mutex<Arc<LinCowCellInner<R>>>,
}

#[derive(Debug)]
struct WriteState<T, R> {
    data: T,
    // The exact active generation is also available under the writer lock.
    // Adopting an owned writer never contends with the short-lived reader lock.
    current: Arc<LinCowCellInner<R>>,
}

#[derive(Debug)]
/// A write txn over a linear cell.
pub struct LinCowCellWriteTxn<'a, T, R, U> {
    caller: &'a LinCowCell<T, R, U>,
    // Allocate the cursor only during original acquisition. Every handoff moves
    // this same box; abort destroys it before releasing its base and lock.
    work: Box<U>,
    next: Arc<MaybeUninit<LinCowCellInner<R>>>,
    base: Arc<LinCowCellInner<R>>,
    guard: MutexGuard<'a, WriteState<T, R>>,
}

#[derive(Debug)]
/// An unpublished writer retaining its original root and base generation.
///
/// The original cursor allocation moves intact through handoff and retry.
/// Field order keeps shared nodes alive until that cursor and its allocation
/// are destroyed, including when the original cell has already been dropped.
pub struct LinCowCellOwned<T, R, U> {
    work: Box<U>,
    next: Arc<MaybeUninit<LinCowCellInner<R>>>,
    base: Arc<LinCowCellInner<R>>,
    root: Arc<Mutex<WriteState<T, R>>>,
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
struct LinCowCellInner<R> {
    // The original writer installs exactly one successor. A once-set link
    // avoids a lazily allocated OS mutex at publication on pthread platforms.
    pin: OnceLock<Arc<LinCowCellInner<R>>>,
    data: R,
}

#[derive(Debug)]
/// A read txn over a linear cell.
pub struct LinCowCellReadTxn<'a, T, R, U> {
    // We must outlive the root
    _caller: &'a LinCowCell<T, R, U>,
    // We pin the current version.
    work: Arc<LinCowCellInner<R>>,
}

impl<R> LinCowCellInner<R> {
    pub fn new(data: R) -> Self {
        LinCowCellInner {
            pin: OnceLock::new(),
            data,
        }
    }
}

impl<R> Drop for LinCowCellInner<R> {
    fn drop(&mut self) {
        // Ensure the default drop won't recursively drop the chain
        // Use Arc::into_inner so we only advance on unique ownership
        let mut current = self.pin.take();

        // Drop the chain iteratively to avoid stack overflow
        while let Some(arc) = current {
            // Try to get exclusive ownership of the next link
            match Arc::into_inner(arc) {
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

impl<T, R, U> LinCowCell<T, R, U>
where
    T: LinCowCellCapable<R, U>,
{
    /// Create a new linear 🐄 cell.
    pub fn new(data: T) -> Self {
        let current = Arc::new(LinCowCellInner::new(data.create_reader()));
        let active = Mutex::new(Arc::clone(&current));
        // pthread targets such as macOS allocate their native mutex lazily on
        // first use. Prepare this one long-lived reader lock during construction
        // so even the first publication, without a prior read, needs no lock
        // allocation. Subsequent generations reuse this same initialized lock.
        drop(active.lock().unwrap());
        LinCowCell {
            updater: PhantomData,
            write: Arc::new(Mutex::new(WriteState { data, current })),
            active,
        }
    }

    /// Begin a read txn
    pub fn read(&self) -> LinCowCellReadTxn<'_, T, R, U> {
        let rwguard = self.active.lock().unwrap();
        LinCowCellReadTxn {
            _caller: self,
            // inc the arc.
            work: rwguard.clone(),
        }
    }

    /// Begin a write txn
    pub fn write(&self) -> LinCowCellWriteTxn<'_, T, R, U> {
        let write_guard = self.write.lock().unwrap();
        let work = Box::new(write_guard.data.create_writer());
        LinCowCellWriteTxn {
            caller: self,
            work,
            next: Arc::new_uninit(),
            base: Arc::clone(&write_guard.current),
            guard: write_guard,
        }
    }

    /// Attempt a write txn
    pub fn try_write(&self) -> Option<LinCowCellWriteTxn<'_, T, R, U>> {
        self.write.try_lock().ok().map(|write_guard| {
            let work = Box::new(write_guard.data.create_writer());
            LinCowCellWriteTxn {
                caller: self,
                work,
                next: Arc::new_uninit(),
                base: Arc::clone(&write_guard.current),
                guard: write_guard,
            }
        })
    }

    /// Reacquire only the original writer lock without copying or allocating.
    ///
    /// Unlike a fully owned EBR value, a linear COW writer contains pointers to
    /// its base tree. Both the physical root and exact base must still match.
    pub fn try_write_owned(
        &self,
        owned: LinCowCellOwned<T, R, U>,
    ) -> Result<LinCowCellWriteTxn<'_, T, R, U>, (LinCowCellOwned<T, R, U>, OwnedWriteError)> {
        if !Arc::ptr_eq(&self.write, &owned.root) {
            return Err((owned, OwnedWriteError::Changed));
        }
        let guard = match self.write.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Err((owned, OwnedWriteError::Busy)),
            Err(TryLockError::Poisoned(_)) => return Err((owned, OwnedWriteError::Poisoned)),
        };
        if !Arc::ptr_eq(&guard.current, &owned.base) {
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

    fn commit(&self, write: LinCowCellWriteTxn<T, R, U>) {
        let LinCowCellWriteTxn {
            caller: _caller,
            work,
            mut next,
            base,
            mut guard,
        } = write;

        // Perform every lock and ownership check before pre_commit transfers
        // node ownership. The shell stays private until it is initialized.
        let slot = Arc::get_mut(&mut next).expect("unpublished successor must be uniquely owned");
        let mut rwguard = self.active.lock().unwrap();
        assert!(Arc::ptr_eq(&base, &guard.current));
        assert!(Arc::ptr_eq(&base, &rwguard));
        assert!(base.pin.get().is_none());

        // Consume the original cursor; moving out deallocates its box without
        // allocating or reconstructing any cursor or node at publication.
        let newdata = guard.data.pre_commit(*work, &base.data);
        slot.write(LinCowCellInner::new(newdata));
        // SAFETY: this is the original, uniquely owned writer shell. The line
        // above initialized its complete payload exactly once, and there is no
        // fallible operation between initialization and this conversion.
        let new_inner = unsafe { next.assume_init() };
        // Only the original writer can reach this link, and the retained base
        // Arc prevents destruction while it is set. No reader sets the link.
        base.pin
            .set(Arc::clone(&new_inner))
            .unwrap_or_else(|_| unreachable!("original generation already has a successor"));
        guard.current = Arc::clone(&new_inner);
        *rwguard = new_inner;
    }
}

impl<T, R, U> Deref for LinCowCellReadTxn<'_, T, R, U> {
    type Target = R;

    #[inline]
    fn deref(&self) -> &R {
        &self.work.data
    }
}

impl<T, R, U> AsRef<R> for LinCowCellReadTxn<'_, T, R, U> {
    #[inline]
    fn as_ref(&self) -> &R {
        &self.work.data
    }
}

impl<T, R, U> LinCowCellWriteTxn<'_, T, R, U>
where
    T: LinCowCellCapable<R, U>,
{
    #[inline]
    /// Get the mutable inner of this type
    pub fn get_mut(&mut self) -> &mut U {
        &mut self.work
    }

    /// Commit the active changes.
    pub fn commit(self) {
        /* Write our data back to the LinCowCell */
        self.caller.commit(self);
    }

    /// Retain the original unpublished work and release its writer lock.
    /// No publication or reconstruction takes place.
    pub fn detach(self) -> LinCowCellOwned<T, R, U> {
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

impl<T, R, U> AsRef<U> for LinCowCellOwned<T, R, U> {
    fn as_ref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U> Deref for LinCowCellWriteTxn<'_, T, R, U> {
    type Target = U;

    #[inline]
    fn deref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U> DerefMut for LinCowCellWriteTxn<'_, T, R, U> {
    #[inline]
    fn deref_mut(&mut self) -> &mut U {
        &mut self.work
    }
}

impl<T, R, U> AsRef<U> for LinCowCellWriteTxn<'_, T, R, U> {
    #[inline]
    fn as_ref(&self) -> &U {
        &self.work
    }
}

impl<T, R, U> AsMut<U> for LinCowCellWriteTxn<'_, T, R, U> {
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        &mut self.work
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
        fn create_reader(&self) -> TestDataReadTxn {
            TestDataReadTxn { x: self.x }
        }

        fn create_writer(&self) -> TestDataWriteTxn {
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
        fn create_reader(&self) -> TestGcWrapperReadTxn<T> {
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }

        fn create_writer(&self) -> TestGcWrapperWriteTxn<T> {
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
        fn create_reader(&self) -> TestGcWrapperReadTxn<T> {
            TestGcWrapperReadTxn {
                _data: self.data.clone(),
            }
        }

        fn create_writer(&self) -> TestGcWrapperWriteTxn<T> {
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
