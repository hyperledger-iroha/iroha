//! Sound transparent STARK used by the first-release ZK-ACE engine.
//!
//! The generic historical `zk_stark` helper commits raw trace rows and then
//! deliberately excludes the only witness-bearing row from every query.  That
//! construction cannot establish knowledge: a malicious prover can commit an
//! unrelated zero composition vector and no verifier query ever reconnects it
//! to the private row.  ZK-ACE therefore uses the self-contained construction
//! below:
//!
//! - every witness byte is range constrained through a bit decomposition;
//! - the two Poseidon2 sponge computations are represented by a complete
//!   quadratic execution trace;
//! - trace columns are interpolated and masked with random multiples of the
//!   trace-domain vanishing polynomial before the verifier sees any opening;
//! - three independently challenged composition quotients share one trace
//!   commitment;
//! - each lane performs an actual low-degree FRI test, stopping at the compiled
//!   blow-up domain and checking the complete terminal polynomial has degree at
//!   most one;
//! - query openings bind the same masked trace rows, quotient values, and FRI
//!   base evaluations.
//!
//! No caller-selected parameter, transcript, proof shape, or backend is carried
//! by the wire value.  All dimensions below are compiled consensus constants.

use std::collections::BTreeSet;

use fastpq_isi::poseidon::{MDS, ROUND_CONSTANTS};
use iroha_data_model::zk::{
    ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
    ZkAcePublicInputsV1, ZkAceWitnessV1, derive_zk_ace_air_public_digest,
    zk_ace_pack_bytes_to_field_limbs,
};
use norito::{Decode, Encode};
use rand::TryRngCore;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

const FIELD_MODULUS: u64 = 0xffff_ffff_0000_0001;
const FIELD_MODULUS_U128: u128 = FIELD_MODULUS as u128;
const FIELD_GENERATOR: u64 = 7;
const TWO_ADICITY: u32 = 32;

/// Base execution trace has exactly 4,096 rows.
pub(crate) const TRACE_LOG2: u8 = 12;
/// The low-degree extension uses a compiled 16x blow-up.
pub(crate) const BLOWUP_LOG2: u8 = 4;
/// Number of transcript-derived FRI queries.
pub(crate) const QUERY_COUNT: usize = 32;
/// Independent constraint/FRI lanes sharing one trace commitment.
pub(crate) const SECURITY_LANES: usize = 3;
/// Degree of the random trace masking polynomial.
const MASK_DEGREE: usize = 255;
/// FRI stops on the complete compiled blow-up domain.
const TERMINAL_SIZE: usize = 1 << BLOWUP_LOG2;
/// Every folded terminal polynomial must be linear or constant.
const TERMINAL_DEGREE_BOUND: usize = 1;
const TRACE_SIZE: usize = 1 << TRACE_LOG2;
const LDE_LOG2: u8 = TRACE_LOG2 + BLOWUP_LOG2;
const LDE_SIZE: usize = 1 << LDE_LOG2;
const FRI_ROUNDS: usize = TRACE_LOG2 as usize;
const PRIVATE_LIMBS: usize = 15;
const LIMB_BITS: usize = 56;
const POSEIDON_FULL_ROUNDS_HALF: usize = 4;
const POSEIDON_ROUNDS: usize = ROUND_CONSTANTS.len();
const PROOF_VERSION: u16 = 1;
const MAX_QUERY_DERIVATION_ATTEMPTS: usize = LDE_SIZE * 2;

const STATE_OFFSET: usize = 0;
const A_OFFSET: usize = STATE_OFFSET + 3;
const X2_OFFSET: usize = A_OFFSET + 3;
const X4_OFFSET: usize = X2_OFFSET + 3;
const X5_OFFSET: usize = X4_OFFSET + 3;
const QUEUE_OFFSET: usize = X5_OFFSET + 3;
const LIMB_OFFSET: usize = QUEUE_OFFSET + PRIVATE_LIMBS;
const BIT_OFFSET: usize = LIMB_OFFSET + 1;
const TRACE_WIDTH: usize = BIT_OFFSET + LIMB_BITS;

