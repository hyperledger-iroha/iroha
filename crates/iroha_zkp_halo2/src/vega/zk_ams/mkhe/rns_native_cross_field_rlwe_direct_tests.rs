use super::super::{
    rns_native_profile::{
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
        ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
        ZkAmsMkheRnsNativeQpcsFriRootV1, ZkAmsMkheRnsNativeQpcsRootsV1,
        ZkAmsMkheRnsNativeTerminalBridgeV1, ZkAmsMkheRnsNativeTerminalRootsV1,
        ZkAmsMkheRnsNativeTranscriptV1,
    },
};
use super::*;
use std::cell::Cell;

fn digest_v1(tag: u8) -> [u8; DIGEST_BYTES_V1] {
    [tag; DIGEST_BYTES_V1]
}

fn transcript_digest_fixture_v1(context: u16, ordinal: u16) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-rlwe-direct.transcript-test");
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

struct TranscriptTestChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: [u8; 1],
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for TranscriptTestChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

struct TranscriptTestSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    context: u16,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for TranscriptTestSnapshotV1 {
    type Chunk = TranscriptTestChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; DIGEST_BYTES_V1] {
        let ordinal = match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => 5,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => 6,
        };
        transcript_digest_fixture_v1(self.context, ordinal)
    }

    fn read_slot(
        &mut self,
        _arena: ZkAmsMkheRnsNativeSourceArenaV1,
        _slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
    }
}

fn transcript_opening_role_v1(ordinal: usize) -> (ZkAmsMkheRnsNativeFamilyV1, u8) {
    match ordinal {
        0 => (ZkAmsMkheRnsNativeFamilyV1::X, 0),
        1..=16 => (ZkAmsMkheRnsNativeFamilyV1::U, (ordinal - 1) as u8),
        17..=32 => (ZkAmsMkheRnsNativeFamilyV1::E, (ordinal - 17) as u8),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        34..=41 => (ZkAmsMkheRnsNativeFamilyV1::W, (ordinal - 34) as u8),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        _ => panic!("opening ordinal outside the canonical 43-record schedule"),
    }
}

fn qpcs_transcript_fixture_v1(context: u16) -> ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("release");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        transcript_digest_fixture_v1(context, 1),
        transcript_digest_fixture_v1(context, 2),
    )
    .expect("layout");
    let receipt = TranscriptTestSnapshotV1 { layout, context }
        .structural_receipt()
        .expect("source receipt");
    let public = ZkAmsMkheRnsNativePublicContextV1::new(
        transcript_digest_fixture_v1(context, 3),
        transcript_digest_fixture_v1(context, 4),
    )
    .expect("public context");
    let transcript =
        ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public).expect("context transcript");
    let openings = core::array::from_fn(|ordinal| {
        let (family, family_index) = transcript_opening_role_v1(ordinal);
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            family_index,
            transcript_digest_fixture_v1(context, 100 + 2 * ordinal as u16),
            transcript_digest_fixture_v1(context, 101 + 2 * ordinal as u16),
        )
        .expect("opening")
    });
    let openings =
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), openings)
            .expect("opening owner");
    let transcript = transcript
        .bind_opening_commitments(openings)
        .expect("opening transcript");
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        transcript_digest_fixture_v1(context, 200),
        transcript_digest_fixture_v1(context, 201),
        transcript_digest_fixture_v1(context, 202),
    )
    .expect("terminal bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            layer as u8,
            transcript_digest_fixture_v1(context, 220 + layer as u16),
        )
        .expect("FRI root")
    });
    let roots = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        transcript_digest_fixture_v1(context, 210),
        transcript_digest_fixture_v1(context, 211),
        transcript_digest_fixture_v1(context, 212),
        fri_roots,
    )
    .expect("qPCS roots");
    transcript.bind_qpcs_roots(roots).expect("qPCS transcript")
}

fn axes_v1() -> RnsNativeCrossFieldRlweFixedAxesV1 {
    RnsNativeCrossFieldRlweFixedAxesV1 {
        profile_manifest_digest: digest_v1(1),
        source_binding_digest: digest_v1(2),
        source_formula_digest: digest_v1(3),
        source_mapping_digest: digest_v1(4),
        terminal_predecessor_binding_digest: digest_v1(5),
        candidate_inventory_axes: RnsNativePreDirectInventoryCandidateAxesV1::test_fixture_v1(
            digest_v1(6),
            digest_v1(7),
        )
        .expect("candidate inventory axes"),
        existing_radix_candidate_root: digest_v1(8),
        rns_aggregation_challenge_seed: digest_v1(9),
        qpcs_parameter_digest: digest_v1(10),
        qpcs_pre_relation_transcript_digest: digest_v1(11),
    }
}

fn safe_axes_v1() -> RnsNativeCrossFieldPreQpcsSafeAxesV1 {
    axes_v1().pre_qpcs_safe_axes_v1()
}

fn completed_qpcs_with_points_v1(
    context: u16,
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    points: [u64; EVALUATIONS_V1],
) -> RnsNativeQpcsCompletedLineageV1 {
    let transcript = qpcs_transcript_fixture_v1(context);
    let expected_qpcs_bound_transcript_state = transcript.binding_digest();
    let lineage = transcript.test_qpcs_relation_lineage_v1();
    let schedule = RnsNativeQpcsRelationScheduleV1::test_fixture_with_lineage_v1(
        axes_v1().qpcs_parameter_digest,
        q_mask_s_root,
        axes_v1().qpcs_pre_relation_transcript_digest,
        relation_seed,
        points,
        lineage,
    );
    RnsNativeQpcsCompletedLineageV1::test_fixture_v1(
        schedule,
        expected_qpcs_bound_transcript_state,
        transcript,
    )
    .expect("matching completed qPCS lineage")
}

fn completed_qpcs_v1(
    context: u16,
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    point: u64,
) -> RnsNativeQpcsCompletedLineageV1 {
    completed_qpcs_with_points_v1(
        context,
        q_mask_s_root,
        relation_seed,
        [point; EVALUATIONS_V1],
    )
}

fn bind_claimed_root_v1(
    schedule: &mut RelationScheduleV1,
    claimed_root: [u8; DIGEST_BYTES_V1],
    context: u16,
) -> ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1 {
    let prior = schedule
        .bound
        .completed_qpcs
        .qpcs_transcript_binding_digest_v1()
        .expect("unconsumed qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        prior,
        claimed_root,
        transcript_digest_fixture_v1(context, 900),
        transcript_digest_fixture_v1(context, 901),
    )
    .expect("claimed terminal roots");
    let (claim, _) = roots.into_cross_field_claim_v1();
    schedule
        .bind_claimed_cross_field_root_v1(claim)
        .expect("provisional cross-field transcript")
}

fn relation_schedule_fixture_v1() -> RelationScheduleV1 {
    derive_relation_schedule_v1(
        bind_direct_q_mask_schedule_v1(
            axes_v1(),
            completed_qpcs_v1(400, digest_v1(20), digest_v1(22), 7),
        )
        .expect("direct binding fixture"),
    )
    .expect("relation schedule fixture")
}

fn claim_equality_pending_fixture_v1<'a>(
    claimed_root: [u8; DIGEST_BYTES_V1],
    recomputed_root: [u8; DIGEST_BYTES_V1],
    successor: &'a [u8],
    context: u16,
) -> RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a> {
    let mut schedule = relation_schedule_fixture_v1();
    let _ = bind_claimed_root_v1(&mut schedule, claimed_root, context);
    let cross_field_root_equality_obligation = schedule
        .take_cross_field_root_equality_obligation_v1()
        .expect("sole claimed-root equality obligation");
    let verified_cross_field_core_root =
        RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(recomputed_root)
            .expect("direct-owned verified-root fixture");
    RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1 {
        successor,
        binding_digest: digest_v1(111),
        q_mask_s_root: digest_v1(112),
        numeric_root: digest_v1(113),
        commitment_root: digest_v1(114),
        cross_field_root_equality_obligation,
        verified_cross_field_core_root,
    }
}

fn prepared_fixture_v1() -> PreparedInputsV1 {
    let schedule = relation_schedule_fixture_v1();
    let modulus = release_modulus_v1(0).expect("release modulus");
    let evaluation = ValidatedEvaluationV1 {
        limb: 0,
        repetition: 0,
        modulus,
        gamma: 3,
        beta: 5,
        point: 7,
        public_a: 11,
        public_b: 13,
        key_evaluation: 68,
        ciphertext_evaluation: 17,
        qpcs_product: 19,
        qpcs_opening_quotient: 23,
    };
    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    let commitments = DerivedCommitmentsV1 {
        positive: generators.g,
        negative: generators.h,
    };
    PreparedInputsV1 {
        schedule,
        evaluations: [evaluation; EVALUATIONS_V1],
        commitments: [commitments; EVALUATIONS_V1],
        numeric_root: digest_v1(17),
        commitment_root: digest_v1(18),
    }
}

fn core_proof_fixture_v1(core: usize) -> [u8; CORE_PROOF_BYTES_V1] {
    let point =
        point_bytes_v1(ZkAmsT256BulletproofSuiteV1::generators().g).expect("canonical proof point");
    let scalar = Scalar::from_u64((core + 1) as u64).to_le_bytes();
    let mut proof = [0_u8; CORE_PROOF_BYTES_V1];
    let mut cursor = 0;
    for _ in 0..FIXED_PROOF_POINTS_V1 {
        proof[cursor..cursor + POINT_BYTES_V1].copy_from_slice(&point);
        cursor += POINT_BYTES_V1;
    }
    for _ in 0..CIRCUIT_PROOF_SCALARS_V1 {
        proof[cursor..cursor + SCALAR_BYTES_V1].copy_from_slice(&scalar);
        cursor += SCALAR_BYTES_V1;
    }
    for _ in 0..IPA_PROOF_POINTS_V1 {
        proof[cursor..cursor + POINT_BYTES_V1].copy_from_slice(&point);
        cursor += POINT_BYTES_V1;
    }
    for _ in 0..IPA_FINAL_SCALARS_V1 {
        proof[cursor..cursor + SCALAR_BYTES_V1].copy_from_slice(&scalar);
        cursor += SCALAR_BYTES_V1;
    }
    assert_eq!(cursor, CORE_PROOF_BYTES_V1);
    proof
}

