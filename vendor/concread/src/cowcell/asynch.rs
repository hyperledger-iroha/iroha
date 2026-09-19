//! Async CowCell - A concurrently readable cell with Arc
//!
//! See `CowCell` for more details.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// A conncurrently readable async cell.
///
/// This structure behaves in a similar manner to a `RwLock<T>`. However unlike
/// a `RwLock`, writes and parallel reads can be performed at the same time. This
/// means readers and writers do no block either other. Writers are serialised.
///
/// To achieve this a form of "copy-on-write" (or for Rust, clone on write) is
/// used. As a write transaction begins, we clone the existing data to a new
/// location that is capable of being mutated.
///
/// Readers are guaranteed that the content of the `CowCell` will live as long
/// as the read transaction is open, and will be consistent for the duration
/// of the transaction. There can be an "unlimited" number of readers in parallel
/// accessing different generations of data of the `CowCell`.
///
/// Writers are serialised and are guaranteed they have exclusive write access
/// to the structure.
#[derive(Debug, Default)]
pub struct CowCell<T> {
    write: Mutex<()>,
    active: Mutex<Arc<T>>,
}

/// A `CowCell` Write Transaction handle.
///
/// This allows mutation of the content of the `CowCell` without blocking or
/// affecting current readers.
///
/// Changes are only stored in this structure until you call commit. To abort/
/// rollback a change, don't call commit and allow the write transaction to
/// be dropped. This causes the `CowCell` to unlock allowing the next writer
/// to proceed.
pub struct CowCellWriteTxn<'a, T> {
    // Hold open the guard, and initiate the copy to here.
    work: Option<T>,
    read: Arc<T>,
    // This way we know who to contact for updating our data ....
    caller: &'a CowCell<T>,
    _guard: MutexGuard<'a, ()>,
}

/// A `CowCell` Read Transaction handle.
///
/// This allows safe reading of the value within the `CowCell`, that allows
/// no mutation of the value, and without blocking writers.
#[derive(Debug)]
pub struct CowCellReadTxn<T>(Arc<T>);

impl<T> Clone for CowCellReadTxn<T> {
    fn clone(&self) -> Self {
        CowCellReadTxn(self.0.clone())
    }
}

impl<T> CowCell<T>
where
    T: Clone,
{
    /// Create a new `CowCell` for storing type `T`. `T` must implement `Clone`
    /// to enable clone-on-write.
    pub fn new(data: T) -> Self {
        CowCell {
            write: Mutex::new(()),
            active: Mutex::new(Arc::new(data)),
        }
    }

    /// Begin a read transaction, returning a read guard. The content of
    /// the read guard is guaranteed to be consistent for the life time of the
    /// read - even if writers commit during.
    pub async fn read<'x>(&'x self) -> CowCellReadTxn<T> {
        let rwguard = self.active.lock().await;
        CowCellReadTxn(rwguard.clone())
        // rwguard ends here
    }

    /// Begin a write transaction, returning a write guard. The content of the
    /// write is only visible to this thread, and is not visible to any reader
    /// until `commit()` is called.
    pub async fn write<'x>(&'x self) -> CowCellWriteTxn<'x, T> {
        /* Take the exclusive write lock first */
        let mguard = self.write.lock().await;
        // We delay copying until the first get_mut.
        let read = {
            let rwguard = self.active.lock().await;
            rwguard.clone()
        };
        /* Now build the write struct */
        CowCellWriteTxn {
            work: None,
            read,
            caller: self,
            _guard: mguard,
        }
    }

    /// Attempt to create a write transaction. If it fails, and err
    /// is returned. On success the `Ok(guard)` is returned. See also
    /// `write(&self)`
    pub async fn try_write<'x>(&'x self) -> Option<CowCellWriteTxn<'x, T>> {
        /* Take the exclusive write lock first */
        if let Ok(mguard) = self.write.try_lock() {
            // We delay copying until the first get_mut.
            let read: Arc<_> = {
                let rwguard = self.active.lock().await;
                rwguard.clone()
            };
            /* Now build the write struct */
            Some(CowCellWriteTxn {
                work: None,
                read,
                caller: self,
                _guard: mguard,
            })
        } else {
            None
        }
    }

    async fn commit(&self, newdata: Option<T>) {
        if let Some(new_data) = newdata {
            let mut rwguard = self.active.lock().await;
            let new_inner = Arc::new(new_data);
            // now over-write the last value in the mutex.
            *rwguard = new_inner;
        }
        // If not some, we do nothing.
        // Done
    }
}

