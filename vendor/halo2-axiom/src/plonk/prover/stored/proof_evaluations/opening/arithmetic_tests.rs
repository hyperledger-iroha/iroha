//! Differential guarded polynomial arithmetic, erased tails and checked scratch accounting.

use super::*;
use halo2curves::pasta::{Fp, Fq};

fn arithmetic<F>()
where
    F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + From<u64>,
{
    for k in [0, 1, 4, 6] {
        let domain = EvaluationDomain::<F>::new(3, k);
        let n = 1 << k;
        let original = domain.coeff_from_vec((0..n).map(|i| F::from((i * i + 3) as u64)).collect());
        let addend = domain.coeff_from_vec((0..n).map(|i| -F::from((i * 7 + 5) as u64)).collect());
        for challenge in [F::ZERO, F::ONE, -F::ONE, F::from(13)] {
            let expected = original.clone() * challenge + &addend;
            let mut actual = original.values.clone();
            let pointer = actual.as_ptr();
            fold(&mut actual, challenge, &addend.values).unwrap();
            assert_eq!(actual, expected.values);
            assert_eq!(actual.as_ptr(), pointer);
            assert_eq!(actual.len(), n);
        }
    }
    for n in [1, 2, 3, 16, 33] {
        for offset in 0..4 {
            let mut actual = (0..n)
                .map(|i| F::from((i * i * 3 + 11) as u64))
                .collect::<Vec<_>>();
            let pointer = actual.as_ptr();
            let capacity = actual.capacity();
            let mut reference = actual.clone();
            let mut live = n;
            for step in 0..n {
                let point = [F::ZERO, F::ONE, -F::ONE, F::from(7)][(step + offset) % 4];
                reference = crate::arithmetic::kate_division(reference.iter(), point);
                divide(&mut actual, &mut live, point).unwrap();
                assert_eq!(live, n - step - 1);
                assert_eq!(&actual[..live], reference.as_slice());
                assert!(actual[live..].iter().all(|value| *value == F::ZERO));
                assert_eq!(actual.len(), n, "removed slots remain initialized for Drop");
                assert_eq!(actual.capacity(), capacity);
                assert_eq!(actual.as_ptr(), pointer);
            }
            assert_eq!(live, 0);
            assert!(actual.iter().all(|value| *value == F::ZERO));
        }
    }
}

#[test]
fn both_pasta_opening_folds_and_every_kate_step_match_ordinary_arithmetic_and_erase_tails() {
    arithmetic::<Fp>();
    arithmetic::<Fq>();
}

fn invalid<F: StoredAssignmentFieldV1 + From<u64>>() {
    let original = vec![F::from(11), F::from(13), F::from(17)];
    for live in [0, original.len() + 1, usize::MAX] {
        let mut actual = original.clone();
        let mut attempted = live;
        assert!(matches!(
            divide(&mut actual, &mut attempted, F::ONE),
            Err(StoredLookupErrorV1::Context)
        ));
        assert_eq!(actual, original);
        assert_eq!(attempted, live);
    }
    let mut empty = Vec::<F>::new();
    assert!(divide(&mut empty, &mut 0, F::ZERO).is_err());
    for len in [0, 2, 4] {
        let mut actual = original.clone();
        let addend = vec![F::from(23); len];
        assert!(matches!(
            fold(&mut actual, F::ZERO, &addend),
            Err(StoredLookupErrorV1::Context)
        ));
        assert_eq!(
            actual, original,
            "length rejection precedes even the ZERO overwrite"
        );
        assert!(addend.iter().all(|value| *value == F::from(23)));
    }
}

#[test]
fn both_pasta_opening_invalid_arithmetic_bounds_refuse_without_mutating_inputs() {
    invalid::<Fp>();
    invalid::<Fq>();
}

fn budgets<F: StoredAssignmentFieldV1>() {
    for values in [
        (usize::MAX, 0, 0, 1),
        (0, usize::MAX, 0, 1),
        (0, 0, usize::MAX, 1),
    ] {
        assert!(matches!(
            field_payload::<F>(values.0, values.1, values.2, values.3),
            Err(StoredLookupErrorV1::Context)
        ));
    }
    assert!(matches!(
        Planner::<F>::minimum_payload(usize::MAX),
        Err(StoredLookupErrorV1::Context)
    ));
    assert!(matches!(
        Planner::<F>::minimum_payload(usize::MAX / std::mem::size_of::<F>() + 1),
        Err(StoredLookupErrorV1::Context)
    ));
    let n = 33;
    assert_eq!(
        field_payload::<F>(n, n, n, n).unwrap(),
        4 * n * std::mem::size_of::<F>()
            + std::mem::size_of::<Workspace<F>>()
            + std::mem::size_of::<Fields<F>>()
    );
    let planner = Planner::<F>::new(7).unwrap();
    assert!(planner.actual_payload().unwrap() >= Planner::<F>::minimum_payload(7).unwrap());
}

#[test]
fn both_pasta_opening_payload_overflow_refuses_before_allocation_and_counts_h_tile() {
    budgets::<Fp>();
    budgets::<Fq>();
}