fn pending_fixture_v1() -> RnsNativeCrossFieldRlweFourCorePendingSealV1 {
    let proofs = core::array::from_fn(core_proof_fixture_v1);
    let transcript_digests = core::array::from_fn(|core| digest_v1(30 + core as u8));
    RnsNativeCrossFieldRlweFourCorePendingSealV1::from_parts_v1(
        prepared_fixture_v1(),
        proofs,
        transcript_digests,
    )
    .expect("pending four-core fixture")
}

struct TouchSourceV1<'a> {
    touches: &'a Cell<usize>,
}

impl TouchSourceV1<'_> {
    fn fail_v1<T>(&self) -> Result<T, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.touches.set(self.touches.get() + 1);
        Err(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable)
    }
}

impl RnsNativeQMaskSCommitmentSourceV1 for TouchSourceV1<'_> {
    fn q_mask_s_digit_commitment_v1(
        &self,
        _limb: usize,
        _repetition: usize,
        _block: usize,
        _digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }
}

impl RnsNativeCrossFieldAuthoritativeSourceV1 for TouchSourceV1<'_> {
    fn authoritative_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.touches.set(self.touches.get() + 1);
        [0; DIGEST_BYTES_V1]
    }

    fn take_numeric_evaluation_v1(
        &mut self,
        _limb: usize,
        _repetition: usize,
        _destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }

    fn message_radix_digit_commitment_v1(
        &self,
        _record: usize,
        _block: usize,
        _digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }

    fn small_signed_commitment_v1(
        &self,
        _record: usize,
        _role: usize,
        _block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }

    fn small_negative_magnitude_commitment_v1(
        &self,
        _record: usize,
        _role: usize,
        _block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }

    fn comparator_final_borrow_commitment_v1(
        &self,
        _record: usize,
        _block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }
}

impl RnsNativeCrossFieldQuotientOpeningSourceV1 for TouchSourceV1<'_> {
    fn take_positive_quotient_owner_v1(
        &mut self,
        _limb: usize,
        _repetition: usize,
        _values: &mut [Scalar],
        _commitment_mask: &mut Scalar,
        _quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }

    fn take_negative_quotient_owner_v1(
        &mut self,
        _limb: usize,
        _repetition: usize,
        _values: &mut [Scalar],
        _commitment_mask: &mut Scalar,
        _quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        self.fail_v1()
    }
}

struct UntouchedRandomV1;

impl ProofRandomSource for UntouchedRandomV1 {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        panic!("preflight must reject before RNG use")
    }
}

#[test]
fn exact_four_core_geometry_and_cap_are_settled() {
    assert_eq!(EVALUATIONS_V1, 200);
    assert_eq!(ACTIVE_GATES_PER_CORE_V1, 10_300);
    assert_eq!(PADDED_GATES_PER_CORE_V1, 16_384);
    assert_eq!(VECTOR_COMMITMENTS_PER_CORE_V1, 100);
    assert_eq!(CORE_VECTOR_OPENING_SCALAR_BYTES_V1, 52_428_800);
    assert_eq!(CONSTRAINTS_PER_CORE_V1, 20_650);
    assert_eq!(PROOF_POINTS_PER_CORE_V1, 237);
    assert_eq!(PROOF_SCALARS_PER_CORE_V1, 5);
    assert_eq!(CORE_PROOF_BYTES_V1, 7_981);
    assert_eq!(ALL_CORE_PROOF_BYTES_V1, 31_924);
    assert_eq!(OWNED_WIRE_BYTES_V1, 32_271);
    assert_eq!(RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1, 32_271);
    assert_eq!(
        RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_MAX_BYTES_V1,
        36_020
    );
    assert_eq!(
        RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1,
        6_747_974
    );
}

#[test]
fn numeric_rlwe_algebra_uses_actual_a_a_b_c0_c1_and_qpcs_pair() {
    let modulus = release_modulus_v1(0).expect("release modulus");
    let challenges = RelationChallengesV1 {
        gamma: 3,
        beta: 5,
        point: 7,
    };
    let factor = mod_add_v1(
        mod_pow_v1(challenges.point, RING_DEGREE_V1 as u64, modulus),
        1,
        modulus,
    );
    assert_ne!(factor, 0);
    let mut numeric = RnsNativeCrossFieldNumericEvaluationV1 {
        a: challenges.point,
        public_a: 11,
        public_b: 13,
        ciphertext_c0: [0; RECORDS_V1],
        ciphertext_c1: [0; RECORDS_V1],
        qpcs_product: mod_mul_v1(factor, 17, modulus),
        qpcs_opening_quotient: 17,
    };
    for record in 0..RECORDS_V1 {
        numeric.ciphertext_c0[record] = record as u64 + 19;
        numeric.ciphertext_c1[record] = 2 * record as u64 + 23;
    }
    let validated = validate_numeric_evaluation_v1(0, 0, modulus, challenges, numeric)
        .expect("valid numeric evaluation");
    assert_eq!(
        validated.key_evaluation,
        mod_add_v1(13, mod_mul_v1(5, 11, modulus), modulus)
    );
    let mut expected_c = 0;
    let mut gamma_power = 1;
    for record in 0..RECORDS_V1 {
        let record_c = mod_add_v1(
            numeric.ciphertext_c0[record],
            mod_mul_v1(5, numeric.ciphertext_c1[record], modulus),
            modulus,
        );
        expected_c = mod_add_v1(
            expected_c,
            mod_mul_v1(gamma_power, record_c, modulus),
            modulus,
        );
        gamma_power = mod_mul_v1(gamma_power, 3, modulus);
    }
    assert_eq!(validated.ciphertext_evaluation, expected_c);

    let mut wrong_a = numeric;
    wrong_a.a += 1;
    assert!(matches!(
        validate_numeric_evaluation_v1(0, 0, modulus, challenges, wrong_a),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation)
    ));
    let mut wrong_qpcs = numeric;
    wrong_qpcs.qpcs_product = mod_add_v1(wrong_qpcs.qpcs_product, 1, modulus);
    assert!(matches!(
        validate_numeric_evaluation_v1(0, 0, modulus, challenges, wrong_qpcs),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation)
    ));
}

struct QMaskPointSourceV1 {
    point: Point,
}

impl RnsNativeQMaskSCommitmentSourceV1 for QMaskPointSourceV1 {
    fn q_mask_s_digit_commitment_v1(
        &self,
        limb: usize,
        repetition: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        if limb >= LIMBS_V1
            || repetition >= REPETITIONS_V1
            || block >= BLOCKS_PER_RECORD_V1
            || digit >= Q_MASK_DIGITS_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(self.point)
    }
}

fn full_relation_points_v1() -> [u64; EVALUATIONS_V1] {
    let mut points = [0_u64; EVALUATIONS_V1];
    for limb in 0..LIMBS_V1 {
        let modulus = release_modulus_v1(limb).expect("release modulus");
        let mut candidate = 2_u64;
        for repetition in 0..REPETITIONS_V1 {
            while mod_add_v1(
                mod_pow_v1(candidate, RING_DEGREE_V1 as u64, modulus),
                1,
                modulus,
            ) == 0
                || mod_pow_v1(candidate, 4 * RING_DEGREE_V1 as u64, modulus) == 1
            {
                candidate = candidate.checked_add(1).expect("small point scan");
            }
            points[limb * REPETITIONS_V1 + repetition] = candidate;
            candidate = candidate.checked_add(1).expect("small point scan");
        }
    }
    points
}

fn full_relation_schedule_v1() -> RelationScheduleV1 {
    let point_source = QMaskPointSourceV1 {
        point: ZkAmsT256BulletproofSuiteV1::generators().h,
    };
    let q_mask_s_root = q_mask_s_root_v1(safe_axes_v1(), &point_source).expect("q-mask root");
    derive_relation_schedule_v1(
        bind_direct_q_mask_schedule_v1(
            axes_v1(),
            completed_qpcs_with_points_v1(
                600,
                q_mask_s_root,
                digest_v1(22),
                full_relation_points_v1(),
            ),
        )
        .expect("direct schedule binding"),
    )
    .expect("full relation schedule")
}

fn full_opening_masks_v1(
    schedule: &RelationScheduleV1,
    limb: usize,
    repetition: usize,
) -> Result<(Scalar, Scalar), RnsNativeCrossFieldRlweDirectErrorV1> {
    let modulus = release_modulus_v1(limb)?;
    let challenges = relation_challenges_v1(schedule, limb, repetition, modulus)?;
    let p_mod_q = t256_mod_q_v1(modulus);
    let block_step = mod_pow_v1(challenges.point, BLOCK_COORDINATES_V1 as u64, modulus);
    let mask_factor = mod_add_v1(
        mod_pow_v1(challenges.point, RING_DEGREE_V1 as u64, modulus),
        1,
        modulus,
    );
    if mask_factor == 0 {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation);
    }
    let mut positive = Scalar::zero();
    let mut negative = Scalar::zero();
    let mut gamma_power = 1;
    for _record in 0..RECORDS_V1 {
        let mut block_power = 1;
        for _block in 0..BLOCKS_PER_RECORD_V1 {
            let mut radix_power = 1;
            for _digit in 0..RADIX_DIGITS_V1 {
                let coefficient = mod_mul_v1(
                    mod_mul_v1(gamma_power, radix_power, modulus),
                    block_power,
                    modulus,
                );
                positive += Scalar::from_u64(coefficient);
                radix_power = mod_mul_v1(radix_power, RADIX_BASE_V1, modulus);
            }
            let r_weight = 0;
            let e0_weight = mod_mul_v1(
                mod_mul_v1(gamma_power, p_mod_q, modulus),
                block_power,
                modulus,
            );
            let e1_weight = mod_mul_v1(e0_weight, challenges.beta, modulus);
            for coefficient in [r_weight, e0_weight, e1_weight] {
                let coefficient = Scalar::from_u64(coefficient);
                // `small_signed + small_negative` is `2H` in the full fixture.
                positive += coefficient;
                positive += coefficient;
                negative += coefficient;
            }
            // `one_vector_commitment - final_borrow` is exactly `H`.
            negative += Scalar::from_u64(e0_weight);
            block_power = mod_mul_v1(block_power, block_step, modulus);
        }
        gamma_power = mod_mul_v1(gamma_power, challenges.gamma, modulus);
    }
    let mut block_power = 1;
    for _block in 0..BLOCKS_PER_RECORD_V1 {
        let mut radix_power = 1;
        for _digit in 0..Q_MASK_DIGITS_V1 {
            let coefficient = mod_mul_v1(
                mod_mul_v1(mask_factor, radix_power, modulus),
                block_power,
                modulus,
            );
            positive += Scalar::from_u64(coefficient);
            radix_power = mod_mul_v1(radix_power, RADIX_BASE_V1, modulus);
        }
        block_power = mod_mul_v1(block_power, block_step, modulus);
    }
    if positive.is_zero() || negative.is_zero() {
        return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
    }
    Ok((positive, negative))
}

