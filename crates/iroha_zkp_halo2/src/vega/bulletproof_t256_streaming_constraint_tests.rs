fn verify_membership_materialized_for_streaming_parity<S>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    expected_chunk_ordinal: u16,
    expected_bound: ZkAmsT256MembershipBoundV1,
    expected_coefficient_count: usize,
    evidence: &ZkAmsT256MembershipProofV1,
) -> Result<[u8; 32], ZkAmsT256MembershipErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let prepared = prepare_zk_ams_t256_membership_verification_v1(
        context_digest,
        expected_chunk_ordinal,
        expected_bound,
        expected_coefficient_count,
        ZkAmsT256MembershipVerificationInputV1::Owned(evidence),
    )?;
    let (padded_gates, constraints) =
        membership_constraints(expected_coefficient_count, expected_bound)?;
    let mut transcript = T256BulletproofVerifierTranscriptV1::<S>::new(
        context_digest,
        generator_basis_digest,
        expected_chunk_ordinal,
        expected_bound as u8,
        prepared.commitment,
        prepared.proof,
    )?;
    ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        constraints,
        vec![prepared.commitment],
        Vec::new(),
    )?
    .verify(&mut transcript)?;
    Ok(transcript.finish()?)
}

#[test]
fn streaming_membership_verifier_matches_materialized_transcript_and_errors() {
    let context = keccak256(b"t256-streaming-membership-context");
    let basis = keccak256(b"t256-streaming-membership-basis");
    for (ordinal, bound, coefficients, blinding, rng_label) in [
        (
            9,
            ZkAmsT256MembershipBoundV1::One,
            &[-1, 0, 1][..],
            Scalar::from_u64(29),
            &b"t256-streaming-bound-one-rng"[..],
        ),
        (
            10,
            ZkAmsT256MembershipBoundV1::Two,
            &[-2, -1, 0, 1, 2][..],
            Scalar::from_u64(31),
            &b"t256-streaming-bound-two-rng"[..],
        ),
    ] {
        let (evidence, prover_digest) = prove_membership_chunk_for_suite::<TinyT256Suite, _>(
            context,
            basis,
            ordinal,
            bound,
            coefficients,
            &blinding,
            &mut KatRandom::new(rng_label),
        )
        .expect("canonical tiny membership proof");
        let lease = Mutex::new(());
        let streaming = verify_membership_input_for_suite_with_lease::<TinyT256Suite>(
            context,
            basis,
            ordinal,
            bound,
            coefficients.len(),
            ZkAmsT256MembershipVerificationInputV1::Owned(&evidence),
            &lease,
        );
        let materialized = verify_membership_materialized_for_streaming_parity::<TinyT256Suite>(
            context,
            basis,
            ordinal,
            bound,
            coefficients.len(),
            &evidence,
        );
        assert_eq!(streaming, Ok(prover_digest));
        assert_eq!(streaming, materialized);
        for index in [0, evidence.proof.len() / 2, evidence.proof.len() - 1] {
            let mut changed = evidence.clone();
            changed.proof[index] ^= 1;
            let streaming = verify_membership_input_for_suite_with_lease::<TinyT256Suite>(
                context,
                basis,
                ordinal,
                bound,
                coefficients.len(),
                ZkAmsT256MembershipVerificationInputV1::Owned(&changed),
                &lease,
            );
            let materialized = verify_membership_materialized_for_streaming_parity::<TinyT256Suite>(
                context,
                basis,
                ordinal,
                bound,
                coefficients.len(),
                &changed,
            );
            assert_eq!(streaming, materialized);
            assert!(streaming.is_err());
        }
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn streaming_logical_payload_reduction_is_pinned_without_rss_multiplier_claims() {
    let scalar = core::mem::size_of::<Scalar>();
    let tuple = core::mem::size_of::<(usize, Scalar)>();
    let vec_header = core::mem::size_of::<Vec<(usize, Scalar)>>();
    let linear_combination = core::mem::size_of::<LinComb<Scalar>>();
    assert_eq!(
        (scalar, tuple, vec_header, linear_combination),
        (32, 40, 24, 200)
    );
    let bound_one = 98_304 * linear_combination
        + 16_384 * ((11 * tuple) + vec_header)
        + 16_384 * (tuple + vec_header)
        + 98_304 * scalar;
    let bound_two = 163_840 * linear_combination
        + 16_384 * ((16 * tuple) + vec_header)
        + 49_152 * (tuple + vec_header)
        + 163_840 * scalar;
    assert_eq!(bound_one, 31_457_280);
    assert_eq!(bound_two, 52_035_584);
}

#[test]
fn streaming_verifier_source_is_pretranscript_closed_and_prover_stays_materialized() {
    let parent = include_str!("bulletproof_t256.rs");
    let verifier = parent
        .split_once("fn verify_prepared_membership_chunk_for_suite<S>(")
        .expect("prepared verifier")
        .1
        .split_once("fn verify_membership_input_for_suite_with_lease_v1<S>(")
        .expect("prepared verifier boundary")
        .0;
    let statement = verifier
        .find("ExactSmallCoefficientVerifierStatementV1::new(")
        .expect("checked exact statement");
    let transcript = verifier
        .find("T256BulletproofVerifierTranscriptV1::<S>::new(")
        .expect("verifier transcript");
    assert!(statement < transcript);
    for forbidden in [
        "membership_constraints(",
        "FnOnce",
        "FnMut",
        "callback",
        "dyn ",
        "rayon",
        "par_",
    ] {
        assert!(
            !verifier.contains(forbidden),
            "forbidden exact verifier source: {forbidden}"
        );
    }
    let prover = parent
        .split_once("fn prove_membership_chunk_for_suite<S, R>(")
        .expect("membership prover")
        .1
        .split_once("enum ZkAmsT256MembershipVerificationInputV1")
        .expect("membership prover boundary")
        .0;
    assert!(prover.contains("membership_constraints(coefficients.len(), bound)?"));
    assert!(parent.lines().count() <= 3_000 && parent.len() <= 120 * 1024);
    let child = include_str!("bulletproof_t256_streaming_constraint_tests.rs");
    assert!(child.lines().count() <= 500 && child.len() <= 24 * 1024);
}
