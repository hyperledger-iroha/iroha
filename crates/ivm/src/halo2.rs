//! Halo2 circuits exercised by the zero-knowledge mode.
//!
//! Each helper described in this module wires the corresponding VM primitive
//! into an actual [`halo2_proofs`] constraint system.  Historically the
//! zero-knowledge flow reused the same plain-Rust routines that the runtime
//! used for fast validation which meant no circuit ever got synthesised.  The
//! implementation below keeps the ergonomics of the previous API but routes all
//! checks through tiny Halo2 gadgets so the proving backend observes real
//! constraints during tests and benchmarks.

use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    halo2curves::pasta::Fp as HaloScalar,
    plonk::{Circuit, Column, ConstraintSystem, Error as PlonkError, Selector},
    poly::Rotation,
};

/// Scalar field element type used for secret scalars in the circuits.
pub type Scalar = u64;
/// Base field element type used for commitments and hashes in tests.
pub type Field = u64;

/// Representation of an elliptic curve point in affine coordinates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ECPoint {
    pub x: Field,
    pub y: Field,
}

/// Poseidon hash helper used across circuits.
fn poseidon_hash(inputs: &[Field]) -> Field {
    if inputs.is_empty() {
        return 0;
    }
    if inputs.len() == 1 {
        return crate::poseidon::poseidon2(inputs[0], 0);
    }
    if inputs.len() == 2 {
        return crate::poseidon::poseidon2(inputs[0], inputs[1]);
    }

    let mut idx = 0usize;
    let mut acc = 0u64;
    let mut first_round = true;
    while idx < inputs.len() {
        if first_round {
            let take = (inputs.len() - idx).min(6);
            let mut state = [0u64; 6];
            state[..take].copy_from_slice(&inputs[idx..idx + take]);
            if take < 6 {
                state[take] = inputs.len() as u64;
            }
            acc = crate::poseidon::poseidon6(state);
            idx += take;
            first_round = false;
        } else {
            let take = (inputs.len() - idx).min(5);
            let mut state = [0u64; 6];
            state[0] = acc;
            if take > 0 {
                state[1..1 + take].copy_from_slice(&inputs[idx..idx + take]);
            }
            if take < 5 {
                state[take + 1] = (inputs.len() - idx) as u64;
            }
            acc = crate::poseidon::poseidon6(state);
            idx += take;
        }
    }
    acc
}

/// Pedersen commitment helper reused by the mock circuits.
fn pedersen_commit(value: Field, blind: Scalar) -> Field {
    crate::pedersen::pedersen_commit_truncated(value, blind)
}

const HALO2_EQ_K: u32 = 9;

fn halo_from_u64(value: u64) -> HaloScalar {
    HaloScalar::from(value)
}

fn halo_from_bool(flag: bool) -> HaloScalar {
    HaloScalar::from(flag as u64)
}

#[derive(Clone, Default)]
struct EqualityBatchCircuit {
    pairs: Vec<(HaloScalar, HaloScalar)>,
}

macro_rules! impl_equality_batch_circuit {
    () => {
        impl_equality_batch_circuit!(@inner type Params = (););
    };
    (@inner $($extra:tt)*) => {
        impl Circuit<HaloScalar> for EqualityBatchCircuit {
            type Config = (
                Column<halo2_proofs::plonk::Advice>,
                Column<halo2_proofs::plonk::Advice>,
                Selector,
            );
            $($extra)*
            type FloorPlanner = SimpleFloorPlanner;

            fn without_witnesses(&self) -> Self {
                Self::default()
            }

            fn configure(meta: &mut ConstraintSystem<HaloScalar>) -> Self::Config {
                let lhs = meta.advice_column();
                let rhs = meta.advice_column();
                meta.enable_equality(lhs);
                meta.enable_equality(rhs);
                let s_eq = meta.selector();
                meta.create_gate("eq", |meta| {
                    let s = meta.query_selector(s_eq);
                    let l = meta.query_advice(lhs, Rotation::cur());
                    let r = meta.query_advice(rhs, Rotation::cur());
                    vec![s * (l - r)]
                });
                (lhs, rhs, s_eq)
            }

            fn synthesize(
                &self,
                (lhs, rhs, sel): Self::Config,
                mut layouter: impl Layouter<HaloScalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "eq region",
                    |mut region| {
                        for (offset, (l, r)) in self.pairs.iter().enumerate() {
                            sel.enable(&mut region, offset)?;
                            region.assign_advice(lhs, offset, Value::known(*l));
                            region.assign_advice(rhs, offset, Value::known(*r));
                        }
                        Ok(())
                    },
                )
            }
        }
    };
    ($($extra:tt)*) => {
        impl_equality_batch_circuit!(@inner $($extra)*);
    };
}

impl_equality_batch_circuit!();

fn enforce_pairs(pairs: Vec<(HaloScalar, HaloScalar)>) -> Result<(), &'static str> {
    if pairs.is_empty() {
        return Ok(());
    }
    let circuit = EqualityBatchCircuit { pairs };
    let prover =
        MockProver::run(HALO2_EQ_K, &circuit, vec![]).map_err(|_| "halo2 synthesis failure")?;
    prover.verify().map_err(|_| "halo2 constraint failure")
}

fn ensure_equal_u64(lhs: u64, rhs: u64) -> Result<(), &'static str> {
    enforce_pairs(vec![(halo_from_u64(lhs), halo_from_u64(rhs))])
}

fn ensure_equal_bool(lhs: bool, rhs: bool) -> Result<(), &'static str> {
    enforce_pairs(vec![(halo_from_bool(lhs), halo_from_bool(rhs))])
}

fn ensure_bytes_equal(lhs: &[u8], rhs: &[u8]) -> Result<(), &'static str> {
    if lhs.len() != rhs.len() {
        return Err("byte slice length mismatch");
    }
    let pairs = lhs
        .iter()
        .zip(rhs.iter())
        .map(|(&l, &r)| (halo_from_u64(l as u64), halo_from_u64(r as u64)))
        .collect();
    enforce_pairs(pairs)
}

fn ensure_slice_equal_u64(lhs: &[u64], rhs: &[u64]) -> Result<(), &'static str> {
    if lhs.len() != rhs.len() {
        return Err("u64 slice length mismatch");
    }
    let pairs = lhs
        .iter()
        .zip(rhs.iter())
        .map(|(&l, &r)| (halo_from_u64(l), halo_from_u64(r)))
        .collect();
    enforce_pairs(pairs)
}

fn ensure_slice_equal_bool(lhs: &[bool], rhs: &[bool]) -> Result<(), &'static str> {
    if lhs.len() != rhs.len() {
        return Err("bool slice length mismatch");
    }
    let pairs = lhs
        .iter()
        .zip(rhs.iter())
        .map(|(&l, &r)| (halo_from_bool(l), halo_from_bool(r)))
        .collect();
    enforce_pairs(pairs)
}

// -----------------------------------------------------------------------------
// Mint Circuit
// -----------------------------------------------------------------------------

/// Private witness inputs for the Mint circuit.
#[derive(Clone, Debug, Default)]
pub struct MintWitness {
    pub recipient: ECPoint,
    pub value: u64,
    pub token_id: Field,
    pub serial: Field,
    pub hook_id: Field,
    pub hook_data: Field,
    pub value_blind: Scalar,
    pub token_blind: Field,
}

/// Public outputs of the Mint circuit.
#[derive(Clone, Debug, Default)]
pub struct MintPublic {
    pub coin_commitment: Field,
    pub value_commitment: ECPoint,
    pub token_commitment: Field,
}

/// Placeholder Mint circuit object.
#[derive(Clone, Debug, Default)]
pub struct MintCircuit {
    pub witness: MintWitness,
    pub public: MintPublic,
}

impl MintCircuit {
    /// Create a new Mint circuit instance.
    pub fn new(witness: MintWitness, public: MintPublic) -> Self {
        Self { witness, public }
    }

