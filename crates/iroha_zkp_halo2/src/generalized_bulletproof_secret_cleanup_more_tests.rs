#[test]
fn inner_product_owner_source_boundary_covers_every_production_caller() {
    let source = include_str!("generalized_bulletproof.rs");
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let inner_product = production
        .split_once("pub fn inner_product<'a>(")
        .expect("inner product function")
        .1
        .split_once("fn pad_with_zeroes")
        .expect("inner product boundary")
        .0;
    assert!(inner_product.contains(") -> SecretScalar<F>"));
    assert!(inner_product.contains("vector: impl Iterator<Item = &'a F>"));
    assert!(inner_product.contains("let mut result = SecretScalar::new(F::ZERO);"));
    assert!(inner_product.contains("for (left, right) in self.0.iter().zip(vector)"));
    assert!(inner_product.contains("*result.expose_mut() += *left * *right;"));
    assert!(inner_product.contains("\n        result\n"));
    assert!(!inner_product.contains("result.expose_copy()"));

    assert_eq!(production.matches(".inner_product(").count(), 9);
    assert!(production.contains("let product = left.inner_product(right.0.iter());"));
    assert!(production.contains("t[left_index + right_index] += *product.expose_ref();"));
    assert!(production.contains("drop(product);"));
    assert!(production.contains("let t_caret = l_eval.inner_product(r_eval.0.iter());"));
    assert!(production.contains("transcript.push_scalar(t_caret.expose_ref())?;"));
    assert!(production.contains("ip_x * *t_caret.expose_ref()"));
    assert!(!production.contains("t_caret.expose_copy()"));
    assert!(production.contains("let delta = r_weights.inner_product(l_weights.0.iter());"));
    assert!(production.contains("let constraint_product ="));
    assert!(
        production
            .contains("z.inner_product(self.constraints.iter().map(|constraint| &constraint.c))")
    );
    assert!(production.contains("drop((delta, constraint_product));"));
    assert!(production.contains("let opening_product = a.inner_product(b.0.iter());"));
    assert!(production.contains("*opening_product.expose_ref() * u_scalar"));
    assert!(production.contains("drop(opening_product);"));
    assert_eq!(
        production
            .matches("let c_left = a_left.inner_product(b_right.0.iter());")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("let c_right = a_right.inner_product(b_left.0.iter());")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("*c_left.expose_ref() * u_scalar")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("*c_right.expose_ref() * u_scalar")
            .count(),
        2
    );
    assert_eq!(production.matches("drop(c_left);").count(), 2);
    assert_eq!(production.matches("drop(c_right);").count(), 2);
    assert_eq!(production.matches("drop(t_caret);").count(), 1);
    assert!(!production.contains("SecretScalar::new(l_eval.inner_product"));
    assert!(!production.contains("SecretScalar::new(a.inner_product"));
    assert!(!production.contains("SecretScalar::new(a_left.inner_product"));
    assert!(!production.contains("SecretScalar::new(a_right.inner_product"));
}
#[test]
fn vector_padding_and_split_clear_replaced_allocations() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut padded = ScalarVector(vec![TrackingScalar(1), TrackingScalar(2)]);
    let source_pointer = padded.0.as_ptr();
    padded
        .pad_with_zeroes(4)
        .expect("tracking vector pads to final length");
    assert_eq!(
        padded.0.as_slice(),
        &[
            TrackingScalar(1),
            TrackingScalar(2),
            TrackingScalar::ZERO,
            TrackingScalar::ZERO,
        ]
    );
    assert_ne!(padded.0.as_ptr(), source_pointer);
    assert_eq!(padded.0.capacity(), padded.len());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
    drop(padded);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut unchanged = ScalarVector(vec![TrackingScalar(5), TrackingScalar(7)]);
    let unchanged_pointer = unchanged.0.as_ptr();
    let unchanged_capacity = unchanged.0.capacity();
    unchanged
        .pad_with_zeroes(2)
        .expect("equal-length padding is a no-op");
    assert_eq!(
        unchanged.0.as_slice(),
        &[TrackingScalar(5), TrackingScalar(7)]
    );
    assert_eq!(unchanged.0.as_ptr(), unchanged_pointer);
    assert_eq!(unchanged.0.capacity(), unchanged_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(unchanged);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut shrinking = ScalarVector(vec![TrackingScalar(11), TrackingScalar(13)]);
    let shrinking_pointer = shrinking.0.as_ptr();
    let shrinking_capacity = shrinking.0.capacity();
    assert!(matches!(
        shrinking.pad_with_zeroes(1),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(
        shrinking.0.as_slice(),
        &[TrackingScalar(11), TrackingScalar(13)]
    );
    assert_eq!(shrinking.0.as_ptr(), shrinking_pointer);
    assert_eq!(shrinking.0.capacity(), shrinking_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(shrinking);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut overflow = ScalarVector(vec![TrackingScalar(17), TrackingScalar(19)]);
    let overflow_pointer = overflow.0.as_ptr();
    let overflow_capacity = overflow.0.capacity();
    assert!(matches!(
        overflow.pad_with_zeroes(usize::MAX),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    ));
    assert_eq!(
        overflow.0.as_slice(),
        &[TrackingScalar(17), TrackingScalar(19)]
    );
    assert_eq!(overflow.0.as_ptr(), overflow_pointer);
    assert_eq!(overflow.0.capacity(), overflow_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(overflow);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut padded = ScalarVector(vec![TrackingScalar(23), TrackingScalar(29)]);
        let source_pointer = padded.0.as_ptr();
        padded
            .pad_with_zeroes(4)
            .expect("tracking unwind vector pads to final length");
        assert_eq!(
            padded.0.as_slice(),
            &[
                TrackingScalar(23),
                TrackingScalar(29),
                TrackingScalar::ZERO,
                TrackingScalar::ZERO,
            ]
        );
        assert_ne!(padded.0.as_ptr(), source_pointer);
        assert_eq!(padded.0.capacity(), padded.len());
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
        panic!("exercise owner-first scalar padding unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

    let production = include_str!("generalized_bulletproof.rs")
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let padding = production
        .split_once(
            "fn pad_with_zeroes(&mut self, len: usize) -> Result<(), GeneralizedBulletproofErrorV1> {",
        )
        .expect("owned scalar padding")
        .1
        .split_once("fn split(mut self) -> Result<(Self, Self), GeneralizedBulletproofErrorV1> {")
        .expect("owned scalar padding boundary")
        .0;
    let mut cursor = 0;
    for step in cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.1") {
        let offset = padding[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owner-first padding step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(padding.matches("Vec::new()").count(), 0);
    assert_eq!(
        padding
            .matches("Self(try_exact_capacity_vec_v1(len)?)")
            .count(),
        1
    );
    assert_eq!(padding.matches("padded.0.push(F::ZERO);").count(), 1);
    assert_eq!(
        padding
            .matches("core::mem::swap(source, destination);")
            .count(),
        1
    );
    assert_eq!(padding.matches("source.clear_secret();").count(), 1);
    assert_eq!(padding.matches("self.0.truncate(0);").count(), 1);
    assert_eq!(
        padding
            .matches("core::mem::swap(&mut self.0, &mut padded.0);")
            .count(),
        1
    );
    assert_eq!(
        padding
            .matches("debug_assert_eq!(padded.0.len(), len);")
            .count(),
        2
    );
    assert_eq!(
        padding
            .matches("debug_assert_eq!(padded.0.capacity(), len);")
            .count(),
        2
    );
    assert_eq!(
        padding
            .matches("debug_assert_eq!(padded.0.as_ptr(), allocation_pointer);")
            .count(),
        2
    );
    for forbidden in
        cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.2")
    {
        assert!(
            !padding.contains(forbidden),
            "owner-first scalar padding path {forbidden}"
        );
    }
    let prover = production
        .split_once("pub fn prove<R, T>(")
        .expect("generalized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("generalized prover boundary")
        .0;
    assert_eq!(prover.matches(".pad_with_zeroes(n)?;").count(), 4);
    let mut cursor = 0;
    for step in cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.3") {
        let offset = prover[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing scalar-padding prover step {step}"));
        cursor += offset + step.len();
    }
    for callsite in
        cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.4")
    {
        assert_eq!(prover.matches(callsite).count(), 1);
    }
    assert!(production.contains("vector: impl Iterator<Item = &'a F>"));
    assert!(!production.contains("inner_product(right.0.iter().copied())"));
    assert!(!production.contains("inner_product(b.0.iter().copied())"));
    assert!(production.contains("map(|constraint| &constraint.c)"));
    let response_fold_start = prover
        .find("let mut tau_ni = SecretScalar::new(S::Scalar::ZERO);")
        .expect("tau-ni response owner");
    let response_fold_end = prover
        .find("let mut p_terms = SecretMultiexpBuilder::<S>::new(1 + (2 * n))?;")
        .expect("private response-fold boundary");
    let response_fold = &prover[response_fold_start..response_fold_end];
    let mut cursor = 0;
    for step in cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.5") {
        let offset = response_fold[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing borrowed private-response fold step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(
        response_fold.matches("scalar_commitment_weights").count(),
        2
    );
    assert_eq!(response_fold.matches("*tau_ni.expose_mut() +=").count(), 1);
    assert_eq!(
        response_fold
            .matches("tau_before.0.iter().enumerate()")
            .count(),
        1
    );
    assert_eq!(response_fold.matches("tau_ni.expose_ref()").count(), 1);
    assert_eq!(
        response_fold
            .matches("tau_after.0.iter().enumerate()")
            .count(),
        1
    );
    assert_eq!(response_fold.matches("*tau_x.expose_mut() +=").count(), 3);
    assert_eq!(response_fold.matches("*u.expose_mut() +=").count(), 3);
    assert_eq!(response_fold.matches("opening.mask").count(), 2);
    for (borrowed, copied) in [
        ("alpha.expose_ref()", "alpha.expose_copy()"),
        ("beta.expose_ref()", "beta.expose_copy()"),
        ("rho.expose_ref()", "rho.expose_copy()"),
    ] {
        assert_eq!(response_fold.matches(borrowed).count(), 1);
        assert_eq!(prover.matches(borrowed).count(), 2);
        assert_eq!(prover.matches(copied).count(), 0);
    }
    for source_drop in
        cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.6")
    {
        assert_eq!(response_fold.matches(source_drop).count(), 1);
    }
    assert_eq!(prover.matches("tau_x_poly").count(), 0);
    for forbidden in
        cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.7")
    {
        assert!(
            !response_fold.contains(forbidden),
            "borrowed private-response fold path {forbidden}"
        );
    }
    let split = production
        .split_once("fn split(mut self) -> Result<(Self, Self), GeneralizedBulletproofErrorV1> {")
        .expect("owned scalar split")
        .1
        .split_once("/// Sample a secret vector incrementally")
        .expect("owned scalar split boundary")
        .0;
    let mut cursor = 0;
    for step in cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.8") {
        let offset = split[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owner-first split step {step}"));
        cursor += offset + step.len();
    }
    assert_eq!(
        split
            .matches("Self(try_exact_filled_vec_v1(half, F::ZERO)?)")
            .count(),
        2
    );
    assert_eq!(
        split
            .matches("core::mem::swap(source, destination);")
            .count(),
        2
    );
    assert_eq!(split.matches("source.clear_secret();").count(), 2);
    assert_eq!(split.matches("self.0.truncate(0);").count(), 1);
    assert_eq!(split.matches("right.0.push(F::ZERO);").count(), 0);
    assert_eq!(
        split
            .matches("debug_assert_eq!(right.0.len(), half);")
            .count(),
        0
    );
    assert!(!split.contains("try_reserve_exact"));
    assert!(!split.contains("allocation_capacity"));
    assert!(!split.contains("allocation_pointer"));
    for forbidden in
        cleanup_contract_strings("vector_padding_and_split_clear_replaced_allocations.9")
    {
        assert!(
            !split.contains(forbidden),
            "owner-first scalar split path {forbidden}"
        );
    }
    let ipa = production
        .split_once("fn prove_inner_product<S, T>(")
        .expect("inner-product prover")
        .1
        .split_once("fn challenge_products<F: ProofScalar>")
        .expect("inner-product prover boundary")
        .0;
    assert_eq!(
        ipa.matches("let (a_left, a_right) = a.split()?;").count(),
        2
    );
    assert_eq!(
        ipa.matches("let (b_left, b_right) = b.split()?;").count(),
        2
    );
    let owned_scaled_pair_call =
        "p.add_scaled_pair_assign(left, challenge.square(), right, inverse.square());";
    assert_eq!(ipa.matches(owned_scaled_pair_call).count(), 2);
    assert!(!ipa.contains(
        "p.add_scaled_pair_assign(&left, challenge.square(), &right, inverse.square());"
    ));
    assert_eq!(
        ipa.matches("a = (a_left * challenge) + &(a_right * inverse);")
            .count(),
        2
    );
    assert_eq!(
        ipa.matches("b = (b_left * inverse) + &(b_right * challenge);")
            .count(),
        2
    );
    assert_eq!(
        ipa.matches("transcript.push_point(left.expose_ref())?;")
            .count(),
        2
    );
    assert_eq!(
        ipa.matches("transcript.push_point(right.expose_ref())?;")
            .count(),
        2
    );
    let identity_checks = ipa
        .match_indices("if left.is_identity() || right.is_identity() {")
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    let left_publications = ipa
        .match_indices("transcript.push_point(left.expose_ref())?;")
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    let right_publications = ipa
        .match_indices("transcript.push_point(right.expose_ref())?;")
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    let scaled_pair_calls = ipa
        .match_indices(owned_scaled_pair_call)
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    assert_eq!(identity_checks.len(), 2);
    assert_eq!(left_publications.len(), 2);
    assert_eq!(right_publications.len(), 2);
    assert_eq!(scaled_pair_calls.len(), 2);
    for round in 0..2 {
        assert!(identity_checks[round] < left_publications[round]);
        assert!(left_publications[round] < right_publications[round]);
        assert!(right_publications[round] < scaled_pair_calls[round]);
    }
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let values = ScalarVector(vec![
        TrackingScalar(1),
        TrackingScalar(2),
        TrackingScalar(3),
        TrackingScalar(4),
    ]);
    let source_pointer = values.0.as_ptr();
    let (left, right) = values.split().expect("tracking vector splits evenly");
    assert_eq!(left.0.as_slice(), &[TrackingScalar(1), TrackingScalar(2)]);
    assert_eq!(right.0.as_slice(), &[TrackingScalar(3), TrackingScalar(4)]);
    assert_ne!(left.0.as_ptr(), source_pointer);
    assert_eq!(left.0.capacity(), left.len());
    let right_pointer = right.0.as_ptr();
    let right_capacity = right.0.capacity();
    assert_ne!(right_pointer, source_pointer);
    assert_eq!(right.len(), 2);
    assert_eq!(right_capacity, right.len());
    assert_ne!(left.0.as_ptr(), right_pointer);
    assert_eq!(right.0.as_ptr(), right_pointer);
    assert_eq!(right.0.capacity(), right_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
    drop(left);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
    drop(right);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 8);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    assert!(matches!(
        ScalarVector(vec![
            TrackingScalar(5),
            TrackingScalar(6),
            TrackingScalar(7),
        ])
        .split(),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let values = ScalarVector(vec![
            TrackingScalar(11),
            TrackingScalar(13),
            TrackingScalar(17),
            TrackingScalar(19),
        ]);
        let (left, right) = values.split().expect("tracking unwind split");
        assert_eq!(left.0.as_slice(), &[TrackingScalar(11), TrackingScalar(13)]);
        assert_eq!(
            right.0.as_slice(),
            &[TrackingScalar(17), TrackingScalar(19)]
        );
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
        panic!("exercise owner-first scalar split unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 8);
}
#[test]
fn random_vector_clears_success_and_partial_failure() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut success = ScriptedRandom {
        requests: 0,
        fail_at: None,
    };
    let values = random_scalar_vector::<TrackingScalar, _>(&mut success, 4)
        .expect("scripted random succeeds");
    assert_eq!(success.requests, 4);
    assert_eq!(values.len(), 4);
    assert_eq!(values.0.capacity(), values.len());
    // Each decoder slot and now-zero sampled owner is cleared after its
    // value enters the retained vector.
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 8);
    drop(values);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 12);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    success.requests = 0;
    let empty = random_scalar_vector::<TrackingScalar, _>(&mut success, 0)
        .expect("empty random vector succeeds without entropy");
    assert_eq!(success.requests, 0);
    assert!(empty.is_empty());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(empty);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    assert!(matches!(
        random_scalar_vector::<TrackingScalar, _>(&mut success, usize::MAX),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    ));
    assert_eq!(success.requests, 0);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut failure = ScriptedRandom {
        requests: 0,
        fail_at: Some(3),
    };
    assert!(matches!(
        random_scalar_vector::<TrackingScalar, _>(&mut failure, 5),
        Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    ));
    assert_eq!(failure.requests, 4);
    // Three decoded and sampled-owner slots clear before the error; the
    // complete destination then clears three values and two zero slots.
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 11);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut random = PanickingRandom {
            requests: 0,
            panic_at: 1,
        };
        let _ = random_scalar_vector::<TrackingScalar, _>(&mut random, 3);
    }));
    assert!(unwind.is_err());
    // The first decoder and now-zero sampled owner clear before the second
    // request; unwinding then clears one value and two zero destinations.
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 5);
    let source = include_str!("generalized_bulletproof.rs");
    let random_vector = source
        .split_once("fn random_scalar_vector<F, R>(")
        .expect("random scalar vector")
        .1
        .split_once("/// Opening of one Pedersen vector commitment")
        .expect("random scalar vector boundary")
        .0;
    let mut cursor = 0;
    for step in cleanup_contract_strings("random_vector_clears_success_and_partial_failure.1") {
        let offset = random_vector[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("missing owner-first random-vector step {step}"));
        cursor += offset + step.len();
    }
    for (needle, expected) in [
        ("try_exact_capacity_vec_v1(len)?", 1),
        ("result.0.push(F::ZERO);", 1),
        ("let mut sampled = random_scalar::<F, _>(rng)?;", 1),
        ("core::mem::swap(destination, sampled.expose_mut());", 1),
        ("drop(sampled);", 1),
        ("debug_assert_eq!(result.0.len(), len);", 2),
        ("capacity(), len);", 2),
        ("as_ptr(), allocation_pointer);", 2),
    ] {
        assert_eq!(random_vector.matches(needle).count(), expected);
    }
    for forbidden in cleanup_contract_strings("random_vector_clears_success_and_partial_failure.2")
    {
        assert!(
            !random_vector.contains(forbidden),
            "owner-first random-vector path {forbidden}"
        );
    }
    let prover = source
        .split_once("pub fn prove<R, T>(")
        .expect("generalized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("generalized prover boundary")
        .0;
    for callsite in cleanup_contract_strings("random_vector_clears_success_and_partial_failure.3") {
        assert_eq!(prover.matches(callsite).count(), 1);
    }
    assert_eq!(
        prover.matches("random_scalar_vector::<S::Scalar").count(),
        4
    );
    assert!(prover.contains("let alpha = random_scalar::<S::Scalar, _>(rng)?;"));
    assert!(prover.contains("let beta = random_scalar::<S::Scalar, _>(rng)?;"));
    assert!(prover.contains("let rho = random_scalar::<S::Scalar, _>(rng)?;"));
    assert!(!prover.contains("SecretScalar::new(random_scalar"));
}
#[test]
fn scalar_commitment_openings_clear_on_success_error_and_unwind() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let opening = VectorCommitmentOpening::new(
        vec![TrackingScalar(2), TrackingScalar(3)],
        TrackingScalar(5),
    );
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(opening);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let legacy = ArithmeticCircuitWitness::<TrackingSuite>::new(
        vec![TrackingScalar(3)],
        vec![TrackingScalar(5)],
        Vec::new(),
    )
    .expect("legacy witness constructor remains valid");
    assert!(legacy.scalar_commitments.is_empty());
    drop(legacy);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let witness = ArithmeticCircuitWitness::<TrackingSuite>::new_with_scalar_commitments(
        vec![TrackingScalar(3)],
        vec![TrackingScalar(5)],
        Vec::new(),
        vec![(TrackingScalar(7), TrackingScalar(11))],
    )
    .expect("scalar-opening witness constructor succeeds");
    drop(witness);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 9);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    assert!(matches!(
        ArithmeticCircuitWitness::<TrackingSuite>::new_with_scalar_commitments(
            vec![TrackingScalar(3)],
            Vec::new(),
            Vec::new(),
            vec![(TrackingScalar(7), TrackingScalar(11))],
        ),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 7);
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _witness = ArithmeticCircuitWitness::<TrackingSuite>::new_with_scalar_commitments(
            vec![TrackingScalar(3)],
            vec![TrackingScalar(5)],
            Vec::new(),
            vec![(TrackingScalar(7), TrackingScalar(11))],
        )
        .expect("unwind fixture witness");
        panic!("exercise scalar-opening witness unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 9);
}
#[test]
fn vector_commitment_mask_slot_handoff_clears_on_success_and_unwind() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut success_mask = TrackingScalar(5);
    let opening = VectorCommitmentOpening::take_mask_from_slot(
        vec![TrackingScalar(2), TrackingScalar(3)],
        &mut success_mask,
    );
    assert_eq!(success_mask, TrackingScalar::ZERO);
    assert_eq!(
        opening.values.0.as_slice(),
        &[TrackingScalar(2), TrackingScalar(3)]
    );
    assert_eq!(opening.mask, TrackingScalar(5));
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(opening);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 5);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut unwind_mask = TrackingScalar(11);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _opening = VectorCommitmentOpening::take_mask_from_slot(
            vec![TrackingScalar(7), TrackingScalar(9)],
            &mut unwind_mask,
        );
        assert_eq!(unwind_mask, TrackingScalar::ZERO);
        panic!("exercise vector-opening mask-slot unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(unwind_mask, TrackingScalar::ZERO);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 5);
}
#[test]
fn vector_commitment_values_rehome_without_copy_or_allocation() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut success_mask = TrackingScalar(5);
    let mut opening = VectorCommitmentOpening::take_mask_from_slot(
        vec![TrackingScalar(2), TrackingScalar(3)],
        &mut success_mask,
    );
    let source_pointer = opening.values.0.as_ptr();
    let source_capacity = opening.values.0.capacity();
    let values = opening.take_values();
    assert_eq!(success_mask, TrackingScalar::ZERO);
    assert!(opening.values.0.is_empty());
    assert_eq!(opening.values.0.capacity(), 0);
    assert_eq!(opening.mask, TrackingScalar(5));
    assert_eq!(values.0.as_slice(), &[TrackingScalar(2), TrackingScalar(3)]);
    assert_eq!(values.0.as_ptr(), source_pointer);
    assert_eq!(values.0.capacity(), source_capacity);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(opening);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    drop(values);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut unwind_mask = TrackingScalar(11);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut opening = VectorCommitmentOpening::take_mask_from_slot(
            vec![TrackingScalar(7), TrackingScalar(9)],
            &mut unwind_mask,
        );
        let source_pointer = opening.values.0.as_ptr();
        let source_capacity = opening.values.0.capacity();
        let values = opening.take_values();
        assert!(opening.values.0.is_empty());
        assert_eq!(opening.values.0.capacity(), 0);
        assert_eq!(opening.mask, TrackingScalar(11));
        assert_eq!(values.0.as_slice(), &[TrackingScalar(7), TrackingScalar(9)]);
        assert_eq!(values.0.as_ptr(), source_pointer);
        assert_eq!(values.0.capacity(), source_capacity);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
        panic!("exercise vector-opening value-owner unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(unwind_mask, TrackingScalar::ZERO);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    let source = include_str!("generalized_bulletproof.rs");
    let production = source
        .split_once("#[cfg(test)]\nmod secret_cleanup_tests")
        .expect("production source boundary")
        .0;
    let opening_owner = production
        .split_once("impl<F: ProofScalar> VectorCommitmentOpening<F> {")
        .expect("vector commitment opening owner")
        .1
        .split_once("/// Owned opening of one scalar Pedersen commitment")
        .expect("vector commitment opening boundary")
        .0;
    let values_handoff = opening_owner
        .split_once("fn take_values(&mut self) -> ScalarVector<F> {")
        .expect("vector value-owner handoff")
        .1
        .split_once("/// Construct an opening from committed values")
        .expect("vector value-owner handoff boundary")
        .0;
    assert!(
        values_handoff.contains("core::mem::replace(&mut self.values, ScalarVector(Vec::new()))")
    );
    assert_eq!(values_handoff.matches("Vec::new()").count(), 1);
    for forbidden in
        cleanup_contract_strings("vector_commitment_values_rehome_without_copy_or_allocation.1")
    {
        assert!(
            !values_handoff.contains(forbidden),
            "retained value-owner handoff path {forbidden}"
        );
    }
    let prover = production
        .split_once("pub fn prove<R, T>(")
        .expect("generalized prover")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("generalized prover boundary")
        .0;
    let commitment_coefficients = prover
        .split_once("for (mut index, (opening, weights)) in witness")
        .expect("vector commitment polynomial coefficients")
        .1
        .split_once("let t_poly_len")
        .expect("vector commitment polynomial boundary")
        .0;
    let value_move = ["l[index] = opening.", "take_values();"].concat();
    let raw_clone = ["opening.values", ".clone()"].concat();
    assert!(commitment_coefficients.contains(".iter_mut()"));
    assert_eq!(commitment_coefficients.matches(&value_move).count(), 1);
    assert!(!commitment_coefficients.contains(&raw_clone));
    assert!(commitment_coefficients.contains("r[reverse] = weights;"));
    assert!(prover.contains("result.add_scaled_assign(coefficient, &x[index]);"));
    assert!(!prover.contains("result = result + &(coefficient.clone() * x[index]);"));
    let value_move_index = commitment_coefficients
        .find(&value_move)
        .expect("moved opening values");
    let public_weights_index = commitment_coefficients
        .find("r[reverse] = weights;")
        .expect("moved public commitment weights");
    assert!(value_move_index < public_weights_index);
}
#[test]
fn scalar_commitment_opening_source_boundary_stays_private_and_zeroizing() {
    let source = include_str!("generalized_bulletproof.rs");
    let fcmp_bulletproof =
        include_str!("../../iroha_core/src/privacy_engines/fcmp_plus_plus/bulletproof.rs");
    let fcmp_circuit =
        include_str!("../../iroha_core/src/privacy_engines/fcmp_plus_plus/circuit.rs");
    assert!(source.contains("struct ScalarCommitmentOpening<F: ProofScalar>"));
    assert!(!source.contains("pub struct ScalarCommitmentOpening"));
    assert!(source.contains("struct ScalarCommitmentOpeningInputs<F: ProofScalar>"));
    assert!(source.contains("pub fn new(values: Vec<F>, mut mask: F) -> Self"));
    let mask_slot_handoff = source
        .split_once("pub fn take_mask_from_slot(values: Vec<F>, mask: &mut F) -> Self")
        .expect("vector-opening mask-slot handoff")
        .1
        .split_once("/// Move the committed values into their next zeroizing owner")
        .expect("vector value-owner handoff boundary")
        .0;
    assert!(mask_slot_handoff.contains("mask: F::ZERO"));
    assert!(mask_slot_handoff.contains("core::mem::swap(&mut opening.mask, mask)"));
    for forbidden in cleanup_contract_strings(
        "scalar_commitment_opening_source_boundary_stays_private_and_zeroizing.1",
    ) {
        assert!(
            !mask_slot_handoff.contains(forbidden),
            "retained vector-opening mask-slot handoff {forbidden}"
        );
    }
    assert!(source.contains("fn new(mut value: F, mut mask: F) -> Self"));
    assert!(source.contains("let incoming_value = BorrowedSecretScalarSlot(&mut value);"));
    assert!(source.contains("let incoming_mask = BorrowedSecretScalarSlot(&mut mask);"));
    assert!(source.contains("drop((incoming_value, incoming_mask));"));
    assert!(source.contains("pub(crate) fn new_with_scalar_commitments("));
    assert!(source.contains("self.value.clear_secret();\n        self.mask.clear_secret();"));
    assert!(source.contains("for (value, mask) in &mut self.0"));
    assert!(source.contains("terms.push(&opening.value, &self.generators.g)?;"));
    assert!(source.contains("terms.push(&opening.mask, &self.generators.h)?;"));
    let scalar_commitment_weights = source
        .split_once("let scalar_commitment_weights = if let Some(weights) = exact_v_weights")
        .expect("scalar-commitment weight selection")
        .1
        .split_once("for (mut index, (opening, weights)) in witness")
        .expect("scalar-commitment weight boundary")
        .0;
    assert!(scalar_commitment_weights.contains(
        "let mut weights = ScalarVector::try_zero_exact_v1(self.scalar_commitments.len())?"
    ));
    assert!(scalar_commitment_weights.contains("accumulate(&mut weights, &constraint.wv, -*z)"));
    assert!(!fcmp_bulletproof.contains("new_with_scalar_commitments"));
    assert!(!fcmp_circuit.contains("new_with_scalar_commitments"));
}

fn polynomial_storage_fixture_v1(
    commitment_count: usize,
    n: usize,
) -> (
    Vec<ScalarVector<TrackingScalar>>,
    Vec<ScalarVector<TrackingScalar>>,
) {
    let ni = 2 + 2 * (commitment_count / 2);
    let half = ni / 2;
    let mut left: Vec<_> = (0..ni + 2).map(|_| ScalarVector(Vec::new())).collect();
    let mut right: Vec<_> = (0..ni + 2).map(|_| ScalarVector(Vec::new())).collect();
    for (ordinal, slot) in [half, ni, ni + 1].into_iter().enumerate() {
        left[slot] = ScalarVector(
            (0..n)
                .map(|j| TrackingScalar((ordinal * n + j + 1) as u64))
                .collect(),
        );
    }
    for (ordinal, slot) in [half, 0, ni + 1].into_iter().enumerate() {
        right[slot] = ScalarVector(
            (0..n)
                .map(|j| TrackingScalar((ordinal * n + j + 7) as u64))
                .collect(),
        );
    }
    for commitment in 0..commitment_count {
        let slot = commitment + usize::from(commitment >= half);
        // Full-length zero-valued vectors are assigned owners too; their
        // pointer must survive exactly like every nonzero opening.
        let values = if commitment == 0 {
            vec![TrackingScalar::ZERO; n]
        } else {
            (0..n)
                .map(|j| TrackingScalar((commitment * n + j + 19) as u64))
                .collect()
        };
        let mut mask = TrackingScalar(13);
        let mut opening = VectorCommitmentOpening::take_mask_from_slot(values, &mut mask);
        let allocation = opening.values.0.as_ptr();
        left[slot] = opening.take_values();
        assert_eq!(left[slot].0.as_ptr(), allocation);
        right[ni - slot] = ScalarVector(
            (0..n)
                .map(|j| TrackingScalar((commitment * n + j + 31) as u64))
                .collect(),
        );
    }
    (left, right)
}

#[test]
fn polynomial_storage_fills_only_public_unassigned_slots_after_owner_moves() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    for commitments in [0, 1, 2, 3, 100] {
        let n = 8;
        let (mut left, mut right) = polynomial_storage_fixture_v1(commitments, n);
        let before: Vec<_> = left
            .iter()
            .chain(&right)
            .map(|coefficient| {
                (
                    coefficient.len(),
                    coefficient.0.capacity(),
                    coefficient.0.as_ptr(),
                )
            })
            .collect();
        let unassigned = before.iter().filter(|(len, _, _)| *len == 0).count();
        assert_eq!(
            unassigned,
            if commitments.is_multiple_of(2) { 2 } else { 0 }
        );
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        fill_unassigned_polynomial_coefficients_v1(&mut left, n).expect("left shape");
        fill_unassigned_polynomial_coefficients_v1(&mut right, n).expect("right shape");
        assert_eq!(
            CLEAR_CALLS.load(Ordering::SeqCst),
            0,
            "no overwritten dense allocation"
        );
        for (coefficient, (len, capacity, allocation)) in left.iter().chain(&right).zip(before) {
            assert_eq!(coefficient.len(), n);
            if len == 0 {
                assert_eq!(coefficient.0.capacity(), n);
                assert!(
                    coefficient
                        .0
                        .iter()
                        .all(|value| *value == TrackingScalar::ZERO)
                );
            } else {
                assert_eq!(coefficient.0.capacity(), capacity);
                assert_eq!(
                    coefficient.0.as_ptr(),
                    allocation,
                    "owned coefficient reallocated"
                );
            }
        }
        let retained_scalars = (left.len() + right.len()) * n;
        drop(left);
        drop(right);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), retained_scalars);
    }
}