const FIX_FULL: usize = 0;
const FIX_PARTIAL: usize = FIX_FULL + 1;
const FIX_ABSORB_0: usize = FIX_PARTIAL + 1;
const FIX_ABSORB_1: usize = FIX_ABSORB_0 + 1;
const FIX_RESET: usize = FIX_ABSORB_1 + 1;
const FIX_LOAD_OFFSET: usize = FIX_RESET + 1;
const FIX_MESSAGE_CONST: usize = FIX_LOAD_OFFSET + PRIVATE_LIMBS;
const FIX_MESSAGE_WITNESS_OFFSET: usize = FIX_MESSAGE_CONST + 1;
const FIX_RC_OFFSET: usize = FIX_MESSAGE_WITNESS_OFFSET + PRIVATE_LIMBS;
const FIX_OUTPUT_OFFSET: usize = FIX_RC_OFFSET + 3;
const FIXED_WIDTH: usize = FIX_OUTPUT_OFFSET + 8;

const TRANSCRIPT_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:transparent-stark:v1";
const TRACE_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:trace-leaf:v1";
const COMPOSITION_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:composition-leaf:v1";
const FRI_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:fri-leaf:v1";
const MERKLE_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:merkle-node:v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct F(u64);

impl F {
    const ZERO: Self = Self(0);
    const ONE: Self = Self(1);

    fn canonical(value: u64) -> Option<Self> {
        (value < FIELD_MODULUS).then_some(Self(value))
    }

    fn reduce(value: u128) -> Self {
        Self((value % FIELD_MODULUS_U128) as u64)
    }

    fn add(self, rhs: Self) -> Self {
        Self::reduce(u128::from(self.0) + u128::from(rhs.0))
    }

    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            Self(FIELD_MODULUS - (rhs.0 - self.0))
        }
    }

    fn neg(self) -> Self {
        if self == Self::ZERO {
            self
        } else {
            Self(FIELD_MODULUS - self.0)
        }
    }

    fn mul(self, rhs: Self) -> Self {
        Self::reduce(u128::from(self.0) * u128::from(rhs.0))
    }

    fn pow(mut self, mut exponent: u128) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }

    fn inv(self) -> Option<Self> {
        (self != Self::ZERO).then(|| self.pow(u128::from(FIELD_MODULUS - 2)))
    }
}

#[derive(Clone, Copy, Debug)]
enum MessageWord {
    Constant(u64),
    Witness(usize),
}

#[derive(Clone, Copy, Debug)]
enum ScheduleOp {
    Hold,
    Reset,
    Load(usize),
    Absorb { position: usize, word: MessageWord },
    FullRound { round: usize },
    PartialRound { round: usize },
    Output { output_index: usize },
}

#[derive(Clone, Copy, Debug)]
struct ScheduleRow {
    op: ScheduleOp,
}

#[derive(Clone)]
struct TraceMaterial {
    trace_columns: Vec<Vec<F>>,
    fixed_columns: Vec<Vec<F>>,
    public_outputs: [F; 8],
}

#[derive(Clone)]
struct MerkleTree {
    levels: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    fn from_leaves(leaves: Vec<[u8; 32]>) -> Result<Self, ZkAceStarkError> {
        if leaves.is_empty() || !leaves.len().is_power_of_two() {
            return Err(ZkAceStarkError::InternalInvariant(
                "Merkle leaf count must be a non-zero power of two",
            ));
        }
        let mut levels = vec![leaves];
        while levels.last().map_or(0, Vec::len) > 1 {
            let previous = levels
                .last()
                .expect("non-empty Merkle level collection");
            let next = previous
                .chunks_exact(2)
                .map(|pair| merkle_node_hash(&pair[0], &pair[1]))
                .collect();
            levels.push(next);
        }
        Ok(Self { levels })
    }

    fn root(&self) -> [u8; 32] {
        self.levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .expect("validated Merkle tree has a root")
    }

    fn path(&self, mut index: usize) -> Result<Vec<[u8; 32]>, ZkAceStarkError> {
        if index >= self.levels[0].len() {
            return Err(ZkAceStarkError::InternalInvariant(
                "Merkle opening index is out of range",
            ));
        }
        let mut path = Vec::with_capacity(self.levels.len() - 1);
        for level in &self.levels[..self.levels.len() - 1] {
            path.push(level[index ^ 1]);
            index >>= 1;
        }
        Ok(path)
    }
}

