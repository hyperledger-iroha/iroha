#![deny(missing_docs)]

//! Halo2 circuits and helpers for Kaigi privacy proofs.
//!
//! The roster join circuit checks that the public commitment and nullifier are
//! domain-separated Poseidon hashes of one prover-supplied account and its
//! private salts. The current roster root is a separate public state binding:
//! a join creates a new roster leaf, so it is deliberately not a membership
//! claim about the pre-join tree. Private leave remains disabled until a
//! dedicated Merkle-membership circuit is available.

use core::array;
use std::sync::OnceLock;

use halo2_proofs::{
    circuit::{Cell, Layouter, Region, SimpleFloorPlanner, Value},
    halo2curves::{
        ff::{Field, PrimeField},
        pasta::Fp,
    },
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error, Fixed, Instance, Selector,
    },
    poly::Rotation,
};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use poseidon_primitives::poseidon::primitives::Spec;

/// Scalar field used by the Kaigi Halo2 circuits (Pasta Fp).
pub type Scalar = Fp;

/// Backend identifier used by the roster join circuit verifier metadata.
pub const KAIGI_ROSTER_BACKEND: &str = "halo2/pasta/kaigi-roster-v1";

/// Default log2 domain size used when instantiating the roster join circuit.
///
/// `k = 8` gives a 256-row domain, which accommodates two complete
/// domain-separated Poseidon permutations plus the public roster-root binding.
pub const KAIGI_ROSTER_CIRCUIT_K: u32 = 8;

/// Number of little-endian 64-bit limbs used to expose the roster root.
pub const KAIGI_ROSTER_ROOT_LIMBS: usize = 4;

/// Backend identifier used by the usage commitment circuit verifier metadata.
pub const KAIGI_USAGE_BACKEND: &str = "halo2/pasta/kaigi-usage-v1";

/// Default log2 domain size for the usage commitment circuit.
pub const KAIGI_USAGE_CIRCUIT_K: u32 = 8;

const POSEIDON_WIDTH: usize = 3;
const POSEIDON_RATE: usize = 2;
const POSEIDON_FULL_ROUNDS: usize = 8;
const POSEIDON_PARTIAL_ROUNDS: usize = 56;
const POSEIDON_ROUNDS: usize = POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS;

// These capacity words are part of the first-release circuit statement. They
// keep commitments from being replayed as nullifiers or usage commitments.
const DOMAIN_ROSTER_COMMITMENT: u64 = 0x4b41_4947_4943_4d54;
const DOMAIN_ROSTER_NULLIFIER: u64 = 0x4b41_4947_494e_554c;
const DOMAIN_USAGE_STAGE: u64 = 0x4b41_4947_5553_4731;
const DOMAIN_USAGE_COMMITMENT: u64 = 0x4b41_4947_5553_4732;

#[derive(Debug)]
struct KaigiPoseidonSpec;

impl Spec<Scalar, POSEIDON_WIDTH, POSEIDON_RATE> for KaigiPoseidonSpec {
    fn full_rounds() -> usize {
        POSEIDON_FULL_ROUNDS
    }

    fn partial_rounds() -> usize {
        POSEIDON_PARTIAL_ROUNDS
    }

    fn sbox(value: Scalar) -> Scalar {
        value.pow_vartime([5])
    }

    fn secure_mds() -> usize {
        0
    }
}

struct PoseidonConstants {
    round_constants: Vec<[Scalar; POSEIDON_WIDTH]>,
    mds: [[Scalar; POSEIDON_WIDTH]; POSEIDON_WIDTH],
}

fn poseidon_constants() -> &'static PoseidonConstants {
    static CONSTANTS: OnceLock<PoseidonConstants> = OnceLock::new();
    CONSTANTS.get_or_init(|| {
        let (round_constants, mds, _) =
            <KaigiPoseidonSpec as Spec<Scalar, POSEIDON_WIDTH, POSEIDON_RATE>>::constants();
        assert_eq!(round_constants.len(), POSEIDON_ROUNDS);
        PoseidonConstants {
            round_constants,
            mds,
        }
    })
}

/// Shared configuration for the fixed Poseidon permutation.
#[derive(Clone, Debug)]
struct KaigiPoseidonConfig {
    state: [Column<Advice>; POSEIDON_WIDTH],
    round_constants: [Column<Fixed>; POSEIDON_WIDTH],
    domain: Column<Fixed>,
    q_init: Selector,
    q_full_round: Selector,
    q_partial_round: Selector,
}

