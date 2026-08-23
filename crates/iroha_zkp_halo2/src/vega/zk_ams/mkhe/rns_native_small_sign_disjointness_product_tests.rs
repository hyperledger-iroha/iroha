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
        derive_t256_generators_v1(b"rns-native-small-sign-codec-test", 2).expect("test points");
    [points[0], points[1]]
}

fn test_commitments_v1() -> SmallSourceProductCommitmentsV1 {
    let [signed, negative_magnitude] = test_points_v1();
    let positive = signed + negative_magnitude;
    assert!(!positive.is_identity());
    SmallSourceProductCommitmentsV1 {
        signed,
        negative_magnitude,
        positive,
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
    for ordinal in 0..CORES_V1 {
        records.extend_from_slice(&(ordinal as u16).to_be_bytes());
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
    wire.push(STATEMENT_V1);
    wire.extend_from_slice(&(BLOCKS_V1 as u16).to_be_bytes());
    wire.push(BLOCKS_PER_CORE_V1 as u8);
    wire.extend_from_slice(&(CORES_V1 as u16).to_be_bytes());
    for value in [COORDINATES_V1, GATES_V1, PADDED_GATES_V1, CONSTRAINTS_V1] {
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
        upstream.statement5_proof_set_root,
        upstream.statement5_verified_transcript_root,
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
fn statement8_codec_is_exact_canonical_capped_and_upstream_bound() {
    let upstream = upstream_v1();
    let residual = b"remaining-comparator-range-qmask-and-global-lookup-proofs";
    let wire = canonical_wire_v1(upstream, residual);
    let commitments = test_commitments_v1();
    let view = SmallSignProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(commitments))
        .expect("canonical statement-8 proof set");
    assert_eq!(view.records.len(), RECORD_SET_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert_ne!(view.proof_set_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);
    assert!(view.core_v1(0).is_ok());
    assert!(view.core_v1(CORES_V1 - 1).is_ok());

    let mut changed_upstream = upstream;
    changed_upstream.statement5_verified_transcript_root[0] ^= 1;
    assert_eq!(
        SmallSignProofSetViewV1::from_components_v1(&wire, changed_upstream, |_| Some(commitments))
            .map(|_| ()),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidHeader)
    );
    assert!(
        SmallSignProofSetViewV1::from_components_v1(&wire[..wire.len() - 1], upstream, |_| Some(
            commitments
        ))
        .is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(
        SmallSignProofSetViewV1::from_components_v1(&trailing, upstream, |_| Some(commitments))
            .is_err()
    );

    let cap_plus_one = vec![0_u8; RNS_NATIVE_COMPARATOR_RANGE_CARRY_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        SmallSignProofSetViewV1::from_components_v1(&cap_plus_one, upstream, |_| Some(commitments))
            .map(|_| ()),
        Err(RnsNativeSmallSignDisjointnessErrorV1::ProofCapExceeded)
    );
}

#[test]
fn real_statement5_residual_digest_is_computed_after_nested_statement8_without_a_cycle() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream-after-statement8");
    let predecessor_upstream =
        super::super::rns_native_comparator_range_carry_product::UpstreamBindingV1 {
            prior_context_digest: upstream.prior_context_digest,
            inventory_root: upstream.inventory_root,
            statement3_proof_set_root: upstream.statement3_proof_set_root,
            statement3_verified_transcript_root: upstream.statement3_verified_transcript_root,
        };
    let predecessor_residual_digest =
        super::super::rns_native_comparator_range_carry_product::canonical_residual_digest_v1(
            predecessor_upstream,
            upstream.statement5_proof_set_root,
            &wire,
        )
        .expect("real statement-5 residual digest over complete statement-8 bytes");
    assert_ne!(predecessor_residual_digest, [0; DIGEST_BYTES_V1]);
    assert!(
        !wire
            .windows(DIGEST_BYTES_V1)
            .any(|window| window == predecessor_residual_digest.as_slice())
    );
    SmallSignProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(test_commitments_v1()))
        .expect("nested statement-8 wire is independently canonical");
}

#[test]
fn statement8_codec_rejects_geometry_point_scalar_order_commitment_and_residual_mutations() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"nonempty-residual");
    let commitments = test_commitments_v1();
    let parse = |wire: &[u8]| {
        SmallSignProofSetViewV1::from_components_v1(wire, upstream, |_| Some(commitments))
            .map(|_| ())
    };

    let mut geometry = wire.clone();
    geometry[15] = 3;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry)
    );

    let mut order = wire.clone();
    order[HEADER_BYTES_V1 + 1] = 1;
    assert_eq!(
        parse(&order),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidGeometry)
    );

    let mut invalid_point = wire.clone();
    let core = HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1;
    invalid_point[core..core + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidPoint)
    );

    let mut invalid_scalar = wire.clone();
    let scalar = core + FIXED_CORE_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar[scalar..scalar + SCALAR_BYTES_V1].fill(0xff);
    assert_eq!(
        parse(&invalid_scalar),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidScalar)
    );

    let changed_signed = commitments.signed + commitments.negative_magnitude;
    let changed = SmallSourceProductCommitmentsV1 {
        signed: changed_signed,
        negative_magnitude: commitments.negative_magnitude,
        positive: changed_signed + commitments.negative_magnitude,
    };
    assert!(!changed.positive.is_identity());
    assert_eq!(
        SmallSignProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(changed)).map(|_| ()),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity)
    );
    let inconsistent = SmallSourceProductCommitmentsV1 {
        signed: changed_signed,
        negative_magnitude: commitments.negative_magnitude,
        positive: commitments.positive,
    };
    assert_eq!(
        SmallSignProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(inconsistent))
            .map(|_| ()),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidContext)
    );

    let mut residual = wire.clone();
    residual[HEADER_BYTES_V1 + RECORD_SET_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&residual),
        Err(RnsNativeSmallSignDisjointnessErrorV1::InvalidIntegrity)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinySmallSignSuiteV1;

