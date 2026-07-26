#![deny(missing_docs)]

//! Halo2 circuits and helpers for Kaigi privacy proofs.
//!
//! The current milestone ships the roster join circuit, which checks that the
//! public commitment and nullifier correspond to the prover-supplied account,
//! domain salt, and nullifier seed. Future work will extend the circuit with
//! Merkle binding and additional flows (leave, usage), but the helpers exposed
//! here already provide deterministic compressors shared between host code and
//! the circuit.

use core::array;

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{ff::PrimeField, pasta::Fp},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Expression, Instance, Selector},
    poly::Rotation,
};
use iroha_crypto::{Hash, HashOf, MerkleTree};

/// Scalar field used by the Kaigi Halo2 circuits (Pasta Fp).
pub type Scalar = Fp;

/// Backend identifier used by the roster join circuit verifier metadata.
pub const KAIGI_ROSTER_BACKEND: &str = "halo2/pasta/kaigi-roster-v1";

/// Default log2 domain size used when instantiating the roster join circuit.
///
/// `k = 6` gives a 64-row domain, which is ample for the two-row layout used by
/// the initial circuit while keeping parameter generation inexpensive.
pub const KAIGI_ROSTER_CIRCUIT_K: u32 = 6;

/// Number of little-endian 64-bit limbs used to expose the roster root.
pub const KAIGI_ROSTER_ROOT_LIMBS: usize = 4;

/// Backend identifier used by the usage commitment circuit verifier metadata.
pub const KAIGI_USAGE_BACKEND: &str = "halo2/pasta/kaigi-usage-v1";

/// Default log2 domain size for the usage commitment circuit.
pub const KAIGI_USAGE_CIRCUIT_K: u32 = 6;

/// Configuration for the roster join Halo2 circuit.
#[derive(Clone, Debug)]
pub struct KaigiRosterConfig {
    account: Column<Advice>,
    parameter: Column<Advice>,
    output: Column<Advice>,
    selector: Selector,
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

impl Circuit<Scalar> for KaigiRosterJoinCircuit {
    type Config = KaigiRosterConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let account = meta.advice_column();
        let parameter = meta.advice_column();
        let output = meta.advice_column();

        meta.enable_equality(account);
        meta.enable_equality(output);

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

        let selector = meta.selector();
        meta.create_gate("kaigi_roster_compress", |meta| {
            let q = meta.query_selector(selector);
            let left = meta.query_advice(account, Rotation::cur());
            let right = meta.query_advice(parameter, Rotation::cur());
            let out = meta.query_advice(output, Rotation::cur());

            let expected = compress_expression(left, right);
            vec![q * (out - expected)]
        });

        KaigiRosterConfig {
            account,
            parameter,
            output,
            selector,
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

        let commitment_value = compress_value(account_value, domain_value);
        let nullifier_value = compress_value(account_value, nullifier_seed_value);

        let (commitment_cell, nullifier_cell) = layouter.assign_region(
            || "kaigi roster rows",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                let account_cell = region.assign_advice(config.account, 0, account_value);
                region.assign_advice(config.parameter, 0, domain_value);
                let commitment_cell = region.assign_advice(config.output, 0, commitment_value);

                config.selector.enable(&mut region, 1)?;
                let account_again = region.assign_advice(config.account, 1, account_value);
                region.constrain_equal(account_cell.cell(), account_again.cell());
                region.assign_advice(config.parameter, 1, nullifier_seed_value);
                let nullifier_cell = region.assign_advice(config.output, 1, nullifier_value);

                Ok((commitment_cell, nullifier_cell))
            },
        )?;

