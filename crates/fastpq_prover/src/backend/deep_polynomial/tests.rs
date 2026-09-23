//! Independent polynomial division and fixed-N checks for the honest producer.

use super::*;
use crate::backend::{GOLDILOCKS_MODULUS, fixed_domain::FixedTraceDomain};
use fastpq_isi::FASTPQ_FINAL_V1;

fn dense(seed: u64) -> F {
    F::new([seed + 1, 2 * seed + 3, 3 * seed + 5, 5 * seed + 7]).unwrap()
}

fn points() -> OodPair {
    let root = FixedTraceDomain::new(&FASTPQ_FINAL_V1, DEGREE_BOUND)
        .unwrap()
        .generator;
    OodPair::new(dense(11), root).unwrap()
}

fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul(point).add(coefficient)
        })
}

fn base_horner(coefficients: &[u64], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul(point).add(F::embed_base(coefficient))
        })
}

// Independent monic polynomial long division, operating on complete vectors.
// It does not call the producer's in-place linear division or OodPair interpolation.
fn long_division(mut numerator: Vec<F>, divisor: &[F]) -> Vec<F> {
    assert_eq!(divisor.last(), Some(&F::ONE));
    if numerator.len() < divisor.len() {
        assert!(numerator.iter().all(|value| value.is_zero()));
        return Vec::new();
    }
    let mut quotient = vec![F::ZERO; numerator.len() + 1 - divisor.len()];
    for degree in (divisor.len() - 1..numerator.len()).rev() {
        let shift = degree + 1 - divisor.len();
        let coefficient = numerator[degree];
        quotient[shift] = coefficient;
        for (index, &value) in divisor.iter().enumerate() {
            numerator[shift + index] = numerator[shift + index].sub(coefficient.mul(value));
        }
    }
    assert!(numerator.iter().all(|value| value.is_zero()));
    quotient
}

fn oracle(trace: &[&[u64]], quotient: [&[F]; 2], pair: OodPair, lambda: F) -> Vec<F> {
    let [z, next] = pair.points();
    let divisor = [z.mul(next), F::ZERO.sub(z.add(next)), F::ONE];
    let mut result = vec![F::ZERO; 16];
    for (column, polynomial) in trace.iter().enumerate() {
        let mut numerator = polynomial
            .iter()
            .map(|&value| F::embed_base(value))
            .collect::<Vec<_>>();
        numerator.resize(numerator.len().max(2), F::ZERO);
        let first = base_horner(polynomial, z);
        let second = base_horner(polynomial, next);
        let slope = second.sub(first).mul(next.sub(z).inverse().unwrap());
        numerator[0] = numerator[0].sub(first.sub(z.mul(slope)));
        numerator[1] = numerator[1].sub(slope);
        let h = long_division(numerator, &divisor);
        for (degree, coefficient) in h.into_iter().enumerate() {
            result[degree] = result[degree].add(lambda.power((2 * column) as u64).mul(coefficient));
            result[degree + 2] =
                result[degree + 2].add(lambda.power((2 * column + 1) as u64).mul(coefficient));
        }
    }
    for (part, polynomial) in quotient.into_iter().enumerate() {
        let mut numerator = polynomial.to_vec();
        numerator.resize(numerator.len().max(1), F::ZERO);
        numerator[0] = numerator[0].sub(horner(polynomial, z));
        let t = long_division(numerator, &[F::ZERO.sub(z), F::ONE]);
        for (degree, coefficient) in t.into_iter().enumerate() {
            let index = 2 * COMMITTED_COLUMN_COUNT + 2 * part;
            result[degree] = result[degree].add(lambda.power(index as u64).mul(coefficient));
            result[degree + 1] =
                result[degree + 1].add(lambda.power((index + 1) as u64).mul(coefficient));
        }
    }
    result
}

fn assert_shape<T>(result: Result<T>) {
    assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
}

fn assert_noncanonical<T>(result: Result<T>, expected: &str, indices: &[usize]) {
    assert!(
        matches!(result, Err(Error::NonCanonicalGoldilocksElement {context, indices: found})
        if context == expected && found == indices)
    );
}

#[test]
fn quotient_splitting_is_exact_borrowed_and_never_reduces_or_truncates() {
    let trace = [&[][..]; COMMITTED_COLUMN_COUNT];
    for length in [
        0,
        1,
        DEGREE_BOUND - 1,
        DEGREE_BOUND,
        DEGREE_BOUND + 1,
        2 * DEGREE_BOUND,
    ] {
        let quotient = (0..length)
            .map(|index| dense(index as u64))
            .collect::<Vec<_>>();
        let source = DeepPolynomialSource::new(&trace, &quotient).unwrap();
        let [low, high] = source.quotient_halves();
        let split = length.min(DEGREE_BOUND);
        assert_eq!(low, &quotient[..split]);
        assert_eq!(high, &quotient[split..]);
        assert_eq!(low.as_ptr(), quotient.as_ptr());
        assert_eq!(low.len() + high.len(), length);
        assert!(low.len() <= DEGREE_BOUND && high.len() <= DEGREE_BOUND);
    }
    assert_shape(DeepPolynomialSource::new(
        &trace,
        &vec![F::ZERO; 2 * DEGREE_BOUND + 1],
    ));
}

