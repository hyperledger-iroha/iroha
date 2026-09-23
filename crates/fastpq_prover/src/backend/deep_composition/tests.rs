//! Independent coefficient checks for the fixed DEEP composition algebra.

use super::*;
use crate::backend::{GOLDILOCKS_MODULUS, fixed_domain::FixedTraceDomain};
use fastpq_isi::FASTPQ_FINAL_V1;

fn root() -> u64 {
    FixedTraceDomain::new(&FASTPQ_FINAL_V1, TRACE_ROWS as usize)
        .unwrap()
        .generator
}

fn dense(seed: u64) -> F {
    F::new([seed + 1, 2 * seed + 3, 3 * seed + 5, 5 * seed + 7]).unwrap()
}

fn bad(lane: usize) -> F {
    let mut words = [0; 4];
    words[lane] = GOLDILOCKS_MODULUS;
    F::from_coefficients_unchecked_for_test(words)
}

fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, &coefficient| {
            value.mul(point).add(coefficient)
        })
}

// Synthetic division constructs coefficients of (P(X)-P(z))/(X-z).
// It uses no inverse, interpolation formula, or production DEEP evaluator.
fn divide_at(coefficients: &[F], point: F) -> Vec<F> {
    if coefficients.len() < 2 {
        return Vec::new();
    }
    let mut result = vec![F::ZERO; coefficients.len() - 1];
    let mut carry = *coefficients.last().unwrap();
    for index in (1..coefficients.len()).rev() {
        result[index - 1] = carry;
        carry = coefficients[index - 1].add(point.mul(carry));
    }
    assert_eq!(carry, horner(coefficients, point));
    result
}

fn shifted(coefficients: &[F], shift: usize) -> Vec<F> {
    let mut result = vec![F::ZERO; shift];
    result.extend_from_slice(coefficients);
    result
}

fn coefficient_components(trace: &[Vec<F>], quotient: &[Vec<F>; 2], points: [F; 2]) -> Vec<Vec<F>> {
    let mut components = Vec::with_capacity(COMPONENTS);
    for polynomial in trace {
        let first = divide_at(polynomial, points[0]);
        let h = divide_at(&first, points[1]);
        components.push(h.clone());
        components.push(shifted(&h, 2));
    }
    for polynomial in quotient {
        let t = divide_at(polynomial, points[0]);
        components.push(t.clone());
        components.push(shifted(&t, 1));
    }
    assert_eq!(components.len(), COMPONENTS);
    components
}

fn coefficient_batch(components: &[Vec<F>], lambda: F) -> Vec<F> {
    let width = components.iter().map(Vec::len).max().unwrap();
    let mut coefficients = vec![F::ZERO; width];
    for (index, component) in components.iter().enumerate() {
        // Deliberately use independent exponentiation, not an incremented power
        // shared with the production component traversal.
        let weight = lambda.power(index as u64);
        for (destination, &value) in coefficients.iter_mut().zip(component) {
            *destination = destination.add(weight.mul(value));
        }
    }
    coefficients
}

fn prepared(points: OodPair, trace: &[Vec<F>], quotient: &[Vec<F>; 2]) -> DeepComposition {
    let [z, next_z] = points.points();
    DeepComposition::new(
        points,
        &trace.iter().map(|p| horner(p, z)).collect::<Vec<_>>(),
        &trace.iter().map(|p| horner(p, next_z)).collect::<Vec<_>>(),
        &quotient.each_ref().map(|p| horner(p, z)),
    )
    .unwrap()
}

fn fixture(base_trace: bool) -> (Vec<Vec<F>>, [Vec<F>; 2]) {
    let trace = (0..TRACE_COLUMNS)
        .map(|column| {
            (0..4 + column % 5)
                .map(|degree| {
                    let value = 17 * column as u64 + 31 * degree as u64 + 1;
                    if base_trace {
                        F::from_base(value).unwrap()
                    } else {
                        dense(value)
                    }
                })
                .collect()
        })
        .collect();
    let quotient = [
        (0..6).map(|i| dense(101 + i)).collect(),
        (0..7).map(|i| dense(211 + 3 * i)).collect(),
    ];
    (trace, quotient)
}