struct FullZeroSourceV1 {
    points: [u64; EVALUATIONS_V1],
    positive_masks: [Scalar; EVALUATIONS_V1],
    negative_masks: [Scalar; EVALUATIONS_V1],
    numeric_taken: [bool; EVALUATIONS_V1],
    positive_taken: [bool; EVALUATIONS_V1],
    negative_taken: [bool; EVALUATIONS_V1],
    algebra_fault: Option<usize>,
}

impl FullZeroSourceV1 {
    fn new_v1(schedule: &RelationScheduleV1) -> Result<Self, RnsNativeCrossFieldRlweDirectErrorV1> {
        let mut points = [0_u64; EVALUATIONS_V1];
        let mut positive_masks = [Scalar::zero(); EVALUATIONS_V1];
        let mut negative_masks = [Scalar::zero(); EVALUATIONS_V1];
        for limb in 0..LIMBS_V1 {
            for repetition in 0..REPETITIONS_V1 {
                let ordinal = limb * REPETITIONS_V1 + repetition;
                points[ordinal] =
                    relation_challenges_v1(schedule, limb, repetition, release_modulus_v1(limb)?)?
                        .point;
                let (positive, negative) = full_opening_masks_v1(schedule, limb, repetition)?;
                positive_masks[ordinal] = positive;
                negative_masks[ordinal] = negative;
            }
        }
        Ok(Self {
            points,
            positive_masks,
            negative_masks,
            numeric_taken: [false; EVALUATIONS_V1],
            positive_taken: [false; EVALUATIONS_V1],
            negative_taken: [false; EVALUATIONS_V1],
            algebra_fault: None,
        })
    }

    fn with_algebra_fault_v1(mut self, ordinal: usize) -> Self {
        self.algebra_fault = Some(ordinal);
        self
    }

    fn ordinal_v1(
        limb: usize,
        repetition: usize,
    ) -> Result<usize, RnsNativeCrossFieldRlweDirectErrorV1> {
        if limb >= LIMBS_V1 || repetition >= REPETITIONS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(limb * REPETITIONS_V1 + repetition)
    }

    fn validate_record_block_v1(
        record: usize,
        block: usize,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        if record >= RECORDS_V1 || block >= BLOCKS_PER_RECORD_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(())
    }
}

impl RnsNativeQMaskSCommitmentSourceV1 for FullZeroSourceV1 {
    fn q_mask_s_digit_commitment_v1(
        &self,
        limb: usize,
        repetition: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        Self::ordinal_v1(limb, repetition)?;
        if block >= BLOCKS_PER_RECORD_V1 || digit >= Q_MASK_DIGITS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(ZkAmsT256BulletproofSuiteV1::generators().h)
    }
}

impl RnsNativeCrossFieldAuthoritativeSourceV1 for FullZeroSourceV1 {
    fn authoritative_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        axes_v1().source_binding_digest
    }

    fn take_numeric_evaluation_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        destination: &mut RnsNativeCrossFieldNumericEvaluationV1,
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let ordinal = Self::ordinal_v1(limb, repetition)?;
        if self.numeric_taken[ordinal] {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable);
        }
        self.numeric_taken[ordinal] = true;
        *destination = RnsNativeCrossFieldNumericEvaluationV1 {
            a: self.points[ordinal],
            public_a: 0,
            public_b: 0,
            ciphertext_c0: [0; RECORDS_V1],
            ciphertext_c1: [0; RECORDS_V1],
            qpcs_product: if self.algebra_fault == Some(ordinal) {
                1
            } else {
                0
            },
            qpcs_opening_quotient: 0,
        };
        Ok(())
    }

    fn message_radix_digit_commitment_v1(
        &self,
        record: usize,
        block: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        Self::validate_record_block_v1(record, block)?;
        if digit >= RADIX_DIGITS_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(ZkAmsT256BulletproofSuiteV1::generators().h)
    }

    fn small_signed_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        Self::validate_record_block_v1(record, block)?;
        if role >= SMALL_SOURCE_ROLES_V1 {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry);
        }
        Ok(ZkAmsT256BulletproofSuiteV1::generators().h)
    }

    fn small_negative_magnitude_commitment_v1(
        &self,
        record: usize,
        role: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        self.small_signed_commitment_v1(record, role, block)
    }

    fn comparator_final_borrow_commitment_v1(
        &self,
        record: usize,
        block: usize,
    ) -> Result<Point, RnsNativeCrossFieldRlweDirectErrorV1> {
        Self::validate_record_block_v1(record, block)?;
        let point = one_vector_commitment_v1() - ZkAmsT256BulletproofSuiteV1::generators().h;
        if point.is_identity() {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint);
        }
        Ok(point)
    }
}

impl RnsNativeCrossFieldQuotientOpeningSourceV1 for FullZeroSourceV1 {
    fn take_positive_quotient_owner_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let ordinal = Self::ordinal_v1(limb, repetition)?;
        if self.positive_taken[ordinal]
            || values.len() != BLOCK_COORDINATES_V1
            || quotient_bits.len() != QUOTIENT_BITS_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable);
        }
        self.positive_taken[ordinal] = true;
        values.fill(Scalar::zero());
        quotient_bits.fill(Scalar::zero());
        *commitment_mask = self.positive_masks[ordinal];
        Ok(())
    }

    fn take_negative_quotient_owner_v1(
        &mut self,
        limb: usize,
        repetition: usize,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
        quotient_bits: &mut [Scalar],
    ) -> Result<(), RnsNativeCrossFieldRlweDirectErrorV1> {
        let ordinal = Self::ordinal_v1(limb, repetition)?;
        if self.negative_taken[ordinal]
            || values.len() != BLOCK_COORDINATES_V1
            || quotient_bits.len() != QUOTIENT_BITS_V1
        {
            return Err(RnsNativeCrossFieldRlweDirectErrorV1::SourceUnavailable);
        }
        self.negative_taken[ordinal] = true;
        values.fill(Scalar::zero());
        quotient_bits.fill(Scalar::zero());
        *commitment_mask = self.negative_masks[ordinal];
        Ok(())
    }
}

struct FullRoundtripRandomV1(u64);

impl ProofRandomSource for FullRoundtripRandomV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        for byte in destination {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *byte = self.0 as u8;
        }
        Ok(())
    }
}

#[test]
fn q_mask_root_uses_only_pre_qpcs_safe_axes_and_minimal_point_source() {
    let source = QMaskPointSourceV1 {
        point: ZkAmsT256BulletproofSuiteV1::generators().g,
    };
    let safe = safe_axes_v1();
    let root = q_mask_s_root_v1(safe, &source).expect("safe q-mask root");

    let mut changed_post_qpcs = axes_v1();
    changed_post_qpcs.terminal_predecessor_binding_digest = digest_v1(41);
    changed_post_qpcs.candidate_inventory_axes =
        RnsNativePreDirectInventoryCandidateAxesV1::test_fixture_v1(digest_v1(42), digest_v1(43))
            .expect("changed candidate inventory axes");
    changed_post_qpcs.existing_radix_candidate_root = digest_v1(44);
    assert_eq!(
        safe.digest_v1().expect("safe axes"),
        changed_post_qpcs
            .pre_qpcs_safe_axes_v1()
            .digest_v1()
            .expect("unchanged safe axes")
    );
    assert_eq!(
        root,
        q_mask_s_root_v1(changed_post_qpcs.pre_qpcs_safe_axes_v1(), &source)
            .expect("post-qPCS-independent q-mask root")
    );
    assert_ne!(
        axes_v1().digest_v1().expect("first full axes"),
        changed_post_qpcs.digest_v1().expect("changed full axes")
    );

    let mut changed_safe = safe;
    changed_safe.source_mapping_digest = digest_v1(45);
    assert_ne!(
        root,
        q_mask_s_root_v1(changed_safe, &source).expect("changed safe q-mask root")
    );
}

#[test]
fn retained_qpcs_schedule_is_combined_with_candidate_pre_direct_axes_only_after_qpcs() {
    let first = derive_relation_schedule_v1(
        bind_direct_q_mask_schedule_v1(
            axes_v1(),
            completed_qpcs_v1(401, digest_v1(20), digest_v1(22), 7),
        )
        .expect("first binding"),
    )
    .expect("first schedule");
    let second = derive_relation_schedule_v1(
        bind_direct_q_mask_schedule_v1(
            axes_v1(),
            completed_qpcs_v1(402, digest_v1(21), digest_v1(23), 11),
        )
        .expect("second binding"),
    )
    .expect("second schedule");
    assert_ne!(first.bound.binding_digest, second.bound.binding_digest);
    assert_ne!(first.relation_seed, second.relation_seed);
    let modulus = release_modulus_v1(0).expect("release modulus");
    assert_ne!(
        relation_challenges_v1(&first, 0, 0, modulus)
            .expect("first challenge")
            .point,
        relation_challenges_v1(&second, 0, 0, modulus)
            .expect("second challenge")
            .point
    );
}

