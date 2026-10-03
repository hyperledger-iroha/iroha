//! Original Copy source, nonblocking contention, poisoning and publication changes.

use super::*;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn committed_copy_preserves_exact_value_without_foreign_or_equal_republication() {
    let cell = Cell::new(7_u64);
    let same_value = Cell::new(7_u64);
    let source = cell.try_committed_copy().unwrap();
    assert_eq!(*source.current(), 7);
    assert!(source.try_matches_current(&cell).unwrap());
    assert!(!source.try_matches_current(&same_value).unwrap());
    cell.block().commit();
    assert!(!source.try_matches_current(&cell).unwrap());
    assert_eq!(*source.current(), 7);
    assert_eq!(*cell.try_committed_copy().unwrap().current(), 7);
}

#[test]
fn committed_copy_busy_retains_actual_current_writer_release() {
    let release_budget = iroha_allocation::AllocationBudget::new(
        1 * iroha_allocation::release::ReleaseRegistration::allocation_layout().size(),
    );
    let mut release_registration_1 = crate::release_test_support::registration(&release_budget);

    let cell = Cell::new(7_u64);
    let unrelated = Cell::new(7_u64);
    let held = cell.block();
    let Err(PublicationPreparationError::Busy(release)) = cell.try_committed_copy() else {
        panic!("held original current writer")
    };
    let mut wait = pin!(release.wait_for_release(&mut release_registration_1));
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Pending);
    drop(unrelated.block());
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Pending);
    drop(held);
    assert_eq!(wait.as_mut().poll(&mut cx), Poll::Ready(()));
    assert_eq!(*cell.try_committed_copy().unwrap().current(), 7);
}

#[test]
fn committed_copy_refuses_poison_without_changing_value_or_waiting() {
    let cell = Cell::new(7_u64);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _writer = cell.blocks.try_acquire_writer().unwrap();
        panic!("poison original writer");
    }));
    assert!(matches!(
        cell.try_committed_copy(),
        Err(PublicationPreparationError::Poisoned)
    ));
}

#[test]
fn committed_copy_concurrent_publication_never_returns_an_invented_scalar() {
    use crate::{BlockAcquisition as _, BlockMode, storage::AdmittedStorageError};
    use std::time::{Duration, Instant};

    let release_budget = iroha_allocation::AllocationBudget::new(
        iroha_allocation::release::ReleaseRegistration::allocation_layout().size(),
    );
    let mut registration = crate::release_test_support::registration(&release_budget);
    let cell = Cell::new(0_u64);
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(5);
            for value in 1..=256 {
                let mut block = loop {
                    assert!(
                        Instant::now() < deadline,
                        "concurrent writer makes progress"
                    );
                    let mut acquisition = cell.block_acquisition();
                    match acquisition.try_initialize(BlockMode::Ordinary) {
                        Ok(()) => break acquisition.into_block(),
                        Err(AdmittedStorageError::Busy { release, .. }) => {
                            // Acquisition slots are one-shot; release any earlier
                            // writer before observing the actual refusing source.
                            drop(acquisition);
                            while registration
                                .poll_wait(&release, &mut Context::from_waker(Waker::noop()))
                                .is_pending()
                            {
                                assert!(Instant::now() < deadline, "original copy reader releases");
                                std::thread::yield_now();
                            }
                            registration.cancel();
                        }
                        Err(other) => panic!("unexpected writer acquisition refusal: {other:?}"),
                    }
                };
                *block.get_mut() = value;
                block.commit();
            }
        });
        for _ in 0..512 {
            match cell.try_committed_copy() {
                Ok(source) => assert!(*source.current() <= 256),
                Err(
                    PublicationPreparationError::Busy(_) | PublicationPreparationError::Changed,
                ) => {}
                Err(other) => panic!("unexpected acquisition refusal: {other:?}"),
            }
        }
        writer.join().unwrap();
    });
    assert_eq!(*cell.try_committed_copy().unwrap().current(), 256);
}

#[test]
fn first_original_copy_on_cold_thread_allocates_no_mutex_or_collector_backing() {
    use crate::allocation_test_support::without_allocations;
    let cell = Cell::new(7_u64);
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let original = without_allocations(|| cell.try_committed_copy().unwrap());
                assert_eq!(*original.current(), 7);
                without_allocations(|| assert!(original.try_matches_current(&cell).unwrap()));
                without_allocations(|| drop(original));
            })
            .join()
            .unwrap();
    });
}