#[derive(Clone)]
struct FriLaneMaterial {
    layers: Vec<Vec<F>>,
    trees: Vec<MerkleTree>,
    roots: Vec<[u8; 32]>,
    terminal_values: Vec<F>,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub(crate) struct ZkAceStarkProofV1 {
    version: u16,
    trace_root: [u8; 32],
    composition_roots: Vec<[u8; 32]>,
    fri_lanes: Vec<ZkAceFriLaneProofV1>,
    queries: Vec<ZkAceQueryProofV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct ZkAceFriLaneProofV1 {
    roots: Vec<[u8; 32]>,
    terminal_values: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct ZkAceQueryProofV1 {
    index: u32,
    current_row: Vec<u64>,
    next_row: Vec<u64>,
    current_row_path: Vec<[u8; 32]>,
    next_row_path: Vec<[u8; 32]>,
    composition_values: Vec<u64>,
    composition_paths: Vec<Vec<[u8; 32]>>,
    fri_lanes: Vec<ZkAceFriLaneQueryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct ZkAceFriLaneQueryV1 {
    rounds: Vec<ZkAceFriRoundOpeningV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct ZkAceFriRoundOpeningV1 {
    low: u64,
    high: u64,
    low_path: Vec<[u8; 32]>,
    high_path: Vec<[u8; 32]>,
}

/// Failure returned by the dedicated ZK-ACE STARK.
#[derive(Debug, Error)]
pub(crate) enum ZkAceStarkError {
    #[error("ZK-ACE public input cannot be encoded canonically")]
    PublicInputEncoding,
    #[error("ZK-ACE public digest is not a canonical Goldilocks field encoding")]
    NonCanonicalPublicDigest,
    #[error("ZK-ACE witness cannot be packed into the compiled 32-byte limb layout")]
    WitnessPacking,
    #[error("ZK-ACE witness does not satisfy the public commitment/nullifier relation")]
    WitnessRelation,
    #[error("operating-system randomness is unavailable for ZK-ACE trace masking")]
    RandomnessUnavailable,
    #[error("ZK-ACE proof is malformed")]
    MalformedProof,
    #[error("ZK-ACE proof shape does not match the compiled profile")]
    ProfileMismatch,
    #[error("ZK-ACE proof contains a non-canonical field element")]
    NonCanonicalField,
    #[error("ZK-ACE proof transcript or query schedule is inconsistent")]
    TranscriptMismatch,
    #[error("ZK-ACE trace opening is invalid")]
    TraceOpening,
    #[error("ZK-ACE composition opening or constraint quotient is invalid")]
    ConstraintOpening,
    #[error("ZK-ACE FRI opening is invalid")]
    FriOpening,
    #[error("ZK-ACE FRI terminal polynomial exceeds the compiled degree bound")]
    FriDegree,
    #[error("ZK-ACE internal invariant failed: {0}")]
    InternalInvariant(&'static str),
}

fn trace_leaf_hash(row: &[F]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRACE_LEAF_DOMAIN);
    hasher.update((row.len() as u64).to_be_bytes());
    for value in row {
        hasher.update(value.0.to_le_bytes());
    }
    hasher.finalize().into()
}

fn composition_leaf_hash(lane: usize, value: F) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(COMPOSITION_LEAF_DOMAIN);
    hasher.update((lane as u64).to_be_bytes());
    hasher.update(value.0.to_le_bytes());
    hasher.finalize().into()
}

fn fri_leaf_hash(lane: usize, round: usize, value: F) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FRI_LEAF_DOMAIN);
    hasher.update((lane as u64).to_be_bytes());
    hasher.update((round as u64).to_be_bytes());
    hasher.update(value.0.to_le_bytes());
    hasher.finalize().into()
}

fn merkle_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MERKLE_NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn verify_merkle_path(
    root: &[u8; 32],
    mut leaf: [u8; 32],
    mut index: usize,
    path: &[[u8; 32]],
    expected_depth: usize,
) -> bool {
    if path.len() != expected_depth {
        return false;
    }
    for sibling in path {
        leaf = if index & 1 == 0 {
            merkle_node_hash(&leaf, sibling)
        } else {
            merkle_node_hash(sibling, &leaf)
        };
        index >>= 1;
    }
    index == 0 && leaf == *root
}

fn primitive_root(log_size: u8) -> Result<F, ZkAceStarkError> {
    if u32::from(log_size) > TWO_ADICITY {
        return Err(ZkAceStarkError::InternalInvariant(
            "requested FFT domain exceeds Goldilocks two-adicity",
        ));
    }
    let order = 1_u128 << log_size;
    let root = F(FIELD_GENERATOR).pow((u128::from(FIELD_MODULUS) - 1) / order);
    if root.pow(order) != F::ONE || (order > 1 && root.pow(order / 2) == F::ONE) {
        return Err(ZkAceStarkError::InternalInvariant(
            "Goldilocks generator produced a non-primitive root",
        ));
    }
    Ok(root)
}

fn fft(values: &mut [F], root: F) -> Result<(), ZkAceStarkError> {
    let n = values.len();
    if n == 0 || !n.is_power_of_two() {
        return Err(ZkAceStarkError::InternalInvariant(
            "FFT length must be a non-zero power of two",
        ));
    }
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            values.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let step = root.pow((n / len) as u128);
        for chunk in values.chunks_exact_mut(len) {
            let mut twiddle = F::ONE;
            let (left, right) = chunk.split_at_mut(len / 2);
            for (a, b) in left.iter_mut().zip(right.iter_mut()) {
                let odd = (*b).mul(twiddle);
                let even = *a;
                *a = even.add(odd);
                *b = even.sub(odd);
                twiddle = twiddle.mul(step);
            }
        }
        len <<= 1;
    }
    Ok(())
}

fn ifft(values: &mut [F], root: F) -> Result<(), ZkAceStarkError> {
    fft(
        values,
        root.inv().ok_or(ZkAceStarkError::InternalInvariant(
            "FFT root must be invertible",
        ))?,
    )?;
    let inv_n = F::reduce(values.len() as u128)
        .inv()
        .ok_or(ZkAceStarkError::InternalInvariant(
            "FFT length must be invertible",
        ))?;
    for value in values {
        *value = value.mul(inv_n);
    }
    Ok(())
}

fn evaluate_coefficients_on_coset(
    coefficients: &[F],
    size: usize,
    root: F,
    shift: F,
) -> Result<Vec<F>, ZkAceStarkError> {
    if coefficients.len() > size || !size.is_power_of_two() {
        return Err(ZkAceStarkError::InternalInvariant(
            "invalid coefficient/coset evaluation shape",
        ));
    }
    let mut evaluations = vec![F::ZERO; size];
    let mut shift_power = F::ONE;
    for (target, coefficient) in evaluations.iter_mut().zip(coefficients.iter().copied()) {
        *target = coefficient.mul(shift_power);
        shift_power = shift_power.mul(shift);
    }
    fft(&mut evaluations, root)?;
    Ok(evaluations)
}

fn transpose_rows(rows: &[Vec<F>], width: usize) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    if rows.len() != TRACE_SIZE || rows.iter().any(|row| row.len() != width) {
        return Err(ZkAceStarkError::InternalInvariant(
            "trace/fixed rows have the wrong compiled shape",
        ));
    }
    let mut columns = vec![Vec::with_capacity(rows.len()); width];
    for row in rows {
        for (column, value) in columns.iter_mut().zip(row.iter().copied()) {
            column.push(value);
        }
    }
    Ok(columns)
}