    /// Verify the circuit by recomputing commitments from the witness
    /// values and comparing them to the provided public outputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        let coin = poseidon_hash(&[
            self.witness.recipient.x,
            self.witness.recipient.y,
            self.witness.value,
            self.witness.token_id,
            self.witness.serial,
            self.witness.hook_id,
            self.witness.hook_data,
        ]);
        let val_commit = pedersen_commit(self.witness.value, self.witness.value_blind);
        let tok_commit = poseidon_hash(&[self.witness.token_id, self.witness.token_blind]);
        ensure_equal_u64(self.public.coin_commitment, coin)?;
        ensure_equal_u64(self.public.value_commitment.x, val_commit)?;
        ensure_equal_u64(self.public.value_commitment.y, 0)?;
        ensure_equal_u64(self.public.token_commitment, tok_commit)
    }
}

// -----------------------------------------------------------------------------
// Burn Circuit
// -----------------------------------------------------------------------------

/// Private witness inputs for the Burn circuit.
#[derive(Clone, Debug, Default)]
pub struct BurnWitness {
    pub secret_key: Scalar,
    pub value: u64,
    pub token_id: Field,
    pub value_blind: Scalar,
    pub token_blind: Field,
    pub serial: Field,
    pub hook_id: Field,
    pub hook_data: Field,
    pub hook_blind: Field,
    pub leaf_index: u32,
    pub merkle_path: [Field; 32],
    pub sig_secret: Scalar,
}

/// Public outputs of the Burn circuit.
#[derive(Clone, Debug, Default)]
pub struct BurnPublic {
    pub nullifier: Field,
    pub value_commitment: ECPoint,
    pub token_commitment: Field,
    pub merkle_root: Field,
    pub hook_commitment: Field,
    pub hook_id: Field,
    pub sig_public: ECPoint,
}

/// Placeholder Burn circuit object.
#[derive(Clone, Debug, Default)]
pub struct BurnCircuit {
    pub witness: BurnWitness,
    pub public: BurnPublic,
}

impl BurnCircuit {
    /// Create a new Burn circuit instance.
    pub fn new(witness: BurnWitness, public: BurnPublic) -> Self {
        Self { witness, public }
    }

    /// Verify the circuit by recomputing all public outputs from the witness
    /// data.  This mirrors the behaviour of the real Halo2 circuit which would
    /// enforce these relationships via constraints.
    pub fn verify(&self) -> Result<(), &'static str> {
        let n = compute_nullifier(self.witness.secret_key, self.witness.serial);
        let val_commit = pedersen_commit(self.witness.value, self.witness.value_blind);
        let tok_commit = poseidon_hash(&[self.witness.token_id, self.witness.token_blind]);
        let pk = derive_public_key(self.witness.secret_key);
        let coin = poseidon_hash(&[
            pk.x,
            pk.y,
            self.witness.value,
            self.witness.token_id,
            self.witness.serial,
            self.witness.hook_id,
            self.witness.hook_data,
        ]);
        let leaf = if self.witness.value == 0 { 0 } else { coin };
        let root = verify_merkle_path(leaf, self.witness.leaf_index, &self.witness.merkle_path);
        let hook_commit = poseidon_hash(&[self.witness.hook_data, self.witness.hook_blind]);
        let sig_pk = derive_public_key(self.witness.sig_secret);
        ensure_equal_u64(self.public.nullifier, n)?;
        ensure_equal_u64(self.public.value_commitment.x, val_commit)?;
        ensure_equal_u64(self.public.value_commitment.y, 0)?;
        ensure_equal_u64(self.public.token_commitment, tok_commit)?;
        ensure_equal_u64(self.public.hook_commitment, hook_commit)?;
        ensure_equal_u64(self.public.hook_id, self.witness.hook_id)?;
        ensure_equal_u64(self.public.merkle_root, root)?;
        ensure_equal_u64(self.public.sig_public.x, sig_pk.x)?;
        ensure_equal_u64(self.public.sig_public.y, sig_pk.y)
    }
}

// -----------------------------------------------------------------------------
// Merkle Inclusion Circuit
// -----------------------------------------------------------------------------

/// Private witness inputs for the Merkle inclusion circuit.
#[derive(Clone, Debug, Default)]
pub struct MerkleWitness {
    pub leaf: Field,
    pub index: u32,
    pub path: [Field; 32],
}

/// Public output for the Merkle inclusion circuit.
#[derive(Clone, Debug, Default)]
pub struct MerklePublic {
    pub root: Field,
}

/// Placeholder Merkle inclusion circuit object.
#[derive(Clone, Debug, Default)]
pub struct MerkleCircuit {
    pub witness: MerkleWitness,
    pub public: MerklePublic,
}

impl MerkleCircuit {
    /// Create a new Merkle inclusion circuit object.
    pub fn new(witness: MerkleWitness, public: MerklePublic) -> Self {
        Self { witness, public }
    }

    /// Verify the circuit by recomputing the Merkle root from the witness
    /// values and comparing it to the provided public root.
    pub fn verify(&self) -> Result<(), &'static str> {
        #[cfg(feature = "ivm_halo2_real")]
        fn verify_path(leaf: Field, index: u32, path: &[Field; 32]) -> Field {
            // Compute using iroha_zkp_halo2 prime field to exercise real backend types.
            let mut cur = iroha_zkp_halo2::PrimeField64::from(leaf);
            for (i, sib) in path.iter().enumerate() {
                let bit = (index >> i) & 1;
                let s = iroha_zkp_halo2::PrimeField64::from(*sib);
                // Preserve left/right ordering
                if bit == 0 {
                    // Use the unified pairwise hash over u64 by converting back and forth.
                    let v = super_hash(cur.to_u64(), s.to_u64());
                    cur = iroha_zkp_halo2::PrimeField64::from(v);
                } else {
                    let v = super_hash(s.to_u64(), cur.to_u64());
                    cur = iroha_zkp_halo2::PrimeField64::from(v);
                }
            }
            cur.to_u64()
        }

        #[cfg(not(feature = "ivm_halo2_real"))]
        fn verify_path(leaf: Field, index: u32, path: &[Field; 32]) -> Field {
            verify_merkle_path(leaf, index, path)
        }

        let root = verify_path(self.witness.leaf, self.witness.index, &self.witness.path);
        ensure_equal_u64(self.public.root, root)
    }
}

// -----------------------------------------------------------------------------
// Merkle Inclusion Gadget
// -----------------------------------------------------------------------------

/// Verify that a leaf is included in a Merkle tree with the given root.
/// This is a simplified Poseidon-based Merkle gadget used by the tests.
pub fn verify_merkle_path(leaf: Field, index: u32, path: &[Field; 32]) -> Field {
    let mut current = leaf;
    for (i, sibling) in path.iter().enumerate() {
        let bit = (index >> i) & 1;
        current = if bit == 0 {
            super_hash(current, *sibling)
        } else {
            super_hash(*sibling, current)
        };
    }
    current
}

/// Verify a Merkle path using an index for a limited `depth`.
/// Mirrors `verify_merkle_path` but only consumes the first `depth` siblings.
pub fn verify_merkle_path_depth(
    leaf: Field,
    index: u32,
    path: &[Field; 32],
    depth: usize,
) -> Field {
    let mut current = leaf;
    for (i, sibling) in path.iter().copied().take(depth.min(32)).enumerate() {
        let bit = (index >> i) & 1;
        current = if bit == 0 {
            poseidon_hash(&[current, sibling])
        } else {
            poseidon_hash(&[sibling, current])
        };
    }
    current
}

