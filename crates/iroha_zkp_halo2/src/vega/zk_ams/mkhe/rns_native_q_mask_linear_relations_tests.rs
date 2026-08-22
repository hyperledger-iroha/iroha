use std::sync::OnceLock;

use super::*;
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitWitness, ProofGenerators, ProofRandomSource, ProverTranscript,
        VectorCommitmentOpening, multiexp,
    },
    vega::derive_t256_generators_v1,
};

fn test_points_v1() -> [Point; 2] {
    let points =
        derive_t256_generators_v1(b"rns-native-q-mask-linear-codec-test", 2).expect("points");
    [points[0], points[1]]
}

fn test_commitments_v1() -> QMaskLinearCommitmentsV1 {
    let [digit, complement] = test_points_v1();
    QMaskLinearCommitmentsV1 {
        digits: [digit; RADIX_DIGITS_V1],
        complement_digits: [complement; RADIX_DIGITS_V1],
    }
}

fn upstream_v1() -> UpstreamBindingV1 {
    UpstreamBindingV1 {
        prior_context_digest: [0x11; DIGEST_BYTES_V1],
        inventory_root: [0x22; DIGEST_BYTES_V1],
        statement3_proof_set_root: [0x33; DIGEST_BYTES_V1],
        statement3_verified_transcript_root: [0x44; DIGEST_BYTES_V1],
        statement5_proof_set_root: [0x55; DIGEST_BYTES_V1],
        statement5_verified_transcript_root: [0x66; DIGEST_BYTES_V1],
        statement8_proof_set_root: [0x77; DIGEST_BYTES_V1],
        statement8_verified_transcript_root: [0x88; DIGEST_BYTES_V1],
    }
}

fn canonical_core_v1() -> Vec<u8> {
    let point = encode_point_v1(test_points_v1()[0]).expect("canonical point");
    let scalar = Scalar::zero().to_le_bytes();
    let mut core = Vec::with_capacity(CORE_BYTES_V1);
    for _ in 0..FIXED_CORE_POINTS_V1 {
        core.extend_from_slice(&point);
    }
    for _ in 0..3 {
        core.extend_from_slice(&scalar);
    }
    for _ in 0..IPA_POINTS_V1 {
        core.extend_from_slice(&point);
    }
    for _ in 0..2 {
        core.extend_from_slice(&scalar);
    }
    assert_eq!(core.len(), CORE_BYTES_V1);
    core
}

fn canonical_records_v1() -> Vec<u8> {
    let core = canonical_core_v1();
    let mut records = Vec::with_capacity(RECORD_SET_BYTES_V1);
    for relation in 0..RELATIONS_V1 {
        records.extend_from_slice(&(relation as u16).to_be_bytes());
        records.extend_from_slice(&(CORE_BYTES_V1 as u16).to_be_bytes());
        records.extend_from_slice(&core);
    }
    assert_eq!(records.len(), RECORD_SET_BYTES_V1);
    records
}