/// Configuration for the roster join Halo2 circuit.
#[derive(Clone, Debug)]
pub struct KaigiRosterConfig {
    poseidon: KaigiPoseidonConfig,
    instance_commitment: Column<Instance>,
    instance_nullifier: Column<Instance>,
    roster_root_limbs: [Column<Advice>; KAIGI_ROSTER_ROOT_LIMBS],
    instance_roster_root_limbs: [Column<Instance>; KAIGI_ROSTER_ROOT_LIMBS],
}

/// Halo2 circuit proving that a commitment/nullifier pair matches the supplied
/// account, domain salt, and nullifier seed.
#[derive(Clone, Debug, Default)]
pub struct KaigiRosterJoinCircuit {
    account: Option<Scalar>,
    domain_salt: Option<Scalar>,
    nullifier_seed: Option<Scalar>,
    roster_root_limbs: [Option<Scalar>; KAIGI_ROSTER_ROOT_LIMBS],
}

impl KaigiRosterJoinCircuit {
    /// Create a circuit instance with the provided witnesses.
    #[must_use]
    pub fn new(
        account: Scalar,
        domain_salt: Scalar,
        nullifier_seed: Scalar,
        roster_root_limbs: [Scalar; KAIGI_ROSTER_ROOT_LIMBS],
    ) -> Self {
        Self {
            account: Some(account),
            domain_salt: Some(domain_salt),
            nullifier_seed: Some(nullifier_seed),
            roster_root_limbs: roster_root_limbs.map(Some),
        }
    }
}

fn configure_poseidon(meta: &mut ConstraintSystem<Scalar>) -> KaigiPoseidonConfig {
    let state = array::from_fn(|_| {
        let column = meta.advice_column();
        meta.enable_equality(column);
        column
    });
    let round_constants = array::from_fn(|_| meta.fixed_column());
    let domain = meta.fixed_column();
    let q_init = meta.selector();
    let q_full_round = meta.selector();
    let q_partial_round = meta.selector();

    meta.create_gate("kaigi Poseidon domain", |meta| {
        let enabled = meta.query_selector(q_init);
        let capacity = meta.query_advice(state[2], Rotation::cur());
        let expected_domain = meta.query_fixed(domain, Rotation::cur());
        vec![enabled * (capacity - expected_domain)]
    });

    let constants = poseidon_constants();
    meta.create_gate("kaigi Poseidon full round", |meta| {
        let enabled = meta.query_selector(q_full_round);
        (0..POSEIDON_WIDTH)
            .map(|row| {
                let expected = (0..POSEIDON_WIDTH).fold(
                    halo2_proofs::plonk::Expression::Constant(Scalar::ZERO),
                    |accumulator, column| {
                        let current = meta.query_advice(state[column], Rotation::cur());
                        let round_constant =
                            meta.query_fixed(round_constants[column], Rotation::cur());
                        let shifted = current + round_constant;
                        let square = shifted.clone() * shifted.clone();
                        let fifth = square.clone() * square * shifted;
                        accumulator + fifth * constants.mds[row][column]
                    },
                );
                let next = meta.query_advice(state[row], Rotation::next());
                enabled.clone() * (expected - next)
            })
            .collect::<Vec<_>>()
    });
    meta.create_gate("kaigi Poseidon partial round", |meta| {
        let enabled = meta.query_selector(q_partial_round);
        let shifted = array::from_fn::<_, POSEIDON_WIDTH, _>(|column| {
            meta.query_advice(state[column], Rotation::cur())
                + meta.query_fixed(round_constants[column], Rotation::cur())
        });
        let square = shifted[0].clone() * shifted[0].clone();
        let first_fifth = square.clone() * square * shifted[0].clone();
        (0..POSEIDON_WIDTH)
            .map(|row| {
                let expected = first_fifth.clone() * constants.mds[row][0]
                    + shifted[1].clone() * constants.mds[row][1]
                    + shifted[2].clone() * constants.mds[row][2];
                let next = meta.query_advice(state[row], Rotation::next());
                enabled.clone() * (expected - next)
            })
            .collect::<Vec<_>>()
    });

    KaigiPoseidonConfig {
        state,
        round_constants,
        domain,
        q_init,
        q_full_round,
        q_partial_round,
    }
}