/// Verify a Merkle path using an explicit direction bitmask and a fixed
/// maximum depth. The `dirs` bitmask encodes the direction at each level:
/// - bit i = 0 => the running accumulator is the left child at level i
/// - bit i = 1 => the running accumulator is the right child at level i
///   Only the lowest `depth` bits of `dirs` are used; the remaining entries of
///   `path` beyond `depth` are ignored.
pub fn verify_merkle_path_with_dirs(
    leaf: Field,
    dirs: u32,
    path: &[Field; 32],
    depth: usize,
) -> Field {
    #[cfg(feature = "ivm_halo2_real")]
    {
        use iroha_zkp_halo2::PrimeField64 as F;
        let mut current = F::from(leaf);
        for (i, sibling) in path.iter().copied().take(depth.min(32)).enumerate() {
            let sibling = F::from(sibling);
            let bit = (dirs >> i) & 1;
            let out = if bit == 0 {
                super_hash(current.to_u64(), sibling.to_u64())
            } else {
                super_hash(sibling.to_u64(), current.to_u64())
            };
            current = F::from(out);
        }
        current.to_u64()
    }
    #[cfg(not(feature = "ivm_halo2_real"))]
    {
        let mut current = leaf;
        for (i, sibling) in path.iter().copied().take(depth.min(32)).enumerate() {
            let bit = (dirs >> i) & 1;
            current = if bit == 0 {
                super_hash(current, sibling)
            } else {
                super_hash(sibling, current)
            };
        }
        current
    }
}

#[inline]
fn super_hash(a: Field, b: Field) -> Field {
    // Use the canonical Poseidon2 permutation over BN254 for pairwise
    // compression in Merkle gadgets so the VM mirrors the Halo2 circuit.
    iroha_zkp_halo2::poseidon::hash2_u64(a, b)
}

// -----------------------------------------------------------------------------
// Nullifier Gadget
// -----------------------------------------------------------------------------

/// Compute the nullifier from a secret key and serial number.
pub fn compute_nullifier(secret: Scalar, serial: Field) -> Field {
    // Use the Poseidon2 hash to derive a unique nullifier. In the real
    // circuits this would be enforced by a Halo2 gadget. Here we delegate to
    // the `poseidon2` helper which provides a small stand-in for the
    // two-input Poseidon permutation.
    crate::poseidon::poseidon2(secret, serial)
}

// -----------------------------------------------------------------------------
// Signature Key Gadget
// -----------------------------------------------------------------------------