#[test]
fn completed_qpcs_lineage_rejects_wrong_session_and_is_one_shot() {
    let legacy_schedule = RnsNativeQpcsRelationScheduleV1::test_fixture_with_binding_v1(
        axes_v1().qpcs_parameter_digest,
        digest_v1(20),
        axes_v1().qpcs_pre_relation_transcript_digest,
        digest_v1(22),
        [7; EVALUATIONS_V1],
    );
    let legacy_qpcs_transcript = qpcs_transcript_fixture_v1(709);
    let legacy_qpcs_bound_transcript_state = legacy_qpcs_transcript.binding_digest();
    assert!(matches!(
        RnsNativeQpcsCompletedLineageV1::test_fixture_v1(
            legacy_schedule,
            legacy_qpcs_bound_transcript_state,
            legacy_qpcs_transcript,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let first_transcript = qpcs_transcript_fixture_v1(710);
    let first_qpcs_bound_transcript_state = first_transcript.binding_digest();
    let first_lineage = first_transcript.test_qpcs_relation_lineage_v1();
    let schedule = RnsNativeQpcsRelationScheduleV1::test_fixture_with_lineage_v1(
        axes_v1().qpcs_parameter_digest,
        digest_v1(20),
        axes_v1().qpcs_pre_relation_transcript_digest,
        digest_v1(22),
        [7; EVALUATIONS_V1],
        first_lineage,
    );
    let foreign_transcript = qpcs_transcript_fixture_v1(711);
    assert!(matches!(
        RnsNativeQpcsCompletedLineageV1::test_fixture_v1(
            schedule,
            first_qpcs_bound_transcript_state,
            foreign_transcript,
        ),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    ));

    let mut completed = completed_qpcs_v1(712, digest_v1(20), digest_v1(22), 7);
    let _ = completed
        .take_qpcs_transcript_v1()
        .expect("sole qPCS transcript");
    assert!(matches!(
        completed.take_qpcs_transcript_v1(),
        Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    ));
}

#[test]
fn claimed_cross_field_root_mismatch_rejects_and_obligation_is_consumed() {
    let mut wrong = relation_schedule_fixture_v1();
    let claimed = digest_v1(91);
    let _ = bind_claimed_root_v1(&mut wrong, claimed, 720);
    assert!(matches!(
        wrong
            .take_cross_field_root_equality_obligation_v1()
            .expect("claimed-root equality obligation")
            .discharge_v1(
                RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(digest_v1(92))
                    .expect("opaque direct-owned wrong-root fixture"),
            ),
        Err(super::super::rns_native_transcript::ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch)
    ));
    assert!(matches!(
        wrong.take_cross_field_root_equality_obligation_v1(),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));

    let mut matching = relation_schedule_fixture_v1();
    let _ = bind_claimed_root_v1(&mut matching, claimed, 721);
    matching
        .take_cross_field_root_equality_obligation_v1()
        .expect("matching equality obligation")
        .discharge_v1(
            RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(claimed)
                .expect("opaque direct-owned matching-root fixture"),
        )
        .expect("matching direct root");
}

#[test]
fn concrete_direct_root_bridge_is_matching_mismatch_and_one_shot() {
    let claimed_root = digest_v1(95);
    let successor = [0x5a, 0xa5, 0x33];
    let terminal = claim_equality_pending_fixture_v1(claimed_root, claimed_root, &successor, 725)
        .discharge_claimed_root_equality_v1()
        .expect("matching concrete direct root");
    assert_eq!(terminal.successor(), successor.as_slice());
    assert_eq!(terminal.binding_digest(), digest_v1(111));
    assert_eq!(terminal.q_mask_s_root(), digest_v1(112));
    assert_eq!(terminal.numeric_root(), digest_v1(113));
    assert_eq!(terminal.commitment_root(), digest_v1(114));

    assert!(matches!(
        claim_equality_pending_fixture_v1(claimed_root, digest_v1(96), &successor, 726)
            .discharge_claimed_root_equality_v1(),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));

    let mut schedule = relation_schedule_fixture_v1();
    let _ = bind_claimed_root_v1(&mut schedule, claimed_root, 727);
    let obligation = schedule
        .take_cross_field_root_equality_obligation_v1()
        .expect("one-shot concrete equality obligation");
    assert!(matches!(
        schedule.take_cross_field_root_equality_obligation_v1(),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));
    obligation
        .discharge_v1(
            RnsNativeCrossFieldRlweVerifiedCoreRootV1::test_fixture_v1(claimed_root)
                .expect("one-shot direct-owned root"),
        )
        .expect("matching one-shot discharge");
}

#[test]
fn concrete_direct_root_is_not_sibling_constructible_or_raw_dischargeable() {
    let direct = include_str!("rns_native_cross_field_rlwe_direct.rs");
    let root_surface = direct
        .split_once("pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1(")
        .expect("opaque direct verified root")
        .1
        .split_once("fn cross_field_core_root_v1(")
        .expect("verified-root surface boundary")
        .0;
    assert!(!root_surface.contains("pub(super) fn new"));
    assert!(!root_surface.contains("pub(super) fn from"));
    assert!(!root_surface.contains("impl From<"));
    assert!(!root_surface.contains("pub(super) const fn root"));
    assert!(!direct.contains("pub(super) fn verified_cross_field_core_root"));
    let fixture = root_surface
        .find("pub(super) fn test_fixture_v1(")
        .expect("test-only direct root fixture");
    assert!(root_surface[fixture.saturating_sub(32)..fixture].contains("#[cfg(test)]"));

    let transcript = include_str!("rns_native_transcript.rs");
    let discharge = transcript
        .split_once("pub(super) fn discharge_v1(")
        .expect("concrete transcript discharge")
        .1
        .split_once("/// Move-only transcript after canonical context/source validation.")
        .expect("concrete discharge boundary")
        .0;
    assert!(discharge.contains("recomputed_root: RnsNativeCrossFieldRlweVerifiedCoreRootV1"));
    assert!(!discharge.contains("recomputed_root: [u8; 32]"));
    assert!(!discharge.contains("fn discharge_v1<R>"));
    assert!(!transcript.contains("VerifiedCrossFieldCoreRootCapabilityV1"));
}

#[test]
fn claimed_relation_owner_keeps_matching_pre_global_and_final_chronology() {
    let schedule = relation_schedule_fixture_v1();
    let expected_qpcs = qpcs_transcript_fixture_v1(400);
    let prior_binding = expected_qpcs.binding_digest();
    let claimed_root = digest_v1(94);
    let expected_cross_field = expected_qpcs
        .bind_cross_field_root(claimed_root)
        .expect("matching expected cross-field stage");
    let expected_pre_global_binding = expected_cross_field.binding_digest();
    let expected_global_seed = expected_cross_field.global_lookup_challenge_seed();
    let global_root = transcript_digest_fixture_v1(724, 900);
    let zero_root = transcript_digest_fixture_v1(724, 901);
    let roots =
        ZkAmsMkheRnsNativeTerminalRootsV1::new(prior_binding, claimed_root, global_root, zero_root)
            .expect("matching terminal roots");

    let claimed = schedule
        .bind_claimed_terminal_roots_v1(roots)
        .expect("atomic claimed terminal chronology");
    assert!(claimed.schedule.has_claimed_cross_field_root_v1());
    assert_eq!(
        claimed
            .pre_global_capability
            .test_post_cross_field_binding_digest_v1(),
        expected_pre_global_binding
    );
    assert_eq!(
        claimed
            .pre_global_capability
            .test_global_lookup_challenge_seed_v1(),
        expected_global_seed
    );
    assert_eq!(
        claimed.final_challenge_seeds.cross_field_root(),
        claimed_root
    );
    assert_eq!(
        claimed.final_challenge_seeds.global_lookup_root(),
        global_root
    );
    assert_eq!(claimed.final_challenge_seeds.zero_padding_root(), zero_root);
    assert_eq!(
        claimed.final_challenge_seeds.global_lookup_challenge_seed(),
        expected_global_seed
    );
}

#[test]
fn claimed_frame_requires_the_inventorys_exact_terminal_transcript() {
    let schedule = relation_schedule_fixture_v1();
    let qpcs = qpcs_transcript_fixture_v1(400);
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        qpcs.binding_digest(),
        digest_v1(98),
        transcript_digest_fixture_v1(728, 900),
        transcript_digest_fixture_v1(728, 901),
    )
    .expect("matching terminal roots");
    let claimed = schedule
        .bind_claimed_terminal_roots_v1(roots)
        .expect("claimed relation");
    let exact = claimed.final_challenge_seeds.transcript_digest();
    validate_claimed_inventory_transcript_v1(&claimed, exact).expect("exact inventory transcript");
    let mut foreign = exact;
    foreign[0] ^= 1;
    assert_eq!(
        validate_claimed_inventory_transcript_v1(&claimed, foreign),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    );
    assert_eq!(
        validate_claimed_inventory_transcript_v1(&claimed, [0; DIGEST_BYTES_V1]),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    );
}

#[test]
fn direct_claim_rejects_a_foreign_qpcs_session_and_fails_closed() {
    let mut schedule = relation_schedule_fixture_v1();
    let foreign_prior = qpcs_transcript_fixture_v1(722).binding_digest();
    let foreign_roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        foreign_prior,
        digest_v1(93),
        transcript_digest_fixture_v1(722, 900),
        transcript_digest_fixture_v1(722, 901),
    )
    .expect("foreign terminal roots");
    let (foreign_claim, _) = foreign_roots.into_cross_field_claim_v1();
    assert!(matches!(
        schedule.bind_claimed_cross_field_root_v1(foreign_claim),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));
    assert!(
        !schedule
            .bound
            .completed_qpcs
            .has_unconsumed_qpcs_transcript_v1()
    );
    assert!(!schedule.has_claimed_cross_field_root_v1());
}

#[test]
fn atomic_claimed_relation_rejects_a_foreign_qpcs_session() {
    let schedule = relation_schedule_fixture_v1();
    let foreign_prior = qpcs_transcript_fixture_v1(723).binding_digest();
    let foreign_roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        foreign_prior,
        digest_v1(97),
        transcript_digest_fixture_v1(723, 900),
        transcript_digest_fixture_v1(723, 901),
    )
    .expect("foreign terminal roots");
    assert!(matches!(
        schedule.bind_claimed_terminal_roots_v1(foreign_roots),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));
}