fn canonical_wire_v1(upstream: UpstreamBindingV1, residual: &[u8]) -> Vec<u8> {
    let records = canonical_records_v1();
    let commitments = test_commitments_v1();
    let proof_set_root = canonical_proof_set_root_v1(upstream, &records, |_| Some(commitments))
        .expect("proof-set root");
    let residual_digest =
        canonical_residual_digest_v1(upstream, proof_set_root, residual).expect("residual digest");
    let total = HEADER_BYTES_V1 + records.len() + residual.len() + CODEC_DIGEST_BYTES_V1;
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.push(VERSION_V1);
    wire.push(FLAGS_V1);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(total as u32).to_be_bytes());
    wire.extend_from_slice(&[
        FIRST_STATEMENT_V1,
        LAST_STATEMENT_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u8,
        REPETITIONS_V1 as u8,
        BLOCKS_PER_RELATION_V1 as u8,
        RADIX_DIGITS_V1 as u8,
    ]);
    wire.extend_from_slice(&(RELATIONS_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(COORDINATES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&(CORES_V1 as u16).to_be_bytes());
    for value in [GATES_V1, PADDED_GATES_V1, CONSTRAINTS_PER_CORE_V1] {
        wire.extend_from_slice(&(value as u32).to_be_bytes());
    }
    wire.extend_from_slice(&[
        COMMITMENTS_PER_CORE_V1 as u8,
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
        LOG_PADDED_GATES_V1 as u8,
    ]);
    wire.extend_from_slice(&(CORE_BYTES_V1 as u32).to_be_bytes());
    for digest in upstream.digests_v1() {
        wire.extend_from_slice(&digest);
    }
    wire.extend_from_slice(&proof_set_root);
    wire.extend_from_slice(&residual_digest);
    wire.extend_from_slice(&(residual.len() as u32).to_be_bytes());
    assert_eq!(wire.len(), HEADER_BYTES_V1);
    wire.extend_from_slice(&records);
    wire.extend_from_slice(residual);
    let codec_digest = codec_digest_v1(&wire);
    wire.extend_from_slice(&codec_digest);
    assert_eq!(wire.len(), total);
    wire
}

#[test]
fn q_mask_linear_codec_is_exact_canonical_capped_and_acyclically_bound() {
    let upstream = upstream_v1();
    let residual = b"remaining-digit-membership-qpcs-same-opening-and-global-lookup";
    let wire = canonical_wire_v1(upstream, residual);
    let commitments = test_commitments_v1();
    let view =
        QMaskLinearProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(commitments))
            .expect("canonical q-mask linear proof set");
    assert_eq!(view.records.len(), RECORD_SET_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert!(view.core_v1(0).is_ok());
    assert!(view.core_v1(RELATIONS_V1 - 1).is_ok());
    assert_ne!(view.proof_set_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);

    let mut changed = upstream;
    changed.statement8_verified_transcript_root[0] ^= 1;
    assert_eq!(
        QMaskLinearProofSetViewV1::from_components_v1(&wire, changed, |_| Some(commitments))
            .map(|_| ()),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidHeader)
    );
    assert!(
        QMaskLinearProofSetViewV1::from_components_v1(&wire[..wire.len() - 1], upstream, |_| {
            Some(commitments)
        })
        .is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(
        QMaskLinearProofSetViewV1::from_components_v1(&trailing, upstream, |_| Some(commitments))
            .is_err()
    );
    let cap_plus_one = vec![0_u8; RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        QMaskLinearProofSetViewV1::from_components_v1(&cap_plus_one, upstream, |_| Some(
            commitments
        ))
        .map(|_| ()),
        Err(RnsNativeQMaskLinearRelationsErrorV1::ProofCapExceeded)
    );
}

#[test]
fn real_statement8_residual_hashes_complete_q_mask_wire_without_a_cycle() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream-after-q-mask-linear");
    let statement8_upstream =
        super::super::rns_native_small_sign_disjointness_product::UpstreamBindingV1 {
            prior_context_digest: upstream.prior_context_digest,
            inventory_root: upstream.inventory_root,
            statement3_proof_set_root: upstream.statement3_proof_set_root,
            statement3_verified_transcript_root: upstream.statement3_verified_transcript_root,
            statement5_proof_set_root: upstream.statement5_proof_set_root,
            statement5_verified_transcript_root: upstream.statement5_verified_transcript_root,
        };
    let statement8_residual =
        super::super::rns_native_small_sign_disjointness_product::canonical_residual_digest_v1(
            statement8_upstream,
            upstream.statement8_proof_set_root,
            &wire,
        )
        .expect("statement-8 residual digest over complete nested q-mask wire");
    assert_ne!(statement8_residual, [0; DIGEST_BYTES_V1]);
    assert!(
        !wire
            .windows(DIGEST_BYTES_V1)
            .any(|window| window == statement8_residual.as_slice())
    );
}

#[test]
fn q_mask_codec_rejects_geometry_order_point_scalar_inventory_and_residual_mutations() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"nonempty-residual");
    let commitments = test_commitments_v1();
    let parse = |bytes: &[u8]| {
        QMaskLinearProofSetViewV1::from_components_v1(bytes, upstream, |_| Some(commitments))
            .map(|_| ())
    };

    let mut geometry = wire.clone();
    geometry[13] ^= 1;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry)
    );
    let mut order = wire.clone();
    order[HEADER_BYTES_V1 + 1] = 1;
    assert_eq!(
        parse(&order),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidGeometry)
    );
    let core = HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1;
    let mut invalid_point = wire.clone();
    invalid_point[core..core + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidPoint)
    );
    let mut invalid_scalar = wire.clone();
    let scalar = core + FIXED_CORE_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar[scalar..scalar + SCALAR_BYTES_V1].fill(0xff);
    assert_eq!(
        parse(&invalid_scalar),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidScalar)
    );

    let mut changed = commitments;
    changed.digits[0] += test_points_v1()[1];
    assert_eq!(
        QMaskLinearProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(changed))
            .map(|_| ()),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity)
    );
    let mut changed_residual = wire.clone();
    changed_residual[HEADER_BYTES_V1 + RECORD_SET_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&changed_residual),
        Err(RnsNativeQMaskLinearRelationsErrorV1::InvalidIntegrity)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyQMaskSuiteV1;