#[test]
fn combined_coefficients_match_independent_long_division_and_bounded_evaluator() {
    let owned = (0..COMMITTED_COLUMN_COUNT)
        .map(|column| {
            (0..column % 9)
                .map(|degree| (31 * column + 17 * degree + 1) as u64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let trace = owned.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let quotient = (0..7).map(|index| dense(101 + index)).collect::<Vec<_>>();
    let source = DeepPolynomialSource::new(&trace, &quotient).unwrap();
    let pair = points();
    let prepared = source.prepare(pair);
    for (row, point) in pair.points().into_iter().enumerate() {
        for (column, source) in trace.iter().enumerate() {
            assert_eq!(
                prepared.trace_answers()[row][column],
                base_horner(source, point)
            );
        }
    }
    assert_eq!(
        *prepared.quotient_answers(),
        [horner(&quotient, pair.points()[0]), F::ZERO]
    );
    let evaluator = prepared.evaluator().unwrap();
    for lambda in [F::ZERO, F::ONE, dense(19), F::new([0, 0, 0, 1]).unwrap()] {
        let polynomial = prepared.compose(lambda, WORKSPACE_BYTES).unwrap();
        let expected = oracle(&trace, source.quotient_halves(), pair, lambda);
        assert_eq!(polynomial.coefficients().len(), DEGREE_BOUND);
        assert_eq!(&polynomial.coefficients()[..expected.len()], expected);
        assert!(
            polynomial.coefficients()[expected.len()..]
                .iter()
                .all(|value| value.is_zero())
        );
        for x in [
            F::ZERO,
            F::ONE,
            dense(23),
            F::from_base(GOLDILOCKS_MODULUS - 1).unwrap(),
        ] {
            let row = trace
                .iter()
                .map(|source| base_horner(source, x))
                .collect::<Vec<_>>();
            let q = source.quotient_halves().map(|half| horner(half, x));
            assert_eq!(
                horner(polynomial.coefficients(), x),
                evaluator.value_at(x, &row, &q, lambda).unwrap()
            );
        }
    }
}

#[test]
fn actual_n_trace_and_both_quotient_boundaries_preserve_degree_and_full_extension() {
    assert_eq!(DEGREE_BOUND, 65_536);
    assert_eq!(COMMITTED_COLUMN_COUNT, 301);
    assert_eq!(WORKSPACE_BYTES, 4_194_304);
    let mut column = vec![0; DEGREE_BOUND];
    column[0] = 71;
    column[DEGREE_BOUND - 1] = 73;
    let mut trace = [&[][..]; COMMITTED_COLUMN_COUNT];
    trace[COMMITTED_COLUMN_COUNT - 1] = &column;
    let mut quotient = vec![F::ZERO; 2 * DEGREE_BOUND];
    quotient[0] = dense(29);
    quotient[DEGREE_BOUND - 1] = dense(31);
    quotient[DEGREE_BOUND] = dense(37);
    quotient[2 * DEGREE_BOUND - 1] = dense(41);
    let source = DeepPolynomialSource::new(&trace, &quotient).unwrap();
    let pair = points();
    let prepared = source.prepare(pair);
    let lambda = dense(43);
    let result = prepared.compose(lambda, WORKSPACE_BYTES).unwrap();
    let degree = (DEGREE_BOUND - 1) as u64;
    for (row, point) in pair.points().into_iter().enumerate() {
        assert_eq!(
            prepared.trace_answers()[row][COMMITTED_COLUMN_COUNT - 1],
            point.power(degree).mul_base(73).add(F::embed_base(71))
        );
    }
    for (part, half) in source.quotient_halves().into_iter().enumerate() {
        assert_eq!(
            prepared.quotient_answers()[part],
            half[0].add(half[DEGREE_BOUND - 1].mul(pair.points()[0].power(degree)))
        );
    }
    let expected_leading = lambda
        .power(601)
        .mul_base(73)
        .add(lambda.power(603).mul(quotient[DEGREE_BOUND - 1]))
        .add(lambda.power(605).mul(quotient[2 * DEGREE_BOUND - 1]));
    assert_ne!(expected_leading, F::ZERO);
    assert_eq!(result.coefficients()[DEGREE_BOUND - 1], expected_leading);
    let evaluator = prepared.evaluator().unwrap();
    for x in [F::from_base(13).unwrap(), dense(47)] {
        let mut row = [F::ZERO; COMMITTED_COLUMN_COUNT];
        row[COMMITTED_COLUMN_COUNT - 1] = x.power(degree).mul_base(73).add(F::embed_base(71));
        let q = source
            .quotient_halves()
            .map(|half| half[0].add(half[DEGREE_BOUND - 1].mul(x.power(degree))));
        assert_eq!(
            horner(result.coefficients(), x),
            evaluator.value_at(x, &row, &q, lambda).unwrap()
        );
        assert_eq!(
            horner(&quotient, x),
            q[0].add(x.power(DEGREE_BOUND as u64).mul(q[1]))
        );
    }
}

#[test]
fn complete_source_shapes_and_coordinates_are_checked_before_preparation() {
    let empty = [&[][..]; COMMITTED_COLUMN_COUNT];
    for width in [
        0,
        COMMITTED_COLUMN_COUNT - 1,
        COMMITTED_COLUMN_COUNT + 1,
        342,
    ] {
        assert_shape(DeepPolynomialSource::new(&vec![&[][..]; width], &[]));
    }
    let oversized = vec![0; DEGREE_BOUND + 1];
    let mut trace = empty;
    trace[COMMITTED_COLUMN_COUNT - 1] = &oversized;
    assert_shape(DeepPolynomialSource::new(&trace, &[]));
    let mut boundary = vec![0; DEGREE_BOUND];
    boundary[DEGREE_BOUND - 1] = GOLDILOCKS_MODULUS;
    trace[COMMITTED_COLUMN_COUNT - 1] = &boundary;
    assert_noncanonical(
        DeepPolynomialSource::new(&trace, &[]),
        "deep_polynomial_trace",
        &[COMMITTED_COLUMN_COUNT - 1, DEGREE_BOUND - 1],
    );
    for column in 0..COMMITTED_COLUMN_COUNT {
        let invalid = [0, GOLDILOCKS_MODULUS];
        let mut trace = empty;
        trace[column] = &invalid;
        assert_noncanonical(
            DeepPolynomialSource::new(&trace, &[]),
            "deep_polynomial_trace",
            &[column, 1],
        );
    }
    for lane in 0..4 {
        let mut words = [0; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        let malformed = F::from_coefficients_unchecked_for_test(words);
        for degree in [0, DEGREE_BOUND - 1, DEGREE_BOUND, 2 * DEGREE_BOUND - 1] {
            let mut quotient = vec![F::ZERO; degree + 1];
            quotient[degree] = malformed;
            assert_noncanonical(
                DeepPolynomialSource::new(&empty, &quotient),
                "deep_polynomial_quotient",
                &[degree, lane],
            );
        }
    }
}

#[test]
fn composition_rejects_noncanonical_challenges_and_insufficient_workspace() {
    let trace = [&[][..]; COMMITTED_COLUMN_COUNT];
    let prepared = DeepPolynomialSource::new(&trace, &[])
        .unwrap()
        .prepare(points());
    for lane in 0..4 {
        let mut words = [0; 4];
        words[lane] = GOLDILOCKS_MODULUS;
        assert_noncanonical(
            prepared.compose(F::from_coefficients_unchecked_for_test(words), 0),
            "deep_polynomial_challenge",
            &[lane],
        );
    }
    assert!(matches!(prepared.compose(dense(3), WORKSPACE_BYTES - 1),
        Err(Error::VerifierLimitExceeded { limit: "max_deep_polynomial_workspace_bytes", actual: WORKSPACE_BYTES, max }) if max == WORKSPACE_BYTES - 1));
    let result = prepared.compose(F::ZERO, WORKSPACE_BYTES).unwrap();
    assert!(result.coefficients().iter().all(|value| value.is_zero()));
}

#[test]
fn linear_division_checks_complete_remainders_and_preserves_zero_padding() {
    for point in [F::ZERO, F::ONE, dense(53)] {
        for length in 0..9 {
            let expected = (0..length)
                .map(|degree| dense(59 + degree as u64))
                .collect::<Vec<_>>();
            let mut product = vec![F::ZERO; length + 1];
            for (degree, &coefficient) in expected.iter().enumerate() {
                product[degree] = product[degree].sub(point.mul(coefficient));
                product[degree + 1] = product[degree + 1].add(coefficient);
            }
            let mut changed = product.clone();
            changed[0] = changed[0].add(dense(61));
            assert_shape(divide_linear_exact(&mut changed, point));
            divide_linear_exact(&mut product, point).unwrap();
            assert_eq!(&product[..length], expected);
            assert_eq!(product[length], F::ZERO);
        }
    }
}

#[test]
fn either_trace_remainder_and_quotient_remainder_abort_before_returning_coefficients() {
    let trace = [&[][..]; COMMITTED_COLUMN_COUNT];
    let source = DeepPolynomialSource::new(&trace, &[]).unwrap();
    let lambda = dense(67);
    for row in 0..2 {
        let mut prepared = source.prepare(points());
        prepared.trace_answers[row][0] = dense(71);
        assert_shape(prepared.compose(lambda, WORKSPACE_BYTES));
    }
    for part in 0..2 {
        let mut prepared = source.prepare(points());
        prepared.quotient_answers[part] = dense(73);
        assert_shape(prepared.compose(lambda, WORKSPACE_BYTES));
    }
}