#[test]
fn pre_direct_candidate_axes_have_no_production_raw_inventory_constructor() {
    let source = include_str!("rns_native_cross_field_rlwe_direct.rs");
    let declaration = source
        .find("pub(super) struct RnsNativePreDirectInventoryCandidateAxesV1")
        .expect("opaque candidate axes");
    let prefix = &source[declaration.saturating_sub(320)..declaration];
    assert!(!prefix.contains("derive(Clone"));
    assert!(!prefix.contains("derive(Copy"));
    assert!(!source.contains("impl Clone for RnsNativeCrossFieldRlweClaimedRelationV1"));
    assert!(!source.contains("impl Copy for RnsNativeCrossFieldRlweClaimedRelationV1"));
    let candidate_impl = source
        .split_once("impl RnsNativePreDirectInventoryCandidateAxesV1")
        .expect("candidate axes implementation")
        .1
        .split_once("impl RnsNativeCrossFieldRlweFixedAxesV1")
        .expect("candidate axes implementation boundary")
        .0;
    assert!(!candidate_impl.contains("pub(super) fn new"));
    assert!(!candidate_impl.contains("RnsNativeCrossFieldInventoryPrerequisiteV1"));
    let fixture = candidate_impl
        .find("fn test_fixture_v1(")
        .expect("test-only candidate fixture");
    assert!(candidate_impl[fixture.saturating_sub(32)..fixture].contains("#[cfg(test)]"));
}

#[test]
#[ignore = "full 4x16,384-gate GBP qualification; run explicitly on a high-memory host"]
fn full_gbp_typed_bind_seal_verify_discharge_and_mutations() {
    let prover_schedule = full_relation_schedule_v1();
    let prover_source = FullZeroSourceV1::new_v1(&prover_schedule).expect("prover source");
    let mut rng = FullRoundtripRandomV1(0x6a09_e667_f3bc_c909);
    let pending = prove_pending_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _, _>(
        prover_schedule,
        prover_source,
        &mut rng,
    )
    .expect("four real GBP cores");
    let claimed_root = pending.cross_field_core_root.0;
    let (bound, cross_field) =
        bind_to_terminal_transcript_v1(pending).expect("typed prover terminal bind");
    let prover_cross_binding = cross_field.binding_digest();
    let prover_global_challenge = cross_field.global_lookup_challenge_seed();
    let successor = prover_global_challenge.to_vec();
    let wire = bound.seal_v1(&successor).expect("typed successor seal");
    assert_eq!(wire.len(), OWNED_WIRE_BYTES_V1 + successor.len());

    let mut verifier_schedule = full_relation_schedule_v1();
    let verifier_cross_field = bind_claimed_root_v1(&mut verifier_schedule, claimed_root, 600);
    let verifier_source = FullZeroSourceV1::new_v1(&verifier_schedule).expect("verifier source");
    let equality_pending = verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
        verifier_schedule,
        verifier_source,
        &wire,
    )
    .expect("four real GBP verifications remain equality-pending");
    assert_eq!(equality_pending.successor(), successor.as_slice());
    assert_ne!(equality_pending.binding_digest(), [0; DIGEST_BYTES_V1]);
    let verified = equality_pending
        .discharge_claimed_root_equality_v1()
        .expect("concrete direct-root equality");
    assert_eq!(verified.successor(), successor.as_slice());
    assert_ne!(verified.binding_digest(), [0; DIGEST_BYTES_V1]);
    assert_eq!(verifier_cross_field.binding_digest(), prover_cross_binding);
    assert_eq!(
        verifier_cross_field.global_lookup_challenge_seed(),
        prover_global_challenge
    );

    let mut mutated_proof = wire.clone();
    let first_scalar =
        HEADER_BYTES_V1 + CORE_RECORD_HEADER_BYTES_V1 + FIXED_PROOF_POINTS_V1 * POINT_BYTES_V1;
    let replacement = if mutated_proof[first_scalar..first_scalar + SCALAR_BYTES_V1]
        .iter()
        .all(|byte| *byte == 0)
    {
        Scalar::one().to_le_bytes()
    } else {
        Scalar::zero().to_le_bytes()
    };
    mutated_proof[first_scalar..first_scalar + SCALAR_BYTES_V1].copy_from_slice(&replacement);
    let codec_offset = OWNED_WIRE_BYTES_V1 - CODEC_DIGEST_BYTES_V1;
    let repaired_codec = codec_digest_v1(&mutated_proof[..codec_offset]);
    mutated_proof[codec_offset..OWNED_WIRE_BYTES_V1].copy_from_slice(&repaired_codec);
    let mut mutated_schedule = full_relation_schedule_v1();
    let _ = bind_claimed_root_v1(&mut mutated_schedule, claimed_root, 601);
    let mutated_source = FullZeroSourceV1::new_v1(&mutated_schedule).expect("mutation source");
    assert!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            mutated_schedule,
            mutated_source,
            &mutated_proof,
        )
        .is_err()
    );

    let mut algebra_schedule = full_relation_schedule_v1();
    let _ = bind_claimed_root_v1(&mut algebra_schedule, claimed_root, 602);
    let algebra_source = FullZeroSourceV1::new_v1(&algebra_schedule)
        .expect("algebra source")
        .with_algebra_fault_v1(0);
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            algebra_schedule,
            algebra_source,
            &wire,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidNumericEvaluation)
    ));
}

#[test]
fn pending_owner_seal_and_frame_preflight_roundtrip_are_mutation_sensitive() {
    let pending = pending_fixture_v1();
    let qpcs_binding = pending
        .inputs
        .schedule
        .bound
        .completed_qpcs
        .qpcs_transcript_binding_digest_v1()
        .expect("pending qPCS binding");
    let (bound, cross_field) =
        bind_to_terminal_transcript_v1(pending).expect("typed terminal bind");
    let cross_field_binding = cross_field.binding_digest();
    let global_lookup_challenge = cross_field.global_lookup_challenge_seed();
    assert_ne!(cross_field_binding, qpcs_binding);
    assert_ne!(global_lookup_challenge, [0; DIGEST_BYTES_V1]);
    let successor = vec![0x5a; 17];
    let wire = bound.seal_v1(&successor).expect("canonical sealed frame");
    assert_eq!(
        wire.len(),
        RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1 + successor.len()
    );
    assert!(RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_FRAME_BYTES_V1 <= 36_020);
    let preflight = FramePreflightV1::decode_exact_v1(&wire).expect("source-free preflight");
    preflight
        .validate_schedule_v1(&relation_schedule_fixture_v1())
        .expect("qPCS-bound schedule header");
    let inputs = prepared_fixture_v1();
    let view = FrameViewV1::decode_exact_v1(&wire, &inputs).expect("decode");
    assert_eq!(view.successor, successor.as_slice());
    assert_eq!(view.core_proofs.len(), CORES_V1);

    let second_pending = pending_fixture_v1();
    let (second_bound, second_cross_field) =
        bind_to_terminal_transcript_v1(second_pending).expect("same typed terminal bind");
    assert_eq!(second_cross_field.binding_digest(), cross_field_binding);
    let alternate_wire = second_bound
        .seal_v1(&[0xa5, 0x5a])
        .expect("alternate successor seal");
    let alternate =
        FramePreflightV1::decode_exact_v1(&alternate_wire).expect("alternate successor preflight");
    assert_ne!(preflight.successor_digest, alternate.successor_digest);

    let mut mutated_core = wire.clone();
    mutated_core[HEADER_BYTES_V1 + CORE_RECORD_HEADER_BYTES_V1 + 17] ^= 1;
    assert!(matches!(
        FramePreflightV1::decode_exact_v1(&mutated_core),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));

    let mut mutated_successor = wire;
    let last = mutated_successor.len() - 1;
    mutated_successor[last] ^= 1;
    assert!(matches!(
        FramePreflightV1::decode_exact_v1(&mutated_successor),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));

    let mut mutated_owner = pending_fixture_v1();
    mutated_owner.proof_set_digest[0] ^= 1;
    assert!(matches!(
        bind_to_terminal_transcript_v1(mutated_owner),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));

    let mut mutated_root_owner = pending_fixture_v1();
    mutated_root_owner.cross_field_core_root.0[0] ^= 1;
    assert!(matches!(
        bind_to_terminal_transcript_v1(mutated_root_owner),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));

    let mut mutated_qpcs_owner = pending_fixture_v1();
    mutated_qpcs_owner.inputs.schedule.relation_seed[0] ^= 1;
    assert!(matches!(
        bind_to_terminal_transcript_v1(mutated_qpcs_owner),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));

    let mut rebuilt_with_changed_core = pending_fixture_v1();
    let scalar_offset = FIXED_PROOF_POINTS_V1 * POINT_BYTES_V1;
    rebuilt_with_changed_core.proofs[0][scalar_offset..scalar_offset + SCALAR_BYTES_V1]
        .copy_from_slice(&Scalar::from_u64(99).to_le_bytes());
    let changed_proofs = rebuilt_with_changed_core.proofs;
    let changed_transcripts = rebuilt_with_changed_core.transcript_digests;
    let changed = RnsNativeCrossFieldRlweFourCorePendingSealV1::from_parts_v1(
        prepared_fixture_v1(),
        changed_proofs,
        changed_transcripts,
    )
    .expect("changed core owner");
    let (_changed_bound, changed_cross_field) =
        bind_to_terminal_transcript_v1(changed).expect("changed typed terminal bind");
    assert_ne!(cross_field_binding, changed_cross_field.binding_digest());
}