fn bytes_as_constant_words(bytes: &[u8]) -> Vec<MessageWord> {
    zk_ace_pack_bytes_to_field_limbs(bytes)
        .limbs
        .into_iter()
        .map(MessageWord::Constant)
        .collect()
}

fn append_framed_constant_part(words: &mut Vec<MessageWord>, bytes: &[u8]) {
    words.push(MessageWord::Constant(bytes.len() as u64));
    words.extend(bytes_as_constant_words(bytes));
}

fn identity_message_words(public_inputs: &ZkAcePublicInputsV1) -> Vec<MessageWord> {
    let mut words = Vec::new();
    let hash_domain = b"zk-ace.identity-commitment.v1";
    words.push(MessageWord::Constant(hash_domain.len() as u64));
    words.extend(bytes_as_constant_words(hash_domain));
    words.push(MessageWord::Constant(3));
    words.push(MessageWord::Constant(32));
    words.extend((0..5).map(MessageWord::Witness));
    words.push(MessageWord::Constant(32));
    words.extend((5..10).map(MessageWord::Witness));
    append_framed_constant_part(&mut words, public_inputs.domain_tag.as_bytes());
    words
}

fn replay_message_words(public_inputs: &ZkAcePublicInputsV1) -> Vec<MessageWord> {
    let mut words = Vec::new();
    let hash_domain = b"zk-ace.replay-nullifier.v1";
    words.push(MessageWord::Constant(hash_domain.len() as u64));
    words.extend(bytes_as_constant_words(hash_domain));
    words.push(MessageWord::Constant(5));
    words.push(MessageWord::Constant(32));
    words.extend((10..15).map(MessageWord::Witness));
    append_framed_constant_part(&mut words, &public_inputs.authorization_digest);
    append_framed_constant_part(&mut words, public_inputs.chain_id.as_str().as_bytes());
    append_framed_constant_part(&mut words, public_inputs.action_class.as_bytes());
    append_framed_constant_part(&mut words, public_inputs.domain_tag.as_bytes());
    words
}

