//! Exact-key cache custody, deterministic policy and bounded retry controls.

use super::*;
use crate::signature::Ed25519BatchItem;

fn batch<'a>(message: &'a [u8], lengths: &[usize]) -> Vec<Ed25519BatchItem<'a>> {
    lengths
        .iter()
        .map(|&len| Ed25519BatchItem {
            message: &message[..len],
            signature: [1; 64],
            public_key: [2; 32],
        })
        .collect()
}
fn profile(geometry: MessageGeometry<'_, '_>, cpu: u64, metal: u64) -> Profile {
    Profile::from_trials(geometry, [cpu; TRIALS], [metal; TRIALS]).unwrap()
}

#[test]
fn exact_key_lookup_and_cpu_winner_are_cached_without_resampling() {
    let bytes = [0; 128];
    let mut lengths = [32; 16];
    lengths[0] = 47;
    lengths[1] = 49;
    let first = batch(&bytes, &lengths);
    lengths[0] = 48;
    lengths[1] = 48;
    let second = batch(&bytes, &lengths);
    let first = MessageGeometry::new(&first).unwrap();
    let second = MessageGeometry::new(&second).unwrap();
    let now = Instant::now();
    for metal in [20, 95] {
        let mut cache = CostCache::default();
        let expected = (metal == 20).then_some(metal);
        assert_eq!(
            cache.qualified_cost(now, first, || Some(now), |_| Ok(profile(first, 100, metal))),
            expected
        );
        assert!(cache.profile.is_some());
        assert_eq!(
            cache.qualified_cost(
                now,
                first,
                || panic!("cached key"),
                |_| panic!("cached profile")
            ),
            expected
        );
        assert_eq!(
            cache.qualified_cost(
                now,
                second,
                || panic!("different geometry cooldown"),
                |_| panic!("cooldown")
            ),
            None
        );
        let later = now + RETRY_AFTER;
        assert_eq!(
            cache.qualified_cost(
                later,
                second,
                || Some(later),
                |_| Ok(profile(second, 100, metal))
            ),
            expected
        );
        assert!(cache.profile.as_ref().unwrap().geometry.matches(second));
        assert!(!cache.profile.as_ref().unwrap().geometry.matches(first));
    }
}

#[test]
fn local_failures_and_unwind_retain_cooldown_but_never_claim_parity_failure() {
    let inputs = batch(&[0; 48], &[48; 16]);
    let geometry = MessageGeometry::new(&inputs).unwrap();
    let now = Instant::now();
    for failure in [
        Failure::Deadline,
        Failure::Allocation,
        Failure::Backend,
        Failure::Overloaded,
        Failure::CpuMismatch,
    ] {
        let mut cache = CostCache::default();
        assert_eq!(
            cache.qualified_cost(now, geometry, || None, |_| panic!("no sampling pass")),
            None
        );
        assert!(cache.retry_after.is_none());
        assert_eq!(
            cache.qualified_cost(now, geometry, || Some(now), |_| Err(failure)),
            None
        );
        assert!(!cache.quarantined);
        assert_eq!(
            cache.qualified_cost(now, geometry, || panic!("cooldown"), |_| panic!("cooldown")),
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
    let mut cache = CostCache::default();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.qualified_cost(
            now,
            geometry,
            || Some(now),
            |_| panic!("sampling unwind")
        )))
        .is_err()
    );
    assert_eq!(cache.retry_after, Some(now + RETRY_AFTER));
    assert!(!cache.quarantined);
    assert_eq!(
        cache.qualified_cost(
            now,
            geometry,
            || panic!("unwind cooldown"),
            |_| panic!("cooldown")
        ),
        None
    );
}

#[test]
fn wrong_geometry_and_parity_failure_cannot_reuse_an_older_success() {
    let first = batch(&[0; 49], &[47; 16]);
    let second = batch(&[0; 49], &[49; 16]);
    let first = MessageGeometry::new(&first).unwrap();
    let second = MessageGeometry::new(&second).unwrap();
    let now = Instant::now();
    for wrong_key in [false, true] {
        let mut cache = CostCache::default();
        assert_eq!(
            cache.qualified_cost(now, first, || Some(now), |_| Ok(profile(first, 100, 20))),
            Some(20)
        );
        let later = now + RETRY_AFTER;
        assert_eq!(
            cache.qualified_cost(
                later,
                second,
                || Some(later),
                |_| if wrong_key {
                    Ok(profile(first, 100, 20))
                } else {
                    Err(Failure::Parity)
                }
            ),
            None
        );
        assert!(cache.quarantined);
        assert_eq!(
            cache.qualified_cost(
                later,
                first,
                || panic!("quarantine"),
                |_| panic!("quarantine")
            ),
            None
        );
    }
}

#[test]
fn conservative_samples_reject_noise_zeroes_and_full_width_margin_without_overflow() {
    let inputs = batch(&[0; 32], &[32; 16]);
    let geometry = MessageGeometry::new(&inputs).unwrap();
    for (cpu, metal) in [
        ([0, 100, 100], [1; 3]),
        ([100; 3], [0, 1, 1]),
        ([100, 401, 100], [1; 3]),
        ([100; 3], [1, 5, 1]),
    ] {
        assert!(matches!(
            Profile::from_trials(geometry, cpu, metal),
            Err(Failure::Overloaded)
        ));
    }
    let measured = Profile::from_trials(geometry, [120, 100, 110], [20, 30, 25]).unwrap();
    assert_eq!(
        (measured.cpu_ns, measured.metal_ns, measured.cost()),
        (100, 30, Some(30))
    );
    assert_eq!(
        profile(geometry, u64::MAX, u64::MAX / 2).cost(),
        Some(u64::MAX / 2)
    );
    assert_eq!(profile(geometry, u64::MAX, u64::MAX).cost(), None);
    assert_eq!(profile(geometry, 100, 90).cost(), None);
}
