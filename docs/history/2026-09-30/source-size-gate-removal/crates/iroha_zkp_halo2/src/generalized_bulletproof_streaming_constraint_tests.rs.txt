use exact_small_coefficient_source_v1::{
    ExactSmallCoefficientBoundV1 as ExactBound, ExactSmallCoefficientConstraintSourceV1,
    ExactSmallCoefficientVerifierStatementV1,
};

fn materialized_exact_rows(
    coefficient_count: usize,
    bound: ExactBound,
) -> (usize, Vec<LinComb<TrackingScalar>>) {
    let (gates_per_coefficient, constraints_per_coefficient) = match bound {
        ExactBound::One => (2_usize, 5_usize),
        ExactBound::Two => (3, 7),
    };
    let actual_gates = coefficient_count * gates_per_coefficient;
    let padded_gates = actual_gates.next_power_of_two();
    let expected_rows =
        coefficient_count * constraints_per_coefficient + (padded_gates - coefficient_count);
    let boolean_rows = |gate| {
        [
            LinComb::empty()
                .term(TrackingScalar::ONE, Variable::aL(gate))
                .term(-TrackingScalar::ONE, Variable::aR(gate)),
            LinComb::empty()
                .term(TrackingScalar::ONE, Variable::aO(gate))
                .term(-TrackingScalar::ONE, Variable::aL(gate)),
        ]
    };
    let mut rows = Vec::with_capacity(expected_rows);
    for coefficient_index in 0..coefficient_count {
        let first_gate = coefficient_index * gates_per_coefficient;
        rows.extend(boolean_rows(first_gate));
        rows.extend(boolean_rows(first_gate + 1));
        match bound {
            ExactBound::One => rows.push(
                LinComb::empty()
                    .term(TrackingScalar::ONE, Variable::aL(first_gate))
                    .term(-TrackingScalar::ONE, Variable::aL(first_gate + 1))
                    .term(
                        -TrackingScalar::ONE,
                        Variable::CG {
                            commitment: 0,
                            index: coefficient_index,
                        },
                    ),
            ),
            ExactBound::Two => {
                rows.extend(boolean_rows(first_gate + 2));
                rows.push(
                    LinComb::empty()
                        .term(TrackingScalar::ONE, Variable::aL(first_gate))
                        .term(TrackingScalar::ONE, Variable::aL(first_gate + 1))
                        .term(-TrackingScalar::from_u64(2), Variable::aL(first_gate + 2))
                        .term(
                            -TrackingScalar::ONE,
                            Variable::CG {
                                commitment: 0,
                                index: coefficient_index,
                            },
                        ),
                );
            }
        }
    }
    for padded_index in coefficient_count..padded_gates {
        rows.push(LinComb::empty().term(
            TrackingScalar::ONE,
            Variable::CG {
                commitment: 0,
                index: padded_index,
            },
        ));
    }
    assert_eq!(rows.len(), expected_rows);
    (padded_gates, rows)
}

type MaterializedAggregates = (
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    TrackingScalar,
);

fn materialized_aggregates(
    padded_gates: usize,
    rows: &[LinComb<TrackingScalar>],
    z_one: TrackingScalar,
) -> Result<MaterializedAggregates, GeneralizedBulletproofErrorV1> {
    let mut l = ScalarVector::try_zero_exact_v1(padded_gates)?;
    let mut r = ScalarVector::try_zero_exact_v1(padded_gates)?;
    let mut o = ScalarVector::try_zero_exact_v1(padded_gates)?;
    let mut cg = ScalarVector::try_zero_exact_v1(padded_gates)?;
    let mut v = ScalarVector::try_zero_exact_v1(0)?;
    let mut constant = TrackingScalar::ZERO;
    let mut z = z_one;
    for row in rows {
        accumulate(&mut l, &row.wl, z);
        accumulate(&mut r, &row.wr, z);
        accumulate(&mut o, &row.wo, z);
        if let Some(weights) = row.wcg.first() {
            accumulate(&mut cg, weights, z);
        }
        accumulate(&mut v, &row.wv, -z);
        constant += row.c * z;
        z *= z_one;
    }
    Ok((l, r, o, cg, v, constant))
}

#[test]
fn exact_small_constraint_aggregates_match_canonical_materialization() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    for (bound, coefficient_count) in [
        (ExactBound::One, 1),
        (ExactBound::One, 3),
        (ExactBound::Two, 1),
        (ExactBound::Two, 3),
    ] {
        for z_one in [TrackingScalar::ONE, TrackingScalar(3)] {
            let source = ExactSmallCoefficientConstraintSourceV1::new(coefficient_count, bound)
                .expect("valid exact source");
            let exact = source.aggregate(z_one).expect("exact aggregates");
            let (padded_gates, rows) = materialized_exact_rows(coefficient_count, bound);
            let (l, r, o, cg, v, constant) = materialized_aggregates(padded_gates, &rows, z_one)
                .expect("materialized aggregate");
            assert!(exact.l_weights == l);
            assert!(exact.r_weights == r);
            assert!(exact.o_weights == o);
            assert!(exact.vector_commitment_weights == cg);
            assert!(exact.scalar_commitment_weights == v);
            assert_eq!(*exact.constraint_product.expose_ref(), constant);
        }
    }
}

