use std::sync::OnceLock;

use super::*;
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitWitness, ProofGenerators, ProofRandomSource, ProverTranscript,
        VectorCommitmentOpening, multiexp,
    },
    vega::derive_t256_generators_v1,
};

fn test_point_v1() -> Point {
    derive_t256_generators_v1(b"rns-native-range-carry-codec-test", 1).expect("test point")[0]
}

fn test_commitments_v1() -> ComparatorRangeCarryCommitmentsV1 {
    let point = test_point_v1();
    ComparatorRangeCarryCommitmentsV1 {
        difference_top: point,
        mixed_top: point,
        borrows: [point; BORROWS_V1],
    }
}

fn upstream_v1() -> UpstreamBindingV1 {
    UpstreamBindingV1 {
        prior_context_digest: [0x11; DIGEST_BYTES_V1],
        inventory_root: [0x22; DIGEST_BYTES_V1],
        statement3_proof_set_root: [0x33; DIGEST_BYTES_V1],
        statement3_verified_transcript_root: [0x44; DIGEST_BYTES_V1],
    }
}

fn canonical_core_v1() -> Vec<u8> {
    let point = encode_point_v1(test_point_v1()).expect("canonical point");
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
    for group in 0..GROUPS_V1 {
        for chunk in 0..CHUNKS_PER_GROUP_V1 {
            records.extend_from_slice(&(group as u16).to_be_bytes());
            records.push(chunk as u8);
            records.extend_from_slice(&(CORE_BYTES_V1 as u16).to_be_bytes());
            records.extend_from_slice(&core);
        }
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
    wire.push(STATEMENT_V1);
    wire.extend_from_slice(&(GROUPS_V1 as u16).to_be_bytes());
    wire.push(CHUNKS_PER_GROUP_V1 as u8);
    for value in [
        COORDINATES_V1,
        BOOLEAN_GATES_V1,
        FINAL_GATES_V1,
        PADDED_GATES_V1,
        BOOLEAN_CONSTRAINTS_V1,
        FINAL_CONSTRAINTS_V1,
    ] {
        wire.extend_from_slice(&(value as u32).to_be_bytes());
    }
    wire.extend_from_slice(&[
        COMMITMENTS_PER_CORE_V1 as u8,
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
        LOG_PADDED_GATES_V1 as u8,
    ]);
    wire.extend_from_slice(&(CORE_BYTES_V1 as u32).to_be_bytes());
    for digest in [
        upstream.prior_context_digest,
        upstream.inventory_root,
        upstream.statement3_proof_set_root,
        upstream.statement3_verified_transcript_root,
        proof_set_root,
        residual_digest,
    ] {
        wire.extend_from_slice(&digest);
    }
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
fn statement5_codec_is_exact_canonical_capped_and_upstream_bound() {
    let upstream = upstream_v1();
    let residual = b"remaining-small-qmask-and-global-lookup-proofs";
    let wire = canonical_wire_v1(upstream, residual);
    let commitments = test_commitments_v1();
    let view = ComparatorRangeCarryProofSetViewV1::from_components_v1(&wire, upstream, |_| {
        Some(commitments)
    })
    .expect("canonical statement-5 proof set");
    assert_eq!(view.records.len(), RECORD_SET_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert_ne!(view.proof_set_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);
    assert!(view.core_v1(0, 0).is_ok());
    assert!(view.core_v1(GROUPS_V1 - 1, CHUNKS_PER_GROUP_V1 - 1).is_ok());

    let mut changed_upstream = upstream;
    changed_upstream.statement3_verified_transcript_root[0] ^= 1;
    assert_eq!(
        ComparatorRangeCarryProofSetViewV1::from_components_v1(&wire, changed_upstream, |_| Some(
            commitments
        ),)
        .map(|_| ()),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidHeader)
    );
    assert!(
        ComparatorRangeCarryProofSetViewV1::from_components_v1(
            &wire[..wire.len() - 1],
            upstream,
            |_| Some(commitments),
        )
        .is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(
        ComparatorRangeCarryProofSetViewV1::from_components_v1(&trailing, upstream, |_| Some(
            commitments
        ),)
        .is_err()
    );

    let cap_plus_one = vec![0_u8; RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        ComparatorRangeCarryProofSetViewV1::from_components_v1(&cap_plus_one, upstream, |_| Some(
            commitments
        ),)
        .map(|_| ()),
        Err(RnsNativeComparatorRangeCarryErrorV1::ProofCapExceeded)
    );
}

#[test]
fn real_statement3_residual_digest_is_computed_after_nested_statement5_without_a_cycle() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream-after-statement5");
    let predecessor_residual_digest =
        super::super::rns_native_comparator_product::canonical_residual_digest_v1(
            upstream.prior_context_digest,
            upstream.inventory_root,
            upstream.statement3_proof_set_root,
            &wire,
        )
        .expect("real statement-3 residual digest over complete statement-5 bytes");
    assert_ne!(predecessor_residual_digest, [0; DIGEST_BYTES_V1]);
    assert!(
        !wire
            .windows(DIGEST_BYTES_V1)
            .any(|window| window == predecessor_residual_digest.as_slice())
    );
    ComparatorRangeCarryProofSetViewV1::from_components_v1(&wire, upstream, |_| {
        Some(test_commitments_v1())
    })
    .expect("nested statement-5 wire is independently canonical");
}

#[test]
fn statement5_codec_rejects_geometry_point_scalar_commitment_and_residual_mutations() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"nonempty-residual");
    let commitments = test_commitments_v1();
    let parse = |wire: &[u8]| {
        ComparatorRangeCarryProofSetViewV1::from_components_v1(wire, upstream, |_| {
            Some(commitments)
        })
        .map(|_| ())
    };

    let mut geometry = wire.clone();
    geometry[15] = 4;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry)
    );

    let mut order = wire.clone();
    order[HEADER_BYTES_V1 + 2] = 1;
    assert_eq!(
        parse(&order),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidGeometry)
    );

    let mut invalid_point = wire.clone();
    let core = HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1;
    invalid_point[core..core + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidPoint)
    );

    let mut invalid_scalar = wire.clone();
    let scalar = core + FIXED_CORE_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar[scalar..scalar + SCALAR_BYTES_V1].fill(0xff);
    assert_eq!(
        parse(&invalid_scalar),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidScalar)
    );

    let changed = ComparatorRangeCarryCommitmentsV1 {
        mixed_top: commitments.mixed_top + commitments.mixed_top,
        ..commitments
    };
    assert_eq!(
        ComparatorRangeCarryProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(changed))
            .map(|_| ()),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity)
    );

    let mut residual = wire.clone();
    residual[HEADER_BYTES_V1 + RECORD_SET_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&residual),
        Err(RnsNativeComparatorRangeCarryErrorV1::InvalidIntegrity)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyRangeCarrySuiteV1;