impl Circuit<Scalar> for KaigiRosterJoinCircuit {
    type Config = KaigiRosterConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let poseidon = configure_poseidon(meta);

        let instance_commitment = meta.instance_column();
        let instance_nullifier = meta.instance_column();
        meta.enable_equality(instance_commitment);
        meta.enable_equality(instance_nullifier);

        let roster_root_limbs = array::from_fn(|_| {
            let column = meta.advice_column();
            meta.enable_equality(column);
            column
        });
        let instance_roster_root_limbs = array::from_fn(|_| {
            let column = meta.instance_column();
            meta.enable_equality(column);
            column
        });

        KaigiRosterConfig {
            poseidon,
            instance_commitment,
            instance_nullifier,
            roster_root_limbs,
            instance_roster_root_limbs,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), Error> {
        let account_value = to_value(self.account);
        let domain_value = to_value(self.domain_salt);
        let nullifier_seed_value = to_value(self.nullifier_seed);
        let roster_root_values = self.roster_root_limbs.map(to_value);

        let (commitment_cell, nullifier_cell) = layouter.assign_region(
            || "kaigi roster Poseidon commitments",
            |mut region| {
                let (commitment_account, commitment_cell) = assign_poseidon_permutation(
                    &mut region,
                    &config.poseidon,
                    0,
                    DOMAIN_ROSTER_COMMITMENT,
                    account_value,
                    domain_value,
                )?;
                let (nullifier_account, nullifier_cell) = assign_poseidon_permutation(
                    &mut region,
                    &config.poseidon,
                    POSEIDON_ROUNDS + 1,
                    DOMAIN_ROSTER_NULLIFIER,
                    account_value,
                    nullifier_seed_value,
                )?;
                region.constrain_equal(commitment_account, nullifier_account);

                Ok((commitment_cell, nullifier_cell))
            },
        )?;

        layouter.constrain_instance(commitment_cell, config.instance_commitment, 0);
        layouter.constrain_instance(nullifier_cell, config.instance_nullifier, 0);

        let root_cells = layouter.assign_region(
            || "kaigi roster root limbs",
            |mut region| {
                let mut cells: Vec<_> = Vec::with_capacity(KAIGI_ROSTER_ROOT_LIMBS);
                for (idx, value) in roster_root_values.iter().enumerate() {
                    let cell = region.assign_advice(config.roster_root_limbs[idx], 0, *value);
                    cells.push(cell);
                }
                Ok(cells)
            },
        )?;

        for (idx, cell) in root_cells.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), config.instance_roster_root_limbs[idx], 0);
        }

        Ok(())
    }
}

fn value_pow5(value: Value<Scalar>) -> Value<Scalar> {
    let square = value * value;
    square * square * value
}

fn poseidon_round(mut state: [Scalar; POSEIDON_WIDTH], round: usize) -> [Scalar; POSEIDON_WIDTH] {
    let constants = poseidon_constants();
    for (word, round_constant) in state
        .iter_mut()
        .zip(constants.round_constants[round].iter())
    {
        *word += round_constant;
    }
    let half_full_rounds = POSEIDON_FULL_ROUNDS / 2;
    if round < half_full_rounds || round >= half_full_rounds + POSEIDON_PARTIAL_ROUNDS {
        for word in &mut state {
            *word = word.pow_vartime([5]);
        }
    } else {
        state[0] = state[0].pow_vartime([5]);
    }
    array::from_fn(|row| {
        (0..POSEIDON_WIDTH).fold(Scalar::ZERO, |accumulator, column| {
            accumulator + constants.mds[row][column] * state[column]
        })
    })
}

fn poseidon_round_value(
    mut state: [Value<Scalar>; POSEIDON_WIDTH],
    round: usize,
) -> [Value<Scalar>; POSEIDON_WIDTH] {
    let constants = poseidon_constants();
    for (word, round_constant) in state
        .iter_mut()
        .zip(constants.round_constants[round].iter())
    {
        *word = *word + Value::known(*round_constant);
    }
    let half_full_rounds = POSEIDON_FULL_ROUNDS / 2;
    if round < half_full_rounds || round >= half_full_rounds + POSEIDON_PARTIAL_ROUNDS {
        for word in &mut state {
            *word = value_pow5(*word);
        }
    } else {
        state[0] = value_pow5(state[0]);
    }
    array::from_fn(|row| {
        (0..POSEIDON_WIDTH).fold(Value::known(Scalar::ZERO), |accumulator, column| {
            accumulator + state[column] * Value::known(constants.mds[row][column])
        })
    })
}