impl ProofSuite for TinySmallSignSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinySmallSignSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points = derive_t256_generators_v1(b"rns-native-small-sign-tiny-suite-v1", 18)
                .expect("tiny small-sign generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..10].to_vec(),
                points[10..18].to_vec(),
            )
            .expect("valid tiny small-sign basis")
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
        core: usize,
        commitments: SmallSignCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        basis: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            state: initial_transcript_state_v1(
                upstream,
                core,
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
struct OwnerValuesV1 {
    signed: Vec<Scalar>,
    negative: Vec<Scalar>,
}

fn honest_owner_values_v1() -> [OwnerValuesV1; BLOCKS_PER_CORE_V1] {
    let zero = Scalar::zero();
    let one = Scalar::one();
    let two = Scalar::from_u64(2);
    let three = Scalar::from_u64(3);
    let four = Scalar::from_u64(4);
    [
        OwnerValuesV1 {
            signed: vec![two, three],
            negative: vec![zero, zero],
        },
        OwnerValuesV1 {
            signed: vec![-one, zero],
            negative: vec![one, zero],
        },
        OwnerValuesV1 {
            signed: vec![zero, -two],
            negative: vec![zero, two],
        },
        OwnerValuesV1 {
            signed: vec![four, -three],
            negative: vec![zero, three],
        },
    ]
}

fn commitment_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinySmallSignSuiteV1::generators();
    let mut terms = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect::<Vec<_>>();
    terms.push((mask, generators.h));
    multiexp::<TinySmallSignSuiteV1>(&terms)
}

fn tiny_commitments_and_witness_v1(
    values: &[OwnerValuesV1; BLOCKS_PER_CORE_V1],
) -> (
    SmallSignCoreCommitmentsV1,
    ArithmeticCircuitWitness<TinySmallSignSuiteV1>,
) {
    let owners = core::array::from_fn(|local| {
        let signed_mask = Scalar::from_u64(11 + 2 * local as u64);
        let negative_mask = Scalar::from_u64(12 + 2 * local as u64);
        let signed = commitment_v1(&values[local].signed, signed_mask);
        let negative_magnitude = commitment_v1(&values[local].negative, negative_mask);
        SmallSourceProductCommitmentsV1 {
            signed,
            negative_magnitude,
            positive: signed + negative_magnitude,
        }
    });
    let commitments =
        SmallSignCoreCommitmentsV1::new_v1(owners).expect("valid derived tiny commitments");
    let mut a_l = Vec::new();
    let mut a_r = Vec::new();
    let mut openings = Vec::new();
    for (local, owner) in values.iter().enumerate() {
        let signed_mask = Scalar::from_u64(11 + 2 * local as u64);
        let negative_mask = Scalar::from_u64(12 + 2 * local as u64);
        let positive = owner
            .signed
            .iter()
            .zip(&owner.negative)
            .map(|(signed, negative)| *signed + *negative)
            .collect::<Vec<_>>();
        a_l.extend_from_slice(&positive);
        a_r.extend_from_slice(&owner.negative);
        openings.push(VectorCommitmentOpening::new(
            positive,
            signed_mask + negative_mask,
        ));
        openings.push(VectorCommitmentOpening::new(
            owner.negative.clone(),
            negative_mask,
        ));
    }
    let witness =
        ArithmeticCircuitWitness::new(a_l, a_r, openings).expect("shape-valid small-sign witness");
    (commitments, witness)
}

type TinyProofV1 = (
    Vec<u8>,
    [u8; DIGEST_BYTES_V1],
    UpstreamBindingV1,
    SmallSignCoreCommitmentsV1,
    usize,
);

fn prove_tiny_v1(
    values: [OwnerValuesV1; BLOCKS_PER_CORE_V1],
) -> Result<TinyProofV1, GeneralizedBulletproofErrorV1> {
    let coordinates = values[0].signed.len();
    let padded_gates = (coordinates * BLOCKS_PER_CORE_V1).next_power_of_two();
    let (commitments, witness) = tiny_commitments_and_witness_v1(&values);
    let upstream = upstream_v1();
    let basis = hash_v1(b"tiny-small-sign-basis");
    let mut transcript = TestProverTranscriptV1::<TinySmallSignSuiteV1>::new(
        upstream,
        0,
        commitments,
        coordinates,
        padded_gates,
        basis,
    );
    build_small_sign_statement_v1::<TinySmallSignSuiteV1>(coordinates, padded_gates, commitments)
        .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?
        .prove(
            &mut TestRandomV1::new(b"tiny-small-sign-proof-rng"),
            &mut transcript,
            witness,
        )?;
    let transcript_digest = hash_v1(&transcript.state);
    Ok((
        transcript.proof,
        transcript_digest,
        upstream,
        commitments,
        padded_gates,
    ))
}

