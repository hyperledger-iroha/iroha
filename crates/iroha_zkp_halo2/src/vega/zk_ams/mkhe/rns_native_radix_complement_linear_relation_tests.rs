use std::sync::OnceLock;

use super::*;
use crate::{
    generalized_bulletproof::{
        ArithmeticCircuitWitness, ProofGenerators, ProofRandomSource, ProverTranscript,
        VectorCommitmentOpening, multiexp,
    },
    vega::derive_t256_generators_v1,
};

fn test_points_v1() -> [Point; 4] {
    let points =
        derive_t256_generators_v1(b"rns-native-radix-complement-codec-test", 4).expect("points");
    [points[0], points[1], points[2], points[3]]
}

fn test_commitments_v1() -> ExistingRadixCommitmentsV1 {
    let [difference, slack, difference_top, slack_top] = test_points_v1();
    ExistingRadixCommitmentsV1 {
        difference_low: [difference; RADIX_LOW_DIGITS_V1],
        slack_low: [slack; RADIX_LOW_DIGITS_V1],
        difference_top,
        slack_top,
    }
}

fn upstream_v1() -> UpstreamBindingV1 {
    UpstreamBindingV1 {
        prior_context_digest: [0x11; DIGEST_BYTES_V1],
        added_inventory_root: [0x22; DIGEST_BYTES_V1],
        statement3_proof_set_root: [0x33; DIGEST_BYTES_V1],
        statement3_verified_transcript_root: [0x44; DIGEST_BYTES_V1],
        statement5_proof_set_root: [0x55; DIGEST_BYTES_V1],
        statement5_verified_transcript_root: [0x66; DIGEST_BYTES_V1],
        statement8_proof_set_root: [0x77; DIGEST_BYTES_V1],
        statement8_verified_transcript_root: [0x88; DIGEST_BYTES_V1],
        q_mask_proof_set_root: [0x99; DIGEST_BYTES_V1],
        q_mask_verified_transcript_root: [0xaa; DIGEST_BYTES_V1],
        pre_z_candidate_root: [0xbb; DIGEST_BYTES_V1],
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
    for group in 0..GROUPS_V1 {
        records.extend_from_slice(&(group as u16).to_be_bytes());
        records.extend_from_slice(&(CORE_BYTES_V1 as u16).to_be_bytes());
        records.extend_from_slice(&core);
    }
    assert_eq!(records.len(), RECORD_SET_BYTES_V1);
    records
}

fn canonical_wire_v1(upstream: UpstreamBindingV1, residual: &[u8]) -> Vec<u8> {
    let records = canonical_records_v1();
    let commitments = test_commitments_v1();
    let mut commitment_at = |_| Some(commitments);
    let proof_set_root = canonical_proof_set_root_v1(upstream, &records, &mut commitment_at)
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
    wire.extend_from_slice(&[
        RADIX_LOW_DIGITS_V1 as u8,
        RADIX_DIGITS_V1 as u8,
        RADIX_LOG2_V1,
    ]);
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

fn existing_radix_residual_digest_v1(
    upstream: UpstreamBindingV1,
    residual: &[u8],
) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.residual");
    hash.update(&[1]);
    for digest in upstream.digests_v1()[..10].iter() {
        hash.update(digest);
    }
    hash.update(&upstream.pre_z_candidate_root);
    hash.update(&(residual.len() as u32).to_be_bytes());
    hash.update(residual);
    hash.finalize()
}

#[test]
fn radix_complement_codec_is_exact_canonical_capped_and_acyclically_bound() {
    let upstream = upstream_v1();
    let residual = b"statement-4-subtraction-and-global-lookup-follow";
    let wire = canonical_wire_v1(upstream, residual);
    let commitments = test_commitments_v1();
    let view =
        RadixComplementProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(commitments))
            .expect("canonical statement-2 proof set");
    assert_eq!(view.records.len(), RECORD_SET_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert!(view.core_v1(0).is_ok());
    assert!(view.core_v1(GROUPS_V1 - 1).is_ok());
    assert_ne!(view.proof_set_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);

    let mut changed = upstream;
    changed.pre_z_candidate_root[0] ^= 1;
    assert_eq!(
        RadixComplementProofSetViewV1::from_components_v1(&wire, changed, |_| Some(commitments))
            .map(|_| ()),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidHeader)
    );
    assert!(
        RadixComplementProofSetViewV1::from_components_v1(
            &wire[..wire.len() - 1],
            upstream,
            |_| Some(commitments),
        )
        .is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(
        RadixComplementProofSetViewV1::from_components_v1(&trailing, upstream, |_| Some(
            commitments
        ))
        .is_err()
    );
    let cap_plus_one = vec![0_u8; RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        RadixComplementProofSetViewV1::from_components_v1(&cap_plus_one, upstream, |_| Some(
            commitments
        ))
        .map(|_| ()),
        Err(RnsNativeRadixComplementLinearErrorV1::ProofCapExceeded)
    );
}

#[test]
fn real_existing_radix_residual_hashes_complete_statement2_wire_without_a_cycle() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream-after-statement-2");
    let existing_residual = existing_radix_residual_digest_v1(upstream, &wire);
    assert_ne!(existing_residual, [0; DIGEST_BYTES_V1]);
    assert!(
        !wire
            .windows(DIGEST_BYTES_V1)
            .any(|window| window == existing_residual.as_slice())
    );
}