#[test]
fn polynomial_storage_preserves_dense_product_and_evaluation_reference() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    for commitments in [0, 1, 2, 3, 100] {
        let n = 3;
        let (mut left, mut right) = polynomial_storage_fixture_v1(commitments, n);
        // Independent scalar arrays model absent public coefficient slots as
        // mathematical zero. Form the reference one coordinate at a time.
        let dense = |polynomial: &[ScalarVector<TrackingScalar>]| -> Vec<Vec<u64>> {
            polynomial
                .iter()
                .map(|coefficient| {
                    if coefficient.is_empty() {
                        vec![0; n]
                    } else {
                        coefficient.0.iter().map(|value| value.0).collect()
                    }
                })
                .collect()
        };
        let expected_l = dense(&left);
        let expected_r = dense(&right);
        let mut reference = vec![0_u64; left.len() + right.len() - 1];
        for coordinate in 0..n {
            for (i, l) in expected_l.iter().enumerate() {
                for (j, r) in expected_r.iter().enumerate() {
                    reference[i + j] =
                        reference[i + j].wrapping_add(l[coordinate].wrapping_mul(r[coordinate]));
                }
            }
        }
        fill_unassigned_polynomial_coefficients_v1(&mut left, n).expect("left shape");
        fill_unassigned_polynomial_coefficients_v1(&mut right, n).expect("right shape");
        assert_eq!(dense(&left), expected_l);
        assert_eq!(dense(&right), expected_r);
        let mut actual = ScalarVector::<TrackingScalar>::zero(reference.len());
        for (i, l) in left.iter().enumerate() {
            for (j, r) in right.iter().enumerate() {
                let product = l.inner_product(r.0.iter());
                actual[i + j] += *product.expose_ref();
            }
        }
        assert_eq!(
            actual.0.iter().map(|value| value.0).collect::<Vec<_>>(),
            reference
        );
        let powers = ScalarVector::powers(TrackingScalar(7), left.len());
        for (polynomial, reference) in [(&left, &expected_l), (&right, &expected_r)] {
            let mut evaluated = ScalarVector::zero(n);
            for (index, coefficient) in polynomial.iter().enumerate() {
                evaluated.add_scaled_assign(coefficient, &powers[index]);
            }
            for coordinate in 0..n {
                let expected = reference.iter().rev().fold(0_u64, |value, coefficient| {
                    value.wrapping_mul(7).wrapping_add(coefficient[coordinate])
                });
                assert_eq!(evaluated[coordinate].0, expected);
            }
        }
    }
}

