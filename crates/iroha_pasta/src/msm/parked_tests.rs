//! Affine-queue collisions, epoch transitions, and their memory budget.
use super::*;

fn compare_epochs<C: PastaCurve, const SECRET: bool>() {
    const BUCKETS: usize = 80;
    let g = C::generator();
    let bases = [g.to_affine(), -g.to_affine(), g.double().to_affine()];
    let mut buckets = Buckets::<C, SECRET>::new(&bases, BUCKETS);
    assert!(buckets.park_limit > 0);
    // At the first wrap a queued collision still has its high bit set.
    buckets.epoch = EPOCH_MASK;
    let mut expected = [C::identity(); BUCKETS];
    for epoch in 0..5 {
        // Repeated signed points exercise doubles, cancellations, and
        // three-way collisions that exceed the single parked slot per bucket.
        for k in 0..BUCKETS {
            for j in 0..6 {
                let bucket = (k * 3 + epoch) % BUCKETS;
                let point = (k + j * 2 + epoch) % bases.len();
                let negative = (k + j + epoch) % 3 == 0;
                buckets.insert(bucket, point, negative);
                let addend = C::from(bases[point]);
                expected[bucket] += if negative { -addend } else { addend };
            }
        }
        buckets.flush();
        assert!(buckets.parked.is_empty() && buckets.queue_bucket.is_empty());
        for (i, e) in expected.iter().enumerate() {
            assert_eq!(buckets.reduce(i, 1), *e);
        }
    }
}

#[test]
fn parked_epochs_match_direct_additions_on_both_curves() {
    compare_epochs::<crate::Ep, false>();
    compare_epochs::<crate::Ep, true>();
    compare_epochs::<crate::Eq, false>();
    compare_epochs::<crate::Eq, true>();
}

#[test]
fn private_wipe_clears_pending_and_parked_collisions() {
    use group::{Curve, Group};
    let g = crate::Ep::generator();
    let bases = [g.to_affine(), -g.to_affine(), g.double().to_affine()];
    let mut buckets = Buckets::<crate::Ep, true>::new(&bases, 80);
    for (point, negative) in [(0, false), (1, false), (2, true), (0, true)] {
        buckets.insert(0, point, negative);
    }
    assert_eq!(buckets.parked.len(), 1);
    assert_eq!(buckets.queue_bucket.len(), 1);
    assert!(buckets.overflow_used[0]);
    buckets.wipe();
    assert!(buckets.parked.is_empty() && buckets.queue_bucket.is_empty());
    assert!(buckets.mark.iter().all(|mark| *mark == 0));
    assert_eq!(buckets.reduce(0, 80), crate::Ep::identity());
    buckets.insert(3, 0, true);
    buckets.flush();
    assert_eq!(buckets.reduce(3, 1), -g);
}

#[test]
fn parked_allocations_fit_the_affine_queue_allowance() {
    for count in [2, 8, 63, 64, 80, 1024, 4096, 1 << 15] {
        let previous = (count / 4).clamp(16, MAX_BATCH);
        let (active, parked) = queue_capacities(count, previous);
        let scratch = active * ENTRY_BYTES + parked * size_of::<[u32; 3]>();
        assert!(scratch <= previous * ENTRY_BYTES);
        assert_eq!(bucket_bytes(count), Some(count * BUCKET_BYTES + scratch));
        assert!(parked < active);
        if count < 64 {
            assert_eq!((active, parked), (previous, 0));
        }
    }
}
