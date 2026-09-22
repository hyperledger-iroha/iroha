//! Original retained hash reader/writer release custody across caller fences.
use super::super::retained_hash_slot::RetainedHashSlot;
use super::*;
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};
struct Probe {
    hashes: Arc<BlockHashes>,
    outer: Arc<crate::publication_lock::PublicationMutex>,
    calls: AtomicUsize,
    busy: AtomicUsize,
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let outer_busy = self.outer.try_lock().is_none();
        let hash_busy = self.hashes.map().unwrap().try_acquire_writer().is_none();
        self.busy.fetch_add(
            usize::from(outer_busy) + usize::from(hash_busy),
            Ordering::SeqCst,
        );
    }
}
struct Group<'a> {
    hash: RetainedHashSlot<'a, ()>,
    outer: Option<crate::publication_lock::PublicationGuard<'a>>,
    outer_retirement: Option<concread::release::DeferredRelease>,
}
impl Drop for Group<'_> {
    fn drop(&mut self) {
        self.hash.release_writers();
        self.outer_retirement = self.outer.take().map(|guard| guard.release_deferred());
    }
}

#[test]
fn retained_hash_slot_keeps_success_refusal_and_callback_unwind_behind_outer_release() {
    for replace in [false, true] {
        for failure in 0..3 {
            let hashes = Arc::new(BlockHashes::new(vec![hash(1)]));
            let outer = Arc::new(crate::publication_lock::PublicationMutex::default());
            let original = detached(&hashes, replace, &[2]);
            let pointer = original.get(0).map(std::ptr::from_ref);
            let probe = Arc::new(Probe {
                hashes: Arc::clone(&hashes),
                outer: Arc::clone(&outer),
                calls: AtomicUsize::new(0),
                busy: AtomicUsize::new(0),
            });
            let mut wait = hashes
                .map()
                .unwrap()
                .observe_reader_release()
                .wait_for_release();
            let waker = Waker::from(Arc::clone(&probe));
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            let mut group = Group {
                hash: RetainedHashSlot::new(original, &hashes),
                outer: Some(outer.lock()),
                outer_retirement: None,
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                group.hash.try_prepare(|_, owner| {
                    // Preserve the existing installation-before-writer contract.
                    assert!(owner.writer_available());
                    match failure {
                        1 => Err("capacity"),
                        2 => panic!("original installation callback"),
                        _ => Ok(()),
                    }
                })
            }));
            assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            let recovered = match failure {
                0 => {
                    assert!(matches!(result, Ok(Ok(()))));
                    Some(group.hash.recover_original())
                }
                1 => {
                    assert!(matches!(
                        result,
                        Ok(Err(mv::PublicationPreparationError::Admission("capacity")))
                    ));
                    Some(group.hash.recover_original())
                }
                _ => {
                    assert!(result.is_err());
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| group.hash.recover_original())).is_err()
                    );
                    None
                }
            };
            assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
            drop(group);
            assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
            assert_eq!(probe.busy.load(Ordering::SeqCst), 0);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_ready()
            );
            assert_eq!(
                hashes.view().iter().copied().collect::<Vec<_>>(),
                vec![hash(1)]
            );
            if let Some(original) = recovered {
                assert_eq!(original.get(0).map(std::ptr::from_ref), pointer);
                // The exact original remains usable after ordinary refusal; a
                // caught admission unwind is terminal and cannot use this path.
                prepare(original, &hashes).publish();
                assert_eq!(hashes.view().get(0).map(std::ptr::from_ref), pointer);
            }
        }
    }
}

#[test]
fn retained_hash_slot_publication_keeps_original_preflight_through_outer_unlock() {
    for replace in [false, true] {
        let hashes = Arc::new(BlockHashes::new(vec![hash(1)]));
        let outer = Arc::new(crate::publication_lock::PublicationMutex::default());
        let original = detached(&hashes, replace, &[2]);
        let probe = Arc::new(Probe {
            hashes: Arc::clone(&hashes),
            outer: Arc::clone(&outer),
            calls: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut wait = hashes
            .map()
            .unwrap()
            .observe_reader_release()
            .wait_for_release();
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let mut group = Group {
            hash: RetainedHashSlot::new(original, &hashes),
            outer: Some(outer.lock()),
            outer_retirement: None,
        };
        group.hash.try_prepare(|_, _| Ok::<_, ()>(())).unwrap();
        let retirement = group.hash.take_prepared().publish();
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        assert!(hashes.map().unwrap().try_acquire_writer().is_some());
        drop(group);
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            0,
            "actual preflight moved into original publication retirement"
        );
        drop(retirement);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.busy.load(Ordering::SeqCst), 0);
        let expected = if replace {
            vec![hash(2)]
        } else {
            vec![hash(1), hash(2)]
        };
        assert_eq!(hashes.view().iter().copied().collect::<Vec<_>>(), expected);
    }
}