impl ProofSuite for TinyRangeCarrySuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyRangeCarrySuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points = derive_t256_generators_v1(b"rns-native-range-carry-tiny-suite-v1", 18)
                .expect("tiny range/carry generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..10].to_vec(),
                points[10..18].to_vec(),
            )
            .expect("valid tiny range/carry basis")
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
        group: usize,
        chunk: usize,
        commitments: RangeCarryChunkCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        basis: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            state: initial_transcript_state_v1(
                upstream,
                group,
                chunk,
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

fn commitment_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinyRangeCarrySuiteV1::generators();
    let mut terms = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect::<Vec<_>>();
    terms.push((mask, generators.h));
    multiexp::<TinyRangeCarrySuiteV1>(&terms)
}

fn chunk_points_v1(
    values: &[Vec<Scalar>; COMMITMENTS_PER_CORE_V1],
    masks: &[Scalar; COMMITMENTS_PER_CORE_V1],
) -> RangeCarryChunkCommitmentsV1 {
    RangeCarryChunkCommitmentsV1 {
        points: core::array::from_fn(|index| commitment_v1(&values[index], masks[index])),
    }
}

fn chunk_witness_v1(
    chunk: usize,
    values: [Vec<Scalar>; COMMITMENTS_PER_CORE_V1],
    masks: [Scalar; COMMITMENTS_PER_CORE_V1],
) -> ArithmeticCircuitWitness<TinyRangeCarrySuiteV1> {
    let coordinates = values[0].len();
    let mut a_l = Vec::new();
    let mut a_r = Vec::new();
    match chunk {
        0..=3 => {
            for coordinate in 0..coordinates {
                for column in &values {
                    let beta = column[coordinate];
                    a_l.push(beta);
                    a_r.push(beta - Scalar::one());
                }
            }
        }
        4 => {
            for ((&beta16, &beta17), &difference_top) in values[BORROW_16_FINAL_COMMITMENT_V1]
                .iter()
                .zip(&values[BORROW_17_FINAL_COMMITMENT_V1])
                .zip(&values[DIFFERENCE_TOP_FINAL_COMMITMENT_V1])
                .take(coordinates)
            {
                a_l.extend([beta16, beta17, difference_top]);
                a_r.extend([beta16 - Scalar::one(), beta17 - Scalar::one(), beta16]);
            }
        }
        _ => panic!("invalid test chunk"),
    }
    let openings = values
        .into_iter()
        .zip(masks)
        .map(|(values, mask)| VectorCommitmentOpening::new(values, mask))
        .collect();
    ArithmeticCircuitWitness::new(a_l, a_r, openings).expect("shape-valid range/carry witness")
}

