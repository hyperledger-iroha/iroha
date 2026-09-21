//! Real writer custody across identity refusal, poison, validation and abort.

use super::*;
use crate::{
    internals::bptree::node::allocation_tests::without_allocations, release::ReleaseNotification,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Waker},
};

#[test]
fn acquired_map_validation_retains_stale_and_poisoned_physical_writers() {
    for poisoned in [false, true] {
        let map: BptreeMap<u64, u64> = [(1, 10)].into_iter().collect();
        let owned = map.write().detach();
        let pointer = owned.get(&1).map(std::ptr::from_ref);
        if poisoned {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _writer = map.write();
                panic!("poison actual writer");
            }))
            .is_err());
        } else {
            let mut writer = map.write();
            writer.insert(1, 20);
            writer.commit();
        }
        let source = ReleaseNotification::default();
        let mut wait = source.observe().wait_for_release();
        let acquired = without_allocations(|| {
            map.try_acquire_owned(owned)
                .unwrap_or_else(|_| panic!("actual acquisition"))
        });
        let (acquired, error) = without_allocations(|| {
            source
                .poisoning_guard(acquired)
                .try_map_preserving_release(|acquired| acquired.validate())
                .err()
                .expect("stale or poisoned validation")
        });
        assert_eq!(
            error,
            if poisoned {
                OwnedWriteError::Poisoned
            } else {
                OwnedWriteError::Changed
            }
        );
        assert!(
            map.try_write().is_none(),
            "refused phase retains its physical guard"
        );
        let (owned, cleanup) =
            without_allocations(|| acquired.release_deferred(|acquired| acquired.abort()));
        assert_eq!(owned.get(&1).map(std::ptr::from_ref), pointer);
        assert!(Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(map.is_poisoned(), poisoned);
        if !poisoned {
            assert!(map.try_write().is_some());
        }
        drop(cleanup);
        assert!(Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready());
    }
}

#[test]
fn acquired_map_foreign_busy_success_and_unwind_preserve_original_custody() {
    let map: BptreeMap<u64, u64> = [(1, 10)].into_iter().collect();
    let foreign: BptreeMap<u64, u64> = [(1, 10)].into_iter().collect();
    let owned = map.write().detach();
    let pointer = owned.get(&1).map(std::ptr::from_ref);
    let foreign_guard = foreign.write();
    let (owned, error) = without_allocations(|| foreign.try_acquire_owned(owned).err().unwrap());
    assert_eq!(
        error,
        OwnedWriteError::Changed,
        "foreign identity checked before target acquisition"
    );
    drop(foreign_guard);
    let held = map.write();
    let (owned, error) = without_allocations(|| map.try_acquire_owned(owned).err().unwrap());
    assert_eq!(error, OwnedWriteError::Busy);
    drop(held);
    let acquired = without_allocations(|| {
        map.try_acquire_owned(owned)
            .unwrap_or_else(|_| panic!("free original writer"))
    });
    let writer = without_allocations(|| {
        acquired
            .validate()
            .unwrap_or_else(|_| panic!("original predecessor"))
    });
    let owned = without_allocations(|| writer.detach());
    assert_eq!(owned.get(&1).map(std::ptr::from_ref), pointer);
    let source = ReleaseNotification::default();
    let observation = source.observe();
    let acquired = map
        .try_acquire_owned(owned)
        .unwrap_or_else(|_| panic!("reacquire original"));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _acquired = source.poisoning_guard(acquired);
        panic!("unwind while original acquired phase is retained");
    }))
    .is_err());
    assert!(map.is_poisoned());
    assert!(observation.is_poisoned());
    let mut wait = observation.wait_for_release();
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready());
}
