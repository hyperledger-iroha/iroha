fn boolean_constraints(gate: usize) -> [LinComb<Scalar>; 2] {
    [
        LinComb::empty()
            .term(Scalar::ONE, Variable::aL(gate))
            .term(-Scalar::ONE, Variable::aR(gate)),
        LinComb::empty()
            .term(Scalar::ONE, Variable::aO(gate))
            .term(-Scalar::ONE, Variable::aL(gate)),
    ]
}

fn membership_constraints(
    coefficient_count: usize,
    bound: ZkAmsT256MembershipBoundV1,
) -> Result<(usize, Vec<LinComb<Scalar>>), ZkAmsT256MembershipErrorV1> {
    let (_, padded_gates, constraint_count) = membership_shape(coefficient_count, bound)?;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    for coefficient_index in 0..coefficient_count {
        let first_gate = coefficient_index * bound.gates_per_coefficient();
        constraints.extend(boolean_constraints(first_gate));
        constraints.extend(boolean_constraints(first_gate + 1));
        match bound {
            ZkAmsT256MembershipBoundV1::One => {
                constraints.push(
                    LinComb::empty()
                        .term(Scalar::ONE, Variable::aL(first_gate))
                        .term(-Scalar::ONE, Variable::aL(first_gate + 1))
                        .term(
                            -Scalar::ONE,
                            Variable::CG {
                                commitment: 0,
                                index: coefficient_index,
                            },
                        ),
                );
            }
            ZkAmsT256MembershipBoundV1::Two => {
                constraints.extend(boolean_constraints(first_gate + 2));
                constraints.push(
                    LinComb::empty()
                        .term(Scalar::ONE, Variable::aL(first_gate))
                        .term(Scalar::ONE, Variable::aL(first_gate + 1))
                        .term(-Scalar::from_u64(2), Variable::aL(first_gate + 2))
                        .term(
                            -Scalar::ONE,
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
        constraints.push(LinComb::empty().term(
            Scalar::ONE,
            Variable::CG {
                commitment: 0,
                index: padded_index,
            },
        ));
    }
    debug_assert_eq!(constraints.len(), constraint_count);
    Ok((padded_gates, constraints))
}

fn prove_membership_materialized_for_streaming_parity<S>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunk_ordinal: u16,
    bound: ZkAmsT256MembershipBoundV1,
    coefficients: &[i8],
    blinding: &Scalar,
    rng_label: &[u8],
) -> Result<(ZkAmsT256MembershipProofV1, [u8; 32]), ZkAmsT256MembershipErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let (padded_gates, constraints) = membership_constraints(coefficients.len(), bound)?;
    let (secret_commitment, witness) = membership_witness::<S>(coefficients, bound, blinding)?;
    let mut transcript = T256BulletproofProverTranscriptV1::<S>::new(
        context_digest,
        generator_basis_digest,
        chunk_ordinal,
        bound as u8,
        secret_commitment.expose_ref(),
        membership_proof_len(padded_gates)?,
    )?;
    ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        constraints,
        vec![*secret_commitment.expose_ref()],
        Vec::new(),
    )?
    .prove(&mut KatRandom::new(rng_label), &mut transcript, witness)?;
    let (proof, transcript_digest) = transcript.complete()?;
    Ok((
        ZkAmsT256MembershipProofV1 {
            bound,
            chunk_ordinal,
            coefficient_count: u32::try_from(coefficients.len())
                .map_err(|_| ZkAmsT256MembershipErrorV1::CoefficientCount)?,
            commitment: *secret_commitment.expose_ref(),
            proof,
        },
        transcript_digest,
    ))
}

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
        let (materialized_evidence, materialized_digest) =
            prove_membership_materialized_for_streaming_parity::<TinyT256Suite>(
                context,
                basis,
                ordinal,
                bound,
                coefficients,
                &blinding,
                rng_label,
            )
            .expect("canonical materialized membership proof");
        assert_eq!(evidence, materialized_evidence);
        assert_eq!(prover_digest, materialized_digest);
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

fn assert_exact_witness_rejected_before_randomness(
    bound: ZkAmsT256MembershipBoundV1,
    opening_values: Vec<Scalar>,
    a_l: Vec<Scalar>,
    a_r: Vec<Scalar>,
    label: &[u8],
) {
    let (_, padded_gates, _) = membership_shape(1, bound).expect("one-coefficient shape");
    assert_eq!(opening_values.len(), padded_gates);
    let generators = TinyT256Suite::generators()
        .reduce(padded_gates)
        .expect("tiny exact basis");
    let mask = Scalar::from_u64(43);
    let mut terms = SecretMultiexpBuilder::<TinyT256Suite>::new(padded_gates + 1)
        .expect("exact commitment capacity");
    for (value, point) in opening_values.iter().zip(generators.g_bold) {
        terms
            .push(value, point)
            .expect("exact commitment value term");
    }
    terms
        .push(&mask, &generators.h)
        .expect("exact commitment mask term");
    let commitment = *terms
        .evaluate()
        .expect("complete exact commitment")
        .expose_ref();
    let witness = ArithmeticCircuitWitness::<TinyT256Suite>::new(
        a_l,
        a_r,
        vec![VectorCommitmentOpening::new(opening_values, mask)],
    )
    .expect("shape-valid malformed exact witness");
    let source = exact_small::ExactSmallCoefficientConstraintSourceV1::new(1, bound.exact_source())
        .expect("one-coefficient exact source");
    let statement =
        exact_small::ExactSmallCoefficientProverStatementV1::new(generators, source, commitment)
            .expect("shape-valid exact statement");
    let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
        keccak256(b"t256-exact-malformed-context"),
        keccak256(b"t256-exact-malformed-basis"),
        12,
        bound as u8,
        &commitment,
        membership_proof_len(padded_gates).expect("valid tiny proof shape"),
    )
    .expect("valid malformed-witness transcript axes");
    let mut rng = KatRandom::new(label);
    assert_eq!(
        statement.prove(&mut rng, &mut transcript, witness),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    );
    assert_eq!(rng.counter, 0);
    assert_eq!(transcript.partial_proof_len(), 0);
}

