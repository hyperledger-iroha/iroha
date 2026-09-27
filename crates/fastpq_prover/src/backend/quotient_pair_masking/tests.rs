//! Coefficient identity, extension evaluation and complete preflight regressions.

use super::*;

fn limits() -> PairMaskingLimits {
    PairMaskingLimits {
        max_chunk_degree_bound: 100,
        max_payload_bytes: 100_000,
        max_work_units: 100_000,
    }
}

fn shape() -> PairMaskingShape {
    PairMaskingShape {
        split: 8,
        quotient_coefficients: 23,
        quotient_degree_bound: 21,
        mask_coefficients: 5,
        mask_degree_bound: 3,
    }
}

fn evaluate(values: &[F], x: F) -> F {
    values
        .iter()
        .rev()
        .fold(F::ZERO, |acc, &coefficient| acc.mul(x).add(coefficient))
}

#[test]
fn unequal_chunks_reconstruct_every_coefficient_and_extension_value() {
    let plan = PairMaskingPlan::new(shape(), limits()).unwrap();
    let quotient: Vec<_> = (0..23)
        .map(|i| {
            if i < 21 {
                F::new([i + 1, i * 17, 3, 19]).unwrap()
            } else {
                F::ZERO
            }
        })
        .collect();
    let mask = [
        F::new([3, 5, 8, 13]).unwrap(),
        F::new([21, 34, 55, 89]).unwrap(),
        F::ONE,
        F::ZERO,
        F::ZERO,
    ];
    let [low, high] = plan.apply(&quotient, &mask).unwrap();
    assert_eq!((low.len(), high.len()), (13, 15));
    assert_eq!(plan.degree_bounds(), [11, 13]);
    assert!(low[11..].iter().all(|&v| v == F::ZERO));
    assert!(high[13..].iter().all(|&v| v == F::ZERO));
    for index in 0_usize..23 {
        let a = low.get(index).copied().unwrap_or(F::ZERO);
        let b = index
            .checked_sub(8)
            .and_then(|i| high.get(i))
            .copied()
            .unwrap_or(F::ZERO);
        assert_eq!(a.add(b), quotient[index]);
    }
    for x in [
        F::ZERO,
        F::ONE,
        F::new([7, 11, 13, 17]).unwrap(),
        F::embed_base(23),
    ] {
        assert_eq!(
            evaluate(&quotient, x),
            evaluate(&low, x).add(x.power(8).mul(evaluate(&high, x)))
        );
    }
    assert_eq!(quotient[20], F::new([21, 340, 3, 19]).unwrap());
    assert_eq!(mask[0], F::new([3, 5, 8, 13]).unwrap());
    let [zero_low, zero_high] = plan.apply(&quotient, &[F::ZERO; 5]).unwrap();
    assert_eq!(&zero_low[..8], &quotient[..8]);
    assert_eq!(&*zero_high, &quotient[8..]);
}

#[test]
fn exact_payload_work_and_every_limit_are_checked_before_input_use() {
    let plan = PairMaskingPlan::new(shape(), limits()).unwrap();
    assert_eq!(plan.payload_bytes(), (23 + 5 + 13 + 15) * 32);
    assert_eq!(plan.work_units(), 56 * 5 + 23 + 10);
    let exact = PairMaskingLimits {
        max_chunk_degree_bound: 13,
        max_payload_bytes: plan.payload_bytes(),
        max_work_units: plan.work_units(),
    };
    assert!(PairMaskingPlan::new(shape(), exact).is_ok());
    for bad in [
        PairMaskingLimits {
            max_chunk_degree_bound: 12,
            ..exact
        },
        PairMaskingLimits {
            max_payload_bytes: exact.max_payload_bytes - 1,
            ..exact
        },
        PairMaskingLimits {
            max_work_units: exact.max_work_units - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            PairMaskingPlan::new(shape(), bad),
            Err(Error::VerifierLimitExceeded { .. })
        ));
    }
    for bad in [
        PairMaskingShape {
            split: 0,
            ..shape()
        },
        PairMaskingShape {
            split: usize::MAX,
            ..shape()
        },
        PairMaskingShape {
            mask_degree_bound: 6,
            ..shape()
        },
        PairMaskingShape {
            quotient_degree_bound: 24,
            ..shape()
        },
    ] {
        assert!(PairMaskingPlan::new(bad, limits()).is_err());
    }
    let mut quotient = [F::ZERO; 23];
    let mut mask = [F::ZERO; 5];
    assert!(plan.apply(&quotient[..22], &mask).is_err());
    assert!(plan.apply(&quotient, &mask[..4]).is_err());
    quotient[22] = F::ONE;
    assert!(plan.apply(&quotient, &mask).is_err());
    quotient[22] = F::ZERO;
    mask[4] = F::ONE;
    assert!(plan.apply(&quotient, &mask).is_err());
}

#[test]
fn masks_longer_than_split_and_short_quotients_are_not_truncated() {
    let shape = PairMaskingShape {
        split: 2,
        quotient_coefficients: 1,
        quotient_degree_bound: 1,
        mask_coefficients: 5,
        mask_degree_bound: 5,
    };
    let plan = PairMaskingPlan::new(shape, limits()).unwrap();
    let mask = [F::ONE; 5];
    let [low, high] = plan.apply(&[F::ONE], &mask).unwrap();
    assert_eq!((low.len(), high.len()), (7, 5));
    let x = F::new([4, 3, 2, 1]).unwrap();
    assert_eq!(
        evaluate(&low, x).add(x.power(2).mul(evaluate(&high, x))),
        F::ONE
    );
}
