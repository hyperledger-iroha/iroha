//! Exact tree geometry, actual CPU baseline and bounded original-owner profiles.

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
fn tree_profile_binds_exact_chunk_tail_and_leaf_count() {
    let geometry = Geometry::new(8_192 * 32, 32).unwrap();
    assert_eq!(geometry.leaves, 8_192);
    assert_ne!(Geometry::new(8_192 * 32 - 1, 32).unwrap(), geometry);
    assert_ne!(Geometry::new(8_192 * 17, 17).unwrap(), geometry);
    assert_ne!(Geometry::new(8_193 * 32, 32).unwrap(), geometry);
    for (bytes, chunk) in [
        (0, 32),
        (8_191 * 32, 32),
        (65_536 * 32 + 1, 32),
        (usize::MAX, 32),
        (8_192, 0),
        (8_192, 33),
    ] {
        assert!(Geometry::new(bytes, chunk).is_none());
    }
    assert!(Geometry::new(65_536, 1).is_some());
    assert!(Geometry::new(65_536 * 32, 32).is_some());
}

#[test]
fn complete_tree_cost_uses_fastest_zero_cpu_and_slowest_native_pattern() {
    let _scalar = Scalar::enter();
    let baseline = Sha256Baseline::capture(Sha256Context::production()).unwrap();
    let geometry = Geometry::new(8_192, 1).unwrap();
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
fn tree_cache_retains_original_shape_cooldown_and_rejects_other_rayon_pools() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let now = Instant::now();
    let geometry = Geometry::new(8_192 * 17, 17).unwrap();
    let tail = Geometry::new(8_192 * 17 - 1, 17).unwrap();
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
fn transient_tree_refusal_retries_and_parity_failure_keeps_owner_quarantined() {
    let _scalar = Scalar::enter();
    let context = Sha256Context::production();
    let baseline = Sha256Baseline::capture(context).unwrap();
    let geometry = Geometry::new(8_193 * 17 - 7, 17).unwrap();
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
