//! Exact retained geometry, actual CPU baseline and bounded original-owner profiles.

use super::*;

struct Scalar(Option<super::super::SimdChoice>);
impl Scalar {
    fn enter() -> Self {
        Self(super::super::set_thread_forced_simd(Some(
            super::super::SimdChoice::Scalar,
        )))
    }
}
impl Drop for Scalar {
    fn drop(&mut self) {
        super::super::set_thread_forced_simd(self.0);
    }
}

fn profile(geometry: Geometry, baseline: Sha256Baseline, cpu: u64, metal: u64) -> Profile {
    Profile::from_trials(geometry, baseline, [[cpu; TRIALS]; 2], [[metal; TRIALS]; 2]).unwrap()
}

#[test]
fn rehash_profile_binds_exact_chunk_tail_and_leaf_count() {
    let geometry = Geometry::new(8_192 * 32, 32, 8_192).unwrap();
    assert_eq!(geometry.leaves, 8_192);
    assert_ne!(Geometry::new(8_192 * 32 - 1, 32, 8_192).unwrap(), geometry);
    assert_ne!(Geometry::new(8_192 * 17, 17, 8_192).unwrap(), geometry);
    assert_ne!(Geometry::new(8_193 * 32, 32, 8_193).unwrap(), geometry);
    assert_eq!(Geometry::new(usize::MAX, 32, 8_192), Some(geometry));
    assert_ne!(Geometry::new(0, 32, 8_192).unwrap(), geometry);
    for (bytes, chunk, leaves) in [
        (0, 32, 0),
        (8_191 * 32, 32, 8_191),
        (0, 32, maximum_memory_leaves() + 1),
        (8_192, 0, 8_192),
        (8_192, 33, 8_192),
        (usize::MAX, 32, usize::MAX),
    ] {
        assert!(Geometry::new(bytes, chunk, leaves).is_none());
    }
    for stack in [crate::Memory::MIN_STACK_SIZE, crate::Memory::STACK_SIZE] {
        let bytes = crate::Memory::image_bytes_for_stack_limit(stack).unwrap();
        let leaves = bytes.div_ceil(32);
        assert!(
            leaves > 65_536,
            "retired profile could not admit actual Memory"
        );
        assert_eq!(Geometry::new(bytes, 32, leaves).unwrap().byte_len, bytes);
    }
    assert_eq!(maximum_memory_leaves(), 229_376);
    assert!(Geometry::new(0, 1, maximum_memory_leaves()).is_some());
}

#[test]
fn complete_rehash_cost_uses_fastest_zero_cpu_and_slowest_native_pattern() {
    let _scalar = Scalar::enter();
    let baseline = Sha256Baseline::capture(Sha256Context::production()).unwrap();
    let geometry = Geometry::new(8_192, 1, 8_192).unwrap();
    let measured = Profile::from_trials(
        geometry,
        baseline,
        [[80, 90, 100], [100, 110, 120]],
        [[40, 45, 50], [50, 55, 60]],
    )
    .unwrap();
    assert_eq!(
        (measured.cpu_ns, measured.metal_ns, measured.cost()),
        (80, 60, Some(60))
    );
    assert_eq!(profile(geometry, baseline, 100, 90).cost(), None);
    assert_eq!(
        profile(geometry, baseline, u64::MAX, u64::MAX - 1).cost(),
        None
    );
    assert_eq!(
        profile(geometry, baseline, u64::MAX, u64::MAX / 2).cost(),
        Some(u64::MAX / 2)
    );
    for cpu in [[[0; TRIALS]; 2], [[1, 5, 1]; 2]] {
        assert_eq!(
            Profile::from_trials(geometry, baseline, cpu, [[1; TRIALS]; 2]).unwrap_err(),
            Failure::Overloaded
        );
    }
}