#[test]
fn exact_small_constraint_source_closes_shape_and_release_row_counts() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    assert_eq!(
        ExactSmallCoefficientConstraintSourceV1::new(0, ExactBound::One),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(
        ExactSmallCoefficientConstraintSourceV1::new(usize::MAX, ExactBound::Two),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!(
        ExactSmallCoefficientConstraintSourceV1::new(16_384, ExactBound::One)
            .expect("release bound-one source")
            .test_shape(),
        (32_768, 98_304)
    );
    assert_eq!(
        ExactSmallCoefficientConstraintSourceV1::new(16_384, ExactBound::Two)
            .expect("release bound-two source")
            .test_shape(),
        (65_536, 163_840)
    );
    let wrong_width = ExactSmallCoefficientVerifierStatementV1::new(
        TrackingSuite::generators()
            .reduce(1)
            .expect("one-generator view"),
        ExactSmallCoefficientConstraintSourceV1::new(1, ExactBound::One)
            .expect("two-gate exact source"),
        TrackingPoint(9),
    );
    assert!(matches!(
        wrong_width,
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
}

#[test]
fn exact_small_constraint_source_has_no_public_row_or_callback_escape() {
    let source = include_str!("generalized_bulletproof/exact_small_coefficient_source_v1.rs");
    let main = include_str!("generalized_bulletproof.rs");
    assert!(source.contains("pub(crate) struct ExactSmallCoefficientConstraintSourceV1"));
    let fields = source
        .split_once("pub(crate) struct ExactSmallCoefficientConstraintSourceV1 {")
        .expect("exact source")
        .1
        .split_once('}')
        .expect("exact source fields")
        .0;
    assert!(!fields.contains("pub"));
    for forbidden in [
        "FnOnce",
        "FnMut",
        "callback",
        "dyn ",
        "impl Iterator",
        "IntoIterator",
        "pub use",
        "pub struct ExactSmallCoefficientConstraintSourceV1",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden source escape: {forbidden}"
        );
    }
    assert!(main.contains("enum VerifierConstraintSourceV1"));
    assert!(main.contains("VerifierConstraintSourceV1::Materialized, transcript"));
    assert!(!main.contains("pub enum VerifierConstraintSourceV1"));
    assert!(source.lines().count() <= 500 && source.len() <= 24 * 1024);
    let tests = include_str!("generalized_bulletproof_streaming_constraint_tests.rs");
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
    let allocation = include_str!("generalized_bulletproof_allocation_capacity_tests.rs");
    assert!(allocation.lines().count() <= 500 && allocation.len() <= 24 * 1024);
}

#[test]
fn exact_small_prover_source_is_sealed_and_validates_before_randomness() {
    let source = include_str!("generalized_bulletproof/exact_small_coefficient_source_v1.rs");
    let main = include_str!("generalized_bulletproof.rs");
    assert!(source.contains("pub(crate) struct ExactSmallCoefficientProverStatementV1"));
    assert!(!source.contains("pub struct ExactSmallCoefficientProverStatementV1"));
    assert_eq!(
        source
            .matches("statement.exact_small_coefficient_prover_source = Some(source);")
            .count(),
        1
    );
    let validation = source
        .split_once("pub(super) fn validate_witness<S: ProofSuite>(")
        .expect("closed exact witness validator")
        .1
        .split_once("/// Aggregate every canonical row once")
        .expect("exact witness validator boundary")
        .0;
    let validation_helpers = source
        .split_once("fn validate_zero<F: ProofScalar>(")
        .expect("exact validation helpers")
        .1
        .split_once("/// Validated prover statement")
        .expect("exact validation helper boundary")
        .0;
    for surface in [validation, validation_helpers] {
        for forbidden in [
            "Vec::", "vec![", "reserve", "collect", "FnOnce", "FnMut", "callback", "dyn ",
            "Iterator",
        ] {
            assert!(
                !surface.contains(forbidden),
                "forbidden exact witness validator surface: {forbidden}"
            );
        }
    }
    let prover = main
        .split_once("pub fn prove<R, T>(")
        .expect("public materialized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("public prover boundary")
        .0;
    let exact_validation = prover
        .find("source.validate_witness(&witness)?;")
        .expect("exact witness validation");
    let materialized_validation = prover
        .find("for constraint in &self.constraints {")
        .expect("materialized witness validation");
    let randomness = prover
        .find("let alpha = random_scalar::<S::Scalar, _>(rng)?;")
        .expect("first prover randomness");
    assert!(exact_validation < materialized_validation && materialized_validation < randomness);
    assert!(main.contains("exact_small_coefficient_prover_source: None,"));
    assert!(main.contains("pub fn prove<R, T>("));
}

/// Allocation and source-routing assertions share the parent tracking owners.
mod allocation_capacity {
    include!("generalized_bulletproof_allocation_capacity_tests.rs");
}

#[test]
fn exact_capacity_helper_is_fail_closed_and_routes_existing_reservations() {
    allocation_capacity::assert_exact_capacity_routes_v1();
}