#[test]
fn codec_rejects_geometry_order_point_scalar_owner_and_residual_mutations() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"nonempty-residual");
    let commitments = test_commitments_v1();
    let parse = |bytes: &[u8]| {
        RadixComplementProofSetViewV1::from_components_v1(bytes, upstream, |_| Some(commitments))
            .map(|_| ())
    };

    let mut geometry = wire.clone();
    geometry[13] ^= 1;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry)
    );
    let mut order = wire.clone();
    order[HEADER_BYTES_V1 + 1] = 1;
    assert_eq!(
        parse(&order),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidGeometry)
    );
    let core = HEADER_BYTES_V1 + RECORD_HEADER_BYTES_V1;
    let mut invalid_point = wire.clone();
    invalid_point[core..core + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidPoint)
    );
    let mut invalid_scalar = wire.clone();
    let scalar = core + FIXED_CORE_POINTS_V1 * POINT_BYTES_V1;
    invalid_scalar[scalar..scalar + SCALAR_BYTES_V1].fill(0xff);
    assert_eq!(
        parse(&invalid_scalar),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidScalar)
    );
    let mut changed = commitments;
    changed.difference_low[0] += test_points_v1()[3];
    assert_eq!(
        RadixComplementProofSetViewV1::from_components_v1(&wire, upstream, |_| Some(changed))
            .map(|_| ()),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity)
    );
    let mut changed_residual = wire;
    changed_residual[HEADER_BYTES_V1 + RECORD_SET_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&changed_residual),
        Err(RnsNativeRadixComplementLinearErrorV1::InvalidIntegrity)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyRadixComplementSuiteV1;