fn poseidon_compress(domain: u64, left: Scalar, right: Scalar) -> Scalar {
    let mut state = [left, right, Scalar::from(domain)];
    for round in 0..POSEIDON_ROUNDS {
        state = poseidon_round(state, round);
    }
    state[0]
}

fn assign_poseidon_permutation(
    region: &mut Region<'_, Scalar>,
    config: &KaigiPoseidonConfig,
    start_row: usize,
    domain: u64,
    left: Value<Scalar>,
    right: Value<Scalar>,
) -> Result<(Cell, Cell), Error> {
    config.q_init.enable(region, start_row)?;
    region.assign_fixed(config.domain, start_row, Scalar::from(domain));

    let mut state_values = [left, right, Value::known(Scalar::from(domain))];
    let mut state_cells =
        array::from_fn(|column| region.assign_advice(config.state[column], start_row, state_values[column]));
    let left_cell = state_cells[0].cell();

    for round in 0..POSEIDON_ROUNDS {
        for column in 0..POSEIDON_WIDTH {
            region.assign_fixed(
                config.round_constants[column],
                start_row + round,
                poseidon_constants().round_constants[round][column],
            );
        }
        let half_full_rounds = POSEIDON_FULL_ROUNDS / 2;
        if round < half_full_rounds || round >= half_full_rounds + POSEIDON_PARTIAL_ROUNDS {
            config.q_full_round.enable(region, start_row + round)?;
        } else {
            config
                .q_partial_round
                .enable(region, start_row + round)?;
        }

        state_values = poseidon_round_value(state_values, round);
        state_cells = array::from_fn(|column| {
            region.assign_advice(
                config.state[column],
                start_row + round + 1,
                state_values[column],
            )
        });
    }

    Ok((left_cell, state_cells[0].cell()))
}

/// Compute the roster commitment as a Pasta field element.
#[must_use]
pub fn compute_commitment(account: Scalar, domain_salt: Scalar) -> Scalar {
    poseidon_compress(DOMAIN_ROSTER_COMMITMENT, account, domain_salt)
}

/// Compute the roster nullifier as a Pasta field element.
#[must_use]
pub fn compute_nullifier(account: Scalar, nullifier_seed: Scalar) -> Scalar {
    poseidon_compress(DOMAIN_ROSTER_NULLIFIER, account, nullifier_seed)
}

/// Compute the usage commitment (duration, gas, segment) as a Pasta field element.
#[must_use]
pub fn compute_usage_commitment(
    duration_ms: Scalar,
    billed_gas: Scalar,
    segment_index: Scalar,
) -> Scalar {
    let stage = poseidon_compress(DOMAIN_USAGE_STAGE, duration_ms, billed_gas);
    poseidon_compress(DOMAIN_USAGE_COMMITMENT, stage, segment_index)
}

/// Compute the roster commitment as a byte array matching the circuit output.
#[must_use]
pub fn compute_commitment_bytes(account: u64, domain_salt: u64) -> [u8; Hash::LENGTH] {
    scalar_to_bytes(compute_commitment(
        Scalar::from(account),
        Scalar::from(domain_salt),
    ))
}

/// Compute the roster nullifier as a byte array matching the circuit output.
#[must_use]
pub fn compute_nullifier_bytes(account: u64, nullifier_seed: u64) -> [u8; Hash::LENGTH] {
    scalar_to_bytes(compute_nullifier(
        Scalar::from(account),
        Scalar::from(nullifier_seed),
    ))
}

/// Compute the usage commitment components as a byte array.
#[must_use]
pub fn compute_usage_commitment_bytes(
    duration_ms: u64,
    billed_gas: u64,
    segment_index: u64,
) -> [u8; Hash::LENGTH] {
    scalar_to_bytes(compute_usage_commitment(
        Scalar::from(duration_ms),
        Scalar::from(billed_gas),
        Scalar::from(segment_index),
    ))
}

/// Convert a roster root hash into its little-endian u64 limb representation.
#[must_use]
pub fn roster_root_limb_values(root: &Hash) -> [u64; KAIGI_ROSTER_ROOT_LIMBS] {
    let bytes = root.as_ref();
    array::from_fn(|idx| {
        let start = idx * 8;
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&bytes[start..start + 8]);
        u64::from_le_bytes(chunk)
    })
}