/// Derive an EC point from a signing secret.
pub fn derive_public_key(secret: Scalar) -> ECPoint {
    ECPoint {
        x: crate::field::mul(secret, 2),
        y: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encoding::wide as wide_enc,
        instruction::wide,
        metadata::ProgramMetadata,
        zk::{Constraint, RegisterState},
    };

    #[test]
    fn super_hash_aligns_with_poseidon_permutation() {
        let samples = [
            (0_u64, 0_u64),
            (1, 2),
            (42, u64::MAX),
            (0xDEAD_BEEF, 0xFEED_FACE),
            (0x0123_4567_89AB_CDEF, 0x0F1E_2D3C_4B5A_6978),
        ];
        for (a, b) in samples {
            let via_super = super_hash(a, b);
            let via_poseidon = crate::poseidon::poseidon2(a, b);
            assert_eq!(
                via_super, via_poseidon,
                "super_hash must mirror the Poseidon implementation for ({a:#x}, {b:#x})"
            );
            let via_canonical = iroha_zkp_halo2::poseidon::hash2_u64(a, b);
            assert_eq!(
                via_super, via_canonical,
                "super_hash must align with canonical Halo2 Poseidon gadget for ({a:#x}, {b:#x})"
            );
        }
    }

    fn build_program(words: &[u32]) -> Vec<u8> {
        let mut bytes = ProgramMetadata::default_for(1, 0, 1).encode();
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    fn base_state(pc: u64) -> RegisterState {
        RegisterState {
            pc,
            gpr: [0u64; 256],
            tags: [false; 256],
        }
    }

    #[test]
    fn vm_execution_circuit_accepts_wrapping_addi_and_sub_semantics() {
        let program = build_program(&[
            wide_enc::encode_rr(wide::arithmetic::ADD, 3, 1, 2),
            wide_enc::encode_ri(wide::arithmetic::ADDI, 4, 3, -1),
            wide_enc::encode_rr(wide::arithmetic::SUB, 5, 4, 1),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = u64::MAX;
        s0.gpr[2] = 2;

        let mut s1 = s0.clone();
        s1.pc = 4;
        s1.gpr[3] = s0.gpr[1].wrapping_add(s0.gpr[2]);

        let mut s2 = s1.clone();
        s2.pc = 8;
        s2.gpr[4] = s1.gpr[3].wrapping_add((-1_i8) as i64 as u64);

        let mut s3 = s2.clone();
        s3.pc = 12;
        s3.gpr[5] = s2.gpr[4].wrapping_sub(s2.gpr[1]);

        let mut s4 = s3.clone();
        s4.pc = 16;

        let trace = vec![s0, s1, s2, s3, s4];
        let constraints = vec![
            Constraint::Eq {
                reg1: 4,
                reg2: 0,
                cycle: 2,
            },
            Constraint::Range {
                reg: 5,
                bits: 1,
                cycle: 3,
            },
        ];

        let circuit = VMExecutionCircuit::new(&program, &trace, &constraints);
        assert!(circuit.verify().is_ok());
    }

    #[test]
    fn vm_execution_circuit_accepts_taken_beq_trace() {
        let program = build_program(&[
            wide_enc::encode_branch(wide::control::BEQ, 1, 2, 2),
            wide_enc::encode_ri(wide::arithmetic::ADDI, 3, 0, 7),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = 9;
        s0.gpr[2] = 9;

        let mut s1 = s0.clone();
        s1.pc = 8;

        let mut s2 = s1.clone();
        s2.pc = 12;

        let trace = vec![s0, s1, s2];
        let circuit = VMExecutionCircuit::new(&program, &trace, &[]);
        assert!(circuit.verify().is_ok());
    }

    #[test]
    fn vm_execution_circuit_accepts_not_taken_bne_trace() {
        let program = build_program(&[
            wide_enc::encode_branch(wide::control::BNE, 1, 2, 2),
            wide_enc::encode_ri(wide::arithmetic::ADDI, 3, 0, 7),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = 9;
        s0.gpr[2] = 9;

        let mut s1 = s0.clone();
        s1.pc = 4;

        let mut s2 = s1.clone();
        s2.pc = 8;
        s2.gpr[3] = 7;

        let mut s3 = s2.clone();
        s3.pc = 12;

        let trace = vec![s0, s1, s2, s3];
        let circuit = VMExecutionCircuit::new(&program, &trace, &[]);
        assert!(circuit.verify().is_ok());
    }

    #[test]
    fn vm_execution_circuit_rejects_unsupported_opcode() {
        let program = build_program(&[
            wide_enc::encode_rr(wide::arithmetic::AND, 3, 1, 2),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = 0xF0;
        s0.gpr[2] = 0xAA;
        let mut s1 = s0.clone();
        s1.pc = 4;
        s1.gpr[3] = s0.gpr[1] & s0.gpr[2];

        let trace = vec![s0, s1];
        let circuit = VMExecutionCircuit::new(&program, &trace, &[]);
        assert_eq!(circuit.verify(), Err("unsupported opcode"));
    }

    #[test]
    fn vm_execution_circuit_rejects_unexpected_register_drift() {
        let program = build_program(&[
            wide_enc::encode_rr(wide::arithmetic::ADD, 3, 1, 2),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = 5;
        s0.gpr[2] = 7;

        let mut s1 = s0.clone();
        s1.pc = 4;
        s1.gpr[3] = 12;
        s1.gpr[8] = 99;

        let mut s2 = s1.clone();
        s2.pc = 8;

        let trace = vec![s0, s1, s2];
        let circuit = VMExecutionCircuit::new(&program, &trace, &[]);
        assert_eq!(circuit.verify(), Err("halo2 constraint failure"));
    }

    #[test]
    fn vm_execution_circuit_rejects_constraint_mismatch() {
        let program = build_program(&[
            wide_enc::encode_rr(wide::arithmetic::ADD, 3, 1, 2),
            wide_enc::encode_halt(),
        ]);

        let mut s0 = base_state(0);
        s0.gpr[1] = 5;
        s0.gpr[2] = 7;

        let mut s1 = s0.clone();
        s1.pc = 4;
        s1.gpr[3] = 12;

        let mut s2 = s1.clone();
        s2.pc = 8;

        let trace = vec![s0, s1, s2];
        let constraints = vec![Constraint::Zero { reg: 3, cycle: 1 }];
        let circuit = VMExecutionCircuit::new(&program, &trace, &constraints);
        assert_eq!(circuit.verify(), Err("constraint failure"));
    }
}

// -----------------------------------------------------------------------------
// Arithmetic and Bitwise Operation Circuit
// -----------------------------------------------------------------------------

/// Arithmetic/logic operation specifier.
#[derive(Clone, Copy)]
pub enum ALUOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Not,
    Sll,
    Srl,
    Sra,
    Rol,
    Ror,
}

/// Simple circuit object verifying a single arithmetic/logic operation.
pub struct ALUCircuit {
    pub op: ALUOp,
    pub a: u64,
    pub b: u64,
    pub result: u64,
}

impl ALUCircuit {
    /// Verify the operation by recomputing the expected result from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        let expected = match self.op {
            ALUOp::Add => self.a.wrapping_add(self.b),
            ALUOp::Sub => self.a.wrapping_sub(self.b),
            ALUOp::And => self.a & self.b,
            ALUOp::Or => self.a | self.b,
            ALUOp::Xor => self.a ^ self.b,
            ALUOp::Not => !self.a,
            ALUOp::Sll => self.a << (self.b & 0x3F),
            ALUOp::Srl => self.a >> (self.b & 0x3F),
            ALUOp::Sra => ((self.a as i64) >> (self.b & 0x3F)) as u64,
            ALUOp::Rol => self.a.rotate_left((self.b & 0x3F) as u32),
            ALUOp::Ror => self.a.rotate_right((self.b & 0x3F) as u32),
        };
        ensure_equal_u64(expected, self.result)
    }
}

/// Circuit verifying a 32-bit addition with carry.
pub struct AddCarryCircuit {
    pub a: u32,
    pub b: u32,
    pub carry_in: u8,
    pub sum: u32,
    pub carry_out: u8,
}

impl AddCarryCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.carry_in > 1 || self.carry_out > 1 {
            return Err("carry bit out of range");
        }
        let sum = self.a as u64 + self.b as u64 + self.carry_in as u64;
        let res32 = sum as u32;
        let c = ((sum >> 32) & 1) as u8;
        ensure_equal_u64(res32 as u64, self.sum as u64)?;
        ensure_equal_u64(c as u64, self.carry_out as u64)
    }
}

// -----------------------------------------------------------------------------
// Field Arithmetic Circuit
// -----------------------------------------------------------------------------

/// Field arithmetic operation specifier.
#[derive(Clone, Copy)]
pub enum FieldOp {
    Add,
    Sub,
    Mul,
    Inv,
}

/// Simple circuit object verifying a single field arithmetic operation.
pub struct FieldCircuit {
    pub op: FieldOp,
    pub a: u64,
    pub b: u64,
    pub result: u64,
}

impl FieldCircuit {
    /// Verify the operation by recomputing the expected result from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        #[cfg(feature = "ivm_halo2_real")]
        fn do_op(op: FieldOp, a: u64, b: u64) -> Result<u64, &'static str> {
            use iroha_zkp_halo2::PrimeField64 as F;
            let aa = F::from(a);
            let bb = F::from(b);
            let out = match op {
                FieldOp::Add => aa + bb,
                FieldOp::Sub => aa - bb,
                FieldOp::Mul => aa * bb,
                FieldOp::Inv => {
                    if a == 0 {
                        return Err("field inverse undefined");
                    }
                    aa.inverse()
                }
            };
            Ok(out.to_u64())
        }

        #[cfg(not(feature = "ivm_halo2_real"))]
        fn do_op(op: FieldOp, a: u64, b: u64) -> Result<u64, &'static str> {
            let v = match op {
                FieldOp::Add => crate::field::add(a, b),
                FieldOp::Sub => crate::field::sub(a, b),
                FieldOp::Mul => crate::field::mul(a, b),
                FieldOp::Inv => match crate::field::inv(a) {
                    Some(v) => v,
                    None => return Err("field inverse undefined"),
                },
            };
            Ok(v)
        }

        let expected = do_op(self.op, self.a, self.b)?;
        ensure_equal_u64(expected, self.result)
    }
}

// -----------------------------------------------------------------------------
// Comparison and Conditional Move Circuit
// -----------------------------------------------------------------------------

/// Comparison/conditional operation specifier.
#[derive(Clone, Copy)]
pub enum CmpOp {
    Slt,
    Sltu,
    Seq,
    Sne,
    Cmov,
}

/// Simple circuit object verifying comparison and conditional move operations.
pub struct CmpCircuit {
    pub op: CmpOp,
    pub a: u64,
    pub b: u64,
    /// Previous value of the destination register (used by CMOV).
    pub prev: u64,
    pub result: u64,
}

impl CmpCircuit {
    /// Verify the operation by recomputing the expected result from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        let expected = match self.op {
            CmpOp::Slt => {
                if (self.a as i64) < (self.b as i64) {
                    1
                } else {
                    0
                }
            }
            CmpOp::Sltu => {
                if self.a < self.b {
                    1
                } else {
                    0
                }
            }
            CmpOp::Seq => {
                if self.a == self.b {
                    1
                } else {
                    0
                }
            }
            CmpOp::Sne => {
                if self.a != self.b {
                    1
                } else {
                    0
                }
            }
            CmpOp::Cmov => {
                if self.b != 0 {
                    self.a
                } else {
                    self.prev
                }
            }
        };
        ensure_equal_u64(expected, self.result)
    }
}

// -----------------------------------------------------------------------------
// Vector Operation Circuit
// -----------------------------------------------------------------------------

/// Vector arithmetic/logic operation specifier.
#[derive(Clone, Copy)]
pub enum VectorOp {
    Vadd32,
    Vadd64,
    Vand,
    Vxor,
    Vor,
    Vrot32,
}

/// Simple circuit object verifying a 128-bit vector operation.
pub struct VectorCircuit {
    pub op: VectorOp,
    pub a: [u32; 4],
    pub b: [u32; 4],
    /// Immediate or shift amount used by VROT32 (ignored otherwise).
    pub k: u32,
    pub result: [u32; 4],
}

impl VectorCircuit {
    /// Verify the operation by recomputing the expected result from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        use crate::vector;
        let expected = match self.op {
            VectorOp::Vadd32 => vector::vadd32(self.a, self.b),
            VectorOp::Vadd64 => vector::vadd64(self.a, self.b),
            VectorOp::Vand => vector::vand(self.a, self.b),
            VectorOp::Vxor => vector::vxor(self.a, self.b),
            VectorOp::Vor => vector::vor(self.a, self.b),
            VectorOp::Vrot32 => vector::vrot32(self.a, self.k & 31),
        };

        for (&exp, &out) in expected.iter().zip(self.result.iter()) {
            ensure_equal_u64(exp as u64, out as u64)?;
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// SHA-256 Compression Circuit
// -----------------------------------------------------------------------------

/// Circuit verifying the SHA256BLOCK vector instruction.
pub struct Sha256BlockCircuit {
    /// Input hash state (8 x 32-bit words).
    pub state: [u32; 8],
    /// Message block to compress (64 bytes).
    pub block: [u8; 64],
    /// Output state after compression.
    pub result: [u32; 8],
}

impl Sha256BlockCircuit {
    /// Verify the SHA-256 compression of one block.
    pub fn verify(&self) -> Result<(), &'static str> {
        let mut st = self.state;
        crate::vector::sha256_compress(&mut st, &self.block);
        for (&exp, &out) in st.iter().zip(self.result.iter()) {
            ensure_equal_u64(exp as u64, out as u64)?;
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// SHA-3 Compression Circuit
// -----------------------------------------------------------------------------

/// Circuit verifying the SHA3BLOCK instruction.
pub struct Sha3BlockCircuit {
    /// Input state (25 x 64-bit words).
    pub state: [u64; 25],
    /// Message block to absorb (136 bytes).
    pub block: [u8; 136],
    /// Output state after absorption and permutation.
    pub result: [u64; 25],
}

impl Sha3BlockCircuit {
    /// Verify the SHA-3 compression of one block.
    pub fn verify(&self) -> Result<(), &'static str> {
        let mut st = self.state;
        crate::sha3::sha3_absorb_block(&mut st, &self.block);
        for (&exp, &out) in st.iter().zip(self.result.iter()) {
            ensure_equal_u64(exp, out)?;
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Poseidon Hash Circuits
// -----------------------------------------------------------------------------

/// Circuit verifying the POSEIDON2 instruction (2-input Poseidon hash).
pub struct Poseidon2Circuit {
    /// First input element.
    pub a: u64,
    /// Second input element.
    pub b: u64,
    /// Expected hash output.
    pub result: u64,
}

impl Poseidon2Circuit {
    /// Verify the Poseidon2 hash by recomputing it from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        #[cfg(feature = "ivm_halo2_real")]
        {
            use iroha_zkp_halo2::PrimeField64 as F;
            let a = F::from(self.a).to_u64();
            let b = F::from(self.b).to_u64();
            let expected = crate::poseidon::poseidon2(a, b);
            return ensure_equal_u64(expected, self.result);
        }
        #[cfg(not(feature = "ivm_halo2_real"))]
        let expected = crate::poseidon::poseidon2(self.a, self.b);
        ensure_equal_u64(expected, self.result)
    }
}

/// Circuit verifying the POSEIDON6 instruction (6-input Poseidon hash).
pub struct Poseidon6Circuit {
    /// Input elements to hash.
    pub inputs: [u64; 6],
    /// Expected hash output.
    pub result: u64,
}

impl Poseidon6Circuit {
    /// Verify the Poseidon6 hash by recomputing it from the inputs.
    pub fn verify(&self) -> Result<(), &'static str> {
        #[cfg(feature = "ivm_halo2_real")]
        {
            use iroha_zkp_halo2::PrimeField64 as F;
            let mut tmp = [0u64; 6];
            for (i, v) in self.inputs.iter().enumerate() {
                tmp[i] = F::from(*v).to_u64();
            }
            let expected = crate::poseidon::poseidon6(tmp);
            return ensure_equal_u64(expected, self.result);
        }
        #[cfg(not(feature = "ivm_halo2_real"))]
        let expected = crate::poseidon::poseidon6(self.inputs);
        ensure_equal_u64(expected, self.result)
    }
}

// -----------------------------------------------------------------------------
// AES Round Circuits
// -----------------------------------------------------------------------------

/// Circuit verifying the AESENC instruction (one AES encryption round).
pub struct AesEncCircuit {
    pub state: [u8; 16],
    pub round_key: [u8; 16],
    pub result: [u8; 16],
}

impl AesEncCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        let expected = crate::aes::aesenc(self.state, self.round_key);
        ensure_bytes_equal(&expected, &self.result)
    }
}

/// Circuit verifying the AESDEC instruction (one AES decryption round).
pub struct AesDecCircuit {
    pub state: [u8; 16],
    pub round_key: [u8; 16],
    pub result: [u8; 16],
}

impl AesDecCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        let expected = crate::aes::aesdec(self.state, self.round_key);
        ensure_bytes_equal(&expected, &self.result)
    }
}

// -----------------------------------------------------------------------------
// Ed25519 Signature Verification Circuit
// -----------------------------------------------------------------------------

/// Circuit verifying an Ed25519 signature over a message.
#[derive(Clone, Debug)]
pub struct Ed25519VerifyCircuit<'a> {
    /// Public key bytes.
    pub public_key: [u8; 32],
    /// Signature bytes (R || S).
    pub signature: [u8; 64],
    /// Message to verify the signature against.
    pub message: &'a [u8],
    /// Expected verification result.
    pub result: bool,
}