impl ProofSuite for TinyRadixComplementSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyRadixComplementSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points =
                derive_t256_generators_v1(b"rns-native-radix-complement-linear-tiny-suite-v1", 6)
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
        group: usize,
        commitments: RadixComplementCoreCommitmentsV1,
        coordinates: usize,
        padded_gates: usize,
        basis: [u8; DIGEST_BYTES_V1],
    ) -> Self {
        Self {
            state: initial_transcript_state_v1(
                upstream,
                group,
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
struct RawRadixValuesV1 {
    difference_low: [Vec<Scalar>; RADIX_LOW_DIGITS_V1],
    slack_low: [Vec<Scalar>; RADIX_LOW_DIGITS_V1],
    difference_top: Vec<Scalar>,
    slack_top: Vec<Scalar>,
}

fn decrement_be_v1(mut value: [u8; 32]) -> [u8; 32] {
    for index in (0..value.len()).rev() {
        let (next, borrow) = value[index].overflowing_sub(1);
        value[index] = next;
        if !borrow {
            return value;
        }
    }
    panic!("nonzero T256 value")
}

fn subtract_small_be_v1(mut value: [u8; 32], amount: u8) -> [u8; 32] {
    for _ in 0..amount {
        value = decrement_be_v1(value);
    }
    value
}

fn radix_components_v1(encoded: [u8; 32]) -> ([Scalar; RADIX_LOW_DIGITS_V1], Scalar) {
    let low = core::array::from_fn(|digit| {
        let mut value = 0_u64;
        for bit in 0..RADIX_LOG2_V1 as usize {
            let absolute = digit * RADIX_LOG2_V1 as usize + bit;
            let byte = encoded[31 - absolute / 8];
            value |= u64::from((byte >> (absolute % 8)) & 1) << bit;
        }
        Scalar::from_u64(value)
    });
    let top = Scalar::from_u64(u64::from(encoded[0] >> 7));
    (low, top)
}

fn honest_values_v1() -> RawRadixValuesV1 {
    let differences = [3_u8, 9_u8];
    let components = differences.map(|difference| {
        let mut difference_bytes = [0_u8; 32];
        difference_bytes[31] = difference;
        let slack_bytes = subtract_small_be_v1(VEGA_T256_SCALAR_MODULUS_BE_V1, difference + 1);
        (
            radix_components_v1(difference_bytes),
            radix_components_v1(slack_bytes),
        )
    });
    RawRadixValuesV1 {
        difference_low: core::array::from_fn(|digit| {
            components.iter().map(|((low, _), _)| low[digit]).collect()
        }),
        slack_low: core::array::from_fn(|digit| {
            components.iter().map(|(_, (low, _))| low[digit]).collect()
        }),
        difference_top: components.iter().map(|((_, top), _)| *top).collect(),
        slack_top: components.iter().map(|(_, (_, top))| *top).collect(),
    }
}

fn commitment_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinyRadixComplementSuiteV1::generators();
    let mut terms = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect::<Vec<_>>();
    terms.push((mask, generators.h));
    multiexp::<TinyRadixComplementSuiteV1>(&terms)
}

fn weighted_values_v1(low: &[Vec<Scalar>; RADIX_LOW_DIGITS_V1], top: &[Scalar]) -> Vec<Scalar> {
    let mut output = vec![Scalar::zero(); top.len()];
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    let mut weight = Scalar::one();
    for digit in low {
        for (output, value) in output.iter_mut().zip(digit) {
            *output += weight * *value;
        }
        weight *= radix;
    }
    for (output, value) in output.iter_mut().zip(top) {
        *output += weight * *value;
    }
    output
}

fn weighted_mask_v1(low: [Scalar; RADIX_LOW_DIGITS_V1], top: Scalar) -> Scalar {
    let radix = Scalar::from_u64(RADIX_BASE_V1);
    let mut weight = Scalar::one();
    let mut output = Scalar::zero();
    for mask in low {
        output += weight * mask;
        weight *= radix;
    }
    output + weight * top
}

fn tiny_commitments_and_witness_v1(
    values: &RawRadixValuesV1,
) -> (
    RadixComplementCoreCommitmentsV1,
    ArithmeticCircuitWitness<TinyRadixComplementSuiteV1>,
) {
    let difference_masks: [Scalar; RADIX_LOW_DIGITS_V1] =
        core::array::from_fn(|digit| Scalar::from_u64(11 + digit as u64));
    let slack_masks: [Scalar; RADIX_LOW_DIGITS_V1] =
        core::array::from_fn(|digit| Scalar::from_u64(41 + digit as u64));
    let difference_top_mask = Scalar::from_u64(71);
    let slack_top_mask = Scalar::from_u64(73);
    let raw = ExistingRadixCommitmentsV1 {
        difference_low: core::array::from_fn(|digit| {
            commitment_v1(&values.difference_low[digit], difference_masks[digit])
        }),
        slack_low: core::array::from_fn(|digit| {
            commitment_v1(&values.slack_low[digit], slack_masks[digit])
        }),
        difference_top: commitment_v1(&values.difference_top, difference_top_mask),
        slack_top: commitment_v1(&values.slack_top, slack_top_mask),
    };
    let commitments =
        RadixComplementCoreCommitmentsV1::new_v1(raw).expect("valid tiny radix commitments");
    let mut combined = weighted_values_v1(&values.difference_low, &values.difference_top);
    for (value, slack) in combined
        .iter_mut()
        .zip(weighted_values_v1(&values.slack_low, &values.slack_top))
    {
        *value += slack;
    }
    let combined_mask = weighted_mask_v1(difference_masks, difference_top_mask)
        + weighted_mask_v1(slack_masks, slack_top_mask);
    let openings = vec![VectorCommitmentOpening::new(combined, combined_mask)];
    let zeros = vec![Scalar::zero(); values.difference_top.len()];
    let witness = ArithmeticCircuitWitness::new(zeros.clone(), zeros, openings)
        .expect("shape-valid radix-complement witness");
    (commitments, witness)
}

type TinyProofV1 = (
    Vec<u8>,
    [u8; DIGEST_BYTES_V1],
    UpstreamBindingV1,
    RadixComplementCoreCommitmentsV1,
);

fn prove_tiny_v1(values: RawRadixValuesV1) -> Result<TinyProofV1, GeneralizedBulletproofErrorV1> {
    let coordinates = values.difference_top.len();
    let (commitments, witness) = tiny_commitments_and_witness_v1(&values);
    let upstream = upstream_v1();
    let basis = hash_v1(b"tiny-radix-complement-linear-basis");
    let mut transcript = TestProverTranscriptV1::<TinyRadixComplementSuiteV1>::new(
        upstream,
        0,
        commitments,
        coordinates,
        coordinates,
        basis,
    );
    build_radix_complement_statement_v1::<TinyRadixComplementSuiteV1>(
        coordinates,
        coordinates,
        commitments,
    )
    .map_err(|_| GeneralizedBulletproofErrorV1::ArithmeticInvariant)?
    .prove(
        &mut TestRandomV1::new(b"tiny-radix-complement-linear-proof-rng"),
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
    group: usize,
    commitments: RadixComplementCoreCommitmentsV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeRadixComplementLinearErrorV1> {
    let core = ExactCoreViewV1 { bytes: proof };
    let basis = hash_v1(b"tiny-radix-complement-linear-basis");
    let mut transcript = RadixComplementVerifierTranscriptV1::<TinyRadixComplementSuiteV1>::new_v1(
        upstream,
        group,
        commitments,
        2,
        2,
        basis,
        core,
    )?;
    build_radix_complement_statement_v1::<TinyRadixComplementSuiteV1>(2, 2, commitments)?
        .verify(&mut transcript)?;
    transcript.finish_v1()
}

#[test]
fn tiny_real_statement2_core_roundtrips_and_binds_proof_context_and_owners() {
    let (proof, prover_digest, upstream, commitments) =
        prove_tiny_v1(honest_values_v1()).expect("valid statement-2 proof");
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
    changed_upstream.pre_z_candidate_root[0] ^= 1;
    assert!(verify_tiny_v1(&proof, changed_upstream, 0, commitments).is_err());
    let mut changed_commitments = commitments;
    changed_commitments.derived[0] += TinyRadixComplementSuiteV1::generators().g;
    assert!(verify_tiny_v1(&proof, upstream, 0, changed_commitments).is_err());

    let mut swapped_roles = commitments;
    core::mem::swap(
        &mut swapped_roles.raw.difference_low,
        &mut swapped_roles.raw.slack_low,
    );
    core::mem::swap(
        &mut swapped_roles.raw.difference_top,
        &mut swapped_roles.raw.slack_top,
    );
    assert!(verify_tiny_v1(&proof, upstream, 0, swapped_roles).is_err());

    let mut swapped_columns = commitments;
    swapped_columns.raw.difference_low.swap(0, 1);
    assert!(verify_tiny_v1(&proof, upstream, 0, swapped_columns).is_err());

    let mut changed_top_alias = commitments;
    changed_top_alias.raw.difference_top += TinyRadixComplementSuiteV1::generators().g;
    assert!(verify_tiny_v1(&proof, upstream, 0, changed_top_alias).is_err());
}

#[test]
fn false_radix_complement_field_relation_cannot_produce_a_proof() {
    let mut values = honest_values_v1();
    values.slack_low[0][0] += Scalar::one();
    assert!(prove_tiny_v1(values).is_err());
}

#[test]
fn out_of_range_radix_digits_remain_only_a_field_valid_witness() {
    let mut values = honest_values_v1();
    values.difference_low[0][0] += Scalar::from_u64(RADIX_BASE_V1);
    values.difference_low[1][0] -= Scalar::one();
    assert!(
        prove_tiny_v1(values).is_ok(),
        "statement 2 alone must not claim 15-bit digit membership"
    );
}

#[test]
fn statement2_boundary_is_private_move_only_and_all_later_claims_fail_closed() {
    assert_eq!(GROUPS_V1, 344);
    assert_eq!(CORES_V1, 344);
    assert_eq!(GATES_V1, 16_384);
    assert_eq!(CONSTRAINTS_PER_CORE_V1, 16_384);
    assert_eq!(COMMITMENTS_PER_CORE_V1, 1);
    assert_eq!(FIXED_CORE_POINTS_V1, 9);
    assert_eq!(CORE_BYTES_V1, 1_381);
    assert_eq!(RECORD_SET_BYTES_V1, 476_440);
    assert_eq!(HEADER_BYTES_V1, 464);
    assert_eq!(MIN_WIRE_BYTES_V1, 476_937);
    assert_eq!(
        RNS_NATIVE_RADIX_COMPLEMENT_LINEAR_RESIDUAL_MAX_BYTES_V1,
        1_340_903
    );

    let source = include_str!("rns_native_radix_complement_linear_relation.rs");
    let declaration = "pub(super) struct RnsNativeRadixComplementLinearPrerequisiteV1";
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
    assert!(source.contains("RADIX_COMPLEMENT_FIELD_RELATION_VERIFIED_V1: bool = true"));
    assert!(source.contains("EXISTING_RADIX_TRANSPORT_ORDER_AUTHENTICATED_V1: bool = true"));
    for flag in [
        "SOLE_Z_GLOBAL_SLOT_PERMUTATION_VERIFIED_V1: bool = false",
        "SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1: bool = false",
        "RADIX_DIGIT_MEMBERSHIP_AND_INVERSES_VERIFIED_V1: bool = false",
        "CANONICAL_RADIX_RECONSTRUCTION_VERIFIED_V1: bool = false",
        "CANONICAL_RADIX_COMPLEMENT_VERIFIED_V1: bool = false",
        "CENTERING_SUBTRACTION_VERIFIED_V1: bool = false",
        "GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false",
        "CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false",
        "RELEASE_READY_V1: bool = false",
    ] {
        assert!(source.contains(flag));
    }
    assert!(source.contains("for group in 0..GROUPS_V1"));
    assert!(source.contains("build_radix_complement_statement_v1"));
    assert!(source.contains(".verify(&mut transcript)?"));
    assert_eq!(source.matches("previous.residual_digest()").count(), 1);
    assert_eq!(source.matches("previous.binding_digest()").count(), 1);

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_radix_complement_linear_relation;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_radix_complement_linear_relation"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
