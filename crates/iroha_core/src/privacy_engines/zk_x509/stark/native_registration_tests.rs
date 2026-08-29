// Lexically included by `zk_x509::stark::tests` to preserve the existing libtest paths.
#[test]
fn der_reduced_retained_commitment_openings_and_deep_are_byte_exact() {
    let native_columns = (0_u64..3)
        .map(|column| {
            (0_u64..8)
                .map(|row| F::reduce(u128::from(column + 13) * u128::from(row + 29) + 7))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let initial_indices = [1, 9, 31];
    let mut replayable_rng = StdRng::from_seed([0xE1; 32]);
    let (replayable_commitment, masks) = aggregate::commit_masked_trace_columns_v1(
        b"zk-x509-der-reduced-retained-leaf",
        b"zk-x509-der-reduced-retained-node",
        0,
        3,
        6,
        native_columns.len(),
        3,
        &initial_indices,
        &mut replayable_rng,
        |column| Ok(native_columns[column].clone()),
    )
    .expect("replayable reduced commitment");
    let mut retained_rng = StdRng::from_seed([0xE1; 32]);
    let (retained_commitment, polynomials) = aggregate::commit_masked_trace_polynomial_columns_v1(
        b"zk-x509-der-reduced-retained-leaf",
        b"zk-x509-der-reduced-retained-node",
        0,
        3,
        6,
        native_columns.len(),
        3,
        &initial_indices,
        &mut retained_rng,
        |column| Ok(native_columns[column].clone()),
    )
    .expect("retained reduced commitment");
    assert_eq!(retained_commitment, replayable_commitment);
    let replay_indices = [0, 7, 17, 63];
    let replayable_openings = aggregate::replay_masked_trace_columns_v1(
        b"zk-x509-der-reduced-retained-leaf",
        b"zk-x509-der-reduced-retained-node",
        0,
        &masks,
        &replay_indices,
        |column| Ok(native_columns[column].clone()),
    )
    .expect("replayable reduced openings");
    let retained_openings = aggregate::replay_masked_trace_polynomial_columns_v1(
        b"zk-x509-der-reduced-retained-leaf",
        b"zk-x509-der-reduced-retained-node",
        0,
        &polynomials,
        &replay_indices,
    )
    .expect("retained reduced openings");
    assert_eq!(retained_openings, replayable_openings);
    let deep_point = E::canonical([101, 3, 5, 7]).expect("DEEP point");
    let replayable_deep =
        aggregate::evaluate_masked_native_columns_at_deep_v1(&masks, deep_point, |column| {
            Ok(native_columns[column].clone())
        })
        .expect("replayable DEEP");
    let retained_deep =
        aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(&polynomials, deep_point)
            .expect("retained DEEP");
    assert_eq!(retained_deep, replayable_deep);
    let mut mutated_columns = native_columns.clone();
    mutated_columns[0][0] = mutated_columns[0][0].add(F::ONE);
    let mut mutation_rng = StdRng::from_seed([0xE1; 32]);
    let (mutated_commitment, _) = aggregate::commit_masked_trace_polynomial_columns_v1(
        b"zk-x509-der-reduced-retained-leaf",
        b"zk-x509-der-reduced-retained-node",
        0,
        3,
        6,
        mutated_columns.len(),
        3,
        &initial_indices,
        &mut mutation_rng,
        |column| Ok(mutated_columns[column].clone()),
    )
    .expect("mutated retained commitment");
    assert_ne!(
        mutated_commitment.commitment.root,
        retained_commitment.commitment.root
    );
}
#[test]
fn der_reduced_deep_coefficient_accumulator_matches_pointwise_oracle_and_rejects_mutations() {
    const REDUCED_NATIVE_LOG2: u8 = 3;
    const REDUCED_LDE_LOG2: u8 = 6;
    const COEFFICIENT_CAP: usize = 16;
    fn evaluate_base(coefficients: &[F], point: E) -> E {
        coefficients
            .iter()
            .rev()
            .copied()
            .fold(E::ZERO, |value, coefficient| {
                value.mul(point).add(E::from_base(coefficient))
            })
    }
    fn evaluate_extension(coefficients: &[E], point: E) -> E {
        coefficients
            .iter()
            .rev()
            .copied()
            .fold(E::ZERO, |value, coefficient| {
                value.mul(point).add(coefficient)
            })
    }
    let base_coefficients = (0..3)
        .map(|column| {
            (0..12)
                .map(|degree| {
                    F(u64::try_from(101 + column * 29 + degree).expect("base coefficient"))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let aux_coefficients = (0..2)
        .map(|column| {
            (0..10)
                .map(
                    |degree| F(u64::try_from(401 + column * 31 + degree).expect("aux coefficient")),
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let composition_coefficients = (0..SECURITY_LANES)
        .map(|lane| {
            (0..COMPOSITION_DEGREE_CHUNKS)
                .map(|chunk| {
                    let length = 9 - chunk;
                    (0..length)
                        .map(|degree| {
                            let value = u64::try_from(701 + lane * 101 + chunk * 17 + degree)
                                .expect("composition coefficient");
                            E::canonical([value, value + 1, value + 2, value + 3])
                                .expect("canonical composition coefficient")
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        base_coefficients
            .iter()
            .chain(&aux_coefficients)
            .all(|coefficients| coefficients.len() - 1 <= COEFFICIENT_CAP)
    );
    assert!(
        composition_coefficients
            .iter()
            .flatten()
            .all(|coefficients| { coefficients.len().saturating_sub(1) <= COEFFICIENT_CAP })
    );
    let deep_point = E::canonical([1_001, 7, 11, 13]).expect("canonical DEEP point");
    let native_root = goldilocks_primitive_root_v1(REDUCED_NATIVE_LOG2).expect("native root");
    let deep_next_point = deep_point.mul_base(native_root);
    let deep_trace = aggregate::AggregateOpenedDeepTraceGroupV1 {
        base_current: base_coefficients
            .iter()
            .map(|coefficients| evaluate_base(coefficients, deep_point))
            .collect(),
        base_next: base_coefficients
            .iter()
            .map(|coefficients| evaluate_base(coefficients, deep_next_point))
            .collect(),
        aux_current: aux_coefficients
            .iter()
            .map(|coefficients| evaluate_base(coefficients, deep_point))
            .collect(),
        aux_next: aux_coefficients
            .iter()
            .map(|coefficients| evaluate_base(coefficients, deep_next_point))
            .collect(),
    };
    let deep_compositions = composition_coefficients
        .iter()
        .map(|chunks| {
            chunks
                .iter()
                .map(|coefficients| evaluate_extension(coefficients, deep_point))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        evaluate_retained_composition_coefficients_at_deep_v1(
            &composition_coefficients,
            deep_point,
        )
        .expect("retained composition DEEP values"),
        deep_compositions
    );
    let mut missing_chunk = composition_coefficients.clone();
    missing_chunk[0].pop();
    assert!(
        evaluate_retained_composition_coefficients_at_deep_v1(&missing_chunk, deep_point).is_err()
    );
    assert!(evaluate_retained_composition_coefficients_at_deep_v1(&[], deep_point).is_err());
    let mixes = (0..SECURITY_LANES)
        .map(|lane| FriMixV1 {
            base: (0..base_coefficients.len())
                .map(|column| {
                    extension_v1(F(
                        u64::try_from(2_001 + lane * 101 + column).expect("base mix")
                    ))
                })
                .collect(),
            base_next: (0..base_coefficients.len())
                .map(|column| {
                    extension_v1(F(
                        u64::try_from(2_201 + lane * 101 + column).expect("next base mix")
                    ))
                })
                .collect(),
            aux: (0..aux_coefficients.len())
                .map(|column| {
                    extension_v1(F(
                        u64::try_from(2_401 + lane * 101 + column).expect("aux mix")
                    ))
                })
                .collect(),
            aux_next: (0..aux_coefficients.len())
                .map(|column| {
                    extension_v1(F(
                        u64::try_from(2_601 + lane * 101 + column).expect("next aux mix")
                    ))
                })
                .collect(),
            composition: (0..COMPOSITION_DEGREE_CHUNKS)
                .map(|chunk| {
                    extension_v1(F(
                        u64::try_from(2_801 + lane * 101 + chunk).expect("composition mix")
                    ))
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let accumulated_coefficients = (0..SECURITY_LANES)
        .map(|lane| {
            let mut accumulator = vec![E::ZERO; COEFFICIENT_CAP];
            for (column, coefficients) in base_coefficients.iter().enumerate() {
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_point,
                    deep_trace.base_current[column],
                    mixes[lane].base[column],
                    &mut accumulator,
                )
                .expect("base current coefficient quotient");
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_next_point,
                    deep_trace.base_next[column],
                    mixes[lane].base_next[column],
                    &mut accumulator,
                )
                .expect("base next coefficient quotient");
            }
            for (column, coefficients) in aux_coefficients.iter().enumerate() {
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_point,
                    deep_trace.aux_current[column],
                    mixes[lane].aux[column],
                    &mut accumulator,
                )
                .expect("aux current coefficient quotient");
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_next_point,
                    deep_trace.aux_next[column],
                    mixes[lane].aux_next[column],
                    &mut accumulator,
                )
                .expect("aux next coefficient quotient");
            }
            for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
                accumulate_extension_deep_quotient_v1(
                    &composition_coefficients[lane][chunk],
                    deep_point,
                    deep_compositions[lane][chunk],
                    mixes[lane].composition[chunk],
                    &mut accumulator,
                )
                .expect("composition coefficient quotient");
            }
            accumulator
        })
        .collect::<Vec<_>>();
    let lde_size = 1_usize << REDUCED_LDE_LOG2;
    let lde_root = goldilocks_primitive_root_v1(REDUCED_LDE_LOG2).expect("LDE root");
    let accumulated_evaluations = accumulated_coefficients
        .iter()
        .map(|coefficients| {
            goldilocks_fp4_evaluate_coset_v1(
                coefficients,
                lde_size,
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .expect("coefficient-accumulated LDE")
        })
        .collect::<Vec<_>>();
    for index in 0..lde_size {
        let x =
            F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(u128::try_from(index).expect("LDE index")));
        let query_point = E::from_base(x);
        let current_inverse = query_point
            .sub(deep_point)
            .inv()
            .expect("current DEEP denominator");
        let next_inverse = query_point
            .sub(deep_next_point)
            .inv()
            .expect("next DEEP denominator");
        for lane in 0..SECURITY_LANES {
            let mut expected = E::ZERO;
            for (column, coefficients) in base_coefficients.iter().enumerate() {
                let value = evaluate_base(coefficients, query_point);
                expected = expected.add(
                    value
                        .sub(deep_trace.base_current[column])
                        .mul(current_inverse)
                        .mul(mixes[lane].base[column]),
                );
                expected = expected.add(
                    value
                        .sub(deep_trace.base_next[column])
                        .mul(next_inverse)
                        .mul(mixes[lane].base_next[column]),
                );
            }
            for (column, coefficients) in aux_coefficients.iter().enumerate() {
                let value = evaluate_base(coefficients, query_point);
                expected = expected.add(
                    value
                        .sub(deep_trace.aux_current[column])
                        .mul(current_inverse)
                        .mul(mixes[lane].aux[column]),
                );
                expected = expected.add(
                    value
                        .sub(deep_trace.aux_next[column])
                        .mul(next_inverse)
                        .mul(mixes[lane].aux_next[column]),
                );
            }
            for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
                expected = expected.add(
                    evaluate_extension(&composition_coefficients[lane][chunk], query_point)
                        .sub(deep_compositions[lane][chunk])
                        .mul(current_inverse)
                        .mul(mixes[lane].composition[chunk]),
                );
            }
            assert_eq!(
                accumulated_evaluations[lane][index], expected,
                "coefficient and pointwise DEEP-ALI paths differ at lane {lane}, row {index}"
            );
        }
    }
    let mut wrong_deep_accumulator = vec![E::ZERO; COEFFICIENT_CAP];
    assert!(matches!(
        accumulate_base_deep_quotient_v1(
            &base_coefficients[0],
            deep_point,
            deep_trace.base_current[0].add(E::ONE),
            E::ONE,
            &mut wrong_deep_accumulator,
        ),
        Err(ZkX509StarkErrorV1::ConstraintOpening)
    ));
    let mut mutated = base_coefficients[0].clone();
    mutated[0] = mutated[0].add(F::ONE);
    let mut mutation_accumulator = vec![E::ZERO; COEFFICIENT_CAP];
    assert!(matches!(
        accumulate_base_deep_quotient_v1(
            &mutated,
            deep_point,
            deep_trace.base_current[0],
            E::ONE,
            &mut mutation_accumulator,
        ),
        Err(ZkX509StarkErrorV1::ConstraintOpening)
    ));
    let mut undersized = vec![E::ZERO; base_coefficients[0].len() - 2];
    assert!(matches!(
        accumulate_base_deep_quotient_v1(
            &base_coefficients[0],
            deep_point,
            deep_trace.base_current[0],
            E::ONE,
            &mut undersized,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    let oversized_extension = vec![E::ONE; COEFFICIENT_CAP + 2];
    let mut bounded = vec![E::ZERO; COEFFICIENT_CAP];
    assert!(matches!(
        accumulate_extension_deep_quotient_v1(
            &oversized_extension,
            deep_point,
            evaluate_extension(&oversized_extension, deep_point),
            E::ONE,
            &mut bounded,
        ),
        Err(ZkX509StarkErrorV1::ProfileMismatch)
    ));
    assert!(matches!(
        accumulate_extension_deep_quotient_v1(&[], deep_point, E::ONE, E::ONE, &mut bounded,),
        Err(ZkX509StarkErrorV1::ConstraintOpening)
    ));
    let before = bounded.clone();
    accumulate_extension_deep_quotient_v1(&[], deep_point, E::ZERO, E::ONE, &mut bounded)
        .expect("the zero polynomial has the zero DEEP opening");
    assert_eq!(bounded, before);
}
#[test]
#[ignore = "native log19/log22/log25 retained-coefficient proof, FRI, and RSS gate"]
fn native_log19_der_proof_roundtrips_and_rejects_cross_layer_mutations() {
    let _guard = proof_guard();
    DER_FIXED_OPENING_EVALUATIONS_V1.store(0, std::sync::atomic::Ordering::SeqCst);
    let document = [0x30, 0x03, 0x02, 0x01, 0x01];
    let private_shape = build_zk_x509_der_stark_base_v1(&[&document])
        .expect("DER base")
        .private_shape;
    private_shape.validate().expect("private DER shape");
    let shape = ZkX509DerStarkShapeV1;
    let mut rng = StdRng::from_seed([0xD3; 32]);
    let prove_started = std::time::Instant::now();
    let proof = build_zk_x509_der_segmented_stark_proof_v1_with_rng(&shape, &[&document], &mut rng)
        .expect("native log19 DER proof");
    let prove_elapsed = prove_started.elapsed();
    assert!(
        prove_elapsed <= std::time::Duration::from_secs(ZK_X509_PROVER_TARGET_SECONDS_V1),
        "reference-host DER proof exceeded the release prover target: {prove_elapsed:?}"
    );
    let digest: [u8; 32] = Sha256::digest(&proof).into();
    assert!(
        proof.len() <= 1_800_000,
        "reference log19 DER proof exceeded its 1.8 MB release ceiling"
    );
    assert_eq!(
        proof.len(),
        1_752_584,
        "update only when the canonical log19 DER proof wire intentionally changes"
    );
    assert_eq!(
        hex::encode(digest),
        "954900070c22680460d07bc35969e1e71c8cf9f7e089d7b3abf97406b3e3df15",
        "update only when the canonical log19 DER proof protocol intentionally changes"
    );
    let first_verify_started = std::time::Instant::now();
    verify_zk_x509_der_segmented_stark_v1(&shape, &proof)
        .expect("native log19 DER first verification");
    let first_verify_elapsed = first_verify_started.elapsed();
    let repeat_verify_started = std::time::Instant::now();
    verify_zk_x509_der_segmented_stark_v1(&shape.clone(), &proof)
        .expect("native log19 DER repeat verification");
    let repeat_verify_elapsed = repeat_verify_started.elapsed();
    assert!(
        first_verify_elapsed <= std::time::Duration::from_secs(15),
        "reference-host first verification exceeded the 15-second release ceiling: \
             {first_verify_elapsed:?}"
    );
    assert!(
        repeat_verify_elapsed <= std::time::Duration::from_secs(15),
        "reference-host repeat verification exceeded the 15-second release ceiling: \
             {repeat_verify_elapsed:?}"
    );
    let fixed_evaluations_after_valid =
        DER_FIXED_OPENING_EVALUATIONS_V1.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(fixed_evaluations_after_valid, 2);
    eprintln!(
        "native log19 DER proof bytes={}, sha256={}, prove={prove_elapsed:?}, \
             first_verify={first_verify_elapsed:?}, repeat_verify={repeat_verify_elapsed:?}",
        proof.len(),
        hex::encode(digest)
    );
    assert_eq!(&proof[..4], b"X5P1");
    let (claims, aggregate_bytes) =
        decode_der_segmented_proof_envelope_v1(&proof).expect("DER envelope");
    let layout = SegmentLayoutV1::for_der(shape.active_rows()).expect("DER layout");
    let aggregate_layout =
        AggregateProofLayoutV1::for_segments(&[layout]).expect("aggregate layout");
    let aggregate = decode_zk_x509_segmented_stark_proof_v1(aggregate_bytes, &aggregate_layout)
        .expect("inner aggregate proof");
    let mut trailing = proof.clone();
    trailing.push(0);
    assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &trailing).is_err());
    let mut draft_magic = proof.clone();
    draft_magic[..4].copy_from_slice(b"X5D2");
    assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &draft_magic).is_err());
    let mut mistyped_claim = proof.clone();
    mistyped_claim[12] ^= 1;
    assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &mistyped_claim).is_err());
    let mut wrong_lane = proof.clone();
    wrong_lane[14] ^= 1;
    assert!(verify_zk_x509_der_segmented_stark_v1(&shape, &wrong_lane).is_err());
    for claim_index in 0..DER_PROOF_CLAIM_COUNT_V1 {
        let mut changed_claims = claims;
        if claim_index < ZK_X509_DER_STARK_BUS_LANES_V1 {
            changed_claims.input_byte[claim_index] =
                changed_claims.input_byte[claim_index].add(F::ONE);
        } else {
            changed_claims.node[claim_index - ZK_X509_DER_STARK_BUS_LANES_V1] =
                changed_claims.node[claim_index - ZK_X509_DER_STARK_BUS_LANES_V1].add(F::ONE);
        }
        let changed = encode_der_segmented_proof_envelope_v1(changed_claims, aggregate_bytes)
            .expect("mutated claim envelope");
        assert!(
            verify_zk_x509_der_segmented_stark_v1(&shape, &changed).is_err(),
            "terminal claim {claim_index} must be transcript- and AIR-bound"
        );
    }
    let mut changed_private_shape = private_shape.clone();
    changed_private_shape.parser_rows += 1;
    assert_ne!(changed_private_shape, private_shape);
    assert_eq!(
        der_public_digest_v1(&shape).expect("fixed public digest"),
        der_public_digest_v1(&ZkX509DerStarkShapeV1).expect("fixed public digest")
    );
    let mut mutations = Vec::new();
    let mut changed = aggregate.clone();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].base_root);
    mutations.push(changed);
    changed = aggregate.clone();
    mutate_stark_digest_v1(&mut changed.trace_groups[0].aux_root);
    mutations.push(changed);
    changed = aggregate.clone();
    mutate_stark_digest_v1(&mut changed.composition_roots[0]);
    mutations.push(changed);
    changed = aggregate.clone();
    mutate_stark_digest_v1(&mut changed.fri_lanes[0].roots[0]);
    mutations.push(changed);
    changed = aggregate.clone();
    changed.fri_lanes[0].terminal_values[0][0] ^= 1;
    mutations.push(changed);
    changed = aggregate.clone();
    changed.queries[0].trace_groups[0].base_current[0] ^= 1;
    mutations.push(changed);
    changed = aggregate.clone();
    changed.queries[0].trace_groups[0].aux_next[0] ^= 1;
    mutations.push(changed);
    changed = aggregate.clone();
    changed.queries[0].composition_values[0][0][0] ^= 1;
    mutations.push(changed);
    changed = aggregate.clone();
    changed.queries[0].fri_lanes[0].rounds[0].low[0] ^= 1;
    mutations.push(changed);
    changed = aggregate;
    changed.grinding_nonce ^= 1;
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        let inner = encode_zk_x509_segmented_stark_proof_v1(mutation, &aggregate_layout)
            .expect("mutated inner proof");
        let outer =
            encode_der_segmented_proof_envelope_v1(claims, &inner).expect("mutated outer proof");
        assert!(
            verify_zk_x509_der_segmented_stark_v1(&shape, &outer).is_err(),
            "aggregate layer mutation {index} must reject"
        );
    }
    assert_eq!(
        DER_FIXED_OPENING_EVALUATIONS_V1.load(std::sync::atomic::Ordering::SeqCst),
        fixed_evaluations_after_valid,
        "all malformed/transcript/Merkle mutations must reject before sampled fixed evaluation"
    );
}
#[test]
fn projection_registration_is_exact_and_every_range_mutation_fails_closed() {
    let layout = projection_aggregate_layout();
    layout
        .validate()
        .expect("canonical projection registration");
    assert!(layout.validate_full_profile_registration().is_err());
    assert_eq!(layout.common_lde_log2, 15 + BLOWUP_LOG2);
    assert_eq!(layout.trace_groups.len(), 1);
    assert_eq!(layout.trace_groups[0].native_trace_log2, 15);
    assert_eq!(layout.trace_groups[0].column_chunks, 1);
    assert_eq!(
        layout.trace_groups[0].base_width,
        ZK_X509_PROJECTION_BASE_WIDTH_V1
    );
    assert_eq!(
        layout.trace_groups[0].aux_width,
        ZK_X509_PROJECTION_AUX_WIDTH_V1
    );
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Projection, 0)
        .expect("projection registration");
    assert_eq!(registration.trace_group, 0);
    assert_eq!(registration.base_start, 0);
    assert_eq!(
        registration.base_end().expect("base end"),
        ZK_X509_PROJECTION_BASE_WIDTH_V1
    );
    assert_eq!(registration.aux_start, 0);
    assert_eq!(
        registration.aux_end().expect("aux end"),
        ZK_X509_PROJECTION_AUX_WIDTH_V1
    );
    assert_eq!(
        registration.segment.active_rows,
        ZK_X509_PROJECTION_TRACE_SIZE_V1
    );
    assert_eq!(registration.segment.trace_log2, 15);
    assert_eq!(registration.segment.lde_log2, 15 + BLOWUP_LOG2);
    assert_eq!(
        registration.segment.fixed_width,
        ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1
    );
    assert_eq!(
        registration.segment.constraint_count,
        ZK_X509_PROJECTION_STARK_CONSTRAINT_COUNT_V1
    );
    assert_eq!(
        registration.segment.constraint_degree,
        ZK_X509_PROJECTION_STARK_CONSTRAINT_DEGREE_V1
    );
    let mut mutations = Vec::new();
    let mut changed = layout.clone();
    changed.registered_segments.clear();
    mutations.push(changed);
    changed = layout.clone();
    changed
        .registered_segments
        .push(changed.registered_segments[0]);
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::ByteMemory;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.instance = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.trace_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.lde_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.base_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.aux_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.fixed_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.constraint_count -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.constraint_degree += 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].trace_group = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].base_start = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].aux_start = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].column_chunks += 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].native_trace_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].base_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].aux_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].column_chunks += 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.common_lde_log2 -= 1;
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        assert!(
            mutation.validate().is_err(),
            "projection registration mutation {index} must fail closed"
        );
    }
}
#[test]
fn sha_batch_registration_is_exact_and_every_range_mutation_fails_closed() {
    let layout = sha_word_aggregate_layout();
    layout.validate().expect("canonical SHA-word registration");
    assert!(layout.validate_full_profile_registration().is_err());
    assert_eq!(layout.common_lde_log2, 25);
    assert_eq!(layout.trace_groups.len(), 1);
    assert_eq!(layout.trace_groups[0].native_trace_log2, 19);
    assert_eq!(
        layout.trace_groups[0].column_chunks,
        ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1
    );
    assert_eq!(
        layout.trace_groups[0].base_width,
        ZK_X509_SHA_BATCH_BASE_WIDTH_V1
    );
    assert_eq!(
        layout.trace_groups[0].aux_width,
        ZK_X509_SHA_BATCH_AUX_WIDTH_V1
    );
    let registration = layout
        .registered_segment(SegmentAdapterIdV1::Sha256CallBus, 0)
        .expect("SHA batch registration");
    assert_eq!(registration.trace_group, 0);
    assert_eq!(registration.base_start, 0);
    assert_eq!(
        registration.base_end().expect("base end"),
        ZK_X509_SHA_BATCH_BASE_WIDTH_V1
    );
    assert_eq!(registration.aux_start, 0);
    assert_eq!(
        registration.aux_end().expect("aux end"),
        ZK_X509_SHA_BATCH_AUX_WIDTH_V1
    );
    assert_eq!(
        registration.segment.active_rows,
        ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0]
    );
    assert_eq!(
        registration.segment.trace_log2,
        ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
    );
    assert_eq!(
        registration.segment.lde_log2,
        ZK_X509_MAIN_COMMON_LDE_LOG2_V1
    );
    assert_eq!(
        registration.segment.fixed_width,
        ZK_X509_SHA_BATCH_FIXED_WIDTH_V1
    );
    assert_eq!(
        registration.segment.constraint_count,
        ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
    );
    assert_eq!(
        registration.segment.constraint_degree,
        ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1
    );
    let mut mutations = Vec::new();
    let mut changed = layout.clone();
    changed.registered_segments.clear();
    mutations.push(changed);
    changed = layout.clone();
    changed
        .registered_segments
        .push(changed.registered_segments[0]);
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.adapter = SegmentAdapterIdV1::StrictDer;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.instance =
        u16::try_from(ZK_X509_SHA_SEGMENT_COUNT_V1).expect("small segment count");
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.trace_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.lde_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.lde_log2 = 22;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.base_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.aux_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.fixed_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.constraint_count -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].segment.constraint_degree -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].trace_group = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].base_start = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].aux_start = 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.registered_segments[0].column_chunks += 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].base_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].aux_width -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.trace_groups[0].column_chunks += 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.common_lde_log2 -= 1;
    mutations.push(changed);
    changed = layout.clone();
    changed.common_lde_log2 = 22;
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        assert!(
            mutation.validate().is_err(),
            "SHA-word registration mutation {index} must fail closed"
        );
    }
}
#[test]
fn p256_role_registrations_are_exact_minimal_and_bucketed_by_native_log() {
    let wallet = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::WalletOwnership)
        .expect("wallet P-256 registration");
    let certificate = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::CertificateOrCrl)
        .expect("certificate P-256 registration");
    wallet
        .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
        .expect("canonical wallet registration");
    certificate
        .validate_p256_registration_v1(P256EcdsaRoleV1::CertificateOrCrl)
        .expect("canonical certificate registration");
    assert_eq!(
        wallet.registered_segments.len(),
        P256_WALLET_REGISTRATION_COUNT_V1
    );
    assert_eq!(
        certificate.registered_segments.len(),
        P256_CERTIFICATE_REGISTRATION_COUNT_V1
    );
    assert_eq!(wallet.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
    assert_eq!(certificate.common_lde_log2, ZK_X509_MAIN_COMMON_LDE_LOG2_V1);
    assert_eq!(wallet.trace_groups.len(), 4);
    assert_eq!(certificate.trace_groups.len(), 4);
    assert!(
        wallet
            .registered_segments
            .iter()
            .any(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256LowS)
    );
    assert!(
        !certificate
            .registered_segments
            .iter()
            .any(|registration| registration.segment.adapter == SegmentAdapterIdV1::P256LowS)
    );
    let wallet_instance =
        |local| p256_instance_v1(P256_SIGNATURE_COUNT_V1 - 1, local).expect("wallet instance");
    let expected = [
        (
            SegmentAdapterIdV1::P256Reduction,
            wallet_instance(0),
            P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1,
            P256_REDUCTION_BASE_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
            P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
            4,
            1,
        ),
        (
            SegmentAdapterIdV1::P256Reduction,
            wallet_instance(1),
            P256_REDUCTION_AGGREGATE_TRACE_LOG2_V1,
            P256_REDUCTION_BASE_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1,
            P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1,
            P256_REDUCTION_REGISTERED_CONSTRAINT_COUNT_V1,
            4,
            1,
        ),
        (
            SegmentAdapterIdV1::P256LowS,
            wallet_instance(0),
            P256_LOW_S_AGGREGATE_TRACE_LOG2_V1,
            P256_LOW_S_BASE_WIDTH_V1,
            P256_LOW_S_AGGREGATE_AUX_WIDTH_V1,
            P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1,
            P256_LOW_S_REGISTERED_CONSTRAINT_COUNT_V1,
            3,
            1,
        ),
        (
            SegmentAdapterIdV1::P256ScalarBitBus,
            wallet_instance(0),
            P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1,
            P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1,
            P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1,
            P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1,
            P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1,
            3,
            1,
        ),
        (
            SegmentAdapterIdV1::P256Window,
            wallet_instance(0),
            P256_WINDOW_AGGREGATE_TRACE_LOG2_V1,
            P256_WINDOW_BASE_WIDTH_V1,
            P256_WINDOW_AGGREGATE_AUX_WIDTH_V1,
            P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1,
            P256_WINDOW_REGISTERED_CONSTRAINT_COUNT_V1,
            4,
            1,
        ),
        (
            SegmentAdapterIdV1::P256ValueBus,
            wallet_instance(2),
            P256_BINDING_SINK_AGGREGATE_TRACE_LOG2_V1,
            P256_BINDING_SINK_BASE_WIDTH_V1,
            super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
            P256_BINDING_SINK_FIXED_WIDTH_V1,
            P256_BINDING_SINK_REGISTERED_CONSTRAINT_COUNT_V1,
            2,
            1,
        ),
        (
            SegmentAdapterIdV1::P256Arithmetic,
            wallet_instance(0),
            P256_ARITHMETIC_AGGREGATE_TRACE_LOG2_V1,
            P256_ARITHMETIC_BASE_WIDTH_V1,
            P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1,
            P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
            P256_ARITHMETIC_REGISTERED_CONSTRAINT_COUNT_V1,
            4,
            4,
        ),
        (
            SegmentAdapterIdV1::P256ValueBus,
            wallet_instance(0),
            P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
            P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
            P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1,
            P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
            P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1,
            3,
            2,
        ),
        (
            SegmentAdapterIdV1::P256ValueBus,
            wallet_instance(1),
            P256_VALUE_BUS_AGGREGATE_TRACE_LOG2_V1,
            P256_VALUE_BUS_STARK_BASE_WIDTH_V1,
            P256_VALUE_BUS_STARK_AUX_WIDTH_V1,
            P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
            P256_VALUE_SORTED_REGISTERED_CONSTRAINT_COUNT_V1,
            2,
            1,
        ),
    ];
    let expected_group = [0, 0, 0, 1, 2, 2, 3, 3, 3];
    let expected_base_start = [0, 56, 112, 0, 0, 61, 0, 211, 245];
    let expected_aux_start = [0, 19, 38, 0, 0, 37, 0, 72, 188];
    for (index, expected) in expected.into_iter().enumerate() {
        let registration = wallet.registered_segments[index];
        assert_eq!(
            (
                registration.segment.adapter,
                registration.segment.instance,
                registration.segment.trace_log2,
                registration.segment.base_width,
                registration.segment.aux_width,
                registration.segment.fixed_width,
                registration.segment.constraint_count,
                registration.segment.constraint_degree,
                registration.column_chunks,
            ),
            expected
        );
        assert_eq!(registration.trace_group, expected_group[index]);
        assert_eq!(registration.base_start, expected_base_start[index]);
        assert_eq!(registration.aux_start, expected_aux_start[index]);
    }
    assert_eq!(
        wallet.trace_groups,
        vec![
            TraceGroupLayoutV1 {
                native_trace_log2: 5,
                column_chunks: 3,
                base_width: 148,
                aux_width: 52,
            },
            TraceGroupLayoutV1 {
                native_trace_log2: 8,
                column_chunks: 1,
                base_width: 6,
                aux_width: 32,
            },
            TraceGroupLayoutV1 {
                native_trace_log2: 16,
                column_chunks: 2,
                base_width: P256_WINDOW_BASE_WIDTH_V1 + P256_BINDING_SINK_BASE_WIDTH_V1,
                aux_width: P256_WINDOW_AGGREGATE_AUX_WIDTH_V1
                    + super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1,
            },
            TraceGroupLayoutV1 {
                native_trace_log2: 19,
                column_chunks: 7,
                base_width: 279,
                aux_width: 200,
            },
        ]
    );
    assert_eq!(
        certificate.trace_groups[0],
        TraceGroupLayoutV1 {
            native_trace_log2: 5,
            column_chunks: 2,
            base_width: 112,
            aux_width: 38,
        }
    );
    assert_eq!(
        P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1, 222,
        "value execution binds value, arithmetic-copy, and writer terminals"
    );
    assert!(
        wallet
            .validate_p256_registration_v1(P256EcdsaRoleV1::CertificateOrCrl)
            .is_err()
    );
    assert!(
        certificate
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .is_err()
    );
}
#[test]
fn p256_registration_rejects_omission_reorder_duplicate_reuse_and_splice() {
    let segments =
        canonical_p256_segment_layouts_v1(P256EcdsaRoleV1::WalletOwnership).expect("segments");
    let layout = AggregateProofLayoutV1::for_p256_v1(P256EcdsaRoleV1::WalletOwnership)
        .expect("wallet layout");
    for omitted in 0..segments.len() {
        let subset = segments
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, segment)| (index != omitted).then_some(segment))
            .collect::<Vec<_>>();
        let missing = AggregateProofLayoutV1::for_equal_log_buckets_v1(&subset)
            .expect("structural equal-log P-256 subset");
        assert!(
            missing
                .validate_p256_instance_set_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err()
        );
    }
    let mut duplicate = layout.clone();
    duplicate
        .registered_segments
        .push(duplicate.registered_segments[0]);
    assert!(
        duplicate
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .is_err()
    );
    let mut reordered = layout.clone();
    reordered.registered_segments.swap(0, 1);
    assert!(
        reordered
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .is_err()
    );
    let mut equal_shape_group_reuse = layout.clone();
    equal_shape_group_reuse.registered_segments[0].base_start =
        equal_shape_group_reuse.registered_segments[1].base_start;
    equal_shape_group_reuse.registered_segments[0].aux_start =
        equal_shape_group_reuse.registered_segments[1].aux_start;
    assert!(equal_shape_group_reuse.validate().is_err());
    assert!(
        equal_shape_group_reuse
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .is_err()
    );
    let mut cross_identity_splice = layout.clone();
    cross_identity_splice.registered_segments.swap(4, 5);
    assert!(cross_identity_splice.validate().is_err());
    assert!(
        cross_identity_splice
            .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
            .is_err()
    );
    let mut mutations = Vec::new();
    for registration_index in 0..layout.registered_segments.len() {
        for field in 0..10 {
            let mut changed = layout.clone();
            let segment = &mut changed.registered_segments[registration_index].segment;
            match field {
                0 => segment.adapter = SegmentAdapterIdV1::Projection,
                1 => segment.instance = segment.instance.saturating_add(7),
                2 => segment.active_rows -= 1,
                3 => segment.trace_log2 -= 1,
                4 => segment.lde_log2 -= 1,
                5 => segment.base_width -= 1,
                6 => segment.aux_width -= 1,
                7 => segment.fixed_width -= 1,
                8 => segment.constraint_count -= 1,
                9 => segment.constraint_degree = segment.constraint_degree.saturating_sub(1),
                _ => unreachable!(),
            }
            mutations.push(changed);
        }
        let mut changed = layout.clone();
        changed.registered_segments[registration_index].base_start = 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[registration_index].aux_start = 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        changed.registered_segments[registration_index].column_chunks += 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        let group_index = changed.registered_segments[registration_index].trace_group;
        changed.trace_groups[group_index].native_trace_log2 -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        let group_index = changed.registered_segments[registration_index].trace_group;
        changed.trace_groups[group_index].base_width -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        let group_index = changed.registered_segments[registration_index].trace_group;
        changed.trace_groups[group_index].aux_width -= 1;
        mutations.push(changed);
        let mut changed = layout.clone();
        let group_index = changed.registered_segments[registration_index].trace_group;
        changed.trace_groups[group_index].column_chunks += 1;
        mutations.push(changed);
    }
    let mut changed = layout;
    changed.common_lde_log2 -= 1;
    mutations.push(changed);
    for (index, mutation) in mutations.iter().enumerate() {
        assert!(
            mutation
                .validate_p256_registration_v1(P256EcdsaRoleV1::WalletOwnership)
                .is_err(),
            "P-256 layout mutation {index} must fail closed"
        );
    }
}
#[test]
fn p256_terminal_registration_and_transcript_reject_all_role_mutations() {
    let role = P256EcdsaRoleV1::WalletOwnership;
    let registration = P256TraceRegistrationV1::new_v1(role).expect("P-256 registration");
    let terminals = p256_terminal_fixture(role);
    let challenge = |registration: &P256TraceRegistrationV1,
                     terminals: &P256TerminalRegistrationV1| {
        let mut transcript = TransparentTranscriptV1::new(
            ZK_X509_DIGEST_CONTEXT_V1,
            b"p256-registration-test",
            &test_stark_digest_v1(0x91),
            &test_stark_digest_v1(0x37),
        )
        .expect("registration transcript");
        absorb_p256_registration_v1(&mut transcript, registration)
            .expect("static P-256 registration");
        absorb_p256_terminal_registration_v1(&mut transcript, registration.role, terminals)
            .expect("terminal registration");
        transcript
            .challenge_field(b"p256-registration-test-challenge-v1")
            .expect("registration challenge")
    };
    let canonical = challenge(&registration, &terminals);
    assert_eq!(
        challenge(&registration, &terminals),
        canonical,
        "registration transcript must replay exactly"
    );
    for index in 0..terminals.cross_sources.len() {
        for coordinate in 0..2 {
            let mut forged = terminals.clone();
            if coordinate == 0 {
                forged.cross_sources[index].start[0] =
                    forged.cross_sources[index].start[0].add(F::ONE);
            } else {
                forged.cross_sources[index].terminal[1] =
                    forged.cross_sources[index].terminal[1].add(F::ONE);
            }
            assert!(forged.validate(role).is_err());
            let mut transcript = TransparentTranscriptV1::new(
                ZK_X509_DIGEST_CONTEXT_V1,
                b"p256-registration-test",
                &test_stark_digest_v1(0x91),
                &test_stark_digest_v1(0x37),
            )
            .expect("registration transcript");
            absorb_p256_registration_v1(&mut transcript, &registration)
                .expect("static registration");
            assert!(absorb_p256_terminal_registration_v1(&mut transcript, role, &forged).is_err());
        }
    }
    let mut omitted = terminals.clone();
    omitted.cross_sources.pop();
    assert!(omitted.validate(role).is_err());
    let mut duplicated = terminals.clone();
    duplicated.cross_sources.push(duplicated.cross_sources[0]);
    assert!(duplicated.validate(role).is_err());
    let mut reordered = terminals.clone();
    reordered.cross_sources.swap(0, 1);
    assert!(reordered.validate(role).is_err());
    let mut zero_start = terminals.clone();
    zero_start.cross_sources[0].start = [F::ZERO; P256_CROSS_TRACE_LANES_V1];
    assert!(zero_start.validate(role).is_err());
    for bus in 0..4 {
        let mut forged = terminals.clone();
        match bus {
            0 => forged.buses.value_sorted[0] = forged.buses.value_sorted[0].add(F::ONE),
            1 => {
                forged.buses.arithmetic_value_copy[0] =
                    forged.buses.arithmetic_value_copy[0].add(F::ONE)
            }
            2 => {
                forged.buses.scalar_bus_arithmetic[0] =
                    forged.buses.scalar_bus_arithmetic[0].add(F::ONE)
            }
            3 => forged.buses.scalar_bus_window[0] = forged.buses.scalar_bus_window[0].add(F::ONE),
            _ => unreachable!(),
        }
        assert!(forged.validate(role).is_err());
    }
    let certificate = P256TraceRegistrationV1::new_v1(P256EcdsaRoleV1::CertificateOrCrl)
        .expect("certificate registration");
    let certificate_terminals = p256_terminal_fixture(P256EcdsaRoleV1::CertificateOrCrl);
    assert_ne!(
        canonical,
        challenge(&certificate, &certificate_terminals),
        "role, segment multiplicity, and terminal-role count are transcript-bound"
    );
    assert_ne!(p256_aggregate_challenges_fixture().value, {
        let mut changed = TransparentTranscriptV1::new(
            ZK_X509_DIGEST_CONTEXT_V1,
            b"p256-aggregate-test",
            &test_stark_digest_v1(0x31),
            &test_stark_digest_v1(0x57),
        )
        .expect("P-256 aggregate transcript");
        changed
            .absorb(b"adversarial-prefix", &[b"role-splice"])
            .expect("prefix mutation");
        derive_p256_aggregate_challenges_v1(&mut changed)
            .expect("mutated challenges")
            .value
    });
}
#[test]
fn value_execution_writer_terminal_is_now_bound_under_coordinated_forgery() {
    let role = P256EcdsaRoleV1::WalletOwnership;
    let terminals = zero_p256_terminal_fixture(role);
    terminals
        .validate(role)
        .expect("zero-valued but consistently chained terminal fixture");
    let mut forged = terminals.clone();
    forged.cross_sources[0].terminal[0] = F::ONE;
    forged.cross_sources[1].start[0] = F::ONE;
    forged
        .validate(role)
        .expect("coordinated host-side chain remains equal");
    let layout = AggregateProofLayoutV1::for_p256_v1(role).expect("wallet layout");
    let registration = layout
        .registered_segment(
            SegmentAdapterIdV1::P256ValueBus,
            p256_instance_v1(P256_SIGNATURE_COUNT_V1 - 1, 0).expect("wallet instance"),
        )
        .expect("value-execution registration");
    let base_current = [F::ZERO; P256_VALUE_BUS_STARK_BASE_WIDTH_V1];
    let base_next = [F::ZERO; P256_VALUE_BUS_STARK_BASE_WIDTH_V1];
    let aux_current = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1];
    let aux_next = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1];
    let opening = RegisteredOpenedRowsV1 {
        base_current: &base_current,
        base_next: &base_next,
        aux_current: &aux_current,
        aux_next: &aux_next,
    };
    let mut fixed = [F::ZERO; P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1];
    let last_selector_column = (0..fixed.len())
        .find(|column| {
            fixed[*column] = F::ONE;
            let selected = p256_value_execution_last_selector_v1(&fixed) == F::ONE;
            fixed[*column] = F::ZERO;
            selected
        })
        .expect("verifier-derived value-execution last selector");
    fixed[last_selector_column] = F::ONE;
    let canonical = p256_opened_residues_v1(
        registration,
        opening,
        &fixed,
        p256_aggregate_challenges_fixture(),
        &terminals,
    )
    .expect("canonical value-execution residues");
    let forged_residues = p256_opened_residues_v1(
        registration,
        opening,
        &fixed,
        p256_aggregate_challenges_fixture(),
        &forged,
    )
    .expect("forged value-execution residues");
    assert_eq!(
        canonical.len(),
        P256_VALUE_EXECUTION_REGISTERED_CONSTRAINT_COUNT_V1
    );
    assert!(
        canonical[P256_VALUE_EXECUTION_AGGREGATE_CONSTRAINT_COUNT_V1..]
            .iter()
            .all(|residue| *residue == F::ZERO)
    );
    let writer_terminal = &forged_residues[forged_residues.len() - P256_CROSS_TRACE_LANES_V1..];
    assert_ne!(writer_terminal[0], F::ZERO);
    assert!(
        writer_terminal[1..]
            .iter()
            .all(|residue| *residue == F::ZERO)
    );
    fixed[last_selector_column] = F::ZERO;
    let nongated = p256_opened_residues_v1(
        registration,
        opening,
        &fixed,
        p256_aggregate_challenges_fixture(),
        &forged,
    )
    .expect("nonterminal value-execution residues");
    assert!(
        nongated[P256_VALUE_EXECUTION_AGGREGATE_CONSTRAINT_COUNT_V1..]
            .iter()
            .all(|residue| *residue == F::ZERO)
    );
}
#[test]
fn p256_opened_material_and_evaluator_reject_every_shape_boundary() {
    fn evaluate(
        material: &P256OpenedMaterialV1,
        challenges: P256AggregateChallengesV1,
        alphas: &[Vec<Vec<E>>],
        mixes: &[Vec<FriMixV1>],
        lde_root: F,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        let mut evaluator = P256OpenedRowEvaluatorV1 {
            material,
            challenges,
            alphas,
            mixes,
            lde_root,
        };
        aggregate::AggregateOpenedRowEvaluatorV1::evaluate_opened_row_v1(
            &mut evaluator,
            query_index,
            lane,
            trace_groups,
            &[E::ZERO; COMPOSITION_DEGREE_CHUNKS],
        )
    }
    let role = P256EcdsaRoleV1::WalletOwnership;
    let registration = P256TraceRegistrationV1::new_v1(role).expect("P-256 registration");
    let query_index = 17;
    let fixed_openings = registration
        .layout
        .registered_segments
        .iter()
        .map(|registered| {
            BTreeMap::from([(query_index, vec![F::ZERO; registered.segment.fixed_width])])
        })
        .collect::<Vec<_>>();
    let material = P256OpenedMaterialV1 {
        registration,
        terminals: zero_p256_terminal_fixture(role),
        fixed_openings,
    };
    material.validate().expect("canonical opened material");
    let trace_groups = material
        .registration
        .layout
        .trace_groups
        .iter()
        .map(|group| aggregate::AggregateOpenedTraceGroupV1 {
            base_current: vec![F::ZERO; group.base_width],
            base_next: vec![F::ZERO; group.base_width],
            aux_current: vec![F::ZERO; group.aux_width],
            aux_next: vec![F::ZERO; group.aux_width],
        })
        .collect::<Vec<_>>();
    let alphas = material
        .registration
        .layout
        .registered_segments
        .iter()
        .map(|registered| {
            (0..SECURITY_LANES)
                .map(|_| vec![E::ONE; registered.segment.constraint_count])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mixes = material
        .registration
        .layout
        .trace_groups
        .iter()
        .map(|group| {
            (0..SECURITY_LANES)
                .map(|_| FriMixV1 {
                    base: vec![E::ONE; group.base_width],
                    base_next: vec![E::ONE; group.base_width],
                    aux: vec![E::ONE; group.aux_width],
                    aux_next: vec![E::ONE; group.aux_width],
                    composition: vec![E::ONE; COMPOSITION_DEGREE_CHUNKS],
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let challenges = p256_aggregate_challenges_fixture();
    let lde_root = goldilocks_primitive_root_v1(material.registration.layout.common_lde_log2)
        .expect("common LDE root");
    evaluate(
        &material,
        challenges,
        &alphas,
        &mixes,
        lde_root,
        query_index,
        0,
        &trace_groups,
    )
    .expect("well-shaped opened evaluation");
    let mut missing_fixed = material.clone();
    missing_fixed.fixed_openings[0].clear();
    assert!(missing_fixed.validate().is_err());
    let mut short_fixed = material.clone();
    short_fixed.fixed_openings[0]
        .get_mut(&query_index)
        .expect("fixed opening")
        .pop();
    assert!(short_fixed.validate().is_err());
    let mut missing_registration = material.clone();
    missing_registration.fixed_openings.pop();
    assert!(missing_registration.validate().is_err());
    let mut forged_terminal = material.clone();
    forged_terminal.terminals.buses.value_sorted[0] =
        forged_terminal.terminals.buses.value_sorted[0].add(F::ONE);
    assert!(forged_terminal.validate().is_err());
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index + 1,
            0,
            &trace_groups,
        )
        .is_err(),
        "an uncompiled verifier-fixed opening must be rejected"
    );
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            SECURITY_LANES,
            &trace_groups,
        )
        .is_err()
    );
    let mut invalid_challenges = challenges;
    invalid_challenges.cross.lanes[0].terms[0] = F::ZERO;
    assert!(
        evaluate(
            &material,
            invalid_challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .is_err()
    );
    let mut short_groups = trace_groups.clone();
    short_groups.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &short_groups,
        )
        .is_err()
    );
    let mut short_base = trace_groups.clone();
    short_base[0].base_current.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &short_base,
        )
        .is_err()
    );
    let mut short_aux = trace_groups.clone();
    short_aux[0].aux_next.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &short_aux,
        )
        .is_err()
    );
    let mut short_alphas = alphas.clone();
    short_alphas.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &short_alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .is_err()
    );
    let mut short_residue_alphas = alphas.clone();
    short_residue_alphas[0][0].pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &short_residue_alphas,
            &mixes,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .is_err()
    );
    let mut short_mixes = mixes.clone();
    short_mixes.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &short_mixes,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .is_err()
    );
    let mut short_base_mix = mixes;
    short_base_mix[0][0].base.pop();
    assert!(
        evaluate(
            &material,
            challenges,
            &alphas,
            &short_base_mix,
            lde_root,
            query_index,
            0,
            &trace_groups,
        )
        .is_err()
    );
}
#[test]
fn retained_registration_plans_are_exact_for_all_49_and_reject_degree_domain_attacks() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().expect("canonical MAIN layout");
    assert_eq!(
        layout.registered_segments.len(),
        FULL_PROFILE_LOGICAL_REGISTRATIONS_V1
    );
    let mut group_counts = [0_usize; FULL_PROFILE_TRACE_GROUPS_V1];
    for registration in layout.registered_segments.iter().copied() {
        let segment = registration.segment;
        let plan = registered_retained_prover_plan_v1(segment, layout.common_lde_log2)
            .expect("canonical registration plan");
        let trace_rows = 1_usize << segment.trace_log2;
        let independently_derived_degree = usize::from(segment.constraint_degree)
            .checked_mul(trace_rows + MASK_DEGREE)
            .and_then(|degree| degree.checked_sub(trace_rows))
            .expect("release dimensions fit usize");
        let independently_derived_rows = independently_derived_degree
            .checked_add(1)
            .and_then(usize::checked_next_power_of_two)
            .expect("release quotient domain");
        assert_eq!(plan.maximum_quotient_degree, independently_derived_degree);
        assert_eq!(plan.quotient_coset_rows, independently_derived_rows);
        assert_eq!(
            plan.quotient_coset_log2,
            u8::try_from(independently_derived_rows.ilog2()).expect("small release log")
        );
        assert_eq!(
            plan.quotient_next_stride,
            independently_derived_rows / trace_rows
        );
        assert!(plan.maximum_quotient_degree < plan.quotient_coset_rows);
        assert!(plan.quotient_coset_log2 <= layout.common_lde_log2);
        group_counts[registration.trace_group] += 1;
        let mut low_degree = segment;
        low_degree.constraint_degree = 1;
        assert!(matches!(
            registered_retained_prover_plan_v1(low_degree, layout.common_lde_log2),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        let mut excessive_degree = segment;
        excessive_degree.constraint_degree = ZK_X509_MAX_CONSTRAINT_DEGREE_V1 + 1;
        assert!(matches!(
            registered_retained_prover_plan_v1(excessive_degree, layout.common_lde_log2),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
        assert!(matches!(
            registered_retained_prover_plan_v1(segment, segment.lde_log2.saturating_sub(1),),
            Err(ZkX509StarkErrorV1::ProfileMismatch)
        ));
    }
    assert_eq!(group_counts, [11, 5, 1, 10, 1, 21]);
}