#[test]
fn polynomial_storage_errors_and_unwind_preserve_zeroizing_owners() {
    let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let mut malformed = vec![
        ScalarVector(Vec::new()),
        ScalarVector(vec![TrackingScalar(7), TrackingScalar(11)]),
        ScalarVector(vec![TrackingScalar(13)]),
    ];
    let allocation = malformed[1].0.as_ptr();
    assert_eq!(
        fill_unassigned_polynomial_coefficients_v1(&mut malformed, 2),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(
        malformed[0].0.capacity(),
        0,
        "late invalid shape must precede zero allocation"
    );
    assert_eq!(malformed[1].0.as_ptr(), allocation);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 0);
    drop(malformed);
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);

    let mut absent = [ScalarVector::<TrackingScalar>(Vec::new())];
    assert_eq!(
        fill_unassigned_polynomial_coefficients_v1(&mut absent, 0),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(
        fill_unassigned_polynomial_coefficients_v1(&mut absent, usize::MAX),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!(absent[0].0.capacity(), 0);

    CLEAR_CALLS.store(0, Ordering::SeqCst);
    let unwind = std::panic::catch_unwind(|| {
        let mut polynomial = [
            ScalarVector(Vec::new()),
            ScalarVector(vec![
                TrackingScalar(17),
                TrackingScalar(19),
                TrackingScalar(23),
            ]),
            ScalarVector(Vec::new()),
        ];
        fill_unassigned_polynomial_coefficients_v1(&mut polynomial, 3).expect("complete shape");
        panic!("exercise completed polynomial ownership unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 9);
}

#[test]
fn polynomial_storage_completion_follows_all_commitment_moves_in_actual_prover() {
    let source = include_str!("generalized_bulletproof.rs");
    let prover = source
        .split_once("pub fn prove<R, T>(")
        .expect("prover owner")
        .1
        .split_once("/// Consume and verify one proof transcript")
        .expect("prover boundary")
        .0;
    let left_move = prover
        .find("l[index] = opening.take_values();")
        .expect("left owner move");
    let right_move = prover
        .find("r[reverse] = weights;")
        .expect("right owner move");
    let left_fill = prover
        .find("fill_unassigned_polynomial_coefficients_v1(&mut l, n)?;")
        .expect("left completion");
    let right_fill = prover
        .find("fill_unassigned_polynomial_coefficients_v1(&mut r, n)?;")
        .expect("right completion");
    let product = prover.find("let t_poly_len").expect("dense product");
    assert!(left_move < left_fill && right_move < left_fill);
    assert!(left_fill < right_fill && right_fill < product);
    assert_eq!(
        prover
            .matches("fill_unassigned_polynomial_coefficients_v1(")
            .count(),
        2
    );
    assert!(!prover.contains("for coefficient in &mut l"));
    assert!(!prover.contains("for coefficient in &mut r"));
    let helper = source
        .split_once("fn fill_unassigned_polynomial_coefficients_v1<")
        .expect("shape helper")
        .1
        .split_once("/// Sample a secret vector")
        .expect("shape helper boundary")
        .0;
    assert!(!helper.contains(".is_zero("));
    assert!(!helper.contains(".clone("));
}