impl<T> Deref for CowCellReadTxn<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> CowCellWriteTxn<'_, T>
where
    T: Clone,
{
    /// Access a mutable pointer of the data in the `CowCell`. This data is only
    /// visible to the write transaction object in this thread, until you call
    /// `commit()`.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        if self.work.is_none() {
            let mut data: Option<T> = Some((*self.read).clone());
            std::mem::swap(&mut data, &mut self.work);
            // Should be the none we previously had.
            debug_assert!(data.is_none())
        }
        self.work.as_mut().expect("can not fail")
    }

    /// Update the inner value with a new one. This function exists to prevent a clone
    /// in the case where you take a read transaction and would otherwise use `mem::swap`
    pub fn replace(&mut self, value: T) {
        self.work = Some(value);
    }

    /// Commit the changes made in this write transactions to the `CowCell`.
    /// This will consume the transaction so no further changes can be made
    /// after this is called. Not calling this in a block, is equivalent to
    /// an abort/rollback of the transaction.
    pub async fn commit(self) {
        /* Write our data back to the CowCell */
        self.caller.commit(self.work).await;
    }
}

impl<T> Deref for CowCellWriteTxn<'_, T>
where
    T: Clone,
{
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        match &self.work {
            Some(v) => v,
            None => &self.read,
        }
    }
}

impl<T> DerefMut for CowCellWriteTxn<'_, T>
where
    T: Clone,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::CowCell;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_deref_mut() {
        let data: i64 = 0;
        let cc = CowCell::new(data);
        {
            /* Take a write txn */
            let mut cc_wrtxn = cc.write().await;
            *cc_wrtxn = 1;
            cc_wrtxn.commit().await;
        }
        let cc_rotxn = cc.read().await;
        assert_eq!(*cc_rotxn, 1);
    }

    #[tokio::test]
    async fn test_try_write() {
        let data: i64 = 0;
        let cc = CowCell::new(data);
        /* Take a write txn */
        let cc_wrtxn_a = cc.try_write().await;
        assert!(cc_wrtxn_a.is_some());
        /* Because we already hold the writ, the second is guaranteed to fail */
        let cc_wrtxn_a = cc.try_write().await;
        assert!(cc_wrtxn_a.is_none());
    }

    #[tokio::test]
    async fn test_simple_create() {
        let data: i64 = 0;
        let cc = CowCell::new(data);

        let cc_rotxn_a = cc.read().await;
        assert_eq!(*cc_rotxn_a, 0);

        {
            /* Take a write txn */
            let mut cc_wrtxn = cc.write().await;
            /* Get the data ... */
            {
                let mut_ptr = cc_wrtxn.get_mut();
                /* Assert it's 0 */
                assert_eq!(*mut_ptr, 0);
                *mut_ptr = 1;
                assert_eq!(*mut_ptr, 1);
            }
            assert_eq!(*cc_rotxn_a, 0);

            let cc_rotxn_b = cc.read().await;
            assert_eq!(*cc_rotxn_b, 0);
            /* The write txn and it's lock is dropped here */
            cc_wrtxn.commit().await;
        }

        /* Start a new txn and see it's still good */
        let cc_rotxn_c = cc.read().await;
        assert_eq!(*cc_rotxn_c, 1);
        assert_eq!(*cc_rotxn_a, 0);
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

    async fn test_gc_operation_thread(cc: Arc<CowCell<TestGcWrapper<i64>>>) {
        while GC_COUNT.load(Ordering::Acquire) < 50 {
            // thread::sleep(std::time::Duration::from_millis(200));
            {
                let mut cc_wrtxn = cc.write().await;
                {
                    let mut_ptr = cc_wrtxn.get_mut();
                    mut_ptr.data += 1;
                }
                cc_wrtxn.commit().await;
            }
        }
    }

    #[tokio::test]
    #[cfg_attr(miri, ignore)]
    async fn test_gc_operation() {
        GC_COUNT.store(0, Ordering::Release);
        let data = TestGcWrapper { data: 0 };
        let cc = Arc::new(CowCell::new(data));

        let _ = tokio::join!(
            tokio::task::spawn(test_gc_operation_thread(cc.clone())),
            tokio::task::spawn(test_gc_operation_thread(cc.clone())),
            tokio::task::spawn(test_gc_operation_thread(cc.clone())),
            tokio::task::spawn(test_gc_operation_thread(cc.clone())),
        );

        assert!(GC_COUNT.load(Ordering::Acquire) >= 50);
    }

    #[test]
    fn test_default() {
        CowCell::<()>::default();
    }
}
