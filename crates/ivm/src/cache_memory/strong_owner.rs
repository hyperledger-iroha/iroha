//! Shared custody that frees its Arc backing before dropping funded payloads.
//!
//! The wrapper never exposes a Weak or raw Arc. Every strong reference uses
//! `Arc::into_inner`, so exactly one concurrent final release moves out the
//! payload after deallocating the Arc. Payload reservations then refund only
//! after that backing and their preceding owned fields have been destroyed.

use std::{ops::Deref, sync::Arc};

#[derive(Debug)]
pub(crate) struct StrongOwner<T>(Option<Arc<T>>);

impl<T> StrongOwner<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(Some(Arc::new(value)))
    }

    pub(crate) fn ptr_eq(left: &Self, right: &Self) -> bool {
        Arc::ptr_eq(left.arc(), right.arc())
    }

    fn arc(&self) -> &Arc<T> {
        self.0.as_ref().expect("live strong owner retains its Arc")
    }
}

impl<T> Clone for StrongOwner<T> {
    fn clone(&self) -> Self {
        Self(Some(Arc::clone(self.arc())))
    }
}

impl<T> Deref for StrongOwner<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.arc()
    }
}

impl<T> Drop for StrongOwner<T> {
    fn drop(&mut self) {
        // Unlike try_unwrap followed by dropping Err, into_inner guarantees
        // that one of the concurrent final releases receives the payload.
        // No Weak can retain the Arc allocation beyond this point.
        drop(self.0.take().and_then(Arc::into_inner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn clones_share_identity_and_final_concurrent_drop_destroys_payload_once() {
        struct Probe(Arc<AtomicUsize>);
        impl Drop for Probe {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let owner = StrongOwner::new(Probe(Arc::clone(&dropped)));
        let barrier = Barrier::new(9);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let borrower = owner.clone();
                assert!(StrongOwner::ptr_eq(&owner, &borrower));
                assert!(Arc::ptr_eq(&borrower.deref().0, &dropped));
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    drop(borrower);
                });
            }
            drop(owner);
            barrier.wait();
        });
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
}