type TinyProofV1 = (
    Vec<u8>,
    [u8; DIGEST_BYTES_V1],
    UpstreamBindingV1,
    RangeCarryChunkCommitmentsV1,
    usize,
    usize,
);

fn prove_tiny_chunk_v1(
    chunk: usize,
    values: [Vec<Scalar>; COMMITMENTS_PER_CORE_V1],
) -> Result<TinyProofV1, GeneralizedBulletproofErrorV1> {
    let coordinates = values[0].len();
    let (gates, _) = chunk_geometry_v1(coordinates, chunk)
        .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
    let padded_gates = gates.next_power_of_two();
    let masks = core::array::from_fn(|index| Scalar::from_u64(11 + index as u64));
    let commitments = chunk_points_v1(&values, &masks);
    let upstream = upstream_v1();
    let basis = hash_v1(b"tiny-range-carry-basis");
    let witness = chunk_witness_v1(chunk, values, masks);
    let mut transcript = TestProverTranscriptV1::<TinyRangeCarrySuiteV1>::new(
        upstream,
        0,
        chunk,
        commitments,
        coordinates,
        padded_gates,
        basis,
    );
    build_range_carry_statement_v1::<TinyRangeCarrySuiteV1>(
        coordinates,
        padded_gates,
        chunk,
        commitments,
    )
    .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?
    .prove(
        &mut TestRandomV1::new(b"tiny-range-carry-proof-rng"),
        &mut transcript,
        witness,
    )?;
    let transcript_digest = hash_v1(&transcript.state);
    Ok((
        transcript.proof,
        transcript_digest,
        upstream,
        commitments,
        chunk,
        padded_gates,
    ))
}

fn honest_boolean_chunk_v1() -> [Vec<Scalar>; COMMITMENTS_PER_CORE_V1] {
    let zero = Scalar::zero();
    let one = Scalar::one();
    [
        vec![zero, one],
        vec![one, zero],
        vec![one, one],
        vec![zero, zero],
    ]
}

fn honest_final_chunk_v1() -> [Vec<Scalar>; COMMITMENTS_PER_CORE_V1] {
    let zero = Scalar::zero();
    let one = Scalar::one();
    [
        vec![zero, one],
        vec![zero, one],
        vec![one, one],
        vec![one, zero],
    ]
}