#[test]
fn source_independent_preflight_rejects_before_source_or_rng_touch() {
    let touches = Cell::new(0);
    let mut rng = UntouchedRandomV1;
    assert!(matches!(
        prove_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &[],
            &mut rng,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidHeader)
    ));
    assert_eq!(touches.get(), 0);

    let oversized_successor =
        vec![0_u8; RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 + 1];
    assert!(matches!(
        prove_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &oversized_successor,
            &mut rng,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded)
    ));
    assert_eq!(touches.get(), 0);
    drop(oversized_successor);

    let malformed_geometry = vec![0_u8; MIN_WIRE_BYTES_V1];
    for malformed in [&[][..], malformed_geometry.as_slice()] {
        assert!(
            verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
                relation_schedule_fixture_v1(),
                TouchSourceV1 { touches: &touches },
                malformed,
            )
            .is_err()
        );
        assert_eq!(touches.get(), 0);
    }

    let oversized_wire = vec![0_u8; RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 + 1];
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &oversized_wire,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::ProofCapExceeded)
    ));
    assert_eq!(touches.get(), 0);
    drop(oversized_wire);

    let inputs = prepared_fixture_v1();
    let proofs = core::array::from_fn(core_proof_fixture_v1);
    let transcript_digests = core::array::from_fn(|core| digest_v1(50 + core as u8));
    let wire = encode_wire_v1(&inputs, &proofs, &transcript_digests, &[0x7a])
        .expect("structurally valid wire");
    let mut invalid_core_codec = wire.clone();
    let first_proof = HEADER_BYTES_V1 + CORE_RECORD_HEADER_BYTES_V1;
    invalid_core_codec[first_proof..first_proof + POINT_BYTES_V1].fill(0);
    let codec_offset = OWNED_WIRE_BYTES_V1 - CODEC_DIGEST_BYTES_V1;
    let repaired_codec = codec_digest_v1(&invalid_core_codec[..codec_offset]);
    invalid_core_codec[codec_offset..OWNED_WIRE_BYTES_V1].copy_from_slice(&repaired_codec);
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &invalid_core_codec,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidPoint)
    ));
    assert_eq!(touches.get(), 0);

    let mut invalid_scalar_codec = wire.clone();
    let first_scalar = first_proof + FIXED_PROOF_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar_codec[first_scalar..first_scalar + SCALAR_BYTES_V1].fill(0xff);
    let repaired_codec = codec_digest_v1(&invalid_scalar_codec[..codec_offset]);
    invalid_scalar_codec[codec_offset..OWNED_WIRE_BYTES_V1].copy_from_slice(&repaired_codec);
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &invalid_scalar_codec,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidScalar)
    ));
    assert_eq!(touches.get(), 0);

    let mut invalid_successor = wire.clone();
    let last = invalid_successor.len() - 1;
    invalid_successor[last] ^= 1;
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            relation_schedule_fixture_v1(),
            TouchSourceV1 { touches: &touches },
            &invalid_successor,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidIntegrity)
    ));
    assert_eq!(touches.get(), 0);

    let wrong_schedule = derive_relation_schedule_v1(
        bind_direct_q_mask_schedule_v1(
            axes_v1(),
            completed_qpcs_v1(403, digest_v1(21), digest_v1(23), 11),
        )
        .expect("wrong qPCS binding"),
    )
    .expect("wrong qPCS schedule");
    assert!(matches!(
        verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(
            wrong_schedule,
            TouchSourceV1 { touches: &touches },
            &wire,
        ),
        Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidContext)
    ));
    assert_eq!(touches.get(), 0);
}

#[test]
fn claimed_relation_surface_is_atomic_move_only_and_source_ordered() {
    let source = include_str!("rns_native_cross_field_rlwe_direct.rs");
    let declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweClaimedRelationV1")
        .expect("claimed-relation owner");
    let prefix = &source[declaration.saturating_sub(320)..declaration];
    assert!(!prefix.contains("derive(Clone"));
    assert!(!prefix.contains("derive(Copy"));
    let owner_surface = source[declaration..]
        .split_once("impl RelationScheduleV1")
        .expect("claimed-relation owner boundary")
        .0;
    assert!(!owner_surface.contains("pub(super) fn new"));
    assert!(!owner_surface.contains("pub(super) fn from"));
    assert!(!owner_surface.contains("pub(super) fn schedule"));
    assert!(!owner_surface.contains("pub(super) fn final_challenge"));

    let transition = source
        .split_once("pub(super) fn bind_claimed_terminal_roots_v1(")
        .expect("atomic claimed-root transition")
        .1
        .split_once("/// Bind the authenticated claimed root before the successor chain")
        .expect("atomic claimed-root transition boundary")
        .0;
    assert!(transition.contains("        mut self,"));
    assert!(!transition.contains("&mut self"));
    assert!(!transition.contains("ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1"));
    let split = transition
        .find("roots.into_cross_field_claim_v1()")
        .expect("typed terminal split");
    let claim = transition
        .find(".bind_claimed_cross_field_root_v1(claim)")
        .expect("typed claim bind");
    let successors = transition
        .find(".bind_remaining_terminal_roots_v1(remaining_roots)")
        .expect("ordered successor bind");
    let obligation = transition
        .find("self.cross_field_root_equality_obligation = Some(equality_obligation)")
        .expect("retained equality obligation");
    let owner = transition
        .find("Ok(RnsNativeCrossFieldRlweClaimedRelationV1")
        .expect("atomic claimed-relation owner");
    assert!(split < claim && claim < successors && successors < obligation && obligation < owner);

    let legacy = source
        .find("pub(super) fn bind_claimed_cross_field_root_v1(")
        .expect("test compatibility split helper");
    assert!(source[legacy.saturating_sub(32)..legacy].contains("#[cfg(test)]"));

    let frame_declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweClaimedFramePreflightV1")
        .expect("claimed-frame preflight owner");
    let frame_prefix = &source[frame_declaration.saturating_sub(320)..frame_declaration];
    assert!(!frame_prefix.contains("derive(Clone"));
    assert!(!frame_prefix.contains("derive(Copy"));
    assert!(!source.contains("impl Clone for RnsNativeCrossFieldRlweClaimedFramePreflightV1"));
    assert!(!source.contains("impl Copy for RnsNativeCrossFieldRlweClaimedFramePreflightV1"));
    assert!(!source.contains("impl Clone for RnsNativeCrossFieldRlweClaimedInventoryParentV1"));
    assert!(!source.contains("impl Copy for RnsNativeCrossFieldRlweClaimedInventoryParentV1"));
    assert_eq!(
        source
            .matches("pub(super) const fn pre_global_lookup_capability_v1(")
            .count(),
        1
    );
    let capability_forward = source
        .split_once("pub(super) const fn pre_global_lookup_capability_v1(")
        .expect("exact-parent opaque capability borrow")
        .1
        .split_once("pub(super) const fn inventory(")
        .expect("opaque capability borrow boundary")
        .0;
    assert!(capability_forward.contains(") -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1"));
    assert!(capability_forward.contains("&self.frame_core.claimed_relation.pre_global_capability"));
    assert!(!capability_forward.contains("final_challenge_seeds"));
    assert!(!capability_forward.contains("test_post_cross_field_binding_digest_v1"));
    assert!(!capability_forward.contains("test_global_lookup_challenge_seed_v1"));
    let mint = source
        .split_once("fn into_claimed_successor_v1(")
        .expect("one-shot claimed successor mint")
        .1
        .split_once("impl RelationScheduleV1")
        .expect("claimed successor mint boundary")
        .0;
    assert_eq!(mint.matches("let Self {").count(), 1);
    let destructure = mint.find("let Self {").expect("single owner destructure");
    let successor_claim = mint
        .find("frame_core.preflight.claimed_successor_slice_v1()")
        .expect("opaque successor claim");
    let carrier_mint = mint
        .find("RnsNativeClaimedSuccessorV1::from_direct_claim_v1(")
        .expect("generic carrier mint");
    assert!(destructure < successor_claim && successor_claim < carrier_mint);
    let preflight = source
        .split_once("pub(super) fn preflight_rns_native_cross_field_rlwe_claimed_frame_v1")
        .expect("claimed frame entry")
        .1
        .split_once("struct FrameViewV1")
        .expect("claimed frame entry boundary")
        .0;
    let transcript_match = preflight
        .find("validate_claimed_inventory_transcript_v1(")
        .expect("exact transcript match");
    let frame_decode = preflight
        .find("FramePreflightV1::decode_exact_v1(inventory.continuation())")
        .expect("exact direct frame decode");
    let schedule_match = preflight
        .find("preflight.validate_schedule_v1(&claimed_relation.schedule)")
        .expect("exact schedule match");
    let carrier = preflight
        .find(".into_claimed_successor_v1()")
        .expect("sole successor carrier");
    assert!(
        transcript_match < frame_decode
            && frame_decode < schedule_match
            && schedule_match < carrier
    );
    assert!(
        source.contains("RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 == 6_747_974")
    );
    let successor_token = source
        .find("pub(super) struct RnsNativeCrossFieldRlweClaimedSuccessorSliceV1")
        .expect("opaque direct successor token");
    let successor_surface = source[successor_token..]
        .split_once("struct FramePreflightV1")
        .expect("successor token boundary")
        .0;
    assert!(!successor_surface.contains("pub(super) fn new"));
    assert!(!successor_surface.contains("pub(super) fn from_raw"));
    assert_eq!(
        source
            .matches("RnsNativeCrossFieldRlweClaimedSuccessorSliceV1 {")
            .count(),
        1
    );
    let fixture = successor_surface
        .find("pub(super) fn test_fixture_v1")
        .expect("test-only successor fixture");
    assert!(successor_surface[fixture.saturating_sub(32)..fixture].contains("#[cfg(test)]"));
}