fn append_poseidon_permutation(schedule: &mut Vec<ScheduleRow>) {
    for round in 0..POSEIDON_ROUNDS {
        let full = round < POSEIDON_FULL_ROUNDS_HALF
            || round >= POSEIDON_FULL_ROUNDS_HALF + 57;
        schedule.push(ScheduleRow {
            op: if full {
                ScheduleOp::FullRound { round }
            } else {
                ScheduleOp::PartialRound { round }
            },
        });
    }
}

fn append_poseidon_hash(
    schedule: &mut Vec<ScheduleRow>,
    words: &[MessageWord],
    output_offset: usize,
) {
    let mut rate_index = 0usize;
    for word in words.iter().copied() {
        schedule.push(ScheduleRow {
            op: ScheduleOp::Absorb {
                position: rate_index,
                word,
            },
        });
        rate_index += 1;
        if rate_index == 2 {
            append_poseidon_permutation(schedule);
            rate_index = 0;
        }
    }

    schedule.push(ScheduleRow {
        op: ScheduleOp::Absorb {
            position: rate_index,
            word: MessageWord::Constant(1),
        },
    });
    rate_index += 1;
    if rate_index == 2 {
        append_poseidon_permutation(schedule);
        rate_index = 0;
    }
    while rate_index != 0 {
        schedule.push(ScheduleRow {
            op: ScheduleOp::Absorb {
                position: rate_index,
                word: MessageWord::Constant(0),
            },
        });
        rate_index += 1;
        if rate_index == 2 {
            append_poseidon_permutation(schedule);
            rate_index = 0;
        }
    }

    for output_index in 0..4 {
        schedule.push(ScheduleRow {
            op: ScheduleOp::Output {
                output_index: output_offset + output_index,
            },
        });
        if output_index != 3 {
            append_poseidon_permutation(schedule);
        }
    }
}

fn build_schedule(public_inputs: &ZkAcePublicInputsV1) -> Result<Vec<ScheduleRow>, ZkAceStarkError> {
    let mut schedule = Vec::with_capacity(TRACE_SIZE);
    for index in 0..PRIVATE_LIMBS {
        schedule.push(ScheduleRow {
            op: ScheduleOp::Load(index),
        });
    }
    schedule.push(ScheduleRow {
        op: ScheduleOp::Reset,
    });
    append_poseidon_hash(&mut schedule, &identity_message_words(public_inputs), 0);
    schedule.push(ScheduleRow {
        op: ScheduleOp::Reset,
    });
    append_poseidon_hash(&mut schedule, &replay_message_words(public_inputs), 4);

    if schedule.len() >= TRACE_SIZE {
        return Err(ZkAceStarkError::InternalInvariant(
            "compiled ZK-ACE schedule exceeds its trace domain",
        ));
    }
    schedule.resize(
        TRACE_SIZE,
        ScheduleRow {
            op: ScheduleOp::Hold,
        },
    );
    Ok(schedule)
}

fn witness_limbs(witness: &ZkAceWitnessV1) -> Result<[F; PRIVATE_LIMBS], ZkAceStarkError> {
    let mut result = [F::ZERO; PRIVATE_LIMBS];
    for (group, bytes) in [
        witness.identity_root,
        witness.identity_blinding,
        witness.replay_secret,
    ]
    .iter()
    .enumerate()
    {
        let packed = zk_ace_pack_bytes_to_field_limbs(bytes);
        if packed.length != 32 || packed.limbs.len() != 5 {
            return Err(ZkAceStarkError::WitnessPacking);
        }
        for (offset, limb) in packed.limbs.into_iter().enumerate() {
            result[group * 5 + offset] =
                F::canonical(limb).ok_or(ZkAceStarkError::WitnessPacking)?;
        }
    }
    Ok(result)
}

