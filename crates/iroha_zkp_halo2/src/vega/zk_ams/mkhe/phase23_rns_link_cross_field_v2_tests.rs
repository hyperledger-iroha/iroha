use super::*;
const POINT_A_V2: &str = "8016f70c3f35b3257896971b306635647bc52eb7cad7a5eca1a42f2340737749e3";
const POINT_B_V2: &str = "00a37dc092877e239385cd8392ba2360ce1859a37f7a2b9c626b336608d2ce4cfe";
fn point_v2(encoded: &str) -> Point {
    Point::from_non_identity_wire_bytes_exact(&hex::decode(encoded).expect("literal hex"))
        .expect("literal canonical non-identity T256 point")
}
fn evaluation_v2() -> AuthenticatedQpcsEvaluationV2 {
    let modulus = RELEASE_MODULI_V1[0];
    let point = 7;
    let masked_h_evaluation = 11;
    let factor = mod_add_v2(
        mod_pow_v2(point, RING_DEGREE_V2 as u64, modulus),
        1,
        modulus,
    );
    AuthenticatedQpcsEvaluationV2 {
        seal: PhantomData,
        initial_root: [0x91; 32],
        complete_entry_digest: [0xa2; 32],
        limb: 0,
        repetition: 2,
        modulus,
        gamma: 2,
        beta: 3,
        point,
        key_evaluation: 5,
        ciphertext_evaluation: 13,
        masked_p_evaluation: mod_mul_v2(factor, masked_h_evaluation, modulus),
        masked_h_evaluation,
    }
}
fn axes_v2() -> CrossFieldAxesV2 {
    CrossFieldAxesV2 {
        fixed_axes_digest: [0x11; 32],
        source_manifest_digest: [0x22; 32],
        source_receipt_digest: [0x33; 32],
        source_formula_digest: [0x44; 32],
        source_mapping_digest: [0x55; 32],
        terminal_binding_digest: [0x66; 32],
        radix_range_binding_digest: [0x77; 32],
        qpcs_parameter_digest: [0x88; 32],
    }
}
struct CommitmentFixtureV2 {
    radix_groups: Vec<RadixGroupCommitmentsV2>,
    comparators: Vec<ComparatorGroupCommitmentsV2>,
    small_source: Vec<SmallSourceBlockCommitmentsV2>,
    q_masks: Vec<QMaskBlockCommitmentsV2>,
}
impl CommitmentFixtureV2 {
    fn new_v2() -> Self {
        let a = point_v2(POINT_A_V2);
        let b = point_v2(POINT_B_V2);
        Self {
            radix_groups: vec![
                RadixGroupCommitmentsV2 {
                    d_low: [a; RADIX_LOW_DIGITS_V2],
                    d_top: b,
                };
                COMPARATOR_GROUPS_V2
            ],
            comparators: vec![
                ComparatorGroupCommitmentsV2 {
                    difference_digits: [a; RADIX_LOW_DIGITS_V2],
                    mixed_top: b,
                    borrows: [b; RADIX_DIGITS_V2],
                    difference_inverses: [a; RADIX_LOW_DIGITS_V2],
                };
                COMPARATOR_GROUPS_V2
            ],
            small_source: vec![
                SmallSourceBlockCommitmentsV2 {
                    signed: a,
                    negative_magnitude: b,
                    positive_lookup_inverse: a,
                    negative_lookup_inverse: b,
                };
                SMALL_SOURCE_BLOCKS_V2
            ],
            q_masks: vec![
                QMaskBlockCommitmentsV2 {
                    digits: [a; MASK_DIGITS_V2],
                    digit_inverses: [b; MASK_DIGITS_V2],
                    complement_digits: [a; MASK_DIGITS_V2],
                    complement_inverses: [b; MASK_DIGITS_V2],
                };
                Q_MASK_BLOCKS_V2
            ],
        }
    }
    fn view_v2(&self) -> BoundCommitmentViewV2<'_> {
        BoundCommitmentViewV2 {
            source_seal: SourceCommitmentSealV2::TestOnly,
            range_seal: RadixRangeSealV2::TestOnly,
            mask_seal: CanonicalQMaskSealV2::TestOnly,
            axes: axes_v2(),
            radix_groups: &self.radix_groups,
            comparators: &self.comparators,
            small_source: &self.small_source,
            q_masks: &self.q_masks,
        }
    }
}
fn derived_fixture_v2() -> DerivedCommitmentsV2 {
    DerivedCommitmentsV2 {
        positive: point_v2(POINT_A_V2),
        negative: point_v2(POINT_B_V2),
        source_binding_digest: [0xb3; 32],
    }
}
fn canonical_proof_bytes_v2() -> Vec<u8> {
    let a = point_v2(POINT_A_V2)
        .to_non_identity_wire_bytes()
        .expect("point A");
    let b = point_v2(POINT_B_V2)
        .to_non_identity_wire_bytes()
        .expect("point B");
    let scalar = Scalar::from_u64(17).to_le_bytes();
    let mut proof = Vec::with_capacity(BP_PROOF_BYTES_V2);
    for _ in 0..13 {
        proof.extend_from_slice(&a);
    }
    for _ in 0..3 {
        proof.extend_from_slice(&scalar);
    }
    for _ in 0..28 {
        proof.extend_from_slice(&b);
    }
    for _ in 0..2 {
        proof.extend_from_slice(&scalar);
    }
    assert_eq!(proof.len(), BP_PROOF_BYTES_V2);
    proof
}
#[test]
fn exact_inventory_wire_and_fail_closed_gates_are_frozen() {
    assert_eq!(COMPARATOR_GROUPS_V2, 344);
    assert_eq!(COMPARATOR_POINTS_PER_GROUP_V2, 17 + 1 + 18 + 17);
    assert_eq!(COMPARATOR_POINTS_V2, 18_232);
    assert_eq!(SMALL_SOURCE_BLOCKS_V2, 1_032);
    assert_eq!(SMALL_SOURCE_POINTS_PER_BLOCK_V2, 4);
    assert_eq!(SMALL_SOURCE_POINTS_V2, 4_128);
    assert_eq!(Q_MASK_BLOCKS_V2, 190 * 8);
    assert_eq!(Q_MASK_POINTS_PER_BLOCK_V2, 4 + 4 + 4 + 4);
    assert_eq!(Q_MASK_POINTS_V2, 24_320);
    assert_eq!(ADDED_RAW_POINTS_V2, 46_680);
    assert_eq!(COMPARATOR_WIRE_BYTES_V2, 601_656);
    assert_eq!(SMALL_SOURCE_WIRE_BYTES_V2, 136_224);
    assert_eq!(Q_MASK_WIRE_BYTES_V2, 802_560);
    assert_eq!(BP_PROOF_BYTES_V2, 1_513);
    assert_eq!(ALL_BP_PROOFS_BYTES_V2, 287_470);
    assert_eq!(CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2, 1_828_422);
    assert_eq!(CONDITIONAL_SUBTOTAL_RESERVE_BYTES_V2, 330_501);
    assert_eq!(EXISTING_LOOKUP_VALUES_V2, 191_627_264);
    assert_eq!(COMPARATOR_LOOKUP_VALUES_V2, 95_813_632);
    assert_eq!(SMALL_SOURCE_LOOKUP_VALUES_V2, 33_816_576);
    assert_eq!(Q_MASK_LOOKUP_VALUES_V2, 199_229_440);
    assert_eq!(EXPANDED_LOOKUP_VALUES_V2, 520_486_912);
    assert_eq!(
        (EXISTING_LOOKUP_ROUNDS_V2, EXPANDED_LOOKUP_ROUNDS_V2),
        (28, 29)
    );
    assert_eq!(CONDITIONAL_MINIMUM_LOOKUP_DELTA_BYTES_V2, 96);
    const {
        assert!(!SOURCE_SET_BOUND_V2);
        assert!(!RANGE_SET_BOUND_V2);
        assert!(!CANONICAL_Q_MASK_SET_BOUND_V2);
        assert!(!AUTHENTICATED_QPCS_SET_BOUND_V2);
        assert!(!GLOBAL_LOOKUP_STATEMENT_INSTANTIATED_V2);
        assert!(!GLOBAL_LOOKUP_VERIFIED_V2);
        assert!(!COMPLETE_WIRE_ACCOUNTING_VERIFIED_V2);
        assert!(!CROSS_FIELD_RELATION_VERIFIED_V2);
        assert!(!ZERO_KNOWLEDGE_THEOREM_ACCEPTED_V2);
        assert!(!AUTHORITY_MINTED_V2);
        assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
        assert!(!MEASURED_RSS_QUALIFIED_V2);
        assert!(!RELEASE_READY_V2);
    }
}
#[test]
fn ordered_inventory_and_binding_have_literal_kat_and_reject_mutation() {
    let mut fixture = CommitmentFixtureV2::new_v2();
    let view = fixture.view_v2();
    assert_eq!(
        hex::encode(view.added_inventory_root_v2().expect("inventory root")),
        "2b4cb872a134301d280bc3e06cbf60a8875027995c1a7940832bc535c77d24e8"
    );
    assert_eq!(
        hex::encode(view.existing_d_root_v2().expect("existing D root")),
        "9ab0a93807f240f1d764078e7a8f532ebb828094aa7ae0daafafa356b8a5c831"
    );
    assert_eq!(
        hex::encode(view.source_binding_digest_v2().expect("binding")),
        "11cead6073a53eb3fd47f3d2c86c9b0e4d21b10cdaaf40932b1b585c3a110c97"
    );
    let original = view.source_binding_digest_v2().expect("original binding");
    fixture.small_source[0].signed = point_v2(POINT_B_V2);
    let mutated = fixture
        .view_v2()
        .source_binding_digest_v2()
        .expect("mutated binding");
    assert_ne!(original, mutated);
}
#[test]
fn deterministic_derived_commitments_change_with_sealed_input() {
    let mut fixture = CommitmentFixtureV2::new_v2();
    let evaluation = evaluation_v2();
    let first = fixture.view_v2().derive_v2(&evaluation).expect("derive");
    let repeated = fixture.view_v2().derive_v2(&evaluation).expect("repeat");
    assert_eq!(first.positive, repeated.positive);
    assert_eq!(first.negative, repeated.negative);
    assert_eq!(first.source_binding_digest, repeated.source_binding_digest);
    fixture.small_source[0].signed = point_v2(POINT_B_V2);
    let mutated = fixture
        .view_v2()
        .derive_v2(&evaluation)
        .expect("mutated derive");
    assert_ne!(first.positive, mutated.positive);
    assert_eq!(first.negative, mutated.negative);
    assert_ne!(first.source_binding_digest, mutated.source_binding_digest);
}
#[test]
fn exact_two_commitment_statement_is_206_gates_and_413_constraints() {
    let evaluation = evaluation_v2();
    let constraints = cross_field_constraints_v2(&evaluation).expect("constraints");
    assert_eq!(MULTIPLICATION_GATES_V2, 206);
    assert_eq!(constraints.len(), 413);
    for gate in 0..MULTIPLICATION_GATES_V2 {
        let equality = &constraints[2 * gate];
        let product = &constraints[2 * gate + 1];
        assert_eq!(equality.highest_a_index, Some(gate));
        assert_eq!(equality.wl, vec![(gate, Scalar::one())]);
        assert_eq!(equality.wr, vec![(gate, -Scalar::one())]);
        assert_eq!(product.highest_a_index, Some(gate));
        assert_eq!(product.wo, vec![(gate, Scalar::one())]);
        assert_eq!(product.wl, vec![(gate, -Scalar::one())]);
    }
    let relation = constraints.last().expect("final relation");
    assert_eq!(relation.highest_a_index, Some(GENERATOR_PREFIX_V2 - 1));
    assert_eq!(relation.highest_c_index, Some(1));
    assert_eq!(relation.wcg.len(), 2);
    assert_eq!(relation.wcg[0].len(), GENERATOR_PREFIX_V2);
    assert_eq!(relation.wcg[1].len(), GENERATOR_PREFIX_V2);
    assert_eq!(relation.wl.len(), 2 * QUOTIENT_BITS_V2);
    assert!(relation.wr.is_empty());
    assert!(relation.wo.is_empty());
    assert!(relation.wv.is_empty());
    build_cross_field_statement_v2(&derived_fixture_v2(), &evaluation)
        .expect("backend accepts the exact n=16384/c=2 statement");
}
#[test]
fn exact_1513_byte_parser_rejects_length_point_and_scalar_mutations() {
    let proof = canonical_proof_bytes_v2();
    assert!(ExactProofViewV2::parse_v2(&proof).is_ok());
    assert_eq!(
        ExactProofViewV2::parse_v2(&proof[..proof.len() - 1]).err(),
        Some(CrossFieldErrorV2::Wire)
    );
    let mut trailing = proof.clone();
    trailing.push(0);
    assert_eq!(
        ExactProofViewV2::parse_v2(&trailing).err(),
        Some(CrossFieldErrorV2::Wire)
    );
    let mut invalid_point = proof.clone();
    invalid_point[0] = 0x41;
    assert_eq!(
        ExactProofViewV2::parse_v2(&invalid_point).err(),
        Some(CrossFieldErrorV2::Wire)
    );
    let mut invalid_scalar = proof;
    let scalar_offset = 13 * POINT_BYTES_V2;
    let mut modulus_le = VEGA_T256_SCALAR_MODULUS_BE_V1;
    modulus_le.reverse();
    invalid_scalar[scalar_offset..scalar_offset + SCALAR_BYTES_V2].copy_from_slice(&modulus_le);
    assert_eq!(
        ExactProofViewV2::parse_v2(&invalid_scalar).err(),
        Some(CrossFieldErrorV2::Wire)
    );
}
#[test]
fn purpose_bound_transcript_has_literal_kat_and_binds_every_context_field() {
    let derived = derived_fixture_v2();
    let evaluation = evaluation_v2();
    let state = initial_transcript_state_v2(&derived, &evaluation).expect("state");
    assert_eq!(
        hex::encode(keccak256(&state)),
        "c7a0e1af173e65083a37a020abc9c69a99cf1cbe15143b26ea4794aad8f872ed"
    );
    let mut changed_entry = evaluation;
    changed_entry.complete_entry_digest[0] ^= 1;
    assert_ne!(
        keccak256(&state),
        keccak256(
            &initial_transcript_state_v2(&derived, &changed_entry).expect("changed entry state")
        )
    );
    let changed_commitment = DerivedCommitmentsV2 {
        positive: derived.negative,
        negative: derived.positive,
        source_binding_digest: derived.source_binding_digest,
    };
    assert_ne!(
        keccak256(&state),
        keccak256(
            &initial_transcript_state_v2(&changed_commitment, &evaluation)
                .expect("changed commitment state")
        )
    );
}
#[test]
fn verifier_transcript_consumes_the_only_canonical_proof_shape() {
    let derived = derived_fixture_v2();
    let evaluation = evaluation_v2();
    let proof = canonical_proof_bytes_v2();
    let view = ExactProofViewV2::parse_v2(&proof).expect("proof view");
    let mut transcript =
        CrossFieldVerifierTranscriptV2::new_v2(&derived, &evaluation, view).expect("transcript");
    for _ in 0..13 {
        <CrossFieldVerifierTranscriptV2<'_> as VerifierTranscript<
            ZkAmsT256BulletproofSuiteV1,
        >>::read_point(&mut transcript)
        .expect("point");
    }
    for _ in 0..3 {
        <CrossFieldVerifierTranscriptV2<'_> as VerifierTranscript<
            ZkAmsT256BulletproofSuiteV1,
        >>::read_scalar(&mut transcript)
        .expect("scalar");
    }
    for _ in 0..28 {
        <CrossFieldVerifierTranscriptV2<'_> as VerifierTranscript<
            ZkAmsT256BulletproofSuiteV1,
        >>::read_point(&mut transcript)
        .expect("point");
    }
    for _ in 0..2 {
        <CrossFieldVerifierTranscriptV2<'_> as VerifierTranscript<
            ZkAmsT256BulletproofSuiteV1,
        >>::read_scalar(&mut transcript)
        .expect("scalar");
    }
    transcript.finish_v2().expect("exact consumption");
}
#[test]
fn release_extrema_no_wrap_and_soundness_literals_are_pinned() {
    assert_eq!(RELEASE_MODULI_V1.iter().copied().min(), Some(Q_MIN_V2));
    assert_eq!(RELEASE_MODULI_V1.iter().copied().max(), Some(Q_MAX_V2));
    assert_eq!(POSITIVE_TERMS_PER_COORDINATE_V2, 7_256);
    assert_eq!(NEGATIVE_TERMS_PER_COORDINATE_V2, 1_376);
    assert_eq!(POSITIVE_TERMS_TOTAL_V2, 118_882_304);
    assert_eq!(NEGATIVE_TERMS_TOTAL_V2, 22_544_384);
    assert_eq!((V_PLUS_BITS_V2, V_MINUS_BITS_V2), (88, 86));
    assert_eq!((U_PLUS_BITS_V2, U_MINUS_BITS_V2), (162, 160));
    assert_eq!(INTEGER_EXPRESSION_BITS_V2, 165);
    assert_eq!(AGGREGATE_DISCREPANCY_DEGREE_V2, 262_185);
    assert_eq!(CROSS_SOUNDNESS_BITS_X100_FLOOR_V2, 20_475);
    assert!(NO_WRAP_FORMULA_V2.contains(&b'8'));
    assert!(SOUNDNESS_FORMULA_V2.ends_with(b"2^-204.75"));
    assert_eq!(VEGA_T256_SCALAR_MODULUS_BE_V1[0], 0xff);
}
#[test]
fn conditional_subtotal_preflight_accepts_only_the_frozen_inventory() {
    let exact = CrossFieldConditionalSubtotalPreflightV2 {
        comparator_points: COMPARATOR_POINTS_V2,
        small_source_points: SMALL_SOURCE_POINTS_V2,
        q_mask_points: Q_MASK_POINTS_V2,
        proof_count: RELATION_COUNT_V2,
        proof_bytes_each: BP_PROOF_BYTES_V2,
        outer_auth_bytes: OUTER_AUTH_FRAMING_BYTES_V2,
        conditional_subtotal_bytes: CONDITIONAL_WIRE_SUBTOTAL_BYTES_V2,
    };
    assert_eq!(exact.validate_v2(), Ok(()));
    let wrong = CrossFieldConditionalSubtotalPreflightV2 {
        conditional_subtotal_bytes: exact.conditional_subtotal_bytes + 1,
        ..exact
    };
    assert_eq!(wrong.validate_v2(), Err(CrossFieldErrorV2::Shape));
}
#[test]
fn source_budget_and_uninhabited_api_boundary_are_static() {
    let production = include_str!("phase23_rns_link_cross_field_v2.rs");
    let tests = include_str!("phase23_rns_link_cross_field_v2_tests.rs");
    let parent = include_str!("phase23_rns_link.rs");
    assert!(production.lines().count() <= 1_200);
    assert!(tests.lines().count() <= 700);
    assert_eq!(parent.matches("mod cross_field_v2;").count(), 1);
    assert!(!production.contains("Vec<Point>"));
    assert!(!production.contains("pub struct BoundCommitmentViewV2"));
    assert!(!production.contains("impl Clone for BoundCommitmentViewV2"));
    assert!(!production.contains("impl Default for BoundCommitmentViewV2"));
    assert!(!production.contains("detached_commitment"));
    assert!(!production.contains("callback"));
    let prover_transcript = production
        .split_once("impl ProverTranscript<ZkAmsT256BulletproofSuiteV1>")
        .expect("cross-field prover transcript")
        .1
        .split_once("struct CrossFieldVerifierTranscriptV2")
        .expect("cross-field prover transcript boundary")
        .0;
    assert!(prover_transcript.contains("fn push_scalar(&mut self, scalar: &Scalar)"));
    assert!(prover_transcript.contains("with_borrowed_t256_scalar_encoding_v1(scalar"));
    assert!(!prover_transcript.contains("scalar.to_le_bytes()"));
    assert!(prover_transcript.contains("fn push_point(&mut self, point: &Point)"));
    assert!(prover_transcript.contains("let encoded = SecretT256PointEncodingV1::new(point)?;"));
    assert!(prover_transcript.contains("self.state.extend_from_slice(encoded.as_ref());"));
    assert!(prover_transcript.contains("let result = self.push_bytes_v2(encoded.as_ref());"));
    assert!(prover_transcript.contains("drop(encoded);"));
    assert!(!prover_transcript.contains("point.to_non_identity_wire_bytes()"));
    assert!(!prover_transcript.contains("fn push_point(&mut self, point: Point)"));
    for seal in [
        "enum SourceCommitmentSealV2",
        "enum RadixRangeSealV2",
        "enum CanonicalQMaskSealV2",
        "enum AuthenticatedQpcsSealV2",
    ] {
        let body = production
            .split(seal)
            .nth(1)
            .expect("seal exists")
            .split("}\n")
            .next()
            .expect("seal body");
        assert!(body.contains("Infallible"));
        assert!(body.contains("Production"));
    }
}
