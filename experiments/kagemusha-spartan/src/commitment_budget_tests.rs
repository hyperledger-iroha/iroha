//! Table allocation is an optimization, not a larger memory allowance.
use super::*;

fn check<C: PastaCurve>() {
    let n = 64;
    let gens = Gens::<C>::new(n);
    let budget = MemoryBudget::new(32 << 10);
    let refused = gens
        .table_with_budget(n, table_window::<C>(), budget)
        .unwrap_err();
    assert!(refused.required > budget.bytes());
    assert_eq!(refused.budget, budget.bytes());
    assert!(gens.tables.lock().unwrap().is_empty());

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
    let bounded = column_sums_with_budget(&gens, n, &columns, true, budget, None).unwrap();
    assert!(
        gens.tables.lock().unwrap().is_empty(),
        "refused tables are not cached"
    );
    let variable = column_sums_with_budget(&gens, n, &columns, false, budget, None).unwrap();
    let table =
        column_sums_with_budget(&gens, n, &columns, true, MemoryBudget::DEFAULT, None).unwrap();
    assert!(
        !gens.tables.lock().unwrap().is_empty(),
        "reference actually uses a table"
    );
    for ((actual, variable), (table, scalars)) in
        bounded.iter().zip(variable).zip(table.iter().zip(&rows))
    {
        let reference = scalars
            .iter()
            .zip(&gens.g)
            .fold(C::identity(), |sum, (scalar, base)| {
                sum + C::from(*base) * scalar
            });
        assert_eq!(*actual, reference);
        assert_eq!(variable, reference);
        assert_eq!(*table, reference);
    }
    // A cached table must not bypass a later smaller budget.
    assert!(
        gens.table_with_budget(n, table_window::<C>(), budget)
            .is_err()
    );
    assert_eq!(
        column_sums_with_budget(&gens, n, &columns, true, budget, None).unwrap(),
        bounded
    );
    assert!(matches!(
        column_sums_with_budget(&gens, n, &columns, true, MemoryBudget::new(0), None),
        Err(MsmError::Budget(_))
    ));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        column_sums_with_budget(&gens, n, &columns, true, budget, Some(&cancellation)),
        Err(MsmError::Cancelled)
    ));
}

#[test]
fn oversized_table_uses_bounded_secret_commitments_on_both_curves() {
    check::<iroha_pasta::Ep>();
    check::<iroha_pasta::Eq>();
}
