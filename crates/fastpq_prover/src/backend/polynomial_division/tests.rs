//! General dense product and remainder oracles for full coefficient division.
use super::super::{GOLDILOCKS_MODULUS, polynomial_field::PolynomialField};
use super::*;
fn f(seed: u64) -> F {
    F::new([seed, seed + 2, seed + 5, seed + 11]).unwrap()
}
fn plan(n: usize, extent: usize, degree: usize, q_extent: usize) -> VanishingDivisionPlan {
    VanishingDivisionPlan::new(n, extent, degree, q_extent, usize::MAX, usize::MAX).unwrap()
}
fn product(left: &[F], right: &[F]) -> Vec<F> {
    let mut out = vec![F::ZERO; left.len() + right.len() - 1];
    for (i, &a) in left.iter().enumerate() {
        for (j, &b) in right.iter().enumerate() {
            out[i + j] = out[i + j].add(a.mul(b));
        }
    }
    out
}
fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul(point).add(value))
}

#[test]
fn exact_division_matches_dense_convolution_with_overlapping_high_terms() {
    for n in [1, 2, 4, 8, 16] {
        for k in [1, n, n + 1, 2 * n + 3] {
            let expected: Vec<_> = (0..k).map(|i| f(11 + i as u64)).collect();
            let mut vanishing = vec![F::ZERO; n + 1];
            vanishing[0] = F::ZERO.sub(F::ONE);
            vanishing[n] = F::ONE;
            let mut numerator = product(&expected, &vanishing);
            let degree = numerator.len();
            numerator.resize(degree + 7, F::ZERO);
            let original = numerator.clone();
            let plan = plan(n, numerator.len(), degree, k + 9);
            let result = plan.divide(&numerator).unwrap();
            assert_eq!(result.degree_bound(), k);
            assert_eq!(plan.quotient_degree_bound(), k);
            assert_eq!(plan.quotient_extent(), k + 9);
            assert_eq!(&result.coefficients()[..k], &expected);
            assert!(result.coefficients()[k..].iter().all(|&c| c == F::ZERO));
            for x in [F::ZERO, F::ONE, f(2), F::new([0, 0, 0, 1]).unwrap()] {
                assert_eq!(
                    horner(&numerator, x),
                    x.power(n as u64)
                        .sub(F::ONE)
                        .mul(horner(result.coefficients(), x))
                );
            }
            assert_eq!(numerator, original);
            numerator[n - 1] = numerator[n - 1].add(F::ONE);
            assert!(plan.divide(&numerator).is_err());
        }
    }
}

#[test]
fn subgroup_interpolant_cannot_replace_full_numerator_or_hide_nonzero_remainder() {
    let n = 4;
    let mut numerator = vec![F::ZERO; 13];
    numerator[0] = F::ZERO.sub(f(7));
    numerator[12] = f(7);
    // (X^12-1)/(X^4-1) has three nonzero blocks. A forward pass or
    // reduction to the all-zero subgroup interpolant loses two high terms.
    let q = plan(n, 13, 13, 12).divide(&numerator).unwrap();
    for degree in [0, 4, 8] {
        assert_eq!(q.coefficients()[degree], f(7));
    }
    assert_ne!(q.coefficients(), &[F::ZERO; 12]);
    assert!(plan(n, 13, 12, 12).divide(&numerator).is_err());
    assert!(
        plan(4, 4, 4, 0)
            .divide(&[F::ZERO, F::ONE, F::ZERO, F::ZERO])
            .is_err()
    );
    let zero = plan(4, 4, 0, 0).divide(&[F::ZERO; 4]).unwrap();
    assert!(zero.coefficients().is_empty());
    assert_eq!(zero.degree_bound(), 0);
}

#[test]
fn all_coordinates_shapes_and_checked_bounds_are_enforced() {
    for coordinate in 0..4 {
        let mut words = [0; 4];
        words[coordinate] = GOLDILOCKS_MODULUS;
        let mut input = [F::ZERO; 8];
        input[7] = F::from_coefficients_unchecked_for_test(words);
        assert!(plan(4, 8, 8, 4).divide(&input).is_err());
    }
    assert!(plan(4, 8, 8, 4).divide(&[F::ZERO; 7]).is_err());
    for args in [
        (0, 8, 8, 8),
        (9, 8, 8, 8),
        (4, 8, 9, 9),
        (4, 8, 8, 3),
        (1, usize::MAX, 1, 0),
    ] {
        assert!(
            VanishingDivisionPlan::new(args.0, args.1, args.2, args.3, usize::MAX, usize::MAX)
                .is_err()
        );
    }
    assert!(
        VanishingDivisionPlan::new(
            1,
            isize::MAX as usize / F::BYTES,
            1,
            0,
            usize::MAX,
            usize::MAX
        )
        .is_err()
    );
}

#[test]
fn large_quotient_padding_is_charged_for_validation_and_erasure() {
    let q_extent = 100_003;
    let exact = plan(1, 2, 2, q_extent);
    assert_eq!(exact.payload_bytes(), (4 + q_extent) * F::BYTES);
    assert_eq!(exact.work_units(), 22 + 10 * q_extent + 5 + 1);
    assert!(
        VanishingDivisionPlan::new(
            1,
            2,
            2,
            q_extent,
            exact.payload_bytes() - 1,
            exact.work_units()
        )
        .is_err()
    );
    assert!(
        VanishingDivisionPlan::new(
            1,
            2,
            2,
            q_extent,
            exact.payload_bytes(),
            exact.work_units() - 1
        )
        .is_err()
    );
    let accepted =
        VanishingDivisionPlan::new(1, 2, 2, q_extent, exact.payload_bytes(), exact.work_units())
            .unwrap();
    let result = accepted.divide(&[F::ZERO.sub(f(3)), f(3)]).unwrap();
    assert_eq!(result.coefficients()[0], f(3));
    assert!(
        result.coefficients()[1..]
            .iter()
            .all(|&value| value == F::ZERO)
    );
}