impl<'a> Ed25519VerifyCircuit<'a> {
    /// Verify the signature using `ed25519-dalek` and compare with the
    /// expected `result` flag.
    pub fn verify(&self) -> Result<(), &'static str> {
        use ed25519_dalek::{Signature, VerifyingKey};
        let pk = VerifyingKey::from_bytes(&self.public_key)
            .map_err(|_| "Ed25519 verifying key decode failed")?;
        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| "Ed25519 signature decode failed")?;
        let valid = pk.verify_strict(self.message, &sig).is_ok();
        ensure_equal_bool(valid, self.result)
    }
}

// -----------------------------------------------------------------------------
// Dilithium Signature Verification Circuit
// -----------------------------------------------------------------------------

/// Dilithium security level.
#[derive(Clone, Copy)]
pub enum DilithiumLevel {
    Level2,
    Level3,
    Level5,
}

/// Circuit verifying a Dilithium signature over a message.
pub struct DilithiumVerifyCircuit<'a> {
    /// Parameter set to use (2, 3 or 5).
    pub level: DilithiumLevel,
    /// Public key bytes.
    pub public_key: &'a [u8],
    /// Signature bytes.
    pub signature: &'a [u8],
    /// Message to verify.
    pub message: &'a [u8],
    /// Expected verification result.
    pub result: bool,
}

impl<'a> DilithiumVerifyCircuit<'a> {
    /// Verify the signature using the appropriate Dilithium level and compare
    /// with the expected `result` flag.
    pub fn verify(&self) -> Result<(), &'static str> {
        use pqcrypto_mldsa::{mldsa44 as dilithium2, mldsa65 as dilithium3, mldsa87 as dilithium5};
        use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};

        let valid = match self.level {
            DilithiumLevel::Level2 => {
                if self.public_key.len() != dilithium2::public_key_bytes() {
                    return Err("Dilithium level2 public key length mismatch");
                }
                if self.signature.len() != dilithium2::signature_bytes() {
                    return Err("Dilithium level2 signature length mismatch");
                }
                let pk = dilithium2::PublicKey::from_bytes(self.public_key)
                    .map_err(|_| "Dilithium level2 public key decode failed")?;
                let sig = dilithium2::DetachedSignature::from_bytes(self.signature)
                    .map_err(|_| "Dilithium level2 signature decode failed")?;
                dilithium2::verify_detached_signature(&sig, self.message, &pk).is_ok()
            }
            DilithiumLevel::Level3 => {
                if self.public_key.len() != dilithium3::public_key_bytes() {
                    return Err("Dilithium level3 public key length mismatch");
                }
                if self.signature.len() != dilithium3::signature_bytes() {
                    return Err("Dilithium level3 signature length mismatch");
                }
                let pk = dilithium3::PublicKey::from_bytes(self.public_key)
                    .map_err(|_| "Dilithium level3 public key decode failed")?;
                let sig = dilithium3::DetachedSignature::from_bytes(self.signature)
                    .map_err(|_| "Dilithium level3 signature decode failed")?;
                dilithium3::verify_detached_signature(&sig, self.message, &pk).is_ok()
            }
            DilithiumLevel::Level5 => {
                if self.public_key.len() != dilithium5::public_key_bytes() {
                    return Err("Dilithium level5 public key length mismatch");
                }
                if self.signature.len() != dilithium5::signature_bytes() {
                    return Err("Dilithium level5 signature length mismatch");
                }
                let pk = dilithium5::PublicKey::from_bytes(self.public_key)
                    .map_err(|_| "Dilithium level5 public key decode failed")?;
                let sig = dilithium5::DetachedSignature::from_bytes(self.signature)
                    .map_err(|_| "Dilithium level5 signature decode failed")?;
                dilithium5::verify_detached_signature(&sig, self.message, &pk).is_ok()
            }
        };

        ensure_equal_bool(valid, self.result)
    }
}

// -----------------------------------------------------------------------------
// secp256k1 Curve Arithmetic Circuits
// -----------------------------------------------------------------------------

