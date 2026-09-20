//! Finite reset demand, original preimage custody and caught cleanup failures.

use super::*;

#[test]
fn clear_exact_limit_and_refusal_preserve_original_checkpoint_allocations() {
    let pool = Pool::new();
    let map = map::<usize>(&pool);
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    for key in 0..129 {
        writer
            .try_insert_admitted(key, key + 1, |d| pool.reserve(d))
            .unwrap();
    }
    let original = (identity(writer.inner.as_ref()), pool.used.get());
    let mut parent = writer.checkpoint().unwrap();
    let before = identity(parent.inner.as_ref());
    let nodes = without_allocations(|| parent.inner.as_ref().admitted_clear_node_count().unwrap());
    assert!(nodes > 1, "exercise actual branch retirement");
    let mut demanded = None;
    let result = without_allocations(|| {
        parent.try_clear_admitted(|d| {
            demanded = Some(d);
            Err::<Policy, _>(())
        })
    });
    assert!(matches!(result, Err(ClearAdmissionError::Refused(()))));
    assert_eq!(identity(parent.inner.as_ref()), before);
    let demand = demanded.unwrap();
    let used = pool.used.get();
    let takes = pool.takes.get();
    let clones = pool.clones.get();
    pool.limit.set(used + demand.bytes() - 1);
    let result = without_allocations(|| {
        parent.try_clear_admitted(|d| {
            assert_eq!(d, demand);
            pool.reserve(d)
        })
    });
    assert!(matches!(result, Err(ClearAdmissionError::Refused(()))));
    assert_eq!(identity(parent.inner.as_ref()), before);
    assert_eq!(pool.used.get(), used);
    assert_eq!(pool.takes.get(), takes);
    pool.limit.set(used + demand.bytes());
    let callbacks = pool.callbacks.get();
    parent.try_clear_admitted(|d| pool.reserve(d)).unwrap();
    assert_eq!(pool.callbacks.get(), callbacks + 1);
    assert_eq!(pool.used.get(), used + demand.bytes());
    assert_eq!(pool.takes.get() - takes, demand.allocations());
    assert_eq!(pool.clones.get(), clones);
    let after = identity(parent.inner.as_ref());
    assert_eq!(after.cursor, before.cursor);
    assert_ne!(after.root, before.root);
    assert_eq!(after.length, 0);
    assert_eq!(after.tracking[0].0, before.tracking[0].0 + 1);
    assert_eq!(after.tracking[1].0, before.tracking[1].0 + nodes);
    let pointer = Layout::new::<*mut Node<usize, usize, Charge>>().size();
    let mut actual = Layout::new::<CachePadded<Leaf<usize, usize, Charge>>>().size();
    for index in 0..2 {
        if before.backing[index] != after.backing[index] {
            actual += after.tracking[index].1 * pointer;
        }
    }
    assert_eq!(
        demand.bytes(),
        actual,
        "actual new root and concrete buffer layouts"
    );
    assert!(parent.is_empty());
    assert_eq!(parent.get_before(&128), Some(&129));
    pool.limit.set(pool.used.get());
    without_allocations(|| drop(parent));
    assert_eq!((identity(writer.inner.as_ref()), pool.used.get()), original);
    assert_eq!(writer.get(&128), Some(&129));
    without_allocations(|| drop(writer));
    without_allocations(|| drop(map));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn clear_publication_retains_actual_old_reader_preimages_and_charges() {
    let pool = Pool::new();
    let map = map::<Option<usize>>(&pool);
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    for key in 0..129 {
        writer
            .try_insert_admitted(key, (key % 2 == 0).then_some(key), |d| pool.reserve(d))
            .unwrap();
    }
    writer.commit();
    let old = map.read();
    let old_root = old.inner.as_ref().get_root();
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let nodes = writer.inner.as_ref().admitted_clear_node_count().unwrap();
    let clones = pool.clones.get();
    writer.try_clear_admitted(|d| pool.reserve(d)).unwrap();
    assert_eq!(writer.inner.as_ref().admitted_tracking()[1].0, nodes);
    assert_eq!(old.inner.as_ref().get_root(), old_root);
    assert_eq!(old.get(&128), Some(&Some(128)));
    assert_eq!(old.get(&127), Some(&None));
    assert_eq!(pool.clones.get(), clones);
    writer.commit();
    assert!(map.read().is_empty());
    assert_eq!(old.len(), 129);
    let held = pool.used.get();
    without_allocations(|| drop(old));
    assert!(
        pool.used.get() < held,
        "old reader release reclaims retired charged nodes"
    );
    // The next block now observes truly empty undo, not stale None/Some entries.
    let mut next = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    assert!(next.is_empty());
    assert_eq!(
        next.try_insert_admitted(128, Some(900), |d| pool.reserve(d))
            .unwrap(),
        None
    );
    next.commit();
    assert_eq!(map.read().get(&128), Some(&Some(900)));
    without_allocations(|| drop(map));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn clear_nested_apply_and_repeated_resets_keep_full_budget_outer_abort() {
    let pool = Pool::new();
    let map = map::<usize>(&pool);
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    writer
        .try_insert_admitted(7, 70, |d| pool.reserve(d))
        .unwrap();
    let original = (identity(writer.inner.as_ref()), pool.used.get());
    let mut outer = writer.checkpoint().unwrap();
    {
        let mut inner = outer.checkpoint().unwrap();
        for round in 0..3 {
            inner.try_clear_admitted(|d| pool.reserve(d)).unwrap();
            assert!(inner.is_empty());
            inner
                .try_insert_admitted(8, round, |d| pool.reserve(d))
                .unwrap();
        }
        inner.try_clear_admitted(|d| pool.reserve(d)).unwrap();
        inner.try_clear_admitted(|d| pool.reserve(d)).unwrap();
        assert_eq!(inner.get_before(&7), Some(&70));
        inner.apply();
    }
    assert!(outer.is_empty());
    assert_eq!(outer.get_before(&7), Some(&70));
    pool.limit.set(pool.used.get());
    without_allocations(|| drop(outer));
    assert_eq!((identity(writer.inner.as_ref()), pool.used.get()), original);
    assert_eq!(writer.get(&7), Some(&70));
    without_allocations(|| drop(writer));
    without_allocations(|| drop(map));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn clear_generation_refusal_precedes_callback_and_preserves_writer_and_parent() {
    let maximum = (TXID_MASK >> TXID_SHF) - 1;
    for via_parent in [false, true] {
        let pool = Pool::new();
        let map = at_generation(
            &pool,
            maximum - if via_parent { 2 } else { 1 },
            Some((7, 70)),
        );
        let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
        let used = pool.used.get();
        if via_parent {
            let mut parent = writer.checkpoint().unwrap();
            let before = identity(parent.inner.as_ref());
            let result = without_allocations(|| {
                parent.try_clear_admitted::<()>(|_| panic!("generation refusal precedes provider"))
            });
            assert!(matches!(
                result,
                Err(ClearAdmissionError::Planning(PlanningError::Overflow))
            ));
            assert_eq!(identity(parent.inner.as_ref()), before);
            without_allocations(|| drop(parent));
        } else {
            let before = identity(writer.inner.as_ref());
            let result = without_allocations(|| {
                writer.try_clear_admitted::<()>(|_| panic!("generation refusal precedes provider"))
            });
            assert!(matches!(
                result,
                Err(ClearAdmissionError::Planning(PlanningError::Overflow))
            ));
            assert_eq!(identity(writer.inner.as_ref()), before);
        }
        assert_eq!(pool.used.get(), used);
        assert_eq!(writer.get(&7), Some(&70));
        without_allocations(|| drop(writer));
        without_allocations(|| drop(map));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn clear_caught_callback_and_provider_panics_leave_original_parent_unusable() {
    for provider_drop in [false, true] {
        let pool = Pool::new();
        let map = map::<usize>(&pool);
        let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
        writer
            .try_insert_admitted(7, 70, |d| pool.reserve(d))
            .unwrap();
        let mut parent = writer.checkpoint().unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = parent.try_clear_admitted(|d| -> Result<Policy, ()> {
                if !provider_drop {
                    panic!("clear admission callback failed");
                }
                let provider = pool.reserve(d)?;
                pool.panic_drop.set(true);
                Ok(provider)
            });
        }))
        .is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| parent.len())).is_err());
        drop(parent);
        assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| writer.try_clear_admitted::<()>(|_| {
                panic!("failed cursor must refuse before admission")
            })))
            .is_err()
        );
        assert!(map.read().is_empty());
        without_allocations(|| drop(writer));
        without_allocations(|| drop(map));
        assert_eq!(pool.used.get(), 0);
    }
}

