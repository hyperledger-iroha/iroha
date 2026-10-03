//! Exact non-Copy pair, native writer custody, cleanup order and stale-source controls.
use super::*;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn committed_borrow_preserves_noncopy_payload_pointer_and_explicit_undo() {
    let cell = Cell::new(String::from("before"));
    let empty = cell.try_committed_borrow().unwrap();
    assert_eq!(empty.current(), "before");
    assert!(empty.undo().is_none());
    assert!(empty.try_matches_current().unwrap());
    drop(empty);
    let mut block = cell.block();
    *block.get_mut() = String::from("after");
    block.commit();
    let held = cell.try_committed_borrow().unwrap();
    assert_eq!(held.current(), "after");
    assert_eq!(held.undo().as_deref(), Some("before"));
    let pointer = held.current().as_ptr();
    assert_eq!(held.current().as_ptr(), pointer);
    assert!(matches!(
        cell.try_committed_borrow(),
        Err(PublicationPreparationError::Busy(_))
    ));
    drop(held);
    assert_eq!(
        cell.try_committed_borrow().unwrap().current().as_ptr(),
        pointer
    );
}

#[test]
fn committed_borrow_preserves_nested_absence_without_cloning() {
    let cell = Cell::new(None::<String>);
    let mut block = cell.block();
    *block.get_mut() = Some(String::from("successor"));
    block.commit();
    let held = cell.try_committed_borrow().unwrap();
    assert_eq!(held.current().as_deref(), Some("successor"));
    assert_eq!(held.undo(), &Some(None));
}

#[test]
fn committed_borrow_partial_busy_releases_undo_and_retains_exact_current_waiter() {
    let cell = Cell::new(String::from("original"));
    let acquired = cell.blocks.try_acquire_writer().unwrap();
    let held = cell.blocks_released.poisoning_guard(acquired);
    let Err(PublicationPreparationError::Busy(release)) = cell.try_committed_borrow() else {
        panic!("actual current owner must refuse")
    };
    assert!(
        cell.revert.try_acquire_writer().is_some(),
        "partial original undo must be released"
    );
    let mut wait = pin!(release.wait_for_release());
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Pending);
    drop(
        cell.revert_released
            .poisoning_guard(cell.revert.try_acquire_writer().unwrap()),
    );
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Pending);
    drop(held);
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Ready(()));
    assert!(cell.try_committed_borrow().is_ok());
}

#[test]
fn committed_borrow_refuses_original_poison_and_releases_any_prior_guard() {
    let cell = Cell::new(String::from("original"));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = cell.blocks.try_acquire_writer().unwrap();
        panic!("poison actual current");
    }));
    assert!(matches!(
        cell.try_committed_borrow(),
        Err(PublicationPreparationError::Poisoned)
    ));
    assert!(cell.revert.try_acquire_writer().is_some());
}

#[test]
fn committed_borrow_concurrent_publication_only_returns_actual_predecessor_pair() {
    let cell = Cell::new(String::from("0"));
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            for n in 1..=128 {
                let mut block = cell.block();
                *block.get_mut() = n.to_string();
                block.commit();
            }
        });
        for _ in 0..256 {
            match cell.try_committed_borrow() {
                Ok(pair) => {
                    let current = pair.current().parse::<u64>().unwrap();
                    assert_eq!(
                        pair.undo().as_ref().map(|s| s.parse::<u64>().unwrap()),
                        current.checked_sub(1)
                    );
                    assert!(pair.try_matches_current().unwrap());
                }
                Err(
                    PublicationPreparationError::Busy(_) | PublicationPreparationError::Changed,
                ) => {}
                Err(other) => panic!("unexpected original refusal: {other:?}"),
            }
        }
        writer.join().unwrap();
    });
    let pair = cell.try_committed_borrow().unwrap();
    assert_eq!(pair.current(), "128");
    assert_eq!(pair.undo().as_deref(), Some("127"));
}

#[test]
fn released_original_observation_never_accepts_equal_republication() {
    let cell = Cell::new(String::from("same"));
    let original = cell
        .try_committed_borrow()
        .unwrap()
        .release_observation()
        .unwrap();
    assert!(cell.blocks.try_acquire_writer().is_some());
    assert!(cell.revert.try_acquire_writer().is_some());
    assert!(original.try_matches_current().unwrap());
    cell.block().commit();
    assert!(!original.try_matches_current().unwrap());
}

#[test]
fn first_original_noncopy_borrow_on_cold_thread_allocates_no_mutex_or_collector_backing() {
    use crate::allocation_test_support::without_allocations;
    let cell = Cell::new(String::from("original"));
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let original = without_allocations(|| cell.try_committed_borrow().unwrap());
                assert_eq!(original.current(), "original");
                assert!(original.undo().is_none());
                without_allocations(|| assert!(original.try_matches_current().unwrap()));
                without_allocations(|| drop(original));
            })
            .join()
            .unwrap();
    });
}