/// Circuit verifying secp256k1 point addition.
pub struct Secp256k1AddCircuit {
    pub p: [u8; 65],
    pub q: [u8; 65],
    pub result: [u8; 65],
}

impl Secp256k1AddCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use k256::{
            EncodedPoint, ProjectivePoint,
            elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint},
        };
        let p_enc =
            EncodedPoint::from_bytes(self.p).map_err(|_| "secp256k1 point P decode failed")?;
        let q_enc =
            EncodedPoint::from_bytes(self.q).map_err(|_| "secp256k1 point Q decode failed")?;
        let r_enc = EncodedPoint::from_bytes(self.result)
            .map_err(|_| "secp256k1 result point decode failed")?;
        let p = Option::<k256::AffinePoint>::from(k256::AffinePoint::from_encoded_point(&p_enc))
            .ok_or("secp256k1 point P not on curve")?;
        let q = Option::<k256::AffinePoint>::from(k256::AffinePoint::from_encoded_point(&q_enc))
            .ok_or("secp256k1 point Q not on curve")?;
        let expected = (ProjectivePoint::from(p) + ProjectivePoint::from(q))
            .to_affine()
            .to_encoded_point(false);
        if expected.as_bytes() == r_enc.as_bytes() {
            Ok(())
        } else {
            Err("ec add mismatch")
        }
    }
}

pub struct Secp256k1MulCircuit {
    pub scalar: [u8; 32],
    pub point: [u8; 65],
    pub result: [u8; 65],
}

impl Secp256k1MulCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use k256::{
            EncodedPoint, ProjectivePoint, Scalar,
            elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint},
        };
        let pt_enc =
            EncodedPoint::from_bytes(self.point).map_err(|_| "secp256k1 point decode failed")?;
        let res_enc = EncodedPoint::from_bytes(self.result)
            .map_err(|_| "secp256k1 result point decode failed")?;
        let pt = Option::<k256::AffinePoint>::from(k256::AffinePoint::from_encoded_point(&pt_enc))
            .ok_or("secp256k1 point not on curve")?;
        use ff::PrimeField;
        let scalar = Scalar::from_repr(self.scalar.into()).unwrap();
        let expected = (ProjectivePoint::from(pt) * scalar)
            .to_affine()
            .to_encoded_point(false);
        if expected.as_bytes() == res_enc.as_bytes() {
            Ok(())
        } else {
            Err("ec mul mismatch")
        }
    }
}

pub struct EcdsaVerifyCircuit {
    pub public_key: [u8; 33],
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],
    pub result: bool,
}

impl EcdsaVerifyCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use k256::ecdsa::{Signature, VerifyingKey};
        use signature::hazmat::PrehashVerifier as _;
        let vk = VerifyingKey::from_sec1_bytes(&self.public_key)
            .map_err(|_| "secp256k1 verifying key decode failed")?;
        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| "secp256k1 signature decode failed")?;
        let valid = vk.verify_prehash(&self.message_hash, &sig).is_ok();
        if valid == self.result {
            Ok(())
        } else {
            Err("ecdsa mismatch")
        }
    }
}

// -----------------------------------------------------------------------------
// Elliptic Curve Arithmetic Circuits (BLS12-381)
// -----------------------------------------------------------------------------

/// Circuit verifying point addition on the BLS12‑381 G1 curve.
pub struct ECAddCircuit {
    pub p: [u8; 48],
    pub q: [u8; 48],
    pub result: [u8; 48],
}

impl ECAddCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use blstrs::{G1Affine, G1Projective};
        use group::Curve;
        let p = Option::<G1Affine>::from(G1Affine::from_compressed(&self.p)).ok_or("p")?;
        let q = Option::<G1Affine>::from(G1Affine::from_compressed(&self.q)).ok_or("q")?;
        let expected = (G1Projective::from(p) + G1Projective::from(q))
            .to_affine()
            .to_compressed();
        if expected == self.result {
            Ok(())
        } else {
            Err("ec add mismatch")
        }
    }
}

/// Circuit verifying variable‑point scalar multiplication on BLS12‑381 G1.
pub struct ECMulVarCircuit {
    pub scalar: [u8; 32],
    pub point: [u8; 48],
    pub result: [u8; 48],
}

impl ECMulVarCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use blstrs::{G1Affine, G1Projective, Scalar};
        use group::Curve;
        let pt = Option::<G1Affine>::from(G1Affine::from_compressed(&self.point)).ok_or("point")?;
        let scalar = Option::<Scalar>::from(Scalar::from_bytes_be(&self.scalar)).ok_or("scalar")?;
        let expected = (G1Projective::from(pt) * scalar)
            .to_affine()
            .to_compressed();
        if expected == self.result {
            Ok(())
        } else {
            Err("ec mul mismatch")
        }
    }
}

/// Circuit verifying public key generation (scalar multiplication by the
/// fixed generator) on BLS12‑381 G1.
pub struct PubKeyGenCircuit {
    pub secret: [u8; 32],
    pub result: [u8; 48],
}

impl PubKeyGenCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use blstrs::{G1Projective, Scalar};
        use group::{Curve, Group};
        let s = Option::<Scalar>::from(Scalar::from_bytes_be(&self.secret)).ok_or("secret")?;
        let expected = (G1Projective::generator() * s).to_affine().to_compressed();
        ensure_bytes_equal(&expected, &self.result)
    }
}

/// Circuit verifying the PAIRING instruction (BLS12‑381 pairing check).
pub struct PairingCircuit {
    /// Scalar multiplier for the G1 generator.
    pub a: u64,
    /// Scalar multiplier for the G2 generator.
    pub b: u64,
    /// Expected output of the pairing helper (truncated u64 form).
    pub result: u64,
}

impl PairingCircuit {
    /// Verify the pairing check by recomputing it from the witness scalars.
    pub fn verify(&self) -> Result<(), &'static str> {
        let expected = crate::ec::pairing_check_truncated(self.a, self.b);
        ensure_equal_u64(expected, self.result)
    }
}

// -----------------------------------------------------------------------------
// Memory Load/Store Circuits
// -----------------------------------------------------------------------------

const CHUNK_SIZE: usize = 32;

// Paths are ordered from leaf → root. Keep this in sync with
// ByteMerkleTree::path and tests under crates/ivm/tests.
fn verify_memory_path(leaf: [u8; 32], index: usize, path: &[[u8; 32]]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut current: [u8; 32] = Sha256::digest(leaf).into();
    let mut idx = index;
    for sib in path.iter() {
        // Treat an all‑zero sibling as a missing node (left‑promotion semantics).
        let zero = [0u8; 32];
        let (l_opt, r_opt) = if idx.is_multiple_of(2) {
            (Some(current), if *sib == zero { None } else { Some(*sib) })
        } else {
            (if *sib == zero { None } else { Some(*sib) }, Some(current))
        };
        current = match (l_opt, r_opt) {
            (Some(mut l), Some(mut r)) => {
                // Mirror `iroha_crypto` Merkle parent: set the LSB of each child byte array
                // to 1 (Hash::prehashed) before hashing left||right.
                l[31] |= 1;
                r[31] |= 1;
                let mut h = Sha256::new();
                h.update(l);
                h.update(r);
                h.finalize().into()
            }
            (Some(l), None) => l,
            (None, Some(_)) => {
                // Invalid: right‑only child cannot exist in a complete tree.
                // Return a sentinel zero root to ensure mismatch.
                [0u8; 32]
            }
            (None, None) => [0u8; 32],
        };
        idx /= 2;
    }
    // ByteMerkleTree roots are returned as Hash::prehashed bytes (marker bit set).
    let mut out = current;
    out[31] |= 1;
    out
}