#[test]
fn rehash_cache_retains_original_shape_cooldown_and_rejects_other_rayon_pools() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let now = Instant::now();
    let geometry = Geometry::new(8_192 * 17, 17, 8_192).unwrap();
    let tail = Geometry::new(8_192 * 17 - 1, 17, 8_192).unwrap();
    let mut cache = CostCache::default();
    assert_eq!(
        cache.qualified_cost(
            now,
            geometry,
            baseline,
            context,
            || None,
            |_| panic!("not scheduled")
        ),
        None
    );
    assert!(cache.retry_after.is_none());
    assert_eq!(
        cache.qualified_cost(
            now,
            geometry,
            baseline,
            context,
            || Some(now),
            |_| Ok(profile(geometry, baseline, 100, 20))
        ),
        Some(20)
    );
    assert_eq!(
        cache.qualified_cost(
            now,
            tail,
            baseline,
            context,
            || panic!("cooldown"),
            |_| panic!("wrong shape")
        ),
        None
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    pool.install(|| {
        assert_eq!(
            cache.qualified_cost(
                now,
                geometry,
                baseline,
                context,
                || panic!("foreign pool"),
                |_| panic!("foreign pool")
            ),
            None
        );
    });
    assert_eq!(
        cache.qualified_cost(
            now,
            geometry,
            baseline,
            context,
            || panic!("cached"),
            |_| panic!("cached")
        ),
        Some(20)
    );
    let later = now + RETRY_AFTER;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.qualified_cost(
                later,
                tail,
                baseline,
                context,
                || Some(later),
                |_| panic!("sample unwind"),
            );
        }))
        .is_err()
    );
    assert_eq!(
        cache.qualified_cost(
            later,
            tail,
            baseline,
            context,
            || panic!("unwind cooldown"),
            |_| panic!("unwind cooldown")
        ),
        None
    );
    assert_eq!(
        cache.qualified_cost(
            later,
            geometry,
            baseline,
            context,
            || panic!("old exact shape"),
            |_| panic!("old exact shape")
        ),
        Some(20)
    );
}

#[test]
fn transient_rehash_refusal_retries_and_parity_failure_keeps_owner_quarantined() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let geometry = Geometry::new(8_193 * 17 - 7, 17, 8_193).unwrap();
    let now = Instant::now();
    for failure in [
        Failure::Deadline,
        Failure::Allocation,
        Failure::Backend,
        Failure::CpuChanged,
        Failure::Overloaded,
    ] {
        let mut cache = CostCache::default();
        assert_eq!(
            cache.qualified_cost(
                now,
                geometry,
                baseline,
                context,
                || Some(now),
                |_| Err(failure)
            ),
            None
        );
        let later = now + RETRY_AFTER;
        assert_eq!(
            cache.qualified_cost(
                later,
                geometry,
                baseline,
                context,
                || Some(later),
                |_| Ok(profile(geometry, baseline, 100, 20))
            ),
            Some(20)
        );
    }
    let mut cache = CostCache::default();
    assert_eq!(
        cache.qualified_cost(
            now,
            geometry,
            baseline,
            context,
            || Some(now),
            |_| Err(Failure::Parity)
        ),
        None
    );
    assert!(cache.quarantined);
    assert_eq!(
        cache.qualified_cost(
            now + RETRY_AFTER,
            geometry,
            baseline,
            context,
            || panic!("original quarantine"),
            |_| panic!("original quarantine")
        ),
        None
    );
}

#[test]
fn expired_rehash_deadline_declines_before_any_sample_allocation() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let geometry = Geometry::new(0, 32, 8_192).unwrap();
    assert!(matches!(
        calibrate(
            geometry,
            baseline,
            context,
            Instant::now() - MAX_CALIBRATION
        ),
        Err(Failure::Deadline)
    ));
}

#[test]
fn exact_device_costs_rank_complete_operations_without_geometry_interpolation() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let smaller = Geometry::new(24_576 * 17, 17, 24_576).unwrap();
    let larger = Geometry::new(28_672 * 17, 17, 28_672).unwrap();
    assert!(
        profile(smaller, baseline, 1_000, 50).cost() < profile(smaller, baseline, 1_000, 60).cost()
    );
    assert!(
        profile(larger, baseline, 1_000, 70).cost() > profile(larger, baseline, 1_000, 60).cost()
    );
    let mut cache = CostCache::default();
    let now = Instant::now();
    assert_eq!(
        cache.qualified_cost(
            now,
            smaller,
            baseline,
            context,
            || Some(now),
            |_| Ok(profile(smaller, baseline, 1_000, 50))
        ),
        Some(50)
    );
    assert_eq!(
        cache.qualified_cost(
            now,
            larger,
            baseline,
            context,
            || panic!("geometry cooldown"),
            |_| panic!("no extrapolation")
        ),
        None
    );
}