impl ProofSuite for TinyQMaskSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyQMaskSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points = derive_t256_generators_v1(b"rns-native-q-mask-linear-tiny-suite-v1", 6)
                .expect("tiny generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..4].to_vec(),
                points[4..6].to_vec(),
            )
            .expect("valid tiny basis")
        })
    }
}

struct TestRandomV1 {
    seed: [u8; DIGEST_BYTES_V1],
    counter: u64,
}

impl TestRandomV1 {
    fn new(label: &[u8]) -> Self {
        Self {
            seed: hash_v1(label),
            counter: 0,
        }
    }
}

impl ProofRandomSource for TestRandomV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut input = Vec::with_capacity(40);
            input.extend_from_slice(&self.seed);
            input.extend_from_slice(&self.counter.to_be_bytes());
            let block = hash_v1(&input);
            self.counter = self
                .counter
                .checked_add(1)
                .ok_or(GeneralizedBulletproofErrorV1::RandomnessUnavailable)?;
            let count = (destination.len() - written).min(block.len());
            destination[written..written + count].copy_from_slice(&block[..count]);
            written += count;
        }
        Ok(())
    }
}

struct TestProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    proof: Vec<u8>,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<S> TestProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn new(
        upstream: UpstreamBindingV1,
        relation: usize,
        commitments: QMaskCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        basis: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            state: initial_transcript_state_v1(
                upstream,
                relation,
                commitments,
                coordinates,
                padded_gates,
                basis,
            )
            .expect("valid tiny transcript context"),
            proof: Vec::new(),
            challenge_ordinal: 0,
            suite: PhantomData,
        }
    }
}

impl<S> ProverTranscript<S> for TestProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn push_scalar(&mut self, scalar: &Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = scalar.to_le_bytes();
        self.state.push(0);
        self.state.extend_from_slice(&encoded);
        self.proof.extend_from_slice(&encoded);
        Ok(())
    }

    fn push_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded =
            encode_point_v1(*point).map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        self.state.push(1);
        self.state.extend_from_slice(&encoded);
        self.proof.extend_from_slice(&encoded);
        Ok(())
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_challenge_v1(&mut self.state, &mut self.challenge_ordinal)
    }
}

#[derive(Clone)]
struct RawValuesV1 {
    digits: [Vec<Scalar>; RADIX_DIGITS_V1],
    complement_digits: [Vec<Scalar>; RADIX_DIGITS_V1],
}

fn radix_digits_v1(value: u64) -> [Scalar; RADIX_DIGITS_V1] {
    core::array::from_fn(|digit| {
        Scalar::from_u64((value >> (digit * RADIX_LOG2_V1 as usize)) & (RADIX_BASE_V1 - 1))
    })
}

fn honest_values_v1() -> [RawValuesV1; BLOCKS_PER_RELATION_V1] {
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0];
    core::array::from_fn(|block| {
        let source = [
            block as u64 + 3,
            if block == 7 { 0 } else { block as u64 + 9 },
        ];
        let complement = source.map(|value| modulus - 1 - value);
        let source_digits = source.map(radix_digits_v1);
        let complement_digits = complement.map(radix_digits_v1);
        RawValuesV1 {
            digits: core::array::from_fn(|digit| {
                vec![source_digits[0][digit], source_digits[1][digit]]
            }),
            complement_digits: core::array::from_fn(|digit| {
                vec![complement_digits[0][digit], complement_digits[1][digit]]
            }),
        }
    })
}

fn commitment_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinyQMaskSuiteV1::generators();
    let mut terms = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect::<Vec<_>>();
    terms.push((mask, generators.h));
    multiexp::<TinyQMaskSuiteV1>(&terms)
}

fn weighted_values_v1(values: &[Vec<Scalar>; RADIX_DIGITS_V1]) -> Vec<Scalar> {
    let mut output = vec![Scalar::zero(); values[0].len()];
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    let mut weight = Scalar::one();
    for digit in values {
        for (output, value) in output.iter_mut().zip(digit) {
            *output += weight * *value;
        }
        weight *= radix;
    }
    output
}