fn region_perm(addr: u64, size: u32, code_len: u64, heap_limit: u64) -> Option<crate::error::Perm> {
    use crate::memory::Memory;
    let end = addr.checked_add(size as u64)?;
    if end <= code_len {
        return Some(crate::error::Perm::READ | crate::error::Perm::EXECUTE);
    }
    if addr >= Memory::HEAP_START && end <= Memory::HEAP_START + heap_limit {
        return Some(crate::error::Perm::READ | crate::error::Perm::WRITE);
    }
    if addr >= Memory::INPUT_START && end <= Memory::INPUT_START + Memory::INPUT_SIZE {
        return Some(crate::error::Perm::READ);
    }
    if addr >= Memory::OUTPUT_START && end <= Memory::OUTPUT_START + Memory::OUTPUT_SIZE {
        return Some(crate::error::Perm::READ | crate::error::Perm::WRITE);
    }
    if addr >= Memory::STACK_START && end <= Memory::STACK_START + Memory::STACK_SIZE {
        return Some(crate::error::Perm::READ | crate::error::Perm::WRITE);
    }
    None
}

/// Simple circuit verifying a memory load operation via its Merkle proof.
pub struct LoadCircuit {
    pub root: [u8; 32],
    pub addr: u64,
    pub value: u128,
    pub size: u8,
    pub leaf: [u8; 32],
    pub path: Vec<[u8; 32]>,
    pub code_len: u64,
    pub heap_limit: u64,
}

impl LoadCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use crate::error::Perm;
        // Alignment
        let align = match self.size {
            1 => 1,
            2 => 2,
            4 => 4,
            8 => 8,
            16 => 16,
            _ => return Err("invalid size"),
        } as u64;
        if !self.addr.is_multiple_of(align) {
            return Err("misaligned access");
        }
        // Permission
        let perm = region_perm(self.addr, self.size as u32, self.code_len, self.heap_limit)
            .ok_or("access violation")?;
        if !perm.contains(Perm::READ) {
            return Err("access violation");
        }

        // Check value bytes inside leaf
        let offset = (self.addr as usize) % CHUNK_SIZE;
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&self.value.to_le_bytes());
        ensure_bytes_equal(
            &self.leaf[offset..offset + self.size as usize],
            &buf[..self.size as usize],
        )?;

        // Verify Merkle path
        let idx = (self.addr as usize) / CHUNK_SIZE;
        let root = verify_memory_path(self.leaf, idx, &self.path);
        ensure_bytes_equal(&root, &self.root)
    }
}

/// Simple circuit verifying a memory store operation and root update.
pub struct StoreCircuit {
    pub root_before: [u8; 32],
    pub root_after: [u8; 32],
    pub addr: u64,
    pub value: u128,
    pub size: u8,
    pub old_leaf: [u8; 32],
    pub path: Vec<[u8; 32]>,
    pub code_len: u64,
    pub heap_limit: u64,
}

impl StoreCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        use crate::error::Perm;
        let align = match self.size {
            1 => 1,
            2 => 2,
            4 => 4,
            8 => 8,
            16 => 16,
            _ => return Err("invalid size"),
        } as u64;
        if !self.addr.is_multiple_of(align) {
            return Err("misaligned access");
        }
        let perm = region_perm(self.addr, self.size as u32, self.code_len, self.heap_limit)
            .ok_or("access violation")?;
        if !perm.contains(Perm::WRITE) {
            return Err("access violation");
        }

        let idx = (self.addr as usize) / CHUNK_SIZE;
        let computed_before = verify_memory_path(self.old_leaf, idx, &self.path);
        ensure_bytes_equal(&computed_before, &self.root_before)?;

        let offset = (self.addr as usize) % CHUNK_SIZE;
        let mut new_leaf = self.old_leaf;
        let mut val_bytes = [0u8; 16];
        val_bytes.copy_from_slice(&self.value.to_le_bytes());
        new_leaf[offset..offset + self.size as usize]
            .copy_from_slice(&val_bytes[..self.size as usize]);

        let computed_after = verify_memory_path(new_leaf, idx, &self.path);
        ensure_bytes_equal(&computed_after, &self.root_after)
    }
}

/// Vector load circuit (16-byte load).
pub struct VectorLoadCircuit {
    pub inner: LoadCircuit,
}

impl VectorLoadCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.inner.size != 16 {
            return Err("invalid size");
        }
        self.inner.verify()
    }
}

/// Vector store circuit (16-byte store).
pub struct VectorStoreCircuit {
    pub inner: StoreCircuit,
}

impl VectorStoreCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.inner.size != 16 {
            return Err("invalid size");
        }
        self.inner.verify()
    }
}

// -----------------------------------------------------------------------------
// System Instruction Circuits
// -----------------------------------------------------------------------------

/// Circuit verifying heap allocation via `SYSCALL_ALLOC`.
pub struct AllocCircuit {
    /// Heap pointer before the allocation (in bytes).
    pub heap_alloc_before: u64,
    /// Current heap limit before the allocation.
    pub heap_limit_before: u64,
    /// Requested allocation size.
    pub size: u64,
    /// Heap pointer after the allocation.
    pub heap_alloc_after: u64,
    /// Address returned to the caller.
    pub addr: u64,
}

impl AllocCircuit {
    /// Verify that the allocation obeys alignment and bounds rules.
    pub fn verify(&self) -> Result<(), &'static str> {
        use crate::memory::Memory;
        let aligned = (self.size + 7) & !7;
        let new_alloc = self.heap_alloc_before + aligned;
        ensure_equal_bool(new_alloc <= self.heap_limit_before, true)?;
        ensure_equal_u64(self.heap_alloc_after, new_alloc)?;
        ensure_equal_u64(self.addr, Memory::HEAP_START + self.heap_alloc_before)?;
        ensure_equal_bool(self.heap_alloc_after <= Memory::HEAP_MAX_SIZE, true)
    }
}

/// Circuit verifying the `GETGAS` instruction.
pub struct GetGasCircuit {
    /// Initial gas supplied to the VM.
    pub initial_gas: u64,
    /// Gas consumed so far.
    pub gas_used: u64,
    /// Value reported by GETGAS.
    pub reported: u64,
}

impl GetGasCircuit {
    /// Verify that the reported gas matches the remaining budget.
    pub fn verify(&self) -> Result<(), &'static str> {
        ensure_equal_bool(self.initial_gas >= self.gas_used, true)?;
        let expected = self.initial_gas - self.gas_used;
        ensure_equal_u64(self.reported, expected)
    }
}

// -----------------------------------------------------------------------------
// Assertion Circuits
// -----------------------------------------------------------------------------

/// Simple circuit verifying that a value is zero.
pub struct AssertZeroCircuit {
    pub value: u64,
}

impl AssertZeroCircuit {
    /// Verify the circuit by checking the value is zero.
    pub fn verify(&self) -> Result<(), &'static str> {
        ensure_equal_u64(self.value, 0)
    }
}

/// Simple circuit verifying that two values are equal.
pub struct AssertEqCircuit {
    pub a: u64,
    pub b: u64,
}

impl AssertEqCircuit {
    /// Verify the circuit by checking the two values are equal.
    pub fn verify(&self) -> Result<(), &'static str> {
        ensure_equal_u64(self.a, self.b)
    }
}

/// Simple circuit verifying that a value lies in a range [0, 2^bits).
pub struct AssertRangeCircuit {
    pub value: u64,
    pub bits: u8,
}

impl AssertRangeCircuit {
    /// Verify the circuit by checking the value fits within the bit range.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.bits <= 64 {
            let mask: u64 = if self.bits == 64 {
                u64::MAX
            } else {
                (1u64 << self.bits) - 1
            };
            ensure_equal_u64(self.value & !mask, 0)
        } else {
            // Bits >64 implies no restriction in this simplified circuit.
            Ok(())
        }
    }
}

// -----------------------------------------------------------------------------
// Control Flow Circuits
// -----------------------------------------------------------------------------

/// Circuit verifying an unconditional jump (JMP).
pub struct JumpCircuit {
    pub pc: u64,
    pub offset: i32,
    pub next_pc: u64,
    pub code_len: u64,
}

impl JumpCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        let target = ((self.pc as i64) + self.offset as i64) as u64;
        ensure_equal_u64(self.next_pc, target)?;
        ensure_equal_u64(self.next_pc & 1, 0)?;
        ensure_equal_bool(self.next_pc < self.code_len, true)
    }
}

