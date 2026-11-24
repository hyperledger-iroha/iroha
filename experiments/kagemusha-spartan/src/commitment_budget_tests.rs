//! A kernel memory budget bounds the constant-time column MSM's plan, never
//! its sums: a small budget commits exactly what the default one does, and a
//! budget below the single-base plan or a cancelled token is an error, never
//! an identity or partial sum. Private commitments build no fixed-base table,
//! so there is no table allowance or cache to account for.
use super::*;

fn check<C: PastaCurve>() {
    let n = 64;
    let gens = Gens::<C>::new(n);
    let small = MemoryBudget::new(32 << 10);
    let mut rng = SpikeRng::new(b"bounded commitment table refusal");
    let bounds = [0usize, 1, 11, 88, 255];
    let rows: Vec<Vec<F<C>>> = bounds
        .iter()
        .map(|bits| {
            (0..n)
                .map(|_| match bits {
                    0 => F::<C>::ZERO,
                    255 => rng.scalar(),
                    _ => rng.below::<F<C>>(*bits as u32),
                })
                .collect()
        })
        .collect();
    let columns: Vec<_> = rows
        .iter()
        .zip(bounds)
        .map(|(row, bits)| BoundedSecretScalars::new(row, bits, None).unwrap())
        .collect();
    let bounded = column_sums_with_budget(&gens, n, &columns, small, None).unwrap();
    let default = column_sums_with_budget(&gens, n, &columns, MemoryBudget::DEFAULT, None).unwrap();
    assert_eq!(bounded, default, "the budget changes the plan only");
    for (actual, scalars) in bounded.iter().zip(&rows) {
        let reference = scalars
            .iter()
            .zip(&gens.g)
            .fold(C::identity(), |sum, (scalar, base)| {
                sum + C::from(*base) * scalar
            });
        assert_eq!(*actual, reference);
    }
    assert!(matches!(
        column_sums_with_budget(&gens, n, &columns, MemoryBudget::new(0), None),
        Err(MsmError::Budget(_))
    ));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        column_sums_with_budget(&gens, n, &columns, small, Some(&cancellation)),
        Err(MsmError::Cancelled)
    ));
}

#[test]
fn small_budget_bounded_secret_commitments_on_both_curves() {
    check::<iroha_pasta::Ep>();
    check::<iroha_pasta::Eq>();
}