fn weighted_masks_v1(masks: [Scalar; RADIX_DIGITS_V1]) -> Scalar {
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    let mut weight = Scalar::one();
    let mut output = Scalar::zero();
    for mask in masks {
        output += weight * mask;
        weight *= radix;
    }
    output
}

fn tiny_commitments_and_witness_v1(
    values: &[RawValuesV1; BLOCKS_PER_RELATION_V1],
) -> (
    QMaskCoreCommitmentsV1,
    ArithmeticCircuitWitness<TinyQMaskSuiteV1>,
) {
    let mut digit_masks = [[Scalar::zero(); RADIX_DIGITS_V1]; BLOCKS_PER_RELATION_V1];
    let mut complement_masks = [[Scalar::zero(); RADIX_DIGITS_V1]; BLOCKS_PER_RELATION_V1];
    let raw = core::array::from_fn(|block| {
        digit_masks[block] =
            core::array::from_fn(|digit| Scalar::from_u64(11 + (block * 8 + digit) as u64));
        complement_masks[block] =
            core::array::from_fn(|digit| Scalar::from_u64(15 + (block * 8 + digit) as u64));
        QMaskLinearCommitmentsV1 {
            digits: core::array::from_fn(|digit| {
                commitment_v1(&values[block].digits[digit], digit_masks[block][digit])
            }),
            complement_digits: core::array::from_fn(|digit| {
                commitment_v1(
                    &values[block].complement_digits[digit],
                    complement_masks[block][digit],
                )
            }),
        }
    });
    let commitments = QMaskCoreCommitmentsV1::new_v1(raw, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0])
        .expect("valid tiny q-mask commitments");
    let mut openings = Vec::with_capacity(COMMITMENTS_PER_CORE_V1);
    for block in 0..BLOCKS_PER_RELATION_V1 {
        let mut combined = weighted_values_v1(&values[block].digits);
        for (value, complement) in combined
            .iter_mut()
            .zip(weighted_values_v1(&values[block].complement_digits))
        {
            *value += complement;
        }
        openings.push(VectorCommitmentOpening::new(
            combined,
            weighted_masks_v1(digit_masks[block]) + weighted_masks_v1(complement_masks[block]),
        ));
    }
    openings.push(VectorCommitmentOpening::new(
        weighted_values_v1(&values[BLOCKS_PER_RELATION_V1 - 1].digits),
        weighted_masks_v1(digit_masks[BLOCKS_PER_RELATION_V1 - 1]),
    ));
    let zeros = vec![Scalar::zero(); values[0].digits[0].len()];
    let witness = ArithmeticCircuitWitness::new(zeros.clone(), zeros, openings)
        .expect("shape-valid q-mask witness");
    (commitments, witness)
}

type TinyProofV1 = (
    Vec<u8>,
    [u8; DIGEST_BYTES_V1],
    UpstreamBindingV1,
    QMaskCoreCommitmentsV1,
);

fn prove_tiny_v1(
    values: [RawValuesV1; BLOCKS_PER_RELATION_V1],
) -> Result<TinyProofV1, GeneralizedBulletproofErrorV1> {
    let coordinates = values[0].digits[0].len();
    let (commitments, witness) = tiny_commitments_and_witness_v1(&values);
    let upstream = upstream_v1();
    let basis = hash_v1(b"tiny-q-mask-linear-basis");
    let mut transcript = TestProverTranscriptV1::<TinyQMaskSuiteV1>::new(
        upstream,
        0,
        commitments,
        coordinates,
        coordinates,
        basis,
    );
    build_q_mask_linear_statement_v1::<TinyQMaskSuiteV1>(coordinates, coordinates, commitments)
        .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?
        .prove(
            &mut TestRandomV1::new(b"tiny-q-mask-linear-proof-rng"),
            &mut transcript,
            witness,
        )?;
    Ok((
        transcript.proof,
        hash_v1(&transcript.state),
        upstream,
        commitments,
    ))
}

