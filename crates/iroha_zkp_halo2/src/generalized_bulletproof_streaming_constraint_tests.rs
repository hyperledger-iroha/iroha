use exact_small_coefficient_source_v1::{
    ExactSmallCoefficientBoundV1 as ExactBound, ExactSmallCoefficientConstraintSourceV1,
    ExactSmallCoefficientVerifierStatementV1,
};

macro_rules! assert_source_steps {
    ($source:expr; [$($step:expr),+ $(,)?]) => {{
        let source = $source;
        let mut offset = 0;
        $(
            let position = source[offset..]
                .find($step)
                .unwrap_or_else(|| panic!("missing ordered source step {:?}", $step));
            offset += position + $step.len();
        )+
        let _ = offset;
    }};
}

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

#[test]
fn exact_capacity_helper_is_fail_closed_and_routes_existing_reservations() {
    struct ZeroSized;
    assert!(matches!(
        try_exact_capacity_vec_v1::<ZeroSized>(1),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    ));
    assert!(matches!(
        try_exact_capacity_vec_v1::<TrackingScalar>(usize::MAX),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    ));
    let values = try_exact_capacity_vec_v1::<TrackingScalar>(3)
        .expect("tracking allocation reports exact capacity");
    assert!(values.is_empty());
    assert_eq!(values.capacity(), 3);
    let zeros = ScalarVector::<TrackingScalar>::try_zero_exact_v1(3).expect("exact zeros");
    let powers = ScalarVector::try_powers_exact_v1(TrackingScalar(3), 3).expect("exact powers");
    assert_eq!((zeros.len(), zeros.0.capacity()), (3, 3));
    assert_eq!(ScalarVector::<TrackingScalar>::zero(2).0.capacity(), 2);
    let expected_powers: &[TrackingScalar] =
        &[TrackingScalar(1), TrackingScalar(3), TrackingScalar(9)];
    assert_eq!(
        (powers.0.as_slice(), powers.0.capacity()),
        (expected_powers, 3)
    );
    assert!(matches!(
        ScalarVector::<TrackingScalar>::try_powers_exact_v1(TrackingScalar(2), 0),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    let batch = BatchVerifier::<TrackingSuite>::new(1, 2).expect("exact tracking batch");
    assert_eq!((batch.g_bold.capacity(), batch.h_bold.capacity()), (1, 1));
    assert_eq!(
        (batch.h_sum.capacity(), batch.additional.capacity()),
        (1, 2)
    );
    let folded = try_collect_public_point_fold_v1::<TrackingSuite, _>(
        &[
            TrackingPoint(1),
            TrackingPoint(2),
            TrackingPoint(3),
            TrackingPoint(4),
        ],
        |index, left, right| TrackingPoint(left.0 + right.0 + index as u64),
    )
    .expect("exact ordered point fold");
    assert_eq!(folded, [TrackingPoint(4), TrackingPoint(7)]);
    assert_eq!(folded.capacity(), 2);
    let (singleton, odd) = ([TrackingPoint(1)], [TrackingPoint(1); 3]);
    for malformed in [&singleton[..], &odd[..]] {
        assert_eq!(
            try_collect_public_point_fold_v1::<TrackingSuite, _>(malformed, |_, left, _| left),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        );
    }

    let main = include_str!("generalized_bulletproof.rs");
    let helper = main
        .split_once("/// Allocate an empty vector whose reported capacity")
        .expect("exact-capacity helper")
        .1
        .split_once("/// Fallible cryptographic byte source")
        .expect("exact-capacity helper boundary")
        .0;
    assert_source_steps!(helper; [
        "core::mem::size_of::<T>()",
        "exact_capacity.checked_mul(element_bytes)",
        ".try_reserve_exact(exact_capacity)",
        "if values.capacity() != exact_capacity",
        "return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);",
        "fn try_exact_filled_vec_v1<T: Copy>(",
        "try_exact_capacity_vec_v1(exact_capacity)?",
        "values.push(value);",
    ]);
    assert_eq!(main.matches(".try_reserve_exact(").count(), 1);
    assert_eq!(main.matches("try_exact_capacity_vec_v1(").count(), 21);
    assert_eq!(main.matches("try_exact_filled_vec_v1(").count(), 8);
    assert_eq!(main.matches("ScalarVector::try_zero_exact_v1(").count(), 14);
    assert_eq!(
        main.matches("ScalarVector::try_powers_exact_v1(").count(),
        6
    );
    let exact = include_str!("generalized_bulletproof/exact_small_coefficient_source_v1.rs");
    assert_eq!(exact.matches("ScalarVector::try_zero_exact_v1(").count(), 5);
    assert_eq!(
        exact
            .matches("let mut vector_commitments = try_exact_capacity_vec_v1(1)?;")
            .count(),
        2
    );
    assert!(!exact.contains("vec![vector_commitment]"));
    let running = exact
        .split_once("impl<F: ProofScalar> RunningExactSmallCoefficientAggregateV1<F> {")
        .unwrap()
        .1
        .split_once("fn add_l(")
        .unwrap()
        .0;
    assert_source_steps!(running; ["fn new(", "Result<Self, GeneralizedBulletproofErrorV1>", "Ok(Self {", "ScalarVector::try_zero_exact_v1(padded_gates)?", "ScalarVector::try_zero_exact_v1(0)?"]);
    assert!(
        exact.contains("RunningExactSmallCoefficientAggregateV1::new(self.padded_gates, z_one)?")
    );
    let scalar = main
        .split_once("impl<F: ProofScalar> ScalarVector<F> {")
        .unwrap()
        .1
        .split_once("fn random_scalar_vector")
        .unwrap()
        .0;
    assert_source_steps!(scalar; ["pub fn zero(", "Self::try_zero_exact_v1(len).expect", "fn try_zero_exact_v1(", "try_exact_filled_vec_v1(len, F::ZERO)?", "pub fn powers(", "Self::try_powers_exact_v1(value, len)", "fn try_powers_exact_v1(", "if len == 0", "Self(try_exact_capacity_vec_v1(len)?)", "result.0.push(F::ONE)", "Ok(result)"]);
    assert!(!scalar.contains("Vec::with_capacity") && !scalar.contains("vec!["));
    assert!(!main.contains("allocation_capacity <"));
    let polynomial_owners = main
        .split_once("let polynomial_count = is")
        .unwrap()
        .1
        .split_once("let (l_weights, r_weights")
        .unwrap()
        .0;
    assert_source_steps!(polynomial_owners; [
        ".checked_add(1)",
        "let mut l = try_exact_capacity_vec_v1(polynomial_count)?;",
        "let mut r = try_exact_capacity_vec_v1(polynomial_count)?;",
        "let l_allocation = l.as_ptr();",
        "let r_allocation = r.as_ptr();",
        "for _ in 0..polynomial_count",
        "l.push(ScalarVector(Vec::new()));",
        "r.push(ScalarVector(Vec::new()));",
        "l.capacity() != polynomial_count",
        "r.capacity() != polynomial_count",
        "return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);",
    ]);
    assert!(!polynomial_owners.contains("vec!["));
    assert_eq!(
        main.matches("let mut cg_weights = try_exact_capacity_vec_v1(1)?;")
            .count(),
        2
    );
    let exact_cg_branches = main
        .split("let cg_weights = if let Some(weights) = exact_cg_weights {")
        .skip(1)
        .map(|branch| branch.split_once("} else {").unwrap().0)
        .collect::<Vec<_>>();
    assert_eq!(exact_cg_branches.len(), 2);
    for branch in exact_cg_branches {
        assert_source_steps!(branch; [
            "let mut cg_weights = try_exact_capacity_vec_v1(1)?;",
            "cg_weights.push(weights);",
            "cg_weights",
        ]);
        assert!(!branch.contains("vec!["));
    }
    assert_eq!(main.matches("cg_weights.push(weights);").count(), 4);
    assert!(!main.contains("vec![weights]"));
    assert!(helper.contains("successful paths only"));
    assert!(helper.contains("transient\n/// over-grants rejected here remain outside"));
    let point_fold = main
        .split_once("fn try_collect_public_point_fold_v1<S, F>(")
        .unwrap()
        .1
        .split_once("/// Fallible cryptographic byte source")
        .unwrap()
        .0;
    assert_source_steps!(point_fold; ["source.len() <= 1", "source.split_at(half)", "try_exact_capacity_vec_v1(half)?", "let allocation = result.as_ptr();", ".into_par_iter()", ".collect_into_vec(&mut result);", "for index in 0..half", "if result.len() != half", "result.capacity() != half || result.as_ptr() != allocation", "fn try_collect_public_point_fold_pair_v1", "rayon::join(g_fold, h_fold)", "Ok((g_fold()?, h_fold()?))"]);
    let batch = main
        .split_once("struct BatchVerifier")
        .unwrap()
        .1
        .split_once("/// Owned scalar vector")
        .unwrap()
        .0;
    assert_source_steps!(batch; ["fn new(", "S::generators().h_sum.len()", "additional_capacity", "fn verify(self)", "let exact_terms", "try_exact_capacity_vec_v1(exact_terms)?", "try_exact_multiexp_v1::<S>(&terms)?"]);
    assert!(!batch.contains("ensure_len") && !batch.contains("push_additional"));
    assert!(main.contains("BatchVerifier::<S>::new(0, polynomial_additional_capacity)?"));
    assert!(main.contains("BatchVerifier::<S>::new(n, ipa_additional_capacity)?"));
    assert_eq!(main.matches("BatchVerifier::<S>::new(").count(), 2);
    assert!(main.contains("buckets.fill(identity);") && main.contains("buckets.iter().copied()"));

    let cpk = include_str!("vega/zk_ams/mkhe/cpk_ceremony.rs");
    let resource = include_str!("vega/zk_ams/mkhe/resource.rs");
    assert!(cpk.contains("state_owned_secret_membership_prover_workspace_enumerated: false"));
    assert!(resource.contains("release_peak_memory_measured: false"));
}