/// Convert a roster root hash into Pasta scalars (limbs) suitable for public inputs.
#[must_use]
pub fn roster_root_limbs(root: &Hash) -> [Scalar; KAIGI_ROSTER_ROOT_LIMBS] {
    roster_root_limb_values(root).map(Scalar::from)
}

fn to_value(input: Option<Scalar>) -> Value<Scalar> {
    input.map(Value::known).unwrap_or_else(Value::unknown)
}

fn scalar_to_bytes(value: Scalar) -> [u8; Hash::LENGTH] {
    let mut out = [0u8; Hash::LENGTH];
    out.copy_from_slice(value.to_repr().as_ref());
    out
}

/// Decode a canonical Pasta scalar stored in a Kaigi commitment hash.
///
/// Kaigi commitments and nullifiers use [`Hash::prehashed`] as a typed
/// 32-byte container for the circuit output. Values at or above the Pasta
/// modulus are therefore rejected instead of being reduced modulo the field.
#[must_use]
pub fn scalar_from_hash(hash: &Hash) -> Option<Scalar> {
    let mut representation = <Scalar as PrimeField>::Repr::default();
    representation.as_mut().copy_from_slice(hash.as_ref());
    Option::from(Scalar::from_repr(representation))
}

/// Domain separation tag for Kaigi roster commitment leaves.
const KAIGI_ROSTER_LEAF_TAG: &[u8] = b"iroha:kaigi:roster:leaf:v1\x00";
/// Seed used for the deterministic empty Kaigi roster root.
const KAIGI_ROSTER_EMPTY_SEED: &[u8] = b"iroha:kaigi:roster:empty:v1\x00";

/// Deterministic empty-root hash shared with the data-model helpers.
#[must_use]
pub fn empty_roster_root_hash() -> Hash {
    Hash::new(KAIGI_ROSTER_EMPTY_SEED)
}

/// Convert a roster leaf commitment into a Merkle leaf hash.
#[must_use]
pub fn roster_leaf_hash(commitment: &Hash) -> HashOf<[u8; 32]> {
    let mut buf = [0u8; KAIGI_ROSTER_LEAF_TAG.len() + Hash::LENGTH];
    buf[..KAIGI_ROSTER_LEAF_TAG.len()].copy_from_slice(KAIGI_ROSTER_LEAF_TAG);
    buf[KAIGI_ROSTER_LEAF_TAG.len()..].copy_from_slice(commitment.as_ref());
    HashOf::from_untyped_unchecked(Hash::new(buf))
}

/// Compute the roster Merkle root from a list of commitments.
#[must_use]
pub fn compute_roster_root_hash(commitments: &[Hash]) -> Hash {
    if commitments.is_empty() {
        return empty_roster_root_hash();
    }
    let mut tree = MerkleTree::<[u8; 32]>::default();
    for commitment in commitments {
        tree.add(roster_leaf_hash(commitment));
    }
    tree.root()
        .map(Hash::from)
        .unwrap_or_else(empty_roster_root_hash)
}

/// Compute the roster commitment hash (bytes + Norito hash wrapper).
#[must_use]
pub fn compute_commitment_hash(account: u64, domain_salt: u64) -> Hash {
    Hash::prehashed(compute_commitment_bytes(account, domain_salt))
}

/// Compute the roster nullifier hash (bytes + Norito hash wrapper).
#[must_use]
pub fn compute_nullifier_hash(account: u64, nullifier_seed: u64) -> Hash {
    Hash::prehashed(compute_nullifier_bytes(account, nullifier_seed))
}

/// Compute the usage commitment hash (bytes + Norito hash wrapper).
#[must_use]
pub fn compute_usage_commitment_hash(
    duration_ms: u64,
    billed_gas: u64,
    segment_index: u64,
) -> Hash {
    Hash::prehashed(compute_usage_commitment_bytes(
        duration_ms,
        billed_gas,
        segment_index,
    ))
}

/// Configuration for the Kaigi usage commitment circuit.
#[derive(Clone, Debug)]
pub struct KaigiUsageConfig {
    poseidon: KaigiPoseidonConfig,
    instance_commitment: Column<Instance>,
}

/// Halo2 circuit proving that a usage commitment matches `(duration, gas, segment)`.
#[derive(Clone, Debug, Default)]
pub struct KaigiUsageCommitmentCircuit {
    duration_ms: Option<Scalar>,
    billed_gas: Option<Scalar>,
    segment_index: Option<Scalar>,
}