fn verify_tiny_v1(
    proof: &[u8],
    upstream: UpstreamBindingV1,
    group: usize,
    chunk: usize,
    commitments: RangeCarryChunkCommitmentsV1,
    padded_gates: usize,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorRangeCarryErrorV1> {
    let core = ExactCoreViewV1 { bytes: proof };
    let basis = hash_v1(b"tiny-range-carry-basis");
    let mut transcript = ComparatorRangeCarryVerifierTranscriptV1::<TinyRangeCarrySuiteV1>::new_v1(
        upstream,
        group,
        chunk,
        commitments,
        2,
        padded_gates,
        basis,
        core,
    )?;
    build_range_carry_statement_v1::<TinyRangeCarrySuiteV1>(2, padded_gates, chunk, commitments)?
        .verify(&mut transcript)?;
    transcript.finish_v1()
}

#[test]
fn tiny_real_boolean_and_terminal_carry_cores_roundtrip_and_bind_every_axis() {
    for (chunk, values) in [(0, honest_boolean_chunk_v1()), (4, honest_final_chunk_v1())] {
        let (proof, prover_digest, upstream, commitments, chunk, padded_gates) =
            prove_tiny_chunk_v1(chunk, values).expect("valid range/carry proof");
        assert_eq!(
            proof.len(),
            (FIXED_CORE_POINTS_V1 + 2 * 3) * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1
        );
        assert_eq!(
            verify_tiny_v1(&proof, upstream, 0, chunk, commitments, padded_gates,),
            Ok(prover_digest)
        );

        let mut changed_proof = proof.clone();
        let changed_index = changed_proof.len() / 2;
        changed_proof[changed_index] ^= 1;
        assert!(
            verify_tiny_v1(
                &changed_proof,
                upstream,
                0,
                chunk,
                commitments,
                padded_gates,
            )
            .is_err()
        );
        assert!(verify_tiny_v1(&proof, upstream, 1, chunk, commitments, padded_gates,).is_err());
        let changed_commitments = RangeCarryChunkCommitmentsV1 {
            points: [
                commitments.points[0] + TinyRangeCarrySuiteV1::generators().g,
                commitments.points[1],
                commitments.points[2],
                commitments.points[3],
            ],
        };
        assert!(
            verify_tiny_v1(
                &proof,
                upstream,
                0,
                chunk,
                changed_commitments,
                padded_gates,
            )
            .is_err()
        );
    }
}

#[test]
fn non_boolean_wrong_m_and_inconsistent_terminal_carry_cannot_produce_a_proof() {
    let mut non_boolean = honest_boolean_chunk_v1();
    non_boolean[0][0] = Scalar::from_u64(2);
    assert!(matches!(
        prove_tiny_chunk_v1(0, non_boolean),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));

    let mut wrong_m = honest_final_chunk_v1();
    wrong_m[MIXED_TOP_FINAL_COMMITMENT_V1][0] = Scalar::one();
    assert!(matches!(
        prove_tiny_chunk_v1(4, wrong_m),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));

    let mut inconsistent = honest_final_chunk_v1();
    inconsistent[BORROW_17_FINAL_COMMITMENT_V1][0] = Scalar::zero();
    assert!(matches!(
        prove_tiny_chunk_v1(4, inconsistent),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
}

#[test]
fn statement5_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    assert_eq!(RADIX_BASE_V1, 32_768);
    assert_eq!(CARRY_INTEGER_ABSOLUTE_BOUND_V1, 2);
    assert_eq!(CONDITIONAL_RADIX_ROW_ABSOLUTE_BOUND_V1, 65_535);
    assert_eq!(
        RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1,
        3_115_199
    );
    assert_eq!(chunk_geometry_v1(COORDINATES_V1, 0), Ok((65_536, 196_608)));
    assert_eq!(chunk_geometry_v1(COORDINATES_V1, 4), Ok((49_152, 163_840)));

    let source = include_str!("rns_native_comparator_range_carry_product.rs");
    let declaration = "pub(super) struct RnsNativeComparatorRangeCarryPrerequisiteV1";
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
    assert!(source.contains("COMPARATOR_RANGE_CARRY_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true"));
    assert!(source.contains("RADIX_DIFFERENCE_DIGIT_RANGE_VERIFIED_V1: bool = false"));
    assert!(source.contains("RADIX_SUBTRACTION_AND_RECONSTRUCTION_VERIFIED_V1: bool = false"));
    assert!(source.contains("SMALL_SIGNED_PRODUCT_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("for chunk in 0..CHUNKS_PER_GROUP_V1"));
    assert!(source.contains("build_range_carry_statement_v1"));
    assert!(source.contains(".verify(&mut transcript)?"));
    assert!(source.contains("REMAINING_RANGE_BOUNDARY_V1"));
    assert_eq!(source.matches("previous.residual_digest()").count(), 1);
    assert_eq!(source.matches("previous.binding_digest()").count(), 1);

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_comparator_range_carry_product;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_comparator_range_carry_product"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