fn assert_shape<T>(result: Result<T>) {
    assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
}

fn assert_noncanonical<T>(result: Result<T>, context: &str, indices: &[usize]) {
    assert!(matches!(result,
        Err(Error::NonCanonicalGoldilocksElement { context: found, indices: found_indices })
            if found == context && found_indices == indices));
}

#[test]
fn ood_pair_preserves_all_coordinates_and_checks_exact_root_order() {
    for z in [
        F::new([19, 31, 0, 0]).unwrap(),
        F::new([19, 0, 31, 0]).unwrap(),
        F::new([19, 0, 0, 31]).unwrap(),
        dense(13),
    ] {
        let pair = OodPair::new(z, root()).unwrap();
        assert_eq!(pair.points(), [z, z.mul_base(root())]);
        assert_ne!(pair.points()[0], pair.points()[1]);
        assert_eq!(pair.points()[1].sub(z).mul(pair.inverse_span), F::ONE);
    }
    for base in [0, 1, 7, GOLDILOCKS_MODULUS - 1] {
        assert_shape(OodPair::new(F::from_base(base).unwrap(), root()));
    }
    for invalid_root in [0, 1, GOLDILOCKS_MODULUS - 1, root().power(2)] {
        assert_shape(OodPair::new(dense(2), invalid_root));
    }
    assert_noncanonical(
        OodPair::new(dense(2), GOLDILOCKS_MODULUS),
        "deep_trace_root",
        &[],
    );
    for lane in 0..4 {
        assert_noncanonical(OodPair::new(bad(lane), root()), "deep_ood_point", &[lane]);
    }
}

#[test]
fn pair_interpolation_matches_independent_linear_coefficients() {
    let points = OodPair::new(dense(13), root()).unwrap();
    let [z, next_z] = points.points();
    for coefficients in [
        [F::ZERO, F::ZERO],
        [dense(5), F::ZERO],
        [dense(7), dense(11)],
    ] {
        let interpolation = points
            .interpolate(horner(&coefficients, z), horner(&coefficients, next_z))
            .unwrap();
        assert_eq!([interpolation.constant, interpolation.slope], coefficients);
        for x in [F::ZERO, F::ONE, z, next_z, dense(23)] {
            assert_eq!(interpolation.value_at(x).unwrap(), horner(&coefficients, x));
        }
    }
    for lane in 0..4 {
        assert_noncanonical(
            points.interpolate(bad(lane), F::ZERO),
            "deep_ood_interpolation",
            &[0, lane],
        );
        assert_noncanonical(
            points.interpolate(F::ZERO, bad(lane)),
            "deep_ood_interpolation",
            &[1, lane],
        );
        let interpolation = points.interpolate(F::ZERO, F::ZERO).unwrap();
        assert_noncanonical(
            interpolation.value_at(bad(lane)),
            "deep_interpolation_evaluation_point",
            &[lane],
        );
    }
}

#[test]
fn all_606_components_match_coefficient_division_and_power_batching() {
    assert_eq!(COMPONENTS, 606);
    let points = OodPair::new(dense(13), root()).unwrap();
    let (trace, quotient) = fixture(false);
    let owner = prepared(points, &trace, &quotient);
    let components = coefficient_components(&trace, &quotient, points.points());
    for lambda in [F::ZERO, F::ONE, dense(29), F::new([0, 0, 0, 1]).unwrap()] {
        let polynomial = coefficient_batch(&components, lambda);
        for point in [
            F::ZERO,
            F::ONE,
            F::from_base(GOLDILOCKS_MODULUS - 1).unwrap(),
            dense(37),
        ] {
            let opened = trace.iter().map(|p| horner(p, point)).collect::<Vec<_>>();
            let quotient_values = quotient.each_ref().map(|p| horner(p, point));
            assert_eq!(
                owner
                    .value_at(point, &opened, &quotient_values, lambda)
                    .unwrap(),
                horner(&polynomial, point),
            );
        }
    }
}

