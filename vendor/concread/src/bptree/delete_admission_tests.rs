//! Finite removal admission through the same canonical tree and original owners.

use super::*;

#[test]
fn removal_retains_first_some_and_explicit_none_under_original_writers() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
    cw.try_insert_admitted(7, 70, |d| pool.reserve(d)).unwrap();
    for (key, expected) in [(7, Some(70)), (7, None), (9, None), (9, None)] {
        let before = identity(uw.inner.as_ref());
        let existing = uw.contains_key(&key);
        let calls = pool.callbacks.get();
        let copies = pool.clones.get();
        assert_eq!(
            cw.try_remove_with_undo_admitted(&mut uw, key, |d, input| {
                assert_eq!(*input, key);
                assert_eq!(pool.clones.get(), copies);
                pool.reserve(d)
            })
            .unwrap(),
            expected
        );
        assert_eq!(pool.callbacks.get(), calls + 1);
        if existing {
            assert_eq!(identity(uw.inner.as_ref()), before);
        }
        assert!(cw.inner.as_ref().verify() && uw.inner.as_ref().verify());
    }
    assert_eq!(uw.get(&7), Some(&Some(70)));
    assert_eq!(uw.get(&9), Some(&None));
    cw.try_insert_admitted(7, 71, |d| pool.reserve(d)).unwrap();
    let before = identity(uw.inner.as_ref());
    assert_eq!(
        cw.try_remove_with_undo_admitted(&mut uw, 7, |d, _| pool.reserve(d))
            .unwrap(),
        Some(71)
    );
    assert_eq!(identity(uw.inner.as_ref()), before);
    assert_eq!(uw.get(&7), Some(&Some(70)));
    without_allocations(|| drop((cw, uw)));
    assert!(current.read().is_empty() && undo.read().is_empty());
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn removal_whole_demand_refusal_and_exact_retry_preserve_parent_custody() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
    for key in 0..80 {
        cw.try_insert_admitted(key, key * 10, |d| pool.reserve(d))
            .unwrap();
    }
    let original = (
        identity(cw.inner.as_ref()),
        identity(uw.inner.as_ref()),
        pool.used.get(),
    );
    let mut cp = cw.checkpoint().unwrap();
    let mut up = uw.checkpoint().unwrap();
    let parent = (identity(cp.inner.as_ref()), identity(up.inner.as_ref()));
    let copies = pool.clones.get();
    let takes = pool.takes.get();
    let calls = pool.callbacks.get();
    let demand = Cell::new(AllocationDemand::new());
    let (key, error) = without_allocations(|| {
        cp.try_remove_with_undo_admitted(&mut up, 0, |d, input| {
            assert_eq!(*input, 0);
            demand.set(d);
            Err::<Policy, _>("observe only")
        })
        .err()
        .unwrap()
    });
    assert!(matches!(error, PairRemoveError::Refused("observe only")));
    assert_eq!(
        (identity(cp.inner.as_ref()), identity(up.inner.as_ref())),
        parent
    );
    assert_eq!((pool.clones.get(), pool.takes.get()), (copies, takes));
    let used = pool.used.get();
    pool.limit.set(used + demand.get().bytes() - 1);
    let (key, error) = without_allocations(|| {
        cp.try_remove_with_undo_admitted(&mut up, key, |d, input| {
            assert_eq!(*input, 0);
            assert_eq!(d, demand.get());
            pool.reserve(d)
        })
        .err()
        .unwrap()
    });
    assert!(matches!(error, PairRemoveError::Refused(())));
    assert_eq!(
        (identity(cp.inner.as_ref()), identity(up.inner.as_ref())),
        parent
    );
    assert_eq!(
        (pool.used.get(), pool.clones.get(), pool.takes.get()),
        (used, copies, takes)
    );
    assert_eq!(pool.callbacks.get(), calls + 1);
    pool.limit.set(used + demand.get().bytes());
    assert_eq!(
        cp.try_remove_with_undo_admitted(&mut up, key, |d, _| {
            assert_eq!(d, demand.get());
            pool.reserve(d)
        })
        .unwrap(),
        Some(0)
    );
    assert_eq!(pool.callbacks.get(), calls + 2);
    assert_eq!(cp.get(&0), None);
    assert_eq!(up.get(&0), Some(&Some(0)));
    pool.limit.set(pool.used.get());
    without_allocations(|| drop((cp, up)));
    assert_eq!(
        (
            identity(cw.inner.as_ref()),
            identity(uw.inner.as_ref()),
            pool.used.get()
        ),
        original
    );
    without_allocations(|| drop((cw, uw)));
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn removal_orders_rebalance_and_abort_without_credit_or_reader_changes() {
    for order in 0..3 {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let mut seed = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        for key in 0..256 {
            seed.try_insert_admitted(key, key + 1000, |d| pool.reserve(d))
                .unwrap();
        }
        seed.commit();
        let old = current.read();
        let published_root = old.inner.as_ref().get_root();
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let original = (
            identity(cw.inner.as_ref()),
            identity(uw.inner.as_ref()),
            pool.used.get(),
        );
        let mut outer_c = cw.checkpoint().unwrap();
        let mut outer_u = uw.checkpoint().unwrap();
        {
            let mut cp = outer_c.checkpoint().unwrap();
            let mut up = outer_u.checkpoint().unwrap();
            for step in 0..256 {
                let key = match order {
                    0 => step,
                    1 => 255 - step,
                    _ => {
                        if step % 2 == 0 {
                            step / 2
                        } else {
                            255 - step / 2
                        }
                    }
                };
                assert_eq!(
                    cp.try_remove_with_undo_admitted(&mut up, key, |d, _| pool.reserve(d))
                        .unwrap(),
                    Some(key + 1000)
                );
                assert_eq!(cp.len(), 255 - step);
                assert!(cp.inner.as_ref().verify() && up.inner.as_ref().verify());
                assert_eq!(up.get(&key), Some(&Some(key + 1000)));
                assert_eq!(old.get(&key), Some(&(key + 1000)));
            }
            assert!(cp.is_empty());
            assert_eq!(
                cp.try_remove_with_undo_admitted(&mut up, 999, |d, _| pool.reserve(d))
                    .unwrap(),
                None
            );
            assert_eq!(up.get(&999), Some(&None));
            without_allocations(|| {
                cp.apply();
                up.apply();
            });
        }
        assert!(outer_c.is_empty());
        assert_eq!(outer_u.len(), 257);
        assert_eq!(old.inner.as_ref().get_root(), published_root);
        pool.limit.set(pool.used.get());
        without_allocations(|| drop((outer_c, outer_u)));
        assert_eq!(
            (
                identity(cw.inner.as_ref()),
                identity(uw.inner.as_ref()),
                pool.used.get()
            ),
            original
        );
        without_allocations(|| drop((cw, uw)));
        assert_eq!(current.read().inner.as_ref().get_root(), published_root);
        without_allocations(|| drop(old));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn removal_generation_refuses_before_callback_and_skips_existing_undo() {
    let maximum = (TXID_MASK >> TXID_SHF) - 1;
    for exhausted_current in [false, true] {
        for entry in [None, Some((7, None)), Some((7, Some(69)))] {
            let pool = Pool::new();
            let current = at_generation::<usize>(
                &pool,
                if exhausted_current { maximum - 2 } else { 1 },
                Some((7, 70)),
            );
            let undo = at_generation(
                &pool,
                if exhausted_current { 1 } else { maximum - 2 },
                entry,
            );
            let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
            let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
            let mut cp = cw.checkpoint().unwrap();
            let mut up = uw.checkpoint().unwrap();
            let before = (identity(cp.inner.as_ref()), identity(up.inner.as_ref()));
            if exhausted_current || entry.is_none() {
                let (key, error) = without_allocations(|| {
                    cp.try_remove_with_undo_admitted(&mut up, 7, |_, _| -> Result<Policy, ()> {
                        panic!("generation before admission")
                    })
                    .err()
                    .unwrap()
                });
                assert_eq!(key, 7);
                assert!(matches!(
                    error,
                    PairRemoveError::Planning(PlanningError::Overflow)
                ));
                assert_eq!(
                    (identity(cp.inner.as_ref()), identity(up.inner.as_ref())),
                    before
                );
            } else {
                assert_eq!(
                    cp.try_remove_with_undo_admitted(&mut up, 7, |d, _| pool.reserve(d))
                        .unwrap(),
                    Some(70)
                );
                assert_eq!(identity(up.inner.as_ref()), before.1);
                assert_eq!(up.get(&7), entry.as_ref().map(|(_, value)| value));
            }
            without_allocations(|| drop((cp, up)));
            without_allocations(|| drop((cw, uw)));
            without_allocations(|| drop((current, undo)));
            assert_eq!(pool.used.get(), 0);
        }
    }
}

#[test]
fn removal_caught_callback_clone_and_provider_panics_invalidate_both_parents() {
    for fault in 0..4 {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        cw.try_insert_admitted(7, 70, |d| pool.reserve(d)).unwrap();
        uw.try_insert_admitted(3, Some(29), |d| pool.reserve(d))
            .unwrap();
        let mut cp = cw.checkpoint().unwrap();
        let mut up = uw.checkpoint().unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = cp.try_remove_with_undo_admitted(&mut up, 7, |d, _| -> Result<Policy, ()> {
                if fault == 0 {
                    panic!("callback failed");
                }
                let provider = pool.reserve(d)?;
                if fault == 1 {
                    pool.panic_clone.set(1);
                }
                if fault == 2 {
                    pool.panic_clone.set(2);
                }
                if fault == 3 {
                    pool.panic_drop.set(true);
                }
                Ok(provider)
            });
        }))
        .is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| cp.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| up.len())).is_err());
        drop((cp, up));
        assert!(catch_unwind(AssertUnwindSafe(|| cw.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| uw.len())).is_err());
        assert!(current.read().is_empty() && undo.read().is_empty());
        without_allocations(|| drop((cw, uw)));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn removal_prefailed_parent_invalidates_other_owner_before_admission() {
    for failed_current in [false, true] {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut cp = cw.checkpoint().unwrap();
        let mut up = uw.checkpoint().unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if failed_current {
                let _ = cp.try_insert_admitted(7, 70, |d| {
                    let p = pool.reserve(d)?;
                    pool.panic_drop.set(true);
                    Ok::<_, ()>(p)
                });
            } else {
                let _ = up.try_insert_admitted(7, Some(70), |d| {
                    let p = pool.reserve(d)?;
                    pool.panic_drop.set(true);
                    Ok::<_, ()>(p)
                });
            }
        }))
        .is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = cp.try_remove_with_undo_admitted(&mut up, 7, |_, _| -> Result<Policy, ()> {
                panic!("must not admit prefailed pair")
            });
        }))
        .is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| cp.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| up.len())).is_err());
        drop((cp, up));
        without_allocations(|| drop((cw, uw)));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.get(), 0);
    }
}

#[path = "delete_payload_tests.rs"]
mod payload;