#[test]
fn exact_prover_rejects_boolean_relations_and_padded_tail_before_randomness() {
    let zero = Scalar::ZERO;
    let one = Scalar::ONE;
    assert_exact_witness_rejected_before_randomness(
        ZkAmsT256MembershipBoundV1::One,
        vec![one, zero],
        vec![one, zero],
        vec![zero, zero],
        b"t256-exact-malformed-boolean",
    );
    assert_exact_witness_rejected_before_randomness(
        ZkAmsT256MembershipBoundV1::One,
        vec![zero, zero],
        vec![one, zero],
        vec![one, zero],
        b"t256-exact-malformed-bound-one-relation",
    );
    assert_exact_witness_rejected_before_randomness(
        ZkAmsT256MembershipBoundV1::Two,
        vec![zero, zero, zero, zero],
        vec![one, zero, zero],
        vec![one, zero, zero],
        b"t256-exact-malformed-bound-two-relation",
    );
    assert_exact_witness_rejected_before_randomness(
        ZkAmsT256MembershipBoundV1::One,
        vec![zero, one],
        vec![zero, zero],
        vec![zero, zero],
        b"t256-exact-malformed-tail",
    );
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

    let (point, term) = (
        core::mem::size_of::<Point>(),
        core::mem::size_of::<(Scalar, Point)>(),
    );
    assert_eq!((scalar, point, term), (32, 96, 128));
    let (h_sum, additional, terms, encodings, buckets) = (17, 36, 131_127, 131_127, 256);
    assert_eq!(2 * 65_536 * scalar, 4_194_304);
    assert_eq!((h_sum * scalar, additional * term), (544, 4_608));
    assert_eq!(
        (terms * term, encodings * 32, buckets * point),
        (16_784_256, 4_196_064, 24_576)
    );
    assert_eq!(terms * term + encodings * 32 + buckets * point, 21_004_896);
    assert_eq!((16_384 + 2 * 49_152) * scalar, 3_670_016);
    assert_eq!((16_384 + 3 * 49_152) * scalar, 5_242_880);
    assert_eq!(6 * 65_536 * scalar, 12_582_912);
}

