// Exact-capacity and source-routing assertions for the documented allocation_capacity
// test child. Included from its inline module so all source paths stay rooted in src.
use super::*;

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

pub(super) fn assert_exact_capacity_routes_v1() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
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
    assert_eq!(main.matches("ScalarVector::try_zero_exact_v1(").count(), 13);
    // Both polynomial sides share one checked allocator body after rehoming.
    let completion = main
        .split_once("fn fill_unassigned_polynomial_coefficients_v1<F: ProofScalar>(")
        .expect("shared polynomial completion owner")
        .1
        .split_once("/// Sample a secret vector incrementally")
        .expect("shared polynomial completion boundary")
        .0;
    assert_eq!(
        completion
            .matches("ScalarVector::try_zero_exact_v1(n)?")
            .count(),
        1
    );
    assert_source_steps!(completion; [
        "if n == 0",
        "return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);",
        "for coefficient in coefficients",
        "if coefficient.is_empty()",
        "*coefficient = ScalarVector::try_zero_exact_v1(n)?;",
    ]);
    let prover = main
        .split_once("pub fn prove<R, T>(")
        .expect("public materialized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("public prover boundary")
        .0;
    assert_source_steps!(prover; [
        "l[index] = opening.take_values();",
        "r[reverse] = weights;",
        "fill_unassigned_polynomial_coefficients_v1(&mut l, n)?;",
        "fill_unassigned_polynomial_coefficients_v1(&mut r, n)?;",
        "let mut t = ScalarVector::try_zero_exact_v1(t_poly_len)?;",
    ]);
    for side in ["l", "r"] {
        let call = format!("fill_unassigned_polynomial_coefficients_v1(&mut {side}, n)?;");
        assert_eq!(prover.matches(call.as_str()).count(), 1);
    }
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