#[test]
fn base_row_evaluation_matches_extension_and_independent_coefficients() {
    let points = OodPair::new(dense(17), root()).unwrap();
    let (trace, quotient) = fixture(true);
    let owner = prepared(points, &trace, &quotient);
    let lambda = dense(41);
    let polynomial = coefficient_batch(
        &coefficient_components(&trace, &quotient, points.points()),
        lambda,
    );
    for point in [0, 1, 7, 1 << 32, GOLDILOCKS_MODULUS - 1] {
        let extended_point = F::from_base(point).unwrap();
        let extended = trace
            .iter()
            .map(|p| horner(p, extended_point))
            .collect::<Vec<_>>();
        let base = extended
            .iter()
            .map(|value| {
                let [value, b, c, d] = value.coefficients();
                assert_eq!([b, c, d], [0; 3]);
                value
            })
            .collect::<Vec<_>>();
        let q = quotient.each_ref().map(|p| horner(p, extended_point));
        let value = owner.base_value_at(point, &base, &q, lambda).unwrap();
        assert_eq!(
            value,
            owner
                .value_at(extended_point, &extended, &q, lambda)
                .unwrap()
        );
        assert_eq!(value, horner(&polynomial, extended_point));
    }
}

#[test]
fn shifted_components_expose_both_trace_and_quotient_degree_overruns() {
    // Small coefficient vectors test the exact algebraic boundary independently
    // of any FFT/FRI code. This evaluator does not itself prove low degree.
    const DEGREE_BOUND: usize = 8;
    let points = OodPair::new(dense(43), root()).unwrap();
    let lambda = dense(47);
    for source in [0, 1, 2] {
        for degree in [DEGREE_BOUND - 1, DEGREE_BOUND] {
            let mut trace = vec![vec![F::ZERO]; TRACE_COLUMNS];
            let mut quotient = [vec![F::ZERO], vec![F::ZERO]];
            let mut monomial = vec![F::ZERO; degree + 1];
            monomial[degree] = dense(53);
            match source {
                0 => trace[0] = monomial,
                1 => quotient[0] = monomial,
                _ => quotient[1] = monomial,
            }
            let components = coefficient_components(&trace, &quotient, points.points());
            let unshifted = if source == 0 {
                0
            } else {
                2 * TRACE_COLUMNS + 2 * (source - 1)
            };
            assert!(components[unshifted].len() <= DEGREE_BOUND);
            assert_eq!(components[unshifted + 1].len(), degree + 1);
            assert!(!components[unshifted + 1].last().unwrap().is_zero());
            let polynomial = coefficient_batch(&components, lambda);
            let actual_degree = polynomial.iter().rposition(|v| !v.is_zero()).unwrap();
            assert_eq!(actual_degree, degree);
            assert_eq!(actual_degree < DEGREE_BOUND, degree == DEGREE_BOUND - 1);
            let owner = prepared(points, &trace, &quotient);
            for x in [F::from_base(7).unwrap(), dense(59)] {
                let values = trace.iter().map(|p| horner(p, x)).collect::<Vec<_>>();
                let q = quotient.each_ref().map(|p| horner(p, x));
                assert_eq!(
                    owner.value_at(x, &values, &q, lambda).unwrap(),
                    horner(&polynomial, x)
                );
            }
        }
    }
}

#[test]
fn fixed_widths_are_required_at_every_boundary() {
    let points = OodPair::new(dense(3), root()).unwrap();
    let row = vec![F::ZERO; TRACE_COLUMNS];
    let q = [F::ZERO; 2];
    let owner = DeepComposition::new(points, &row, &row, &q).unwrap();
    for width in [0, TRACE_COLUMNS - 1, TRACE_COLUMNS + 1, 342] {
        let malformed = vec![F::ZERO; width];
        assert_shape(DeepComposition::new(points, &malformed, &row, &q));
        assert_shape(DeepComposition::new(points, &row, &malformed, &q));
        assert_shape(owner.value_at(F::ONE, &malformed, &q, F::ONE));
        assert_shape(owner.base_value_at(1, &vec![0; width], &q, F::ONE));
    }
    for count in [0, 1, 3] {
        let malformed = vec![F::ZERO; count];
        assert_shape(DeepComposition::new(points, &row, &row, &malformed));
        assert_shape(owner.value_at(F::ONE, &row, &malformed, F::ONE));
        assert_shape(owner.base_value_at(1, &[0; TRACE_COLUMNS], &malformed, F::ONE));
    }
}

