//! Exact public geometry, conservative complete costs and original cache state.

use super::*;

fn profile(geometry: Geometry, cpu: u64, metal: u64) -> Profile {
    Profile::from_trials(geometry, [[cpu; TRIALS]; 2], [[metal; TRIALS]; 2]).unwrap()
}

#[test]
fn root_geometry_binds_chunk_tail_and_count_without_extrapolation() {
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
fn whole_root_cost_uses_fastest_cpu_pattern_and_slowest_native_pattern() {
    let geometry = Geometry::new(8_192, 1).unwrap();
    let measured = Profile::from_trials(
        geometry,
        [[100, 110, 120], [80, 90, 100]],
        [[40, 45, 50], [50, 55, 60]],
    )
    .unwrap();
    assert_eq!(
        (measured.cpu_ns, measured.metal_ns, measured.cost()),
        (80, 60, Some(60))
    );
    assert_eq!(profile(geometry, 100, 90).cost(), None);
    assert_eq!(profile(geometry, u64::MAX, u64::MAX - 1).cost(), None);
    assert_eq!(
        profile(geometry, u64::MAX, u64::MAX / 2).cost(),
        Some(u64::MAX / 2)
    );
    assert!(matches!(
        Profile::from_trials(geometry, [[0; TRIALS]; 2], [[1; TRIALS]; 2]),
        Err(Failure::Overloaded)
    ));
    assert!(matches!(
        Profile::from_trials(geometry, [[1, 5, 1]; 2], [[1; TRIALS]; 2]),
        Err(Failure::Overloaded)
    ));
}

#[test]
fn cache_requires_exact_shape_and_retains_original_cooldown_through_unwind() {
    let now = Instant::now();
    let original = Geometry::new(8_192 * 32, 32).unwrap();
    let tail = Geometry::new(8_192 * 32 - 1, 32).unwrap();
    let mut cache = CostCache::default();
    assert_eq!(
        cache.qualified_cost(now, original, || None, |_| panic!("unscheduled")),
        None
    );
    assert!(cache.retry_after.is_none());
    assert_eq!(
        cache.qualified_cost(
            now,
            original,
            || Some(now),
            |_| Ok(profile(original, 100, 20))
        ),
        Some(20)
    );
    assert_eq!(
        cache.qualified_cost(
            now,
            tail,
            || panic!("cooldown"),
            |_| panic!("wrong geometry")
        ),
        None
    );
    assert_eq!(
        cache.qualified_cost(now, original, || panic!("cached"), |_| panic!("cached")),
        Some(20)
    );
    let later = now + RETRY_AFTER;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.qualified_cost(later, tail, || Some(later), |_| panic!("sample unwind"));
        }))
        .is_err()
    );
    assert_eq!(
        cache.qualified_cost(
            later,
            tail,
            || panic!("unwind keeps cooldown"),
            |_| panic!("unwind keeps cooldown")
        ),
        None
    );
    assert_eq!(
        cache.qualified_cost(
            later,
            original,
            || panic!("old exact cached shape"),
            |_| panic!("cached")
        ),
        Some(20)
    );
}

#[test]
fn allocation_failure_retries_but_parity_failure_cannot_reuse_a_profile() {
    let geometry = Geometry::new(8_192 * 17 - 7, 17).unwrap();
    let now = Instant::now();
    for failure in [
        Failure::Deadline,
        Failure::Allocation,
        Failure::Backend,
        Failure::Overloaded,
    ] {
        let mut cache = CostCache::default();
        assert_eq!(
            cache.qualified_cost(now, geometry, || Some(now), |_| Err(failure)),
            None
        );
        let later = now + RETRY_AFTER;
        assert_eq!(
            cache.qualified_cost(
                later,
                geometry,
                || Some(later),
                |_| Ok(profile(geometry, 100, 20))
            ),
            Some(20)
        );
    }
    for wrong_shape in [false, true] {
        let mut cache = CostCache::default();
        assert_eq!(
            cache.qualified_cost(
                now,
                geometry,
                || Some(now),
                |_| {
                    if wrong_shape {
                        Ok(profile(Geometry::new(8_192, 1).unwrap(), 100, 20))
                    } else {
                        Err(Failure::Parity)
                    }
                }
            ),
            None
        );
        assert!(cache.quarantined);
        assert_eq!(
            cache.qualified_cost(
                now + RETRY_AFTER,
                geometry,
                || panic!("quarantined"),
                |_| panic!("quarantined")
            ),
            None
        );
    }
}