#[test]
fn fixed_membership_scalar_owners_are_exact_and_bounded() {
    let mut values = ZeroizingT256ScalarVecV1::try_with_exact_capacity(1).expect("exact owner");
    assert_eq!((values.len(), values.0.capacity()), (0, 1));
    values
        .try_push_within_capacity(Scalar::ONE)
        .expect("one fixed slot");
    assert_eq!(
        values.try_push_within_capacity(Scalar::ONE),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    );
    assert_eq!((values.len(), values.0.capacity()), (1, 1));

    let parent = include_str!("bulletproof_t256.rs");
    let production = parent.rsplit_once("\n#[cfg(test)]\nmod tests {").unwrap().0;
    let owner = source_between(
        production,
        "impl ZeroizingT256ScalarVecV1 {",
        "impl core::fmt::Debug for ZeroizingT256ScalarVecV1",
    );
    assert!(owner.contains("Ok(Self(try_exact_capacity_vec_v1(capacity)?))"));
    assert_source_order(
        owner,
        &[
            "fn try_push_within_capacity(",
            "let incoming = BorrowedT256ScalarCopyV1",
            "if self.0.len() >= self.0.capacity()",
            "self.0.push(incoming.get());",
            "drop(incoming);",
            "Ok(())",
        ],
    );
    let boolean = source_between(
        production,
        "fn append_boolean_witness(",
        "fn membership_commitment_for_suite",
    );
    assert_eq!(boolean.matches("try_push_within_capacity").count(), 2);
    let fixed = source_between(
        production,
        "fn membership_commitment_for_suite",
        "fn prove_membership_chunk_for_suite",
    );
    assert_eq!(fixed.matches("try_with_exact_capacity(").count(), 3);
    assert_eq!(fixed.matches("try_push_within_capacity(").count(), 1);
    assert_eq!(fixed.matches("append_boolean_witness(").count(), 5);
    for forbidden in [
        "ZeroizingT256ScalarVecV1::with_capacity",
        "values.push(",
        "a_l.push(",
        "a_r.push(",
    ] {
        assert!(!fixed.contains(forbidden));
    }
}

#[test]
fn exact_membership_sources_are_pretranscript_closed() {
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
    let source = prover
        .find("ExactSmallCoefficientConstraintSourceV1::new(")
        .expect("checked exact prover source");
    let statement = prover
        .find("ExactSmallCoefficientProverStatementV1::new(")
        .expect("checked exact prover statement");
    let transcript = prover
        .find("T256BulletproofProverTranscriptV1::<S>::new_with_exact_proof_buffer(")
        .expect("prover transcript");
    assert!(source < statement && statement < transcript);
    assert!(prover.contains("statement.prove(rng, &mut transcript, witness)?;"));
    for forbidden in [
        "membership_constraints(",
        "ArithmeticCircuitStatement::new(",
        "FnOnce",
        "FnMut",
        "callback",
        "dyn ",
        "rayon",
        "par_",
    ] {
        assert!(
            !prover.contains(forbidden),
            "forbidden exact prover source: {forbidden}"
        );
    }
    assert!(parent.lines().count() <= 3_000 && parent.len() <= 120 * 1024);
    let child = include_str!("bulletproof_t256_streaming_constraint_tests.rs");
    assert!(child.lines().count() <= 500 && child.len() <= 24 * 1024);
}
