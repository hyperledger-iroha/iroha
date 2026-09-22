//! Exact funded hash publication retains native reader/writer wake custody.
use super::*;
use mv::PublicationPreparationError;
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};
fn hash(n: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::new([n]))
}
struct ObserveFence {
    fence: Arc<Mutex<()>>,
    calls: AtomicUsize,
    busy: AtomicUsize,
}
impl Wake for ObserveFence {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if matches!(
            self.fence.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ) {
            self.busy.fetch_add(1, Ordering::SeqCst);
        }
    }
}
struct OriginalGroup<'a> {
    hash: BlockHashField<'a>,
    fence: Option<MutexGuard<'a, ()>>,
}
impl Drop for OriginalGroup<'_> {
    fn drop(&mut self) {
        mv::BlockRetirement::release_writers(&mut self.hash);
        drop(self.fence.take());
    }
}

#[test]
fn attached_hash_preserves_original_nodes_and_both_wakes_until_joint_drop() {
    for replacement in [false, true] {
        for publish in [false, true] {
            let owner = BlockHashes::new(vec![hash(1), hash(2)]);
            let old = owner.view();
            let mut original = if replacement {
                owner.block_and_revert()
            } else {
                owner.block()
            };
            original.push(hash(3));
            let pointer = std::ptr::from_ref(original.get(0).unwrap());
            let expected = original.iter().copied().collect::<Vec<_>>();
            let fence = Arc::new(Mutex::new(()));
            let callback = Arc::new(ObserveFence {
                fence: Arc::clone(&fence),
                calls: AtomicUsize::new(0),
                busy: AtomicUsize::new(0),
            });
            let waker = Waker::from(Arc::clone(&callback));
            let mut cx = Context::from_waker(&waker);
            let mut writer = owner.released.observe().wait_for_release();
            let mut reader = owner
                .map()
                .unwrap()
                .observe_reader_release()
                .wait_for_release();
            assert!(Pin::new(&mut writer).poll(&mut cx).is_pending());
            assert!(Pin::new(&mut reader).poll(&mut cx).is_pending());
            let mut group = OriginalGroup {
                hash: BlockHashField::new(original),
                fence: Some(fence.lock().unwrap()),
            };
            group.hash.try_prepare_publication().unwrap();
            assert_eq!(
                callback.calls.load(Ordering::SeqCst),
                0,
                "preparation must not release an advisory reader"
            );
            assert_eq!(owner.committed_height(), 2);
            assert_eq!(
                old.iter().copied().collect::<Vec<_>>(),
                vec![hash(1), hash(2)]
            );
            if publish {
                group.hash.publish_prepared();
            } else {
                mv::BlockRetirement::release_writers(&mut group.hash);
            }
            assert_eq!(callback.calls.load(Ordering::SeqCst), 0);
            assert!(Pin::new(&mut writer).poll(&mut cx).is_pending());
            assert!(Pin::new(&mut reader).poll(&mut cx).is_pending());
            drop(group);
            assert_eq!(callback.calls.load(Ordering::SeqCst), 2);
            assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
            assert!(Pin::new(&mut writer).poll(&mut cx).is_ready());
            assert!(Pin::new(&mut reader).poll(&mut cx).is_ready());
            let current = owner.view();
            assert_eq!(
                current.iter().copied().collect::<Vec<_>>(),
                if publish {
                    expected
                } else {
                    vec![hash(1), hash(2)]
                }
            );
            if publish {
                assert_eq!(std::ptr::from_ref(current.get(0).unwrap()), pointer);
            }
            assert_eq!(owner.committed_height(), current.len());
        }
    }
}

#[test]
fn attached_hash_busy_stale_and_unfilled_refusals_are_terminal_and_keep_exact_roots() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let mut a = owner.block();
    a.push(hash(2));
    let mut b = owner.block();
    b.push(hash(3));
    let mut a = BlockHashField::new(a);
    let mut b = BlockHashField::new(b);
    a.try_prepare_publication().unwrap();
    assert!(matches!(
        b.try_prepare_publication(),
        Err(PublicationPreparationError::Busy(_))
    ));
    assert!(catch_unwind(AssertUnwindSafe(|| b.try_prepare_publication())).is_err());
    drop(b);
    a.publish_prepared();
    drop(a);
    assert_eq!(
        owner.view().iter().copied().collect::<Vec<_>>(),
        vec![hash(1), hash(2)]
    );

    let mut stale = owner.block();
    stale.push(hash(4));
    let mut next = owner.block();
    next.push(hash(5));
    next.commit();
    let mut stale = BlockHashField::new(stale);
    assert!(matches!(
        stale.try_prepare_publication(),
        Err(PublicationPreparationError::Changed)
    ));
    assert!(
        !owner.writer_available(),
        "stale refusal keeps its actual acquired writer"
    );
    mv::BlockRetirement::release_writers(&mut stale);
    assert!(owner.writer_available());
    assert!(catch_unwind(AssertUnwindSafe(|| stale.publish_prepared())).is_err());
    drop(stale);
    let unchanged = owner.view().iter().copied().collect::<Vec<_>>();
    let mut unfilled = owner.block();
    unfilled.reserved_tip = Some(unfilled.len());
    let mut unfilled = BlockHashField::new(unfilled);
    assert!(matches!(
        unfilled.try_prepare_publication(),
        Err(PublicationPreparationError::Changed)
    ));
    drop(unfilled);
    assert_eq!(owner.view().iter().copied().collect::<Vec<_>>(), unchanged);
}

#[test]
fn attached_hash_outer_unwind_keeps_native_notifications_after_enclosing_fence() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let mut original = owner.block();
    original.push(hash(2));
    let fence = Arc::new(Mutex::new(()));
    let callback = Arc::new(ObserveFence {
        fence: Arc::clone(&fence),
        calls: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut cx = Context::from_waker(&waker);
    let wait = owner.released.observe();
    let mut future = wait.clone().wait_for_release();
    assert!(Pin::new(&mut future).poll(&mut cx).is_pending());
    let mut group = OriginalGroup {
        hash: BlockHashField::new(original),
        fence: Some(fence.lock().unwrap()),
    };
    group.hash.try_prepare_publication().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(move || {
            let _original = group;
            panic!("later component preparation failed");
        }))
        .is_err()
    );
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
    // The test fence is poisoned by that same unwind but physically available.
    assert!(
        fence
            .try_lock()
            .is_err_and(|e| matches!(e, std::sync::TryLockError::Poisoned(_)))
    );
    assert!(Pin::new(&mut future).poll(&mut cx).is_ready());
    assert!(wait.is_poisoned());
    assert_eq!(owner.committed_height(), 1);
}

#[test]
fn attached_hash_capture_moves_the_same_unpublished_root_and_revokes_terminal_access() {
    let owner = BlockHashes::new(vec![hash(1)]);
    let mut original = owner.block();
    original.push(hash(2));
    let pointer = std::ptr::from_ref(original.get(0).unwrap());
    let original = BlockHashField::new(original).into_executing();
    assert_eq!(std::ptr::from_ref(original.get(0).unwrap()), pointer);
    let mut field = BlockHashField::new(original);
    assert!(catch_unwind(AssertUnwindSafe(|| field.publish_prepared())).is_err());
    mv::BlockRetirement::release_writers(&mut field);
    assert!(catch_unwind(AssertUnwindSafe(|| field.get(0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| field.try_prepare_publication())).is_err());
    drop(field);
    assert_eq!(
        owner.view().iter().copied().collect::<Vec<_>>(),
        vec![hash(1)]
    );
}