fn verify_tiny_v1(
    proof: &[u8],
    upstream: UpstreamBindingV1,
    core_ordinal: usize,
    commitments: SmallSignCoreCommitmentsV1,
    padded_gates: usize,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSmallSignDisjointnessErrorV1> {
    let core = ExactCoreViewV1 { bytes: proof };
    let basis = hash_v1(b"tiny-small-sign-basis");
    let mut transcript = SmallSignVerifierTranscriptV1::<TinySmallSignSuiteV1>::new_v1(
        upstream,
        core_ordinal,
        commitments,
        2,
        padded_gates,
        basis,
        core,
    )?;
    build_small_sign_statement_v1::<TinySmallSignSuiteV1>(2, padded_gates, commitments)?
        .verify(&mut transcript)?;
    transcript.finish_v1()
}

#[test]
fn tiny_real_statement8_core_roundtrips_and_binds_every_axis() {
    let (proof, prover_digest, upstream, commitments, padded_gates) =
        prove_tiny_v1(honest_owner_values_v1()).expect("valid small-sign proof");
    assert_eq!(padded_gates, 8);
    assert_eq!(
        proof.len(),
        (FIXED_CORE_POINTS_V1 + 2 * 3) * POINT_BYTES_V1 + CORE_SCALARS_V1 * SCALAR_BYTES_V1
    );
    assert_eq!(
        verify_tiny_v1(&proof, upstream, 0, commitments, padded_gates),
        Ok(prover_digest)
    );

    let mut changed_proof = proof.clone();
    let changed_index = changed_proof.len() / 2;
    changed_proof[changed_index] ^= 1;
    assert!(verify_tiny_v1(&changed_proof, upstream, 0, commitments, padded_gates).is_err());
    assert!(verify_tiny_v1(&proof, upstream, 1, commitments, padded_gates).is_err());

    let mut changed_upstream = upstream;
    changed_upstream.statement5_proof_set_root[0] ^= 1;
    assert!(verify_tiny_v1(&proof, changed_upstream, 0, commitments, padded_gates).is_err());

    let mut owners = commitments.owners;
    owners[0].signed += TinySmallSignSuiteV1::generators().g;
    owners[0].positive = owners[0].signed + owners[0].negative_magnitude;
    let changed_commitments =
        SmallSignCoreCommitmentsV1::new_v1(owners).expect("valid substituted commitment tuple");
    assert!(verify_tiny_v1(&proof, upstream, 0, changed_commitments, padded_gates,).is_err());
}

#[test]
fn overlapping_positive_and_negative_values_cannot_produce_a_proof() {
    let mut invalid = honest_owner_values_v1();
    invalid[1].signed[0] = Scalar::zero();
    assert!(matches!(
        prove_tiny_v1(invalid),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
}

#[test]
fn statement8_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    assert_eq!(BLOCKS_V1, 1_032);
    assert_eq!(CORES_V1, 258);
    assert_eq!(GATES_V1, 65_536);
    assert_eq!(CONSTRAINTS_V1, 196_608);
    assert_eq!(COMMITMENTS_PER_CORE_V1, 8);
    assert_eq!(FIXED_CORE_POINTS_V1, 25);
    assert_eq!(CORE_BYTES_V1, 2_041);
    assert_eq!(RECORD_SET_BYTES_V1, 527_610);
    assert_eq!(MIN_WIRE_BYTES_V1, 527_945);
    assert_eq!(
        RNS_NATIVE_SMALL_SIGN_DISJOINTNESS_RESIDUAL_MAX_BYTES_V1,
        2_587_255
    );

    let source = include_str!("rns_native_small_sign_disjointness_product.rs");
    let declaration = "pub(super) struct RnsNativeSmallSignDisjointnessPrerequisiteV1";
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
    assert!(
        source.contains("SMALL_SIGN_DISJOINTNESS_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true")
    );
    assert!(source.contains("COMPARATOR_RADIX_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("SMALL_SIGNED_RANGE_AND_INVERSES_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("for core in 0..CORES_V1"));
    assert!(source.contains("build_small_sign_statement_v1"));
    assert!(source.contains(".verify(&mut transcript)?"));
    assert!(source.contains("REMAINING_BOUNDARY_V1"));
    assert_eq!(source.matches("previous.residual_digest()").count(), 1);
    assert_eq!(source.matches("previous.binding_digest()").count(), 1);
    assert_eq!(source.matches("statement3.residual_digest()").count(), 1);
    assert_eq!(source.matches("statement3.binding_digest()").count(), 1);

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_small_sign_disjointness_product;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_small_sign_disjointness_product"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