fn public_output_words(public_inputs: &ZkAcePublicInputsV1) -> Result<[F; 8], ZkAceStarkError> {
    let mut words = [F::ZERO; 8];
    for (word_index, chunk) in public_inputs
        .identity_commitment
        .chunks_exact(8)
        .chain(public_inputs.replay_nullifier.chunks_exact(8))
        .enumerate()
    {
        let raw = u64::from_le_bytes(
            chunk
                .try_into()
                .expect("chunks_exact produces eight-byte digest words"),
        );
        words[word_index] =
            F::canonical(raw).ok_or(ZkAceStarkError::NonCanonicalPublicDigest)?;
    }
    Ok(words)
}

fn apply_mds(state: [F; 3]) -> [F; 3] {
    let mut result = [F::ZERO; 3];
    for row in 0..3 {
        for (column, value) in state.iter().copied().enumerate() {
            result[row] = result[row].add(F(MDS[row][column]).mul(value));
        }
    }
    result
}

fn trace_row(
    state: [F; 3],
    queue: [F; PRIVATE_LIMBS],
    limb: F,
    round_constants: [F; 3],
) -> Vec<F> {
    let mut row = vec![F::ZERO; TRACE_WIDTH];
    row[STATE_OFFSET..STATE_OFFSET + 3].copy_from_slice(&state);
    row[QUEUE_OFFSET..QUEUE_OFFSET + PRIVATE_LIMBS].copy_from_slice(&queue);
    row[LIMB_OFFSET] = limb;
    for bit in 0..LIMB_BITS {
        row[BIT_OFFSET + bit] = F((limb.0 >> bit) & 1);
    }
    for index in 0..3 {
        let a = state[index].add(round_constants[index]);
        let x2 = a.mul(a);
        let x4 = x2.mul(x2);
        let x5 = x4.mul(a);
        row[A_OFFSET + index] = a;
        row[X2_OFFSET + index] = x2;
        row[X4_OFFSET + index] = x4;
        row[X5_OFFSET + index] = x5;
    }
    row
}

fn fixed_row(schedule: ScheduleRow) -> Vec<F> {
    let mut fixed = vec![F::ZERO; FIXED_WIDTH];
    match schedule.op {
        ScheduleOp::Hold => {}
        ScheduleOp::Reset => fixed[FIX_RESET] = F::ONE,
        ScheduleOp::Load(index) => fixed[FIX_LOAD_OFFSET + index] = F::ONE,
        ScheduleOp::Absorb { position, word } => {
            fixed[if position == 0 {
                FIX_ABSORB_0
            } else {
                FIX_ABSORB_1
            }] = F::ONE;
            match word {
                MessageWord::Constant(value) => fixed[FIX_MESSAGE_CONST] = F(value),
                MessageWord::Witness(index) => {
                    fixed[FIX_MESSAGE_WITNESS_OFFSET + index] = F::ONE;
                }
            }
        }
        ScheduleOp::FullRound { round } => {
            fixed[FIX_FULL] = F::ONE;
            for index in 0..3 {
                fixed[FIX_RC_OFFSET + index] = F(ROUND_CONSTANTS[round][index]);
            }
        }
        ScheduleOp::PartialRound { round } => {
            fixed[FIX_PARTIAL] = F::ONE;
            for index in 0..3 {
                fixed[FIX_RC_OFFSET + index] = F(ROUND_CONSTANTS[round][index]);
            }
        }
        ScheduleOp::Output { output_index } => {
            fixed[FIX_OUTPUT_OFFSET + output_index] = F::ONE;
        }
    }
    fixed
}