impl KaigiUsageCommitmentCircuit {
    /// Create a circuit with the provided witnesses.
    #[must_use]
    pub fn new(duration_ms: Scalar, billed_gas: Scalar, segment_index: Scalar) -> Self {
        Self {
            duration_ms: Some(duration_ms),
            billed_gas: Some(billed_gas),
            segment_index: Some(segment_index),
        }
    }
}

impl Circuit<Scalar> for KaigiUsageCommitmentCircuit {
    type Config = KaigiUsageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let poseidon = configure_poseidon(meta);

        let instance_commitment = meta.instance_column();
        meta.enable_equality(instance_commitment);

        KaigiUsageConfig {
            poseidon,
            instance_commitment,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), Error> {
        let duration_value = to_value(self.duration_ms);
        let billed_value = to_value(self.billed_gas);
        let segment_value = to_value(self.segment_index);

        let commitment_cell = layouter.assign_region(
            || "kaigi usage Poseidon commitment",
            |mut region| {
                let (_, stage_cell) = assign_poseidon_permutation(
                    &mut region,
                    &config.poseidon,
                    0,
                    DOMAIN_USAGE_STAGE,
                    duration_value,
                    billed_value,
                )?;
                let stage_value = duration_value
                    .zip(billed_value)
                    .map(|(duration, billed)| {
                        poseidon_compress(DOMAIN_USAGE_STAGE, duration, billed)
                    });
                let (stage_again, commitment_cell) = assign_poseidon_permutation(
                    &mut region,
                    &config.poseidon,
                    POSEIDON_ROUNDS + 1,
                    DOMAIN_USAGE_COMMITMENT,
                    stage_value,
                    segment_value,
                )?;
                region.constrain_equal(stage_cell, stage_again);

                Ok(commitment_cell)
            },
        )?;

        layouter.constrain_instance(commitment_cell, config.instance_commitment, 0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{EqAffine as Curve, Fp as FieldScalar},
        plonk::{keygen_pk, keygen_vk},
        poly::{commitment::ParamsProver, ipa::commitment::ParamsIPA},
    };

    use super::*;

    #[test]
    fn compressors_return_distinct_outputs() {
        let commitment = compute_commitment_bytes(11, 31);
        let nullifier = compute_nullifier_bytes(11, 57);
        let usage = compute_usage_commitment_bytes(1_200, 345, 2);

        assert_ne!(commitment, nullifier);
        assert_ne!(commitment, usage);
        assert_ne!(nullifier, usage);
    }

    #[test]
    fn poseidon_rejects_a_collision_in_the_retired_quintic_compressor() {
        // x -> x^5 is a permutation in Pasta Fp. The retired separable
        // compressor therefore made chosen-input collisions directly
        // constructible by solving one fifth root.
        let inverse_five_exponent = [
            0xe0f0_f3f0_cccc_cccd,
            0x4e9e_e0c9_a10a_60e2,
            0x3333_3333_3333_3333,
            0x3333_3333_3333_3333,
        ];
        let inverse_three =
            Option::<Scalar>::from(Scalar::from(3u64).invert()).expect("three is non-zero");
        let fifth_root = (Scalar::from(2u64) * inverse_three)
            .pow_vartime(inverse_five_exponent);

        let first = (Scalar::ONE - Scalar::from(7u64), -Scalar::from(13u64));
        let second = (
            -Scalar::from(7u64),
            fifth_root - Scalar::from(13u64),
        );
        let retired_compressor = |left: Scalar, right: Scalar| {
            Scalar::from(2u64) * (left + Scalar::from(7u64)).pow_vartime([5])
                + Scalar::from(3u64) * (right + Scalar::from(13u64)).pow_vartime([5])
        };

        assert_eq!(
            retired_compressor(first.0, first.1),
            retired_compressor(second.0, second.1)
        );
        assert_ne!(
            compute_commitment(first.0, first.1),
            compute_commitment(second.0, second.1)
        );
    }

    #[test]
    fn commitment_hash_scalar_decoding_is_canonical() {
        let scalar = compute_commitment(Scalar::from(11u64), Scalar::from(31u64));
        let hash = Hash::prehashed(scalar_to_bytes(scalar));
        assert_eq!(scalar_from_hash(&hash), Some(scalar));

        let non_canonical = Hash::prehashed([u8::MAX; Hash::LENGTH]);
        assert_eq!(scalar_from_hash(&non_canonical), None);
    }

    #[test]
    fn roster_circuit_keygen_succeeds() {
        let params: ParamsIPA<Curve> = ParamsIPA::new(KAIGI_ROSTER_CIRCUIT_K);
        let account = FieldScalar::from(3u64);
        let domain_salt = FieldScalar::from(17u64);
        let nullifier_seed = FieldScalar::from(25u64);
        let root_hash = empty_roster_root_hash();
        let root_limbs = roster_root_limbs(&root_hash);
        let circuit = KaigiRosterJoinCircuit::new(account, domain_salt, nullifier_seed, root_limbs);

        let vk = keygen_vk(&params, &circuit).expect("vk");
        let _pk = keygen_pk(&params, vk, &circuit).expect("pk");
    }

    #[test]
    fn roster_circuit_binds_every_public_root_limb() {
        let account = FieldScalar::from(3u64);
        let domain_salt = FieldScalar::from(17u64);
        let nullifier_seed = FieldScalar::from(25u64);
        let root_hash = empty_roster_root_hash();
        let root_limbs = roster_root_limbs(&root_hash);
        let circuit =
            KaigiRosterJoinCircuit::new(account, domain_salt, nullifier_seed, root_limbs);
        let commitment = compute_commitment(account, domain_salt);
        let nullifier = compute_nullifier(account, nullifier_seed);
        let mut public_inputs = vec![vec![commitment], vec![nullifier]];
        public_inputs.extend(root_limbs.map(|limb| vec![limb]));

        MockProver::run(KAIGI_ROSTER_CIRCUIT_K, &circuit, public_inputs.clone())
            .expect("valid roster circuit")
            .assert_satisfied();

        public_inputs[2][0] += FieldScalar::ONE;
        let mismatched =
            MockProver::run(KAIGI_ROSTER_CIRCUIT_K, &circuit, public_inputs)
                .expect("mismatched public root still constructs");
        assert!(
            mismatched.verify().is_err(),
            "the proof statement must bind the advertised roster root"
        );
    }

    #[test]
    fn roster_root_limbs_match_hash_bytes() {
        let root = empty_roster_root_hash();
        let bytes = root.as_ref();
        let limb_values = roster_root_limb_values(&root);
        let limb_scalars = roster_root_limbs(&root);
        for (idx, limb) in limb_values.iter().enumerate() {
            let start = idx * 8;
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(&bytes[start..start + 8]);
            assert_eq!(u64::from_le_bytes(chunk), *limb);
        }

        for (scalar, limb) in limb_scalars.iter().zip(limb_values.iter()) {
            let repr = scalar.to_repr();
            let (lo, hi) = repr.as_ref().split_at(8);
            let mut chunk = [0u8; 8];
            chunk.copy_from_slice(lo);
            assert_eq!(u64::from_le_bytes(chunk), *limb);
            assert!(hi.iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn usage_circuit_keygen_succeeds() {
        let params: ParamsIPA<Curve> = ParamsIPA::new(KAIGI_USAGE_CIRCUIT_K);
        let duration = FieldScalar::from(1_200u64);
        let billed = FieldScalar::from(345u64);
        let segment = FieldScalar::from(2u64);
        let circuit = KaigiUsageCommitmentCircuit::new(duration, billed, segment);

        let vk = keygen_vk(&params, &circuit).expect("vk");
        let _pk = keygen_pk(&params, vk, &circuit).expect("pk");
    }

    #[test]
    fn usage_circuit_enforces_the_domain_separated_poseidon_commitment() {
        let duration = FieldScalar::from(1_200u64);
        let billed = FieldScalar::from(345u64);
        let segment = FieldScalar::from(2u64);
        let circuit = KaigiUsageCommitmentCircuit::new(duration, billed, segment);
        let commitment = compute_usage_commitment(duration, billed, segment);

        MockProver::run(
            KAIGI_USAGE_CIRCUIT_K,
            &circuit,
            vec![vec![commitment]],
        )
        .expect("valid usage circuit")
        .assert_satisfied();

        let mismatched = MockProver::run(
            KAIGI_USAGE_CIRCUIT_K,
            &circuit,
            vec![vec![commitment + FieldScalar::ONE]],
        )
        .expect("mismatched public commitment still constructs");
        assert!(mismatched.verify().is_err());
    }
}