#[test]
fn rehash_scheduler_unwind_keeps_later_calibration_and_original_quarantine() {
    use super::super::metal_owner::{DeviceRegistry, FairProgress};
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::Mutex,
    };
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let geometry = Geometry::new(16_384 * 32, 32, 16_384).unwrap();
    let registry = DeviceRegistry::<Mutex<CostCache>>::new();
    let credit = iroha_allocation::AllocationBudget::new(16384);
    assert!(registry.prepare(2, |layout| credit.try_reserve(layout).ok()));
    for identity in [11, 22] {
        let record = registry
            .observe(identity, 2, |bytes| credit.try_reserve_bytes(bytes).ok())
            .unwrap();
        assert!(record.initialize(|| Some(Mutex::new(CostCache::default()))));
    }
    let first = registry.record(0, 2).unwrap();
    let second = registry.record(1, 2).unwrap();
    let first_cache = first.value().unwrap();
    let progress = FairProgress::new();
    let now = Instant::now();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut pass = progress.try_pass(2, now, MAX_CALIBRATION).unwrap();
            let _ = registry.select_costed(2, 2, pass.start(), |index, record, cache| {
                cache.try_lock().ok()?.qualified_cost(
                    now,
                    geometry,
                    baseline,
                    context,
                    || pass.begin_attempt(index, now),
                    |_| {
                        assert_eq!(record.health().identity(), 11);
                        record.health().quarantine(true);
                        panic!("backend unwind through original cache and family pass")
                    },
                )
            });
        }))
        .is_err()
    );
    assert!(!first.health().usable());
    assert!(first_cache.is_poisoned());
    assert!(
        second
            .value()
            .unwrap()
            .lock()
            .unwrap()
            .retry_after
            .is_none(),
        "unvisited device receives no panic penalty"
    );
    let later = now + Duration::from_millis(1);
    let mut pass = progress
        .try_pass(2, later, MAX_CALIBRATION)
        .expect("fair scheduling recovers scalar cursor");
    assert_eq!(pass.start(), 1);
    assert!(progress.try_pass(2, later, MAX_CALIBRATION).is_none());
    let selected = registry
        .select_costed(2, 1, pass.start(), |index, record, cache| {
            cache.try_lock().ok()?.qualified_cost(
                later,
                geometry,
                baseline,
                context,
                || pass.begin_attempt(index, later),
                |_| {
                    assert_eq!(record.health().identity(), 22);
                    Ok(profile(geometry, baseline, 100, 20))
                },
            )
        })
        .expect("later healthy device calibrates after scheduling recovery");
    assert_eq!(selected.health().identity(), 22);
    assert!(
        !first.health().usable(),
        "cursor recovery cannot revive physical health"
    );
    assert!(
        first_cache.is_poisoned(),
        "coupled profile owner is not recovered as scalar state"
    );
}

#[test]
fn fair_rehash_sampling_reaches_later_owner_and_keeps_cached_cost_after_deadline() {
    use super::super::metal_owner::{DeviceRegistry, FairProgress};
    use std::{cell::Cell, sync::Mutex};
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let geometry = Geometry::new(16_384 * 32, 32, 16_384).unwrap();
    let registry = DeviceRegistry::<Mutex<CostCache>>::new();
    let credit = iroha_allocation::AllocationBudget::new(16384);
    assert!(registry.prepare(2, |layout| credit.try_reserve(layout).ok()));
    for identity in [11, 22] {
        let record = registry
            .observe(identity, 2, |bytes| credit.try_reserve_bytes(bytes).ok())
            .unwrap();
        assert!(record.initialize(|| Some(Mutex::new(CostCache::default()))));
    }
    let progress = FairProgress::new();
    let base = Instant::now();
    let first_attempts = Cell::new(0);
    let second_attempts = Cell::new(0);
    for turn in 0..3 {
        let now = base + RETRY_AFTER * turn;
        let clock = Cell::new(now);
        let mut pass = progress.try_pass(2, now, MAX_CALIBRATION).unwrap();
        let selected = registry.select_costed(2, 2, pass.start(), |index, record, cache| {
            cache.lock().unwrap().qualified_cost(
                clock.get(),
                geometry,
                baseline,
                context,
                || pass.begin_attempt(index, clock.get()),
                |started| {
                    if record.health().identity() == 11 {
                        first_attempts.set(first_attempts.get() + 1);
                        clock.set(started + MAX_CALIBRATION);
                        Err(Failure::Deadline)
                    } else {
                        second_attempts.set(second_attempts.get() + 1);
                        Ok(profile(geometry, baseline, 100, 20))
                    }
                },
            )
        });
        if turn == 0 {
            assert!(selected.is_none());
            let later = registry.record(1, 2).unwrap();
            assert!(
                later.value().unwrap().lock().unwrap().retry_after.is_none(),
                "unvisited owner receives no cooldown"
            );
        } else {
            assert_eq!(selected.unwrap().health().identity(), 22);
        }
    }
    assert_eq!(first_attempts.get(), 3);
    assert_eq!(second_attempts.get(), 1);
    let _busy = progress.try_pass(2, base, MAX_CALIBRATION).unwrap();
    assert!(progress.try_pass(2, base, MAX_CALIBRATION).is_none());
    let later = registry.record(1, 2).unwrap();
    assert_eq!(
        later.value().unwrap().lock().unwrap().qualified_cost(
            base + RETRY_AFTER * 3 + MAX_CALIBRATION,
            geometry,
            baseline,
            context,
            || panic!("cached qualification needs no family pass"),
            |_| panic!("cached owner needs no recalibration")
        ),
        Some(20)
    );
}