fn build_trace_material(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<TraceMaterial, ZkAceStarkError> {
    if public_inputs.domain_tag != ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG
        || public_inputs.action_class != ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER
    {
        return Err(ZkAceStarkError::WitnessRelation);
    }
    let schedule = build_schedule(public_inputs)?;
    let witness_limbs = witness_limbs(witness)?;
    let public_outputs = public_output_words(public_inputs)?;
    let mut trace_rows = Vec::with_capacity(TRACE_SIZE);
    let mut fixed_rows = Vec::with_capacity(TRACE_SIZE);
    let mut state = [F::ZERO; 3];
    let mut queue = [F::ZERO; PRIVATE_LIMBS];

    for schedule_row in schedule.iter().copied() {
        let fixed = fixed_row(schedule_row);
        let round_constants = [
            fixed[FIX_RC_OFFSET],
            fixed[FIX_RC_OFFSET + 1],
            fixed[FIX_RC_OFFSET + 2],
        ];
        let limb = match schedule_row.op {
            ScheduleOp::Load(index) => witness_limbs[index],
            _ => F::ZERO,
        };
        let row = trace_row(state, queue, limb, round_constants);

        match schedule_row.op {
            ScheduleOp::Hold | ScheduleOp::Output { .. } => {}
            ScheduleOp::Reset => state = [F::ZERO; 3],
            ScheduleOp::Load(index) => queue[index] = limb,
            ScheduleOp::Absorb { position, word } => {
                let message = match word {
                    MessageWord::Constant(value) => F(value),
                    MessageWord::Witness(index) => queue[index],
                };
                state[position] = state[position].add(message);
            }
            ScheduleOp::FullRound { .. } => {
                state = apply_mds([
                    row[X5_OFFSET],
                    row[X5_OFFSET + 1],
                    row[X5_OFFSET + 2],
                ]);
            }
            ScheduleOp::PartialRound { .. } => {
                state = apply_mds([row[X5_OFFSET], row[A_OFFSET + 1], row[A_OFFSET + 2]]);
            }
        }
        if let ScheduleOp::Output { output_index } = schedule_row.op {
            if row[STATE_OFFSET] != public_outputs[output_index] {
                return Err(ZkAceStarkError::WitnessRelation);
            }
        }
        trace_rows.push(row);
        fixed_rows.push(fixed);
    }

    Ok(TraceMaterial {
        trace_columns: transpose_rows(&trace_rows, TRACE_WIDTH)?,
        fixed_columns: transpose_rows(&fixed_rows, FIXED_WIDTH)?,
        public_outputs,
    })
}

fn random_field<R: TryRngCore>(rng: &mut R) -> Result<F, ZkAceStarkError> {
    for _ in 0..16 {
        let mut bytes = [0u8; 8];
        rng.try_fill_bytes(&mut bytes)
            .map_err(|_| ZkAceStarkError::RandomnessUnavailable)?;
        if let Some(value) = F::canonical(u64::from_le_bytes(bytes)) {
            return Ok(value);
        }
    }
    Err(ZkAceStarkError::RandomnessUnavailable)
}

fn masked_lde_columns<R: TryRngCore>(
    base_columns: &[Vec<F>],
    rng: &mut R,
) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    let trace_root = primitive_root(TRACE_LOG2)?;
    let lde_root = primitive_root(LDE_LOG2)?;
    let coset_shift = F(FIELD_GENERATOR);
    if coset_shift.pow(TRACE_SIZE as u128) == F::ONE {
        return Err(ZkAceStarkError::InternalInvariant(
            "compiled LDE coset intersects the trace subgroup",
        ));
    }
    base_columns
        .iter()
        .map(|column| {
            if column.len() != TRACE_SIZE {
                return Err(ZkAceStarkError::InternalInvariant(
                    "base trace column length mismatch",
                ));
            }
            let mut coefficients = column.clone();
            ifft(&mut coefficients, trace_root)?;
            coefficients.resize(LDE_SIZE, F::ZERO);
            for degree in 0..=MASK_DEGREE {
                let random = random_field(rng)?;
                coefficients[degree] = coefficients[degree].sub(random);
                coefficients[TRACE_SIZE + degree] =
                    coefficients[TRACE_SIZE + degree].add(random);
            }
            evaluate_coefficients_on_coset(&coefficients, LDE_SIZE, lde_root, coset_shift)
        })
        .collect()
}

fn fixed_lde_columns(base_columns: &[Vec<F>]) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    let trace_root = primitive_root(TRACE_LOG2)?;
    let lde_root = primitive_root(LDE_LOG2)?;
    let coset_shift = F(FIELD_GENERATOR);
    base_columns
        .iter()
        .map(|column| {
            if column.len() != TRACE_SIZE {
                return Err(ZkAceStarkError::InternalInvariant(
                    "base fixed column length mismatch",
                ));
            }
            let mut coefficients = column.clone();
            ifft(&mut coefficients, trace_root)?;
            evaluate_coefficients_on_coset(&coefficients, LDE_SIZE, lde_root, coset_shift)
        })
        .collect()
}

fn row_at(columns: &[Vec<F>], index: usize) -> Result<Vec<F>, ZkAceStarkError> {
    columns
        .iter()
        .map(|column| {
            column
                .get(index)
                .copied()
                .ok_or(ZkAceStarkError::InternalInvariant(
                    "column opening index out of range",
                ))
        })
        .collect()
}