        layouter.constrain_instance(commitment_cell.cell(), config.instance_commitment, 0);
        layouter.constrain_instance(nullifier_cell.cell(), config.instance_nullifier, 0);

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

/// Compute the roster commitment as a Pasta field element.
#[must_use]
pub fn compute_commitment(account: Scalar, domain_salt: Scalar) -> Scalar {
    compress_scalar(account, domain_salt)
}

/// Compute the roster nullifier as a Pasta field element.
#[must_use]
pub fn compute_nullifier(account: Scalar, nullifier_seed: Scalar) -> Scalar {
    compress_scalar(account, nullifier_seed)
}

/// Compute the usage commitment (duration, gas, segment) as a Pasta field element.
#[must_use]
pub fn compute_usage_commitment(
    duration_ms: Scalar,
    billed_gas: Scalar,
    segment_index: Scalar,
) -> Scalar {
    let stage = compress_scalar(duration_ms, billed_gas);
    compress_scalar(stage, segment_index)
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

fn compress_value(left: Value<Scalar>, right: Value<Scalar>) -> Value<Scalar> {
    left.zip(right).map(|(l, r)| compress_scalar(l, r))
}

fn compress_scalar(left: Scalar, right: Scalar) -> Scalar {
    let a = left + Scalar::from(7u64);
    let b = right + Scalar::from(13u64);

    let a2 = a * a;
    let a4 = a2 * a2;
    let a5 = a4 * a;

    let b2 = b * b;
    let b4 = b2 * b2;
    let b5 = b4 * b;

    Scalar::from(2u64) * a5 + Scalar::from(3u64) * b5
}

fn compress_expression(left: Expression<Scalar>, right: Expression<Scalar>) -> Expression<Scalar> {
    let seven = Expression::Constant(Scalar::from(7u64));
    let thirteen = Expression::Constant(Scalar::from(13u64));
    let two = Expression::Constant(Scalar::from(2u64));
    let three = Expression::Constant(Scalar::from(3u64));

    let a = left + seven;
    let b = right + thirteen;

    let a2 = a.clone() * a.clone();
    let a4 = a2.clone() * a2.clone();
    let a5 = a4 * a.clone();

    let b2 = b.clone() * b.clone();
    let b4 = b2.clone() * b2.clone();
    let b5 = b4 * b;

    two * a5 + three * b5
}

fn scalar_to_bytes(value: Scalar) -> [u8; Hash::LENGTH] {
    let mut out = [0u8; Hash::LENGTH];
    out.copy_from_slice(value.to_repr().as_ref());
    out
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
    lhs: Column<Advice>,
    rhs: Column<Advice>,
    output: Column<Advice>,
    selector: Selector,
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
        let lhs = meta.advice_column();
        let rhs = meta.advice_column();
        let output = meta.advice_column();

        meta.enable_equality(lhs);
        meta.enable_equality(output);

        let instance_commitment = meta.instance_column();
        meta.enable_equality(instance_commitment);

        let selector = meta.selector();
        meta.create_gate("kaigi_usage_compress", |meta| {
            let q = meta.query_selector(selector);
            let left = meta.query_advice(lhs, Rotation::cur());
            let right = meta.query_advice(rhs, Rotation::cur());
            let out = meta.query_advice(output, Rotation::cur());

            let expected = compress_expression(left, right);
            vec![q * (out - expected)]
        });

        KaigiUsageConfig {
            lhs,
            rhs,
            output,
            selector,
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

        let stage_value = compress_value(duration_value, billed_value);
        let commitment_value = compress_value(stage_value, segment_value);

        let commitment_cell = layouter.assign_region(
            || "kaigi usage rows",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                region.assign_advice(config.lhs, 0, duration_value);
                region.assign_advice(config.rhs, 0, billed_value);
                let stage_cell = region.assign_advice(config.output, 0, stage_value);

                config.selector.enable(&mut region, 1)?;
                let stage_again = region.assign_advice(config.lhs, 1, stage_value);
                region.constrain_equal(stage_cell.cell(), stage_again.cell());
                region.assign_advice(config.rhs, 1, segment_value);
                let commitment_cell = region.assign_advice(config.output, 1, commitment_value);

                Ok(commitment_cell)
            },
        )?;

        layouter.constrain_instance(commitment_cell.cell(), config.instance_commitment, 0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
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
}
