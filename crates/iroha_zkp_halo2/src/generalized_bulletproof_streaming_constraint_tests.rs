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

fn materialized_aggregates(
    padded_gates: usize,
    rows: &[LinComb<TrackingScalar>],
    z_one: TrackingScalar,
) -> (
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    ScalarVector<TrackingScalar>,
    TrackingScalar,
) {
    let mut l = ScalarVector::zero(padded_gates);
    let mut r = ScalarVector::zero(padded_gates);
    let mut o = ScalarVector::zero(padded_gates);
    let mut cg = ScalarVector::zero(padded_gates);
    let mut v = ScalarVector::zero(0);
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
    (l, r, o, cg, v, constant)
}

#[test]
fn exact_small_constraint_aggregates_match_canonical_materialization() {
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
            let (l, r, o, cg, v, constant) = materialized_aggregates(padded_gates, &rows, z_one);
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
}