#[test]
fn every_ood_trace_coordinate_and_quotient_coordinate_is_checked() {
    let points = OodPair::new(dense(3), root()).unwrap();
    let zero = vec![F::ZERO; TRACE_COLUMNS];
    for row_index in 0..2 {
        for column in 0..TRACE_COLUMNS {
            for lane in 0..4 {
                let mut changed = zero.clone();
                changed[column] = bad(lane);
                let (current, next) = if row_index == 0 {
                    (&changed, &zero)
                } else {
                    (&zero, &changed)
                };
                assert_noncanonical(
                    DeepComposition::new(points, current, next, &[F::ZERO; 2]),
                    "deep_ood_trace_answers",
                    &[row_index, column, lane],
                );
            }
        }
    }
    for part in 0..2 {
        for lane in 0..4 {
            let mut q = [F::ZERO; 2];
            q[part] = bad(lane);
            assert_noncanonical(
                DeepComposition::new(points, &zero, &zero, &q),
                "deep_ood_quotient_answers",
                &[part, lane],
            );
        }
    }
}

#[test]
fn all_query_inputs_are_checked_even_with_zero_challenge_or_singular_point() {
    let points = OodPair::new(dense(3), root()).unwrap();
    let zero = vec![F::ZERO; TRACE_COLUMNS];
    let q = [F::ZERO; 2];
    let owner = DeepComposition::new(points, &zero, &zero, &q).unwrap();
    for column in 0..TRACE_COLUMNS {
        for lane in 0..4 {
            let mut changed = zero.clone();
            changed[column] = bad(lane);
            assert_noncanonical(
                owner.value_at(points.points()[0], &changed, &q, F::ZERO),
                "deep_trace_opening",
                &[column, lane],
            );
        }
        let mut changed = [0; TRACE_COLUMNS];
        changed[column] = GOLDILOCKS_MODULUS;
        assert_noncanonical(
            owner.base_value_at(0, &changed, &q, F::ZERO),
            "deep_trace_opening",
            &[column],
        );
    }
    for lane in 0..4 {
        assert_noncanonical(
            owner.value_at(bad(lane), &zero, &q, F::ONE),
            "deep_evaluation_point",
            &[lane],
        );
        assert_noncanonical(
            owner.value_at(F::ONE, &zero, &q, bad(lane)),
            "deep_composition_challenge",
            &[lane],
        );
        assert_noncanonical(
            owner.base_value_at(1, &[0; TRACE_COLUMNS], &q, bad(lane)),
            "deep_composition_challenge",
            &[lane],
        );
        for part in 0..2 {
            let mut changed = q;
            changed[part] = bad(lane);
            assert_noncanonical(
                owner.value_at(F::ONE, &zero, &changed, F::ZERO),
                "deep_quotient_opening",
                &[part, lane],
            );
            assert_noncanonical(
                owner.base_value_at(1, &[0; TRACE_COLUMNS], &changed, F::ZERO),
                "deep_quotient_opening",
                &[part, lane],
            );
        }
    }
    assert_noncanonical(
        owner.base_value_at(GOLDILOCKS_MODULUS, &[0; TRACE_COLUMNS], &q, F::ZERO),
        "deep_evaluation_point",
        &[],
    );
    for point in points.points() {
        assert_shape(owner.value_at(point, &zero, &q, F::ZERO));
    }
    assert_eq!(
        owner
            .base_value_at(0, &[0; TRACE_COLUMNS], &q, F::ZERO)
            .unwrap(),
        F::ZERO
    );
}
