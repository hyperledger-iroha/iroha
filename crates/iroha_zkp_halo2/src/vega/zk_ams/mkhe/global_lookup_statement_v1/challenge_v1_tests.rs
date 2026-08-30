//! Tests for the exact Phase-23 global-lookup challenge manifest.
use super::*;
fn context_v1() -> GlobalLookupContextV1 {
    GlobalLookupContextV1 {
        fixed_axes_digest: [0x11; 32],
        source_binding_digest: [0x22; 32],
        radix_range_digest: [0x33; 32],
        packing_digest: [0x44; 32],
        cross_field_digest: [0x55; 32],
        qpcs_initial_root: [0x66; 32],
    }
}
fn frames_v1() -> BoundTranscriptFramesV1 {
    BoundTranscriptFramesV1 {
        commitment_digest: [0x71; 32],
        inverse_digest: [0x72; 32],
        opening_digest: [0x75; 32],
        existing_commitments: EXISTING_ACTIVE_PLANES_V1,
        added_commitments: ADDED_PRE_Z_PLANES_V1,
        existing_inverses: EXISTING_ACTIVE_PLANES_V1,
        added_inverses: ADDED_INVERSE_PLANES_V1,
        cubic_messages: REQUIRED_CUBIC_MESSAGES_V1,
        hidden_endpoints: HIDDEN_ENDPOINTS_V1,
        multiplicity_commitments: MULTIPLICITY_COMMITMENTS_V1,
        sumcheck_mask_commitments: SUMCHECK_MASK_COMMITMENTS_V1,
        ipas: COEFFICIENT_IPAS_V1,
        table_ipas: TABLE_IPAS_V1,
        mask_ipas: MASK_IPAS_V1,
        gates: ENDPOINT_GATES_V1,
    }
}
fn seals_v1() -> BoundOwnerSealsV1 {
    BoundOwnerSealsV1 {
        source_packing_seal: SourcePackingOwnerSealV1::TestOnly,
        lookup_seal: LookupOwnerSealV1::TestOnly,
        proof_seal: ProofOwnerSealV1::TestOnly,
    }
}
fn gtilde_v1(ordinal: usize) -> [u8; CUBIC_MESSAGE_BYTES_V1] {
    let mut bytes = [0_u8; CUBIC_MESSAGE_BYTES_V1];
    for coefficient in 0..3 {
        let value = Scalar::from_u64((ordinal * 3 + coefficient + 1) as u64).to_le_bytes();
        bytes[coefficient * 32..(coefficient + 1) * 32].copy_from_slice(&value);
    }
    bytes
}
fn endpoint_v1(ordinal: usize) -> [u8; 33] {
    Point::canonical_generator()
        .expect("canonical generator")
        .mul_scalar(Scalar::from_u64((ordinal + 1) as u64))
        .to_non_identity_wire_bytes()
        .expect("non-identity endpoint")
}
#[rustfmt::skip]
fn coefficient_residual_stage_v1() -> GlobalLookupTranscriptV1<CoefficientResidualCommitmentStageV1> {
    GlobalLookupTranscriptV1::begin_v1(context_v1(), seals_v1(), frames_v1())
        .unwrap()
        .absorb_commitments_and_derive_z_v1()
        .unwrap()
        .absorb_inverses_and_derive_relation_v1()
        .unwrap()
}
fn sumcheck_stage_v1() -> GlobalLookupTranscriptV1<SumcheckStageV1> {
    coefficient_residual_stage_v1()
        .absorb_coefficient_residual_commitments_v1(
            core::array::from_fn(|ordinal| endpoint_v1(HIDDEN_ENDPOINTS_V1 + ordinal)),
            CoefficientResidualCommitmentSealV1::TestOnly,
        )
        .unwrap()
}
fn endpoint_stage_v1() -> GlobalLookupTranscriptV1<EndpointStageV1> {
    let mut stage = sumcheck_stage_v1();
    for ordinal in 0..REQUIRED_CUBIC_MESSAGES_V1 {
        stage = stage.absorb_gtilde_v1(ordinal, gtilde_v1(ordinal)).unwrap();
    }
    stage.finish_sumcheck_v1().unwrap()
}
struct TranscriptKatV1 {
    digest: [u8; 32],
    z: [u8; 32],
    tau_first: [u8; 32],
    tau_last: [u8; 32],
    kappa: [u8; 32],
    delta: [u8; 32],
    first_sumcheck: [u8; 32],
    last_sumcheck: [u8; 32],
    first_endpoint_batch: [u8; 32],
    last_endpoint_batch: [u8; 32],
    mask_batch: [u8; 32],
}
fn transcript_kat_v1() -> TranscriptKatV1 {
    let mut stage = endpoint_stage_v1();
    for ordinal in 0..HIDDEN_ENDPOINTS_V1 {
        stage = stage
            .absorb_endpoint_commitment_v1(ordinal, endpoint_v1(ordinal))
            .unwrap();
    }
    let stage = stage.derive_opening_batches_v1().unwrap();
    let kat = TranscriptKatV1 {
        digest: [0; 32],
        z: stage.challenges.z.to_le_bytes(),
        tau_first: stage.challenges.tau[0].to_le_bytes(),
        tau_last: stage.challenges.tau[13].to_le_bytes(),
        kappa: stage.challenges.kappa.to_le_bytes(),
        delta: stage.challenges.delta.to_le_bytes(),
        first_sumcheck: stage.challenges.sumcheck[0].to_le_bytes(),
        last_sumcheck: stage.challenges.sumcheck[233].to_le_bytes(),
        first_endpoint_batch: stage.challenges.endpoint_batches[0].to_le_bytes(),
        last_endpoint_batch: stage.challenges.endpoint_batches[15].to_le_bytes(),
        mask_batch: stage.challenges.mask_batch.to_le_bytes(),
    };
    TranscriptKatV1 {
        digest: stage.absorb_openings_and_finish_v1().unwrap(),
        ..kat
    }
}
fn oracle_absorb_frame_v1(state: &mut Keccak256, label: &[u8], payload: &[u8]) {
    state.update(&[0x52]);
    state.update(&(label.len() as u16).to_be_bytes());
    state.update(label);
    state.update(&(payload.len() as u64).to_be_bytes());
    state.update(payload);
}
fn oracle_absorb_purpose_v1(state: &mut Keccak256, label: &[u8], statement: u16, coordinate: u16) {
    state.update(&[0x52]);
    state.update(&(b"challenge-purpose".len() as u16).to_be_bytes());
    state.update(b"challenge-purpose");
    state.update(&((label.len() + 4) as u64).to_be_bytes());
    state.update(label);
    state.update(&statement.to_be_bytes());
    state.update(&coordinate.to_be_bytes());
}
fn oracle_challenge_v1(
    state: &mut Keccak256,
    ordinal: &mut u32,
    label: &[u8],
    statement: u16,
    coordinate: u16,
    first_accepted_attempt: u8,
) -> Scalar {
    for attempt in 0_u8..=127 {
        let mut wide = [0_u8; 64];
        for branch in 0_u8..=1 {
            let mut fork = state.fork_v1();
            fork.update(b"iroha.zk-ams.v1.phase23.global-lookup.challenge\0");
            fork.update(&ordinal.to_be_bytes());
            fork.update(&[attempt, branch]);
            oracle_absorb_purpose_v1(&mut fork, label, statement, coordinate);
            let start = usize::from(branch) * 32;
            wide[start..start + 32].copy_from_slice(&fork.finalize());
        }
        let scalar = Scalar::from_uniform_le_bytes(wide);
        if attempt >= first_accepted_attempt && !scalar.is_zero() {
            oracle_absorb_purpose_v1(state, label, statement, coordinate);
            oracle_absorb_frame_v1(state, b"challenge-ordinal", &ordinal.to_be_bytes());
            oracle_absorb_frame_v1(state, b"challenge-attempt", &[attempt]);
            oracle_absorb_frame_v1(state, b"challenge-scalar", &scalar.to_le_bytes());
            *ordinal += 1;
            return scalar;
        }
    }
    panic!("literal oracle exhausted")
}
#[test]
#[rustfmt::skip]
fn executable_coordinate_manifest_has_exact_ordinals_and_predicates() {
    for ordinal in 0..COMPLETE_CHALLENGE_ORDINAL_V1 {
        let coordinate = challenge_coordinate_v1(ordinal).expect("manifest coordinate");
        assert_eq!(coordinate.ordinal, ordinal);
        let expected = match ordinal { 0 => ChallengePredicateV1::OutsideLookupTable, 47 | 48 => ChallengePredicateV1::OutsideBooleanSet, _ => ChallengePredicateV1::Nonzero };
        assert_eq!(coordinate.predicate, expected);
    }
    assert_eq!(challenge_coordinate_v1(COMPLETE_CHALLENGE_ORDINAL_V1).err(), Some(GlobalLookupErrorV1::Order));
    let mu = challenge_coordinate_v1(32).unwrap().purpose;
    let tau_first = challenge_coordinate_v1(33).unwrap().purpose;
    let first_round = challenge_coordinate_v1(49).unwrap().purpose;
    assert_eq!((mu.label, mu.statement, mu.coordinate), (b"lookup-mu".as_slice(), u16::MAX, u16::MAX));
    assert_eq!((tau_first.label, tau_first.statement, tau_first.coordinate), (b"coefficient-tau-coordinate".as_slice(), u16::MAX, 0));
    assert_eq!((first_round.label, first_round.statement, first_round.coordinate), (b"equation-sumcheck-round".as_slice(), 0, 0));
    for (ordinal, label, statement, coordinate) in [(46, b"coefficient-tau-coordinate".as_slice(), u16::MAX, 13), (47, b"coefficient-kappa".as_slice(), u16::MAX, u16::MAX), (48, b"coefficient-delta".as_slice(), u16::MAX, u16::MAX), (62, b"equation-sumcheck-round".as_slice(), 0, 13), (63, b"equation-sumcheck-round".as_slice(), 1, 0), (245, b"group-sumcheck-round".as_slice(), 14, 0), (253, b"group-sumcheck-round".as_slice(), 14, 8), (254, b"lookup-sumcheck-round".as_slice(), 15, 0), (282, b"lookup-sumcheck-round".as_slice(), 15, 28), (283, b"evaluation-opening-batch".as_slice(), 0, u16::MAX), (298, b"evaluation-opening-batch".as_slice(), 15, u16::MAX), (299, b"mask-opening-batch".as_slice(), u16::MAX, u16::MAX)] {
        let purpose = challenge_coordinate_v1(ordinal).unwrap().purpose;
        assert_eq!((purpose.label, purpose.statement, purpose.coordinate), (label, statement, coordinate));
    }
}
#[test]
fn literal_domain_oracle_matches_and_rejections_do_not_mutate() {
    let seed = b"independent-global-lookup-challenge-oracle";
    let mut production = Keccak256::new();
    production.update(seed);
    let mut oracle = Keccak256::new();
    oracle.update(seed);
    let mut production_ordinal = 30;
    let mut oracle_ordinal = 30;
    let expected = oracle_challenge_v1(
        &mut oracle,
        &mut oracle_ordinal,
        b"lookup-alpha",
        u16::MAX,
        u16::MAX,
        0,
    );
    let actual = derive_coordinate_challenge_v1(&mut production, &mut production_ordinal).unwrap();
    assert_eq!(actual, expected);
    assert_eq!((production_ordinal, oracle_ordinal), (31, 31));
    assert_eq!(production.finalize(), oracle.finalize());
    let mut rejected = Keccak256::new();
    rejected.update(seed);
    let untouched = rejected.fork_v1().finalize();
    let mut ordinal = 7;
    assert_eq!(
        derive_challenge_with_policy_v1(
            &mut rejected,
            &mut ordinal,
            ChallengePurposeV1::scoped_v1(b"hostile-retry", 15, 6),
            |_, _| false,
        ),
        Err(GlobalLookupErrorV1::ChallengeExhausted)
    );
    assert_eq!(ordinal, 7);
    assert_eq!(rejected.finalize(), untouched);
}
#[test]
fn retry_attempt_and_hostile_label_match_literal_oracle() {
    let seed = b"forced-retry-seed";
    let mut production = Keccak256::new();
    production.update(seed);
    let mut oracle = Keccak256::new();
    oracle.update(seed);
    let mut production_ordinal = 91;
    let mut oracle_ordinal = 91;
    let purpose = ChallengePurposeV1::scoped_v1(b"forced-retry", 9, 4);
    let actual = derive_challenge_with_policy_v1(
        &mut production,
        &mut production_ordinal,
        purpose,
        |attempt, value| attempt >= 2 && !value.is_zero(),
    )
    .unwrap();
    let expected = oracle_challenge_v1(&mut oracle, &mut oracle_ordinal, b"forced-retry", 9, 4, 2);
    assert_eq!(actual, expected);
    assert_eq!(production.finalize(), oracle.finalize());
    let mut hostile = Keccak256::new();
    hostile.update(seed);
    let mut hostile_ordinal = 91;
    let changed = oracle_challenge_v1(
        &mut hostile,
        &mut hostile_ordinal,
        b"forced-retry-hostile",
        9,
        4,
        2,
    );
    assert_ne!(actual, changed);
}
#[test]
#[rustfmt::skip]
fn z_excludes_the_exact_table_and_every_other_coordinate_is_nonzero() {
    assert!(!challenge_is_outside_table_v1(Scalar::zero()));
    assert!(!challenge_is_outside_table_v1(Scalar::from_u64(32_767)));
    assert!(challenge_is_outside_table_v1(Scalar::from_u64(32_768)));
    assert!(!challenge_is_outside_boolean_set_v1(Scalar::zero()));
    assert!(!challenge_is_outside_boolean_set_v1(Scalar::one()));
    assert!(challenge_is_outside_boolean_set_v1(Scalar::from_u64(2)));
    let kat = transcript_kat_v1();
    assert!(challenge_is_outside_table_v1(
        Scalar::from_le_bytes_exact(kat.z).unwrap()
    ));
    for scalar in [
        kat.tau_first,
        kat.tau_last,
        kat.kappa,
        kat.delta,
        kat.first_sumcheck,
        kat.last_sumcheck,
        kat.first_endpoint_batch,
        kat.last_endpoint_batch,
        kat.mask_batch,
    ] {
        assert!(!Scalar::from_le_bytes_exact(scalar).unwrap().is_zero());
    }
    for scalar in [kat.kappa, kat.delta] {
        assert!(challenge_is_outside_boolean_set_v1(Scalar::from_le_bytes_exact(scalar).unwrap()));
    }
}
#[test]
#[rustfmt::skip]
fn missing_extra_reordered_and_malformed_frames_fail_closed() {
    let mut malformed_residuals = core::array::from_fn(|ordinal| endpoint_v1(HIDDEN_ENDPOINTS_V1 + ordinal));
    malformed_residuals[1] = [0; 33];
    assert!(matches!(coefficient_residual_stage_v1().absorb_coefficient_residual_commitments_v1(malformed_residuals, CoefficientResidualCommitmentSealV1::TestOnly), Err(GlobalLookupErrorV1::Encoding)));
    assert!(matches!(
        sumcheck_stage_v1().absorb_gtilde_v1(1, gtilde_v1(1)),
        Err(GlobalLookupErrorV1::Order)
    ));
    let mut incomplete = sumcheck_stage_v1();
    for ordinal in 0..REQUIRED_CUBIC_MESSAGES_V1 - 1 {
        incomplete = incomplete
            .absorb_gtilde_v1(ordinal, gtilde_v1(ordinal))
            .unwrap();
    }
    assert!(matches!(
        incomplete.finish_sumcheck_v1(),
        Err(GlobalLookupErrorV1::Order)
    ));
    let mut malformed = gtilde_v1(0);
    let mut modulus_le = VEGA_T256_SCALAR_MODULUS_BE_V1;
    modulus_le.reverse();
    malformed[..32].copy_from_slice(&modulus_le);
    assert!(matches!(
        sumcheck_stage_v1().absorb_gtilde_v1(0, malformed),
        Err(GlobalLookupErrorV1::Encoding)
    ));
    assert!(matches!(
        endpoint_stage_v1().derive_opening_batches_v1(),
        Err(GlobalLookupErrorV1::Order)
    ));
    assert!(matches!(
        endpoint_stage_v1().absorb_endpoint_commitment_v1(1, endpoint_v1(1)),
        Err(GlobalLookupErrorV1::Order)
    ));
    assert!(matches!(
        endpoint_stage_v1().absorb_endpoint_commitment_v1(0, [0; 33]),
        Err(GlobalLookupErrorV1::Encoding)
    ));
    let mut complete = endpoint_stage_v1();
    for ordinal in 0..HIDDEN_ENDPOINTS_V1 {
        complete = complete
            .absorb_endpoint_commitment_v1(ordinal, endpoint_v1(ordinal))
            .unwrap();
    }
    assert!(matches!(
        complete.absorb_endpoint_commitment_v1(HIDDEN_ENDPOINTS_V1, endpoint_v1(0)),
        Err(GlobalLookupErrorV1::Order)
    ));
}
#[test]
fn mask_segments_terminal_functionals_and_padding_are_exact() {
    for statement in 0..14 {
        assert_eq!(
            mask_segment_v1(statement).unwrap(),
            statement * 14..statement * 14 + 14
        );
    }
    assert_eq!(mask_segment_v1(14).unwrap(), 196..205);
    assert_eq!(mask_segment_v1(15).unwrap(), 205..234);
    assert_eq!(mask_segment_v1(16), Err(GlobalLookupErrorV1::Shape));
    let challenges = core::array::from_fn(|index| Scalar::from_u64((index + 2) as u64));
    let masks: [Scalar; MASK_COMMITTED_SCALARS_V1] =
        core::array::from_fn(|index| Scalar::from_u64((index % 31 + 1) as u64));
    let half = Scalar::from_u64(2).inverse().unwrap();
    for statement in 0..ENDPOINT_BATCHES_V1 {
        let segment = mask_segment_v1(statement).unwrap();
        let mut carry = Scalar::zero();
        let mut functional = Scalar::zero();
        for (local, message) in segment.clone().enumerate() {
            let [a, b, c] = [
                masks[3 * message],
                masks[3 * message + 1],
                masks[3 * message + 2],
            ];
            let r = challenges[message];
            carry = half * carry
                + a * (r.square() * r - half)
                + b * (r.square() - half)
                + c * (r - half);
            for coefficient in 0..3 {
                functional += masks[3 * message + coefficient]
                    * mask_terminal_weight_v1(statement, local, coefficient, &challenges).unwrap();
            }
        }
        assert_eq!(functional, carry);
    }
    let xi = Scalar::from_u64(37);
    for scalar_ordinal in [0, 41, 587, 701] {
        let message = scalar_ordinal / 3;
        let statement = match message {
            0..=195 => message / 14,
            196..=204 => 14,
            _ => 15,
        };
        let local = message - mask_segment_v1(statement).unwrap().start;
        let expected = scalar_pow_v1(xi, statement)
            * mask_terminal_weight_v1(statement, local, scalar_ordinal % 3, &challenges).unwrap();
        assert_eq!(
            batched_mask_weight_v1(scalar_ordinal, &challenges, xi).unwrap(),
            expected
        );
    }
    assert_eq!(
        batched_mask_weight_v1(702, &challenges, xi),
        Ok(Scalar::zero())
    );
    assert_eq!(
        batched_mask_weight_v1(1_023, &challenges, xi),
        Ok(Scalar::zero())
    );
    assert_eq!(
        batched_mask_weight_v1(1_024, &challenges, xi),
        Err(GlobalLookupErrorV1::Shape)
    );
}
#[test]
#[rustfmt::skip]
fn lookup_relation_has_literal_oracle_and_rejects_every_hostile_term() {
    let y: [Scalar; 15] = core::array::from_fn(|index| Scalar::from_u64(index as u64 + 2));
    let total = (0..1 << 15).fold(Scalar::zero(), |sum, index| sum + equality_weight_v1(index, &y).unwrap());
    assert_eq!(total, Scalar::one());
    let (candidate, inverse, nu) = (Scalar::from_u64(5), Scalar::from_u64(7), Scalar::from_u64(11));
    assert_eq!(lookup_evaluation_target_v1(candidate, inverse, nu), candidate + nu * inverse);
    let generator = Point::canonical_generator().unwrap();
    let term = lookup_evaluation_commitment_term_v1(generator.mul_scalar(candidate), generator.mul_scalar(inverse), Scalar::from_u64(13), nu);
    assert_eq!(term, generator.mul_scalar(Scalar::from_u64(13) * (candidate + nu * inverse)));
    // Independent literal oracle at r=0: chi=(-1)^29=-1, S=E0=1, Qz=1/z.
    let point = [Scalar::zero(); LOOKUP_DIMENSIONS_V1];
    let rho = [Scalar::from_u64(2); LOOKUP_DIMENSIONS_V1];
    let (z, a, u, m) = (Scalar::from_u64(19), Scalar::from_u64(3), Scalar::from_u64(2), Scalar::from_u64(11));
    let (alpha, lambda, mu) = (Scalar::from_u64(3), Scalar::from_u64(5), Scalar::from_u64(7));
    let v = (z - a) * u;
    let rhs = alpha * -Scalar::one() * (v - Scalar::one()) + lambda * (u - m * z.inverse().unwrap()) + mu * (m - Scalar::one());
    let masked = Scalar::from_u64(29);
    assert_eq!(multilinear_equality_v1(&rho, &point), -Scalar::one());
    assert_eq!(active_selector_v1(&point), Ok(Scalar::one()));
    assert_eq!(coordinate_zero_selector_v1(&point), Scalar::one());
    assert_eq!(fixed_table_inverse_mle_v1(z, &point), Ok(z.inverse().unwrap()));
    let relation = LookupRelationEvaluationV1 { z, inverse: u, inverse_product: v, multiplicity: m, residual: rhs, alpha, rho: &rho, point: &point, lambda, mu };
    let gate = |relation, candidate, public_claim| lookup_gate_residuals_v1(LookupGateEvaluationV1 { relation, candidate, masked_accumulator: masked, public_claim }).unwrap();
    assert_eq!(gate(relation, a, rhs + masked), [Scalar::zero(); 3]);
    let residual = |evaluation| lookup_relation_residual_v1(evaluation).unwrap();
    assert_ne!(residual(LookupRelationEvaluationV1 { alpha: alpha+Scalar::one(), ..relation }), Scalar::zero());
    let mut hostile_rho=rho; hostile_rho[0]+=Scalar::one(); assert_ne!(residual(LookupRelationEvaluationV1 { rho: &hostile_rho, ..relation }), Scalar::zero());
    let mut hostile_point=point; hostile_point[0]=Scalar::from_u64(2); assert_ne!(residual(LookupRelationEvaluationV1 { point: &hostile_point, ..relation }), Scalar::zero());
    for changed in [residual(LookupRelationEvaluationV1 { lambda: lambda+Scalar::one(), ..relation }), residual(LookupRelationEvaluationV1 { mu: mu+Scalar::one(), ..relation }), residual(LookupRelationEvaluationV1 { inverse_product: v+Scalar::one(), ..relation }), residual(LookupRelationEvaluationV1 { inverse: u+Scalar::one(), ..relation }), residual(LookupRelationEvaluationV1 { multiplicity: m+Scalar::one(), ..relation }), residual(LookupRelationEvaluationV1 { z: z+Scalar::one(), ..relation })] { assert_ne!(changed, Scalar::zero()); }
    let hostile_table_rhs = alpha * -Scalar::one() * (v - Scalar::one()) + lambda * (u - m * (z-Scalar::one()).inverse().unwrap()) + mu * (m-Scalar::one());
    assert_ne!(residual(LookupRelationEvaluationV1 { residual: hostile_table_rhs, ..relation }), Scalar::zero());
    assert_ne!(gate(relation, a+Scalar::one(), rhs+masked)[0], Scalar::zero());
    assert_ne!(gate(relation, a, rhs+masked+Scalar::one())[2], Scalar::zero());
    assert_eq!(coefficient_gate_residuals_v1(Scalar::from_u64(3), Scalar::from_u64(5), Scalar::from_u64(15), Scalar::from_u64(7), Scalar::from_u64(22)), [Scalar::zero(); 2]);
    assert_eq!(mask_constraint_residuals_v1(14, Scalar::from_u64(5), Scalar::from_u64(7), Scalar::from_u64(12), Scalar::from_u64(99)).unwrap(), [Scalar::zero(); 2]);
    assert_eq!(mask_constraint_residuals_v1(7, Scalar::zero(), Scalar::zero(), Scalar::zero(), Scalar::zero()), Err(GlobalLookupErrorV1::Shape));
    assert_eq!(mask_constraint_residuals_v1(15, masked, rhs, rhs + masked, masked).unwrap(), [Scalar::zero(); 2]);
    assert_eq!(fixed_table_inverse_mle_v1(Scalar::from_u64(32_767), &point), Err(GlobalLookupErrorV1::Context));
}
#[test]
#[rustfmt::skip]
fn manifest_topology_and_complete_transcript_have_literal_kats() {
    assert_eq!(hex::encode(challenge_manifest_digest_v1()), "e3730911785cb1e23332ee9a1361810c435f76b93becd54e3b0d189644b32d99");
    assert_eq!(hex::encode(global_lookup_topology_digest_v1()), "3af9a6ad67383c32b06bb5d95a05863b8cb0b3338660177bc2a92e1bbf40b4ab");
    let kat = transcript_kat_v1();
    assert_eq!(hex::encode(kat.digest), "b7c4568000e11ee2a9833593cd8609ea2a026f2cf70be2335b498064c6860744");
    for (actual, expected) in [(kat.z,"20aafcea0445adace67ff1a4677c2110d66278f3c9b1d2fd105f5e4ebefa47ad"),(kat.tau_first,"5756541f6ddd69a4bdebffa4ac0282ca3cb855aa889111e99066f74b948d9e8f"),(kat.tau_last,"b27c447101e15cba92ee4023bb6e3f03536601b39ab8e4a989df0966e2bb5bed"),(kat.kappa,"930624f6d7f13156fa180f46ca76fd8b0049d92d2f2b77aa2b27f2054a19f7df"),(kat.delta,"713f67d5cf0db941965cfa1c7a75e2e615ee400c6b389ee014bf66cd1e77f086"),(kat.first_sumcheck,"f8e0c2376974bbfb5aa519251d2a286dab433e6e3d01368f2d214ad662d1dae3"),(kat.last_sumcheck,"1f97c2753fb175fe3231dd93be1a867b96d42489c398c64561b7371b4f7901e0"),(kat.first_endpoint_batch,"901623682faf5c74cbf3decd1878cdf91d689004dc7f380fdf55a24436e3e00a"),(kat.last_endpoint_batch,"d04f00b81892dc166b33e41c2651c61dea79a175f5c3cc45a15e179cc78d5fcc"),(kat.mask_batch,"76b5a1018385470c355df610ca647238af1c5405fc42e5efac7ffbdc800d6c1d")] { assert_eq!(hex::encode(actual), expected); }
}
#[test]
#[rustfmt::skip]
fn source_guards_keep_the_manifest_private_bounded_and_non_replayable() {
    let production = include_str!("challenge_v1.rs");
    let tests = include_str!("challenge_v1_tests.rs");
    let parent = include_str!("../global_lookup_statement_v1.rs");
    assert!(production.lines().count() <= 900);
    assert!(tests.lines().count() <= 500);
    assert!(parent.lines().count() <= 900);
    assert!(!production.contains("Vec<"));
    assert!(!production.contains("pub struct"));
    assert!(!production.contains("pub enum"));
    assert!(!production.contains("relation_rhs"));
    assert!(!production.contains("impl Clone for GlobalLookupTranscriptV1"));
    assert_eq!(
        production
            .matches("derive_challenge_with_policy_v1(")
            .count(),
        2
    );
    assert!(production.contains("fn challenge_coordinate_v1(ordinal: u32)"));
    assert!(production.contains("global-lookup.bound-context\\0"));
    assert!(production.contains("bound_context_digest: [u8; 32]"));
    assert_eq!(
        production
            .matches("mod global_lookup_external_sumcheck_v1;")
            .count(),
        1
    );
    assert!(production.contains("hash_coefficient_manifest_suffix_v1(&mut hash);"));
    assert_eq!(production.matches("mod coefficient_residual_v1;").count(), 1);
    assert!(production.contains("FIRST_SUMCHECK_ORDINAL_V1 == DELTA_ORDINAL_V1 + 1"));
    assert_eq!(
        parent
            .matches("pub(super) fn global_lookup_topology_digest_v1()")
            .count(),
        1
    );
    assert_eq!(parent.matches("mod challenge_v1;").count(), 1);
    assert!(!parent.contains("struct GlobalLookupTranscriptV1"));
}