#[test]
fn source_and_transcript_privacy_invariants_are_source_settled() {
    let source = include_str!("rns_native_cross_field_rlwe_direct.rs");
    for needle in [
        "pub(super) trait RnsNativeQMaskSCommitmentSourceV1",
        "pub(super) trait RnsNativeCrossFieldAuthoritativeSourceV1:",
        "fn take_numeric_evaluation_v1(",
        "pub(super) trait RnsNativeCrossFieldQuotientOpeningSourceV1:",
        "fn take_positive_quotient_owner_v1(",
        "fn take_negative_quotient_owner_v1(",
        "VectorCommitmentOpening::take_mask_from_slot(",
        "value.clear_secret();",
        "rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1",
        "RnsNativeQpcsCompletedLineageV1",
        "pub(super) fn prepare_direct_relation_schedule_after_qpcs_v1<",
        "let q_mask_s_root = q_mask_s_root_v1(axes.pre_qpcs_safe_axes_v1(), source)?;",
        "if q_mask_s_root != qpcs_schedule.q_mask_s_root()",
        "let bound = bind_direct_q_mask_schedule_v1(axes, completed_qpcs)?;",
        "derive_relation_schedule_v1(bound)",
        "source.take_numeric_evaluation_v1(limb, repetition, &mut numeric)?;",
        "source-independent-successor-and-wire-structure/header/codec/cap-preflight-before-any-authoritative-source-call",
        "pub(super) struct RnsNativeCrossFieldRlweCoreRootV1(",
        "pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1(",
        "pub(super) struct RnsNativeCrossFieldRlweFourCorePendingSealV1",
        "pub(super) struct RnsNativeCrossFieldRlweTerminalBoundPendingSealV1",
        "pub(super) struct RnsNativeCrossFieldRlweClaimedRelationV1",
        "pub(super) struct RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1",
        "pub(super) struct RnsNativeCrossFieldRlweTerminalBoundVerifiedV1",
        "pub(super) struct RnsNativePreDirectInventoryCandidateAxesV1",
        "pub(super) fn bind_to_terminal_transcript_v1(",
        "pub(super) fn bind_claimed_terminal_roots_v1(",
        "pub(super) fn bind_claimed_cross_field_root_v1(",
        "pub(super) fn prove_rns_native_cross_field_rlwe_direct_pending_v1<",
        "pub(super) fn seal_v1(",
        "const PRE_QPCS_Q_MASK_TOKEN_INTEGRATED_V1: bool = false;",
        "const POST_CORE_INVENTORY_LINK_INTEGRATED_V1: bool = false;",
        "const PRE_DIRECT_CANDIDATE_AXIS_CONTRACT_SETTLED_V1: bool = true;",
        "const PRODUCTION_PRE_DIRECT_INVENTORY_AXES_INTEGRATED_V1: bool = false;",
        "const STAGED_TERMINAL_TRANSCRIPT_API_AVAILABLE_V1: bool = true;",
        "const DIRECT_VERIFIED_ROOT_TYPE_BRIDGE_INTEGRATED_V1: bool = true;",
        "const DIRECT_STAGED_TERMINAL_ADAPTER_INTEGRATED_V1: bool = false;",
        "const COMPOSITE_ACCEPTANCE_AVAILABLE_V1: bool = false;",
        "const MEASURED_RSS_QUALIFIED_V1: bool = false;",
        "const RELEASE_READY_V1: bool = false;",
    ] {
        assert!(source.contains(needle), "missing invariant: {needle}");
    }
    assert!(!source.contains("fn derive_qpcs_relation_point_v1("));
    assert!(!source.contains("QPCS_RELATION_POINT_DOMAIN_V1"));
    assert!(!source.contains("STAGED_TERMINAL_ROOT_TRANSCRIPT_INTEGRATED_V1"));
    assert!(!source.contains("pub(super) const fn cross_field_core_root("));
    assert!(!source.contains("pub(super) const fn core_transcript_digest("));
    assert!(!source.contains("ZkAmsMkheRnsNativeVerifiedCrossFieldCoreRootCapabilityV1"));
    assert!(!source.contains("ZkAmsMkheRnsNativeVerifiedCrossFieldCoreRootV1"));
    let root_capability = source
        .split_once("pub(super) struct RnsNativeCrossFieldRlweCoreRootV1(")
        .expect("opaque root capability")
        .0;
    let root_prefix = &root_capability[root_capability.len().saturating_sub(320)..];
    assert!(!root_prefix.contains("derive(Clone"));
    assert!(!root_prefix.contains("derive(Copy"));
    let verified_root_declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1(")
        .expect("opaque verifier-derived root capability");
    let verified_root_prefix =
        &source[verified_root_declaration.saturating_sub(320)..verified_root_declaration];
    assert!(!verified_root_prefix.contains("derive(Clone"));
    assert!(!verified_root_prefix.contains("derive(Copy"));
    let verified_root_surface = source[verified_root_declaration..]
        .split_once("fn cross_field_core_root_v1(")
        .expect("verified-root implementation boundary")
        .0;
    assert!(verified_root_surface.contains("fn matches_claimed_cross_field_root_v1("));
    assert!(!verified_root_surface.contains("pub(super) fn new"));
    assert!(!verified_root_surface.contains("pub(super) fn from"));
    assert!(!verified_root_surface.contains("impl From<"));
    assert!(!verified_root_surface.contains("pub(super) const fn root"));
    let verified_root_fixture = verified_root_surface
        .find("pub(super) fn test_fixture_v1(")
        .expect("test-only direct-root fixture");
    assert!(
        verified_root_surface[verified_root_fixture.saturating_sub(32)..verified_root_fixture]
            .contains("#[cfg(test)]")
    );
    let equality_pending_declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1")
        .expect("claim-equality-pending verifier owner");
    let equality_pending_prefix =
        &source[equality_pending_declaration.saturating_sub(360)..equality_pending_declaration];
    assert!(!equality_pending_prefix.contains("derive(Clone"));
    assert!(!equality_pending_prefix.contains("derive(Copy"));
    let equality_pending_owner = source[equality_pending_declaration..]
        .split_once("impl<'a> RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1<'a>")
        .expect("claim-equality-pending owner boundary")
        .0;
    assert!(equality_pending_owner.contains("cross_field_root_equality_obligation:"));
    assert!(
        equality_pending_owner.contains("ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1")
    );
    assert!(
        equality_pending_owner
            .contains("verified_cross_field_core_root: RnsNativeCrossFieldRlweVerifiedCoreRootV1")
    );
    let equality_transition = source
        .split_once("pub(super) fn discharge_claimed_root_equality_v1(")
        .expect("concrete one-shot equality transition")
        .1
        .split_once("pub(super) const fn successor(&self)")
        .expect("equality transition boundary")
        .0;
    assert!(equality_transition.contains("self,"));
    assert!(equality_transition.contains("} = self;"));
    assert!(equality_transition.contains(".discharge_v1(verified_cross_field_core_root)"));
    assert!(equality_transition.contains("Ok(RnsNativeCrossFieldRlweTerminalBoundVerifiedV1"));
    let terminal_declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweTerminalBoundVerifiedV1")
        .expect("terminal-bound verified owner");
    let terminal_prefix = &source[terminal_declaration.saturating_sub(360)..terminal_declaration];
    assert!(!terminal_prefix.contains("derive(Clone"));
    assert!(!terminal_prefix.contains("derive(Copy"));
    let pending_declaration = source
        .find("pub(super) struct RnsNativeCrossFieldRlweFourCorePendingSealV1")
        .expect("pending owner declaration");
    let pending_prefix = &source[pending_declaration.saturating_sub(260)..pending_declaration];
    assert!(!pending_prefix.contains("derive(Clone"));
    assert!(!pending_prefix.contains("derive(Copy"));
    let pending_impl = source
        .split_once("impl RnsNativeCrossFieldRlweFourCorePendingSealV1")
        .expect("pending owner implementation")
        .1
        .split_once("/// Four-core owner after its opaque root")
        .expect("pending owner implementation boundary")
        .0;
    assert!(!pending_impl.contains("pub(super) fn seal_v1("));
    assert!(pending_impl.contains("fn seal_preflighted_v1("));
    let compatibility_seal = pending_impl
        .find("fn seal_preflighted_v1(")
        .expect("test-only compatibility seal");
    assert!(
        pending_impl[compatibility_seal.saturating_sub(24)..compatibility_seal]
            .contains("#[cfg(test)]")
    );
    let typed_bind = source
        .split_once("pub(super) fn bind_to_terminal_transcript_v1(")
        .expect("typed terminal bind")
        .1
        .split_once("/// Equality-pending owner")
        .expect("typed terminal bind boundary")
        .0;
    let typed_bind_signature = typed_bind
        .split_once(") -> Result<")
        .expect("typed terminal bind signature")
        .0;
    assert!(!typed_bind_signature.contains("transcript:"));
    assert!(typed_bind.contains(".take_qpcs_transcript_v1()"));
    assert!(typed_bind.contains(".bind_cross_field_root(pending.cross_field_core_root.0)"));
    assert!(typed_bind.contains("cross_field_core_root: _,"));
    let bound_pending = source
        .split_once("pub(super) struct RnsNativeCrossFieldRlweTerminalBoundPendingSealV1")
        .expect("terminal-bound pending owner")
        .1
        .split_once("impl RnsNativeCrossFieldRlweTerminalBoundPendingSealV1")
        .expect("terminal-bound pending boundary")
        .0;
    assert!(!bound_pending.contains("cross_field_core_root"));
    assert!(!bound_pending.contains("core_transcript_digest"));
    let fixed_axes = source
        .split_once("pub(super) struct RnsNativeCrossFieldRlweFixedAxesV1")
        .expect("fixed axes")
        .1
        .split_once("/// Opaque successor-independent inventory projection")
        .expect("fixed axes boundary")
        .0;
    for candidate_axis in [
        "terminal_predecessor_binding_digest",
        "candidate_inventory_axes: RnsNativePreDirectInventoryCandidateAxesV1",
        "existing_radix_candidate_root",
    ] {
        assert!(
            fixed_axes.contains(candidate_axis),
            "missing future pre-direct candidate axis: {candidate_axis}"
        );
    }
    for prohibited_current_inventory_axis in [
        "pub(super) inventory_prior_context_digest:",
        "pub(super) inventory_root:",
    ] {
        assert!(
            !fixed_axes.contains(prohibited_current_inventory_axis),
            "current successor-dependent inventory axis present: {prohibited_current_inventory_axis}"
        );
    }
    for self_referential_axis in [
        "cross_proof_digest",
        "cross_link_digest",
        "packing_binding_digest",
        "inventory_binding_digest",
        "continuation_digest",
        "radix_binding_digest",
    ] {
        assert!(
            !fixed_axes.contains(self_referential_axis),
            "self-referential direct-core axis present: {self_referential_axis}"
        );
    }
    let candidate_axes = source
        .split_once("pub(super) struct RnsNativePreDirectInventoryCandidateAxesV1")
        .expect("opaque pre-direct candidate axes")
        .1
        .split_once("impl RnsNativeCrossFieldRlweFixedAxesV1")
        .expect("opaque candidate axes boundary")
        .0;
    assert!(candidate_axes.contains("context_digest: [u8; DIGEST_BYTES_V1]"));
    assert!(candidate_axes.contains("inventory_root: [u8; DIGEST_BYTES_V1]"));
    assert!(!candidate_axes.contains("RnsNativeCrossFieldInventoryPrerequisiteV1"));
    assert!(!candidate_axes.contains("pub(super) fn new"));
    let candidate_fixture = candidate_axes
        .find("fn test_fixture_v1(")
        .expect("test-only candidate axes fixture");
    assert!(
        candidate_axes[candidate_fixture.saturating_sub(32)..candidate_fixture]
            .contains("#[cfg(test)]")
    );
    let q_mask_root = source
        .split_once("pub(super) fn q_mask_s_root_v1")
        .expect("q-mask root")
        .1
        .split_once("fn bind_direct_q_mask_schedule_v1")
        .expect("q-mask root boundary")
        .0;
    for post_qpcs_axis in [
        "terminal_predecessor_binding_digest",
        "candidate_inventory_axes",
        "RnsNativePreDirectInventoryCandidateAxesV1",
        "existing_radix_candidate_root",
        "RnsNativeCrossFieldRlweFixedAxesV1",
    ] {
        assert!(
            !q_mask_root.contains(post_qpcs_axis),
            "post-qPCS axis leaked into q-mask root: {post_qpcs_axis}"
        );
    }
    for required_inventory_no_go in [
        "current inventory `prior_context_digest_v1` and canonical inventory root",
        "cross-field, global-lookup, and zero-padding roots",
        "final transcript state and challenges",
        "cross-section and zero-padding digests",
        "continuation state",
        "current-inventory-prior-context-and-canonical-root-must-not-be-adapted",
    ] {
        assert!(
            source.contains(required_inventory_no_go),
            "missing current-inventory NO-GO contract: {required_inventory_no_go}"
        );
    }
    let chronology = source
        .split_once("pub(super) fn prepare_direct_relation_schedule_after_qpcs_v1")
        .expect("chronology function")
        .1
        .split_once("fn validate_relation_schedule_v1")
        .expect("chronology boundary")
        .0;
    let root = chronology.find("q_mask_s_root_v1").expect("S root");
    let bind = chronology
        .find("bind_direct_q_mask_schedule_v1")
        .expect("post-qPCS direct bind");
    let schedule = chronology
        .find("derive_relation_schedule_v1")
        .expect("relation schedule");
    assert!(root < bind && bind < schedule);

    let prepare = source
        .split_once("fn prepare_inputs_v1")
        .expect("prepare function")
        .1
        .split_once("fn boolean_constraints_v1")
        .expect("prepare boundary")
        .0;
    let schedule = prepare
        .find("validate_relation_schedule_v1")
        .expect("validate schedule");
    let root = prepare.find("q_mask_s_root_v1").expect("recheck S root");
    let numeric = prepare
        .find("take_numeric_evaluation_v1")
        .expect("numeric source");
    assert!(schedule < root && root < numeric);

    let verifier = source
        .split_once("fn verify_kernel_for_suite_v1")
        .expect("direct verifier")
        .1
        .split_once("/// Produce the four successor-independent direct cores")
        .expect("direct verifier boundary")
        .0;
    let frame_preflight = verifier
        .find("FramePreflightV1::decode_exact_v1(wire)")
        .expect("source-independent frame preflight");
    let claim_check = verifier
        .find("has_claimed_cross_field_root_v1()")
        .expect("typed root claim check");
    let source_traversal = verifier
        .find("prepare_inputs_v1(schedule, &mut source)")
        .expect("authoritative source traversal");
    assert!(frame_preflight < claim_check && claim_check < source_traversal);
    assert!(verifier.contains(".take_cross_field_root_equality_obligation_v1()?"));
    assert!(verifier.contains("RnsNativeCrossFieldRlweVerifiedCoreRootV1(cross_field_core_root)"));
    assert!(verifier.contains("RnsNativeCrossFieldRlweClaimEqualityPendingVerifiedV1"));
    assert!(verifier.contains("cross_field_root_equality_obligation,"));
    assert!(verifier.contains("verified_cross_field_core_root,"));
    assert!(!verifier.contains(".discharge_v1("));

    let encoder = source
        .split_once("fn encode_wire_v1")
        .expect("encoder")
        .1
        .split_once("struct DecoderV1")
        .expect("encoder boundary")
        .0;
    for secret_name in [
        "ciphertext_c0",
        "ciphertext_c1",
        "positive_values",
        "negative_values",
        "quotient_bits",
    ] {
        assert!(!encoder.contains(secret_name));
    }

    let core_transcript = source
        .split_once("fn initial_core_transcript_state_v1")
        .expect("direct core transcript")
        .1
        .split_once("struct CoreProverTranscriptV1")
        .expect("direct core transcript boundary")
        .0;
    for post_core_value in [
        "successor_digest",
        "proof_set_digest",
        "codec_digest",
        "final_binding",
    ] {
        assert!(
            !core_transcript.contains(post_core_value),
            "post-core value leaked into direct challenge: {post_core_value}"
        );
    }
    let final_binding = source
        .split_once("fn final_binding_digest_v1")
        .expect("post-core final binding")
        .1
        .split_once("/// Move-only four-core proof owner")
        .expect("post-core final binding boundary")
        .0;
    assert!(final_binding.contains("view.successor_digest"));
    assert!(final_binding.contains("cross_field_core_root"));

    let legacy_prover = source
        .split_once("fn prove_kernel_for_suite_v1")
        .expect("legacy prover")
        .1
        .split_once("fn verify_kernel_for_suite_v1")
        .expect("legacy prover boundary")
        .0;
    let successor_preflight = legacy_prover
        .find("SuccessorPreflightV1::new_v1(successor)")
        .expect("successor preflight");
    let pending_proof = legacy_prover
        .find("prove_pending_kernel_for_suite_v1")
        .expect("pending proof");
    assert!(successor_preflight < pending_proof);
    let legacy_declaration = source
        .find("fn prove_kernel_for_suite_v1")
        .expect("legacy declaration");
    assert!(
        source[legacy_declaration.saturating_sub(24)..legacy_declaration].contains("#[cfg(test)]")
    );
    let public_legacy = source
        .find("pub(super) fn prove_rns_native_cross_field_rlwe_direct_v1")
        .expect("public legacy declaration");
    assert!(source[public_legacy.saturating_sub(240)..public_legacy].contains("#[cfg(test)]"));

    let verifier = source
        .split_once("fn verify_kernel_for_suite_v1")
        .expect("verifier")
        .1
        .split_once("/// Produce the four successor-independent direct cores")
        .expect("verifier boundary")
        .0;
    let wire_preflight = verifier
        .find("FramePreflightV1::decode_exact_v1(wire)")
        .expect("wire preflight");
    let schedule_preflight = verifier
        .find("preflight.validate_schedule_v1(&schedule)")
        .expect("schedule preflight");
    let source_traversal = verifier
        .find("prepare_inputs_v1(schedule, &mut source)")
        .expect("source traversal");
    assert!(wire_preflight < schedule_preflight && schedule_preflight < source_traversal);

    let transcript = include_str!("rns_native_transcript.rs");
    let qpcs_terminal = transcript
        .split_once("pub(super) fn bind_cross_field_root(")
        .expect("staged cross-field root")
        .1
        .split_once("/// Verifier convenience")
        .expect("staged cross-field boundary")
        .0;
    assert!(qpcs_terminal.contains("ChallengePurposeV1::GlobalLookup"));
    let cross_field_terminal = transcript
        .split_once("pub(super) fn bind_global_lookup_root(")
        .expect("staged global-lookup root")
        .1
        .split_once("/// Move-only transcript after the global-lookup root")
        .expect("staged global-lookup boundary")
        .0;
    assert!(cross_field_terminal.contains("ChallengePurposeV1::ZeroPadding"));
    assert!(transcript.contains("pub(super) fn bind_zero_padding_root("));
    let convenience = transcript
        .split_once("pub fn bind_terminal_roots(")
        .expect("terminal convenience")
        .1
        .split_once("/// Move-only transcript after the cross-field root")
        .expect("terminal convenience boundary")
        .0;
    let cross = convenience
        .find("bind_cross_field_root")
        .expect("cross bind");
    let global = convenience
        .find("bind_global_lookup_root")
        .expect("global bind");
    let zero = convenience
        .find("bind_zero_padding_root")
        .expect("zero bind");
    assert!(cross < global && global < zero);

    let facade = include_str!("../mkhe.rs");
    assert!(facade.contains(
        "#[path = \"mkhe/rns_native_cross_field_rlwe_direct.rs\"]\nmod rns_native_cross_field_rlwe_direct;"
    ));
    assert!(!facade.contains("pub mod rns_native_cross_field_rlwe_direct;"));
    assert!(facade.contains(
        "#[path = \"mkhe/rns_native_public_polynomial_reader.rs\"]\nmod rns_native_public_polynomial_reader;"
    ));
    assert!(facade.contains(
        "#[path = \"mkhe/rns_native_source_packing_same_opening.rs\"]\nmod rns_native_source_packing_same_opening;"
    ));
}
