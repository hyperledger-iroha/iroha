//! Borrowed pair admission through actual original writers and parent checkpoints.

use super::*;

#[test]
fn borrowed_writers_keep_original_locks_and_first_preimages() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
    cw.try_insert_admitted(3, 30, |d| pool.reserve(d)).unwrap();
    let shells = (cw.inner.as_ref() as *const _, uw.inner.as_ref() as *const _);
    for (key, value, before) in [
        (7, 70, None),
        (7, 71, Some(70)),
        (3, 31, Some(30)),
        (3, 32, Some(31)),
    ] {
        let calls = pool.callbacks.get();
        let copies = pool.clones.get();
        assert_eq!(
            cw.try_insert_with_undo_admitted(&mut uw, key, value, |d, incoming| {
                assert_eq!(*incoming, key, "borrow the original incoming key");
                assert_eq!(
                    pool.clones.get(),
                    copies,
                    "borrow precedes every payload copy"
                );
                pool.reserve(d)
            })
            .unwrap(),
            before
        );
        assert_eq!(pool.callbacks.get(), calls + 1);
        assert_eq!(
            (cw.inner.as_ref() as *const _, uw.inner.as_ref() as *const _),
            shells
        );
        assert_eq!(uw.get(&7), Some(&None));
        if key == 3 {
            assert_eq!(uw.get(&3), Some(&Some(30)));
        }
    }
    let busy_current = without_allocations(|| {
        current.try_write_admitted::<()>(|_| panic!("original current lock still held"))
    });
    let busy_undo = without_allocations(|| {
        undo.try_write_admitted::<()>(|_| panic!("original undo lock still held"))
    });
    assert!(matches!(busy_current, Err(InsertAdmissionError::Busy)));
    assert!(matches!(busy_undo, Err(InsertAdmissionError::Busy)));
    drop((busy_current, busy_undo));
    assert!(current.read().is_empty() && undo.read().is_empty());
    without_allocations(|| drop((cw, uw)));
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn parent_refusal_and_full_budget_abort_preserve_exact_original_buffers() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
    cw.try_insert_admitted(3, 30, |d| pool.reserve(d)).unwrap();
    uw.try_insert_admitted(3, Some(29), |d| pool.reserve(d))
        .unwrap();
    let original = (
        identity(cw.inner.as_ref()),
        identity(uw.inner.as_ref()),
        pool.used.get(),
    );
    let mut cp = cw.checkpoint().unwrap();
    let mut up = uw.checkpoint().unwrap();
    let parent = (identity(cp.inner.as_ref()), identity(up.inner.as_ref()));
    let plan = without_allocations(|| {
        plan_pair::<_, _, Policy>(cp.inner.as_ref(), up.inner.as_ref(), &7).unwrap()
    });
    pool.limit.set(pool.used.get() + plan.demand.bytes() - 1);
    let clones = pool.clones.get();
    let ((key, value), error) = without_allocations(|| {
        cp.try_insert_with_undo_admitted(&mut up, 7, 70, |d, _key| {
            assert_eq!(d, plan.demand);
            pool.reserve(d)
        })
        .err()
        .unwrap()
    });
    assert!(matches!(error, PairInsertError::Refused(())));
    assert_eq!((key, value), (7, 70));
    assert_eq!(
        (identity(cp.inner.as_ref()), identity(up.inner.as_ref())),
        parent
    );
    assert_eq!(pool.clones.get(), clones);
    pool.limit.set(pool.limit.get() + 1);
    assert_eq!(
        cp.try_insert_with_undo_admitted(&mut up, key, value, |d, _key| pool.reserve(d))
            .unwrap(),
        None
    );
    assert_eq!(cp.get(&7), Some(&70));
    assert_eq!(up.get(&7), Some(&None));
    assert_eq!(cp.get_before(&3), Some(&30));
    assert_eq!(cp.get_before(&7), None);
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
    assert_eq!(cw.get(&3), Some(&30));
    assert_eq!(uw.get(&3), Some(&Some(29)));
    without_allocations(|| drop((cw, uw)));
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn parent_apply_keeps_private_successors_and_outer_abort_restores_custody() {
    let pool = Pool::new();
    let current = map::<usize>(&pool);
    let undo = map::<Option<usize>>(&pool);
    let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
    cw.try_insert_admitted(7, 69, |d| pool.reserve(d)).unwrap();
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
        for (key, value) in [(7, 70), (8, 80), (7, 71), (8, 81)] {
            cp.try_insert_with_undo_admitted(&mut up, key, value, |d, _key| pool.reserve(d))
                .unwrap();
        }
        assert_eq!(up.get(&7), Some(&Some(69)));
        assert_eq!(up.get(&8), Some(&None));
        assert_eq!(cp.get_before(&7), Some(&69));
        assert_eq!(cp.get_before(&8), None);
        cp.apply();
        up.apply();
    }
    assert_eq!(outer_c.get(&7), Some(&71));
    assert_eq!(outer_c.get(&8), Some(&81));
    assert_eq!(outer_u.get(&7), Some(&Some(69)));
    assert_eq!(outer_u.get(&8), Some(&None));
    assert!(current.read().is_empty() && undo.read().is_empty());
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
    without_allocations(|| drop((current, undo)));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn borrowed_parent_generation_refuses_before_callback_and_skips_existing_undo() {
    let maximum = (TXID_MASK >> TXID_SHF) - 1;
    for exhausted_current in [false, true] {
        for entry in [None, Some((7, None)), Some((7, Some(69)))] {
            let pool = Pool::new();
            let current = at_generation::<usize>(
                &pool,
                if exhausted_current { maximum - 2 } else { 1 },
                None,
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
            let calls = pool.callbacks.get();
            if exhausted_current || entry.is_none() {
                let ((key, value), error) = without_allocations(|| {
                    cp.try_insert_with_undo_admitted(
                        &mut up,
                        7,
                        70,
                        |_, _key| -> Result<Policy, ()> {
                            panic!("generation refusal before callback")
                        },
                    )
                    .err()
                    .unwrap()
                });
                assert!(matches!(
                    error,
                    PairInsertError::Planning(PlanningError::Overflow)
                ));
                assert_eq!((key, value), (7, 70));
                assert_eq!(
                    (identity(cp.inner.as_ref()), identity(up.inner.as_ref())),
                    before
                );
                assert_eq!(pool.callbacks.get(), calls);
            } else {
                cp.try_insert_with_undo_admitted(&mut up, 7, 70, |d, _key| pool.reserve(d))
                    .unwrap();
                assert_eq!(identity(up.inner.as_ref()), before.1);
                assert_eq!(up.get(&7), entry.as_ref().map(|(_, v)| v));
                assert_eq!(pool.callbacks.get(), calls + 1);
            }
            without_allocations(|| drop((cp, up)));
            without_allocations(|| drop((cw, uw)));
            without_allocations(|| drop((current, undo)));
            assert_eq!(pool.used.get(), 0);
        }
    }
}

#[test]
fn caught_callback_clone_and_provider_panics_invalidate_both_borrowed_parents() {
    for fault in 0..4 {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        cw.try_insert_admitted(3, 30, |d| pool.reserve(d)).unwrap();
        uw.try_insert_admitted(3, Some(29), |d| pool.reserve(d))
            .unwrap();
        let mut cp = cw.checkpoint().unwrap();
        let mut up = uw.checkpoint().unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ =
                cp.try_insert_with_undo_admitted(&mut up, 7, 70, |d, _key| -> Result<Policy, ()> {
                    if fault == 0 {
                        panic!("joined callback failed");
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
        // The physical guards survived the catch. Failure must reside in both
        // actual cursors, and remain after the parent rollback destructors run.
        drop((cp, up));
        assert!(catch_unwind(AssertUnwindSafe(|| cw.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| uw.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| cw.checkpoint().map(drop))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| uw.try_insert_admitted(
            9,
            Some(90),
            |_| -> Result<Policy, ()> { panic!("failed undo before admission") }
        )))
        .is_err());
        assert!(current.read().is_empty() && undo.read().is_empty());
        without_allocations(|| drop((cw, uw)));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn first_and_second_apply_cleanup_failures_invalidate_both_attached_writers() {
    for second_apply in [false, true] {
        let pool = Pool::new();
        let current = map::<usize>(&pool);
        let undo = map::<Option<usize>>(&pool);
        let mut cw = current.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let mut uw = undo.try_write_admitted(|d| pool.reserve(d)).unwrap();
        cw.try_insert_admitted(7, 70, |d| pool.reserve(d)).unwrap();
        if second_apply {
            // Warm the current retirement capacity without another generation.
            cw.try_insert_admitted(8, 80, |d| pool.reserve(d)).unwrap();
        }
        uw.try_insert_admitted(3, Some(29), |d| pool.reserve(d))
            .unwrap();
        let plan = plan_pair::<_, _, Policy>(cw.inner.as_ref(), uw.inner.as_ref(), &7).unwrap();
        let current_grows = plan.current.first.is_some() || plan.current.last.is_some();
        let undo_plan = plan.undo.as_ref().unwrap();
        assert_eq!(current_grows, !second_apply);
        assert!(undo_plan.first.is_some() || undo_plan.last.is_some());
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = cw.try_insert_with_undo_admitted(&mut uw, 7, 71, |d, _key| {
                let provider = pool.reserve(d)?;
                // No charge destruction can precede final provider cleanup in
                // these scalar edits. The next charge is the saved buffer of
                // the first apply, or of the second after current applied cleanly.
                pool.arm_charge_panic_on_provider_drop.set(true);
                Ok::<_, ()>(provider)
            });
        }))
        .is_err());
        assert!(
            !pool.panic_next_charge.get(),
            "the selected apply cleanup must execute"
        );
        assert!(!pool.arm_charge_panic_on_provider_drop.get());
        assert!(catch_unwind(AssertUnwindSafe(|| cw.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| uw.len())).is_err());
        assert!(current.read().is_empty() && undo.read().is_empty());
        without_allocations(|| drop((cw, uw)));
        without_allocations(|| drop((current, undo)));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn prefailed_parent_entry_invalidates_the_other_original_cursor_before_admission() {
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
                let _ = cp.try_insert_admitted(9, 90, |d| {
                    let provider = pool.reserve(d)?;
                    pool.panic_drop.set(true);
                    Ok::<_, ()>(provider)
                });
            } else {
                let _ = up.try_insert_admitted(9, Some(90), |d| {
                    let provider = pool.reserve(d)?;
                    pool.panic_drop.set(true);
                    Ok::<_, ()>(provider)
                });
            }
        }))
        .is_err());
        if failed_current {
            assert_eq!(up.len(), 0);
        } else {
            assert_eq!(cp.len(), 0);
        }
        let admitted = Cell::new(false);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ =
                cp.try_insert_with_undo_admitted(&mut up, 7, 70, |_, _key| -> Result<Policy, ()> {
                    admitted.set(true);
                    panic!("prefailed pair must not reach admission");
                });
        }))
        .is_err());
        assert!(!admitted.get());
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
