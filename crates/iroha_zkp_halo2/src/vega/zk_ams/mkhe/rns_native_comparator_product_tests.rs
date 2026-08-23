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
    derive_t256_generators_v1(b"rns-native-comparator-product-codec-test", 1).expect("test point")
        [0]
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
        records.extend_from_slice(&(group as u16).to_be_bytes());
        records.extend_from_slice(&(CORE_BYTES_V1 as u16).to_be_bytes());
        records.extend_from_slice(&core);
    }
    assert_eq!(records.len(), RECORD_SET_BYTES_V1);
    records
}

fn canonical_wire_v1(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    inventory_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Vec<u8> {
    let records = canonical_records_v1();
    let point = test_point_v1();
    let proof_set_root =
        canonical_proof_set_root_v1(prior_context_digest, inventory_root, &records, |_| {
            Some((point, point))
        })
        .expect("proof-set root");
    let residual_digest = canonical_residual_digest_v1(
        prior_context_digest,
        inventory_root,
        proof_set_root,
        residual,
    )
    .expect("residual digest");
    let total = HEADER_BYTES_V1 + records.len() + residual.len() + CODEC_DIGEST_BYTES_V1;
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.push(VERSION_V1);
    wire.push(FLAGS_V1);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(total as u32).to_be_bytes());
    wire.push(STATEMENT_V1);
    wire.extend_from_slice(&(GROUPS_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(COORDINATES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&(GATES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&(PADDED_GATES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&(CONSTRAINTS_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&[
        COMMITMENTS_V1 as u8,
        POINT_BYTES_V1 as u8,
        SCALAR_BYTES_V1 as u8,
        LOG_PADDED_GATES_V1 as u8,
    ]);
    wire.extend_from_slice(&(CORE_BYTES_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&prior_context_digest);
    wire.extend_from_slice(&inventory_root);
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
fn proof_set_codec_is_exact_canonical_capped_and_context_bound() {
    let prior = [0x31; DIGEST_BYTES_V1];
    let inventory = [0x52; DIGEST_BYTES_V1];
    let residual = b"remaining-carry-small-qmask-and-global-lookup-proofs";
    let wire = canonical_wire_v1(prior, inventory, residual);
    let point = test_point_v1();
    let view = ComparatorProofSetViewV1::from_components_v1(&wire, prior, inventory, |_| {
        Some((point, point))
    })
    .expect("canonical statement-3 proof set");
    assert_eq!(view.records.len(), RECORD_SET_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert_ne!(view.proof_set_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);
    assert!(view.core_v1(0).is_ok());
    assert!(view.core_v1(GROUPS_V1 - 1).is_ok());

    assert_eq!(
        ComparatorProofSetViewV1::from_components_v1(
            &wire,
            [0x32; DIGEST_BYTES_V1],
            inventory,
            |_| Some((point, point)),
        )
        .map(|_| ()),
        Err(RnsNativeComparatorProductErrorV1::InvalidHeader)
    );
    assert!(
        ComparatorProofSetViewV1::from_components_v1(
            &wire[..wire.len() - 1],
            prior,
            inventory,
            |_| Some((point, point)),
        )
        .is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(
        ComparatorProofSetViewV1::from_components_v1(&trailing, prior, inventory, |_| Some((
            point, point
        )),)
        .is_err()
    );
    assert_eq!(
        RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1,
        6_180_515
    );
    let cap_plus_one = vec![0_u8; RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1 + 1];
    assert_eq!(
        ComparatorProofSetViewV1::from_components_v1(&cap_plus_one, prior, inventory, |_| Some((
            point, point
        )),)
        .map(|_| ()),
        Err(RnsNativeComparatorProductErrorV1::ProofCapExceeded)
    );
}

#[test]
fn proof_set_rejects_geometry_point_scalar_commitment_and_residual_substitution() {
    let prior = [0x71; DIGEST_BYTES_V1];
    let inventory = [0x82; DIGEST_BYTES_V1];
    let wire = canonical_wire_v1(prior, inventory, b"nonempty-residual");
    let point = test_point_v1();
    let parse = |wire: &[u8]| {
        ComparatorProofSetViewV1::from_components_v1(wire, prior, inventory, |_| {
            Some((point, point))
        })
        .map(|_| ())
    };

    let mut geometry = wire.clone();
    geometry[12] = 5;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeComparatorProductErrorV1::InvalidGeometry)
    );

    let mut order = wire.clone();
    order[HEADER_BYTES_V1..HEADER_BYTES_V1 + 2].copy_from_slice(&1_u16.to_be_bytes());
    assert_eq!(
        parse(&order),
        Err(RnsNativeComparatorProductErrorV1::InvalidGeometry)
    );

    let mut invalid_point = wire.clone();
    let core = HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1;
    invalid_point[core..core + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeComparatorProductErrorV1::InvalidPoint)
    );

    let mut invalid_scalar = wire.clone();
    let scalar = core + FIXED_CORE_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar[scalar..scalar + SCALAR_BYTES_V1].fill(0xff);
    assert_eq!(
        parse(&invalid_scalar),
        Err(RnsNativeComparatorProductErrorV1::InvalidScalar)
    );

    assert_eq!(
        ComparatorProofSetViewV1::from_components_v1(&wire, prior, inventory, |_| {
            Some((point, point + point))
        })
        .map(|_| ()),
        Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity)
    );

    let mut residual = wire.clone();
    residual[HEADER_BYTES_V1 + RECORD_SET_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&residual),
        Err(RnsNativeComparatorProductErrorV1::InvalidIntegrity)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyComparatorSuiteV1;

impl ProofSuite for TinyComparatorSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyComparatorSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points =
                derive_t256_generators_v1(b"rns-native-comparator-product-tiny-suite-v1", 18)
                    .expect("tiny comparator generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..10].to_vec(),
                points[10..18].to_vec(),
            )
            .expect("valid tiny comparator basis")
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
    fn new(context: ComparatorTranscriptContextV1) -> Self {
        Self {
            state: initial_transcript_state_v1(context).expect("valid tiny transcript context"),
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
    let generators = TinyComparatorSuiteV1::generators();
    let mut terms = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect::<Vec<_>>();
    terms.push((mask, generators.h));
    multiexp::<TinyComparatorSuiteV1>(&terms)
}

fn witness_v1(
    difference_values: Vec<Scalar>,
    sum_values: Vec<Scalar>,
    difference_mask: Scalar,
    sum_mask: Scalar,
) -> ArithmeticCircuitWitness<TinyComparatorSuiteV1> {
    let mut a_l = Vec::with_capacity(3 * difference_values.len());
    let mut a_r = Vec::with_capacity(3 * difference_values.len());
    for (difference, sum) in difference_values
        .iter()
        .copied()
        .zip(sum_values.iter().copied())
    {
        a_l.extend([difference, sum, difference]);
        a_r.extend([difference - Scalar::one(), sum - Scalar::one(), sum]);
    }
    ArithmeticCircuitWitness::new(
        a_l,
        a_r,
        vec![
            VectorCommitmentOpening::new(difference_values, difference_mask),
            VectorCommitmentOpening::new(sum_values, sum_mask),
        ],
    )
    .expect("shape-valid comparator witness")
}

type TinyProofV1 = (
    Vec<u8>,
    [u8; DIGEST_BYTES_V1],
    [u8; DIGEST_BYTES_V1],
    [u8; DIGEST_BYTES_V1],
    Point,
    Point,
);

fn prove_tiny_v1(
    difference_values: Vec<Scalar>,
    sum_values: Vec<Scalar>,
) -> Result<TinyProofV1, GeneralizedBulletproofErrorV1> {
    let coordinates = difference_values.len();
    let padded_gates = (coordinates * PRODUCTS_PER_COORDINATE_V1).next_power_of_two();
    let difference_mask = Scalar::from_u64(17);
    let sum_mask = Scalar::from_u64(29);
    let difference = commitment_v1(&difference_values, difference_mask);
    let sum = commitment_v1(&sum_values, sum_mask);
    let prior = [0x91; DIGEST_BYTES_V1];
    let inventory = [0xa2; DIGEST_BYTES_V1];
    let basis = hash_v1(b"tiny-comparator-product-basis");
    let witness = witness_v1(difference_values, sum_values, difference_mask, sum_mask);
    let transcript_context = ComparatorTranscriptContextV1 {
        prior_context_digest: prior,
        inventory_root: inventory,
        group: 0,
        difference,
        sum,
        coordinates,
        padded_gates,
        generator_basis_digest: basis,
    };
    let mut transcript = TestProverTranscriptV1::<TinyComparatorSuiteV1>::new(transcript_context);
    build_comparator_statement_v1::<TinyComparatorSuiteV1>(
        coordinates,
        padded_gates,
        difference,
        sum,
    )
    .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?
    .prove(
        &mut TestRandomV1::new(b"tiny-comparator-product-proof-rng"),
        &mut transcript,
        witness,
    )?;
    let transcript_digest = hash_v1(&transcript.state);
    Ok((
        transcript.proof,
        transcript_digest,
        prior,
        inventory,
        difference,
        sum,
    ))
}

#[test]
fn tiny_real_product_proof_roundtrips_and_binds_every_axis() {
    let zero = Scalar::zero();
    let one = Scalar::one();
    let (proof, prover_digest, prior, inventory, difference, sum) =
        prove_tiny_v1(vec![zero, one], vec![one, zero]).expect("valid product proof");
    assert_eq!(
        proof.len(),
        (13 + 2 * 3) * POINT_BYTES_V1 + 5 * SCALAR_BYTES_V1
    );
    let basis = hash_v1(b"tiny-comparator-product-basis");
    let verify = |proof: &[u8],
                  group,
                  difference,
                  sum|
     -> Result<[u8; DIGEST_BYTES_V1], RnsNativeComparatorProductErrorV1> {
        let core = ExactCoreViewV1 { bytes: proof };
        let transcript_context = ComparatorTranscriptContextV1 {
            prior_context_digest: prior,
            inventory_root: inventory,
            group,
            difference,
            sum,
            coordinates: 2,
            padded_gates: 8,
            generator_basis_digest: basis,
        };
        let mut transcript = ComparatorVerifierTranscriptV1::<TinyComparatorSuiteV1>::new_v1(
            transcript_context,
            core,
        )?;
        build_comparator_statement_v1::<TinyComparatorSuiteV1>(2, 8, difference, sum)?
            .verify(&mut transcript)?;
        transcript.finish_v1()
    };
    assert_eq!(verify(&proof, 0, difference, sum), Ok(prover_digest));

    let mut changed = proof.clone();
    let changed_index = changed.len() / 2;
    changed[changed_index] ^= 1;
    assert!(verify(&changed, 0, difference, sum).is_err());
    assert!(verify(&proof, 1, difference, sum).is_err());
    assert!(
        verify(
            &proof,
            0,
            difference + TinyComparatorSuiteV1::generators().g,
            sum,
        )
        .is_err()
    );
}

#[test]
fn adversarial_non_boolean_and_overlapping_vectors_cannot_produce_a_proof() {
    let zero = Scalar::zero();
    let one = Scalar::one();
    assert!(matches!(
        prove_tiny_v1(vec![Scalar::from_u64(2), zero], vec![zero, one]),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
    assert!(matches!(
        prove_tiny_v1(vec![one, zero], vec![one, zero]),
        Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
    ));
}

#[test]
fn production_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    let source = include_str!("rns_native_comparator_product.rs");
    let declaration = "pub(super) struct RnsNativeComparatorProductPrerequisiteV1";
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
        source.contains("COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_VERIFIER_IMPLEMENTED_V1: bool = true")
    );
    assert!(source.contains("COMPARATOR_RANGE_AND_CARRY_PRODUCT_VERIFIED_V1: bool = false"));
    assert!(source.contains("SMALL_SIGNED_PRODUCT_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("for group in 0..GROUPS_V1"));
    assert!(source.contains("build_comparator_statement_v1"));
    assert!(source.contains(".verify(&mut transcript)?"));
    assert!(source.contains("RNS_NATIVE_COMPARATOR_PRODUCT_RESIDUAL_MAX_BYTES_V1 == 6_180_515"));
    assert!(source.contains("RnsNativeClaimedSuccessorV1<"));
    assert!(source.contains("RnsNativeCrossFieldRlweClaimedInventoryParentV1<"));
    assert!(stage.contains("_parent: RnsNativeClaimedSuccessorV1<"));
    let capability_forward = source
        .split_once("pub(super) const fn pre_global_lookup_capability_v1(")
        .expect("comparator capability forwarding")
        .1
        .split_once("pub(super) const fn inventory(")
        .expect("comparator capability forwarding boundary")
        .0;
    assert!(capability_forward.contains(") -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1"));
    assert!(capability_forward.contains("self._parent"));
    assert!(capability_forward.contains(".parent()"));
    assert!(capability_forward.contains(".pre_global_lookup_capability_v1()"));
    assert!(!capability_forward.contains("test_fixture_v1"));
    assert!(!capability_forward.contains("post_cross_field_binding_digest"));
    assert!(!capability_forward.contains("global_lookup_challenge_seed"));
    let production = source
        .split_once("pub(super) fn verify_rns_native_comparator_product_v1")
        .expect("claimed-successor comparator entry")
        .1
        .split_once("/// Test-only compatibility entry")
        .expect("production comparator boundary")
        .0;
    assert!(production.contains("parent: RnsNativeClaimedSuccessorV1<"));
    assert!(production.contains("parent.successor()"));
    assert!(!production.contains("inventory.continuation()"));
    let legacy = source
        .find("fn verify_rns_native_comparator_product_from_inventory_v1")
        .expect("legacy raw inventory entry");
    assert!(source[legacy.saturating_sub(160)..legacy].contains("#[cfg(test)]"));

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent.matches("mod rns_native_comparator_product;").count(),
        1
    );
    assert_eq!(
        parent.matches("mod rns_native_claimed_successor;").count(),
        1
    );
    assert_eq!(
        parent
            .matches("mod rns_native_cross_field_rlwe_direct;")
            .count(),
        1
    );
    assert!(!parent.contains("pub mod rns_native_cross_field_rlwe_direct"));
    assert!(!parent.contains("pub use rns_native_comparator_product"));
    let claimed_facade = include_str!("rns_native_claimed_successor.rs");
    assert!(claimed_facade.contains("from_direct_claim_v1"));
    assert!(!claimed_facade.contains("pub(super) fn new"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