fn verify_tiny_v1(
    proof: &[u8],
    upstream: UpstreamBindingV1,
    relation: usize,
    commitments: QMaskCoreCommitmentsV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQMaskLinearRelationsErrorV1> {
    let core = ExactCoreViewV1 { bytes: proof };
    let basis = hash_v1(b"tiny-q-mask-linear-basis");
    let mut transcript = QMaskLinearVerifierTranscriptV1::<TinyQMaskSuiteV1>::new_v1(
        upstream,
        relation,
        commitments,
        2,
        2,
        basis,
        core,
    )?;
    build_q_mask_linear_statement_v1::<TinyQMaskSuiteV1>(2, 2, commitments)?
        .verify(&mut transcript)?;
    transcript.finish_v1()
}

#[test]
fn tiny_real_q_mask_core_roundtrips_and_binds_proof_context_and_inventory() {
    let (proof, prover_digest, upstream, commitments) =
        prove_tiny_v1(honest_values_v1()).expect("valid q-mask proof");
    assert_eq!(
        proof.len(),
        (FIXED_CORE_POINTS_V1 + 2) * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1
    );
    assert_eq!(
        verify_tiny_v1(&proof, upstream, 0, commitments),
        Ok(prover_digest)
    );

    let mut changed_proof = proof.clone();
    let index = changed_proof.len() / 2;
    changed_proof[index] ^= 1;
    assert!(verify_tiny_v1(&changed_proof, upstream, 0, commitments).is_err());
    assert!(verify_tiny_v1(&proof, upstream, 1, commitments).is_err());
    let mut changed_upstream = upstream;
    changed_upstream.statement8_proof_set_root[0] ^= 1;
    assert!(verify_tiny_v1(&proof, changed_upstream, 0, commitments).is_err());
    let mut changed_commitments = commitments;
    changed_commitments.derived[0] += TinyQMaskSuiteV1::generators().g;
    assert!(verify_tiny_v1(&proof, upstream, 0, changed_commitments).is_err());
}

#[test]
fn false_complement_and_nonzero_structural_top_cannot_produce_proofs() {
    let mut bad_complement = honest_values_v1();
    bad_complement[2].complement_digits[0][0] += Scalar::one();
    assert!(prove_tiny_v1(bad_complement).is_err());

    let mut bad_top = honest_values_v1();
    bad_top[7].digits[0][1] = Scalar::one();
    let complement = radix_digits_v1(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0] - 2);
    for (digit, value) in complement.into_iter().enumerate() {
        bad_top[7].complement_digits[digit][1] = value;
    }
    assert!(prove_tiny_v1(bad_top).is_err());
}

#[test]
fn q_mask_linear_boundary_is_private_move_only_and_fully_fail_closed() {
    assert_eq!(RELATIONS_V1, 200);
    assert_eq!(CORES_V1, 200);
    assert_eq!(GATES_V1, 16_384);
    assert_eq!(CONSTRAINTS_PER_CORE_V1, 131_073);
    assert_eq!(COMMITMENTS_PER_CORE_V1, 9);
    assert_eq!(FIXED_CORE_POINTS_V1, 25);
    assert_eq!(CORE_BYTES_V1, 1_909);
    assert_eq!(RECORD_SET_BYTES_V1, 382_600);
    assert_eq!(MIN_WIRE_BYTES_V1, 383_003);
    assert_eq!(RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1, 2_204_253);

    let source = include_str!("rns_native_q_mask_linear_relations.rs");
    let declaration = "pub(super) struct RnsNativeQMaskLinearRelationsPrerequisiteV1";
    let declaration_offset = source.find(declaration).expect("stage declaration");
    let attributes = source[..declaration_offset]
        .rsplit_once("\n\n")
        .map_or(&source[..declaration_offset], |(_, block)| block);
    let stage = source[declaration_offset + declaration.len()..]
        .split_once("\n}\n")
        .map(|(body, _)| body)
        .expect("stage body");
    assert!(!attributes.contains("derive(Clone"));
    assert!(!attributes.contains("derive(Copy"));
    assert!(!stage.contains("pub fn"));
    assert!(!stage.contains("VerifiedReceipt"));
    assert!(!stage.contains("ReleaseAuthorization"));
    assert!(source.contains("Q_MASK_LINEAR_RELATIONS_VERIFIER_IMPLEMENTED_V1: bool = true"));
    assert!(source.contains("Q_MASK_RADIX_SAME_OPENING_VERIFIED_V1: bool = false"));
    assert!(source.contains("Q_MASK_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false"));
    assert!(source.contains("RELEASE_READY_V1: bool = false"));
    assert!(source.contains("for relation in 0..RELATIONS_V1"));
    assert!(source.contains("build_q_mask_linear_statement_v1"));
    assert!(source.contains(".verify(&mut transcript)?"));
    assert_eq!(source.matches("previous.residual_digest()").count(), 1);
    assert_eq!(source.matches("previous.binding_digest()").count(), 1);

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_q_mask_linear_relations;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_q_mask_linear_relations"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