#[test]
fn clear_apply_cleanup_panic_cannot_expose_a_usable_partial_writer() {
    let pool = Pool::new();
    let map = map::<usize>(&pool);
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    // Zero-capacity start ensures both original charged tracking buffers are
    // retained by the reset checkpoint and dropped by its successful apply.
    assert_eq!(writer.inner.as_ref().admitted_tracking(), [(0, 0), (0, 0)]);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = writer.try_clear_admitted(|d| {
            let provider = pool.reserve(d)?;
            pool.arm_charge_panic_on_provider_drop.set(true);
            Ok::<_, ()>(provider)
        });
    }))
    .is_err());
    assert!(!pool.arm_charge_panic_on_provider_drop.get());
    assert!(
        !pool.panic_next_charge.get(),
        "actual saved-buffer charge cleanup ran"
    );
    assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
    assert!(map.read().is_empty());
    without_allocations(|| drop(writer));
    without_allocations(|| drop(map));
    assert_eq!(pool.used.get(), 0);
}

#[test]
fn clear_empty_root_has_exact_finite_layouts_and_never_copies_payloads() {
    let pool = Pool::new();
    let map = map::<usize>(&pool);
    let mut writer = map.try_write_admitted(|d| pool.reserve(d)).unwrap();
    let mut parent = writer.checkpoint().unwrap();
    let original = identity(parent.inner.as_ref());
    let used = pool.used.get();
    let expected = Layout::new::<CachePadded<Leaf<usize, usize, Charge>>>().size()
        + 2 * Layout::new::<*mut Node<usize, usize, Charge>>().size();
    pool.limit.set(used + expected);
    pool.panic_clone.set(1);
    parent
        .try_clear_admitted(|d| {
            assert_eq!(d.bytes(), expected);
            assert_eq!(d.allocations(), 3);
            pool.reserve(d)
        })
        .unwrap();
    assert_eq!(pool.panic_clone.get(), 1, "no payload clone was attempted");
    assert_eq!(pool.used.get(), used + expected);
    assert_ne!(identity(parent.inner.as_ref()).root, original.root);
    assert!(parent.is_empty());
    pool.limit.set(pool.used.get());
    without_allocations(|| drop(parent));
    assert_eq!(pool.used.get(), used);
    without_allocations(|| drop(writer));
    without_allocations(|| drop(map));
    assert_eq!(pool.used.get(), 0);
}