/// Circuit verifying a jump and link instruction.
pub struct JalCircuit {
    pub pc: u64,
    pub offset: i32,
    pub next_pc: u64,
    pub link: u64,
    pub code_len: u64,
}

impl JalCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        ensure_equal_u64(self.link, self.pc.wrapping_add(4))?;
        JumpCircuit {
            pc: self.pc,
            offset: self.offset,
            next_pc: self.next_pc,
            code_len: self.code_len,
        }
        .verify()
    }
}

/// Circuit verifying a register-based jump (JALR/JR).
pub struct JumpRegCircuit {
    pub pc: u64,
    pub base: u64,
    pub imm: i16,
    pub next_pc: u64,
    pub link: Option<u64>,
    pub code_len: u64,
}

impl JumpRegCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        if let Some(l) = self.link {
            ensure_equal_u64(l, self.pc.wrapping_add(4))?;
        }
        let mut target = self.base.wrapping_add(self.imm as i64 as u64);
        target &= !1u64;
        ensure_equal_u64(self.next_pc, target)?;
        ensure_equal_bool(self.next_pc < self.code_len, true)
    }
}

/// Circuit verifying a conditional branch.
pub struct BranchCircuit {
    pub pc: u64,
    pub offset: i16,
    pub take_branch: bool,
    pub next_pc: u64,
    pub code_len: u64,
}

impl BranchCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        let taken_pc = ((self.pc as i64) + self.offset as i64) as u64;
        let not_pc = self.pc.wrapping_add(4);
        let expected = if self.take_branch { taken_pc } else { not_pc };
        ensure_equal_u64(self.next_pc, expected)?;
        ensure_equal_u64(self.next_pc & 1, 0)?;
        ensure_equal_bool(self.next_pc < self.code_len, true)
    }
}

/// Circuit verifying a HALT instruction.
pub struct HaltCircuit {
    pub pc: u64,
    pub next_pc: u64,
}

impl HaltCircuit {
    pub fn verify(&self) -> Result<(), &'static str> {
        ensure_equal_u64(self.next_pc, self.pc.wrapping_add(4))
    }
}

// -----------------------------------------------------------------------------
// Full VM Execution Circuit
// -----------------------------------------------------------------------------

/// Verify an entire IVM execution trace.
///
/// The witness consists of the executed program bytes together with the
/// recorded [`RegisterState`] trace and constraint log.  `verify` replays the
/// program and ensures the produced trace matches the witness while also
/// checking all constraints via [`zk::verify_trace`].  This serves as a simple
/// stand-in for a real Halo2 circuit proving correct execution.
pub struct VMExecutionCircuit<'a> {
    pub program: &'a [u8],
    pub trace: &'a [crate::zk::RegisterState],
    pub constraints: &'a [crate::zk::Constraint],
}

impl<'a> VMExecutionCircuit<'a> {
    /// Create a new execution circuit over the given program and trace.
    pub fn new(
        program: &'a [u8],
        trace: &'a [crate::zk::RegisterState],
        constraints: &'a [crate::zk::Constraint],
    ) -> Self {
        Self {
            program,
            trace,
            constraints,
        }
    }

    /// Verify the trace row by row using the opcode circuits.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.trace.is_empty() {
            return Err("empty trace");
        }

        let offset = crate::metadata::ProgramMetadata::parse(self.program)
            .map_err(|_| "program")?
            .code_offset;
        let code = &self.program[offset..];

        for window in self.trace.windows(2) {
            let curr = &window[0];
            let next = &window[1];

            if curr.pc as usize >= code.len() {
                ensure_equal_u64(curr.pc, next.pc)?;
                ensure_slice_equal_u64(&curr.gpr, &next.gpr)?;
                ensure_slice_equal_bool(&curr.tags, &next.tags)?;
                continue;
            }

            let bytes: [u8; 4] = code[curr.pc as usize..curr.pc as usize + 4]
                .try_into()
                .map_err(|_| "pc out of bounds")?;
            let instr = u32::from_le_bytes(bytes);

            let op = crate::instruction::wide::opcode(instr);

            match op {
                x if x == crate::instruction::wide::arithmetic::ADD => {
                    let (_op, rd, rs1, rs2) = crate::encoding::wide::decode_rr(instr);
                    let rd = rd as usize;
                    let rs1 = rs1 as usize;
                    let rs2 = rs2 as usize;
                    let c = ALUCircuit {
                        op: ALUOp::Add,
                        a: curr.gpr[rs1],
                        b: curr.gpr[rs2],
                        result: next.gpr[rd],
                    };
                    c.verify()?;
                    Self::check_pc(curr.pc.wrapping_add(4), next.pc)?;
                    Self::check_regs(curr, next, Some(rd))?;
                }
                x if x == crate::instruction::wide::arithmetic::SUB => {
                    let (_op, rd, rs1, rs2) = crate::encoding::wide::decode_rr(instr);
                    let rd = rd as usize;
                    let rs1 = rs1 as usize;
                    let rs2 = rs2 as usize;
                    let c = ALUCircuit {
                        op: ALUOp::Sub,
                        a: curr.gpr[rs1],
                        b: curr.gpr[rs2],
                        result: next.gpr[rd],
                    };
                    c.verify()?;
                    Self::check_pc(curr.pc.wrapping_add(4), next.pc)?;
                    Self::check_regs(curr, next, Some(rd))?;
                }
                x if x == crate::instruction::wide::arithmetic::ADDI => {
                    let (_op, rd, rs1, imm) = crate::encoding::wide::decode_ri(instr);
                    let rd = rd as usize;
                    let rs1 = rs1 as usize;
                    let c = ALUCircuit {
                        op: ALUOp::Add,
                        a: curr.gpr[rs1],
                        b: imm as i64 as u64,
                        result: next.gpr[rd],
                    };
                    c.verify()?;
                    Self::check_pc(curr.pc.wrapping_add(4), next.pc)?;
                    Self::check_regs(curr, next, Some(rd))?;
                }
                x if x == crate::instruction::wide::control::BEQ
                    || x == crate::instruction::wide::control::BNE =>
                {
                    let (_op, rs1_raw, rs2_raw, off) = crate::encoding::wide::decode_mem(instr);
                    let rs1 = rs1_raw as usize;
                    let rs2 = rs2_raw as usize;
                    let take = if op == crate::instruction::wide::control::BEQ {
                        curr.gpr[rs1] == curr.gpr[rs2]
                    } else {
                        curr.gpr[rs1] != curr.gpr[rs2]
                    };
                    let branch_delta = (off as i16 as i32) * 4;
                    let bc = BranchCircuit {
                        pc: curr.pc,
                        offset: branch_delta as i16,
                        take_branch: take,
                        next_pc: next.pc,
                        code_len: code.len() as u64,
                    };
                    bc.verify()?;
                    Self::check_regs(curr, next, None)?;
                }
                x if x == crate::instruction::wide::control::HALT => {
                    let hc = HaltCircuit {
                        pc: curr.pc,
                        next_pc: next.pc,
                    };
                    hc.verify()?;
                    Self::check_regs(curr, next, None)?;
                }
                _ => return Err("unsupported opcode"),
            }
        }

        crate::zk::verify_trace(self.trace, self.constraints, &[], &[])
            .map_err(|_| "constraint failure")?;

        Ok(())
    }

    fn check_pc(expected: u64, actual: u64) -> Result<(), &'static str> {
        ensure_equal_u64(expected, actual)
    }

    fn check_regs(
        curr: &crate::zk::RegisterState,
        next: &crate::zk::RegisterState,
        changed: Option<usize>,
    ) -> Result<(), &'static str> {
        for (i, (&a, &b)) in curr.gpr.iter().zip(next.gpr.iter()).enumerate() {
            if Some(i) == changed {
                continue;
            }
            ensure_equal_u64(a, b)?;
        }
        ensure_slice_equal_bool(&curr.tags, &next.tags)
    }
}
