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

use fastpq_prover::poseidon_manifest;
use iroha_data_model::zk::{
    ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
    ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
    ZkAcePublicInputsV1, ZkAceWitnessV1, derive_zk_ace_air_public_digest,
    derive_zk_ace_transfer_digest, zk_ace_pack_bytes_to_field_limbs,
};
use norito::{Decode, Encode};
use rand::TryRngCore;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

#[cfg(test)]
static PROOF_TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn proof_test_guard() -> std::sync::MutexGuard<'static, ()> {
    PROOF_TEST_MUTEX
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("ZK-ACE proof test mutex must not be poisoned")
}

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
/// Hard ceiling enforced before Norito decoding allocates proof vectors.
pub(crate) const MAX_PROOF_BYTES: usize = 2 * 1024 * 1024;
/// Complete consensus-relevant algebraic and commitment profile.
pub(crate) const COMPILED_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"version=1|field=goldilocks:0xffffffff00000001|generator=7|poseidon2=width3:rate2:full8:partial57|trace_rows=4096|trace_width=88|trace_mask_degree=255|lde_rows=65536|blowup=16|constraint_lanes=3|queries=32|merkle=sha256:binary|fri=fold2:rounds12:terminal16:degree1|max_proof_bytes=2097152|decode=max_sequence88:max_total_elements65536:max_alloc8388608:max_depth32|domains=iroha:privacy:zk-ace:{transparent-stark,trace-leaf,composition-leaf,fri-leaf,merkle-node,field-challenge,composition-transcript,fri-lane-transcript,fri-round-transcript,query-transcript,query-index}:v1";
/// Untrusted proof decoding is bounded independently of the byte ceiling.
///
/// A short archive can advertise a hostile vector count, so enforcing only
/// `MAX_PROOF_BYTES` before decoding is insufficient. These limits exceed the
/// exact canonical shape while keeping every allocation and nested sequence
/// inside a small, deterministic budget.
const PROOF_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    TRACE_WIDTH,
    MAX_PROOF_BYTES,
    65_536,
    8 * MAX_PROOF_BYTES,
    32,
);
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
const POSEIDON_ROUNDS: usize = 65;
const PROOF_VERSION: u16 = 1;
const MAX_QUERY_DERIVATION_ATTEMPTS: usize = LDE_SIZE * 2;

const STATE_OFFSET: usize = 0;
const A_OFFSET: usize = STATE_OFFSET + 3;
const X2_OFFSET: usize = A_OFFSET + 3;
const X4_OFFSET: usize = X2_OFFSET + 3;
const X5_OFFSET: usize = X4_OFFSET + 3;
const QUEUE_OFFSET: usize = X5_OFFSET + 3;
const LIMB_OFFSET: usize = QUEUE_OFFSET + PRIVATE_LIMBS;
const MESSAGE_OFFSET: usize = LIMB_OFFSET + 1;
const BIT_OFFSET: usize = MESSAGE_OFFSET + 1;
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
            let previous = levels.last().expect("non-empty Merkle level collection");
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
    #[error("ZK-ACE public inputs do not match the compiled transfer relation")]
    InvalidPublicInputs,
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
    #[error("ZK-ACE proof exceeds the compiled byte ceiling")]
    ProofTooLarge,
    #[error("ZK-ACE proof is malformed")]
    MalformedProof,
    #[error("ZK-ACE proof is not a canonical Norito encoding")]
    NonCanonicalProof,
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
        let full = round < POSEIDON_FULL_ROUNDS_HALF || round >= POSEIDON_FULL_ROUNDS_HALF + 57;
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

fn build_schedule(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<Vec<ScheduleRow>, ZkAceStarkError> {
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
        words[word_index] = F::canonical(raw).ok_or(ZkAceStarkError::NonCanonicalPublicDigest)?;
    }
    Ok(words)
}

fn apply_mds(state: [F; 3]) -> [F; 3] {
    let mds = poseidon_manifest().mds();
    let mut result = [F::ZERO; 3];
    for row in 0..3 {
        for (column, value) in state.iter().copied().enumerate() {
            result[row] = result[row].add(F(mds[row][column]).mul(value));
        }
    }
    result
}

fn trace_row(
    state: [F; 3],
    queue: [F; PRIVATE_LIMBS],
    limb: F,
    message: F,
    round_constants: [F; 3],
) -> Vec<F> {
    let mut row = vec![F::ZERO; TRACE_WIDTH];
    row[STATE_OFFSET..STATE_OFFSET + 3].copy_from_slice(&state);
    row[QUEUE_OFFSET..QUEUE_OFFSET + PRIVATE_LIMBS].copy_from_slice(&queue);
    row[LIMB_OFFSET] = limb;
    row[MESSAGE_OFFSET] = message;
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
                fixed[FIX_RC_OFFSET + index] =
                    F(poseidon_manifest().round_constants()[round][index]);
            }
        }
        ScheduleOp::PartialRound { round } => {
            fixed[FIX_PARTIAL] = F::ONE;
            for index in 0..3 {
                fixed[FIX_RC_OFFSET + index] =
                    F(poseidon_manifest().round_constants()[round][index]);
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
        let message = match schedule_row.op {
            ScheduleOp::Absorb { word, .. } => match word {
                MessageWord::Constant(value) => F(value),
                MessageWord::Witness(index) => queue[index],
            },
            _ => F::ZERO,
        };
        let row = trace_row(state, queue, limb, message, round_constants);

        match schedule_row.op {
            ScheduleOp::Hold | ScheduleOp::Output { .. } => {}
            ScheduleOp::Reset => state = [F::ZERO; 3],
            ScheduleOp::Load(index) => queue[index] = limb,
            ScheduleOp::Absorb { position, .. } => {
                state[position] = state[position].add(message);
            }
            ScheduleOp::FullRound { .. } => {
                state = apply_mds([row[X5_OFFSET], row[X5_OFFSET + 1], row[X5_OFFSET + 2]]);
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
    if coset_shift.pow(LDE_SIZE as u128) == F::ONE || coset_shift.pow(TRACE_SIZE as u128) == F::ONE
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "compiled LDE shift lies in an evaluation subgroup",
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
                coefficients[TRACE_SIZE + degree] = coefficients[TRACE_SIZE + degree].add(random);
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

fn batch_invert(values: &mut [F]) -> Result<(), ZkAceStarkError> {
    let mut prefixes = Vec::with_capacity(values.len());
    let mut product = F::ONE;
    for value in values.iter().copied() {
        if value == F::ZERO {
            return Err(ZkAceStarkError::InternalInvariant(
                "batch inversion input must be non-zero",
            ));
        }
        prefixes.push(product);
        product = product.mul(value);
    }
    let mut inverse = product.inv().ok_or(ZkAceStarkError::InternalInvariant(
        "batch inversion product must be non-zero",
    ))?;
    for index in (0..values.len()).rev() {
        let value = values[index];
        values[index] = inverse.mul(prefixes[index]);
        inverse = inverse.mul(value);
    }
    Ok(())
}

fn accumulate_fixed_row(result: &mut [F], schedule_row: ScheduleRow, weight: F) {
    let mut add = |index: usize, value: F| {
        result[index] = result[index].add(weight.mul(value));
    };
    match schedule_row.op {
        ScheduleOp::Hold => {}
        ScheduleOp::Reset => add(FIX_RESET, F::ONE),
        ScheduleOp::Load(index) => add(FIX_LOAD_OFFSET + index, F::ONE),
        ScheduleOp::Absorb { position, word } => {
            add(
                if position == 0 {
                    FIX_ABSORB_0
                } else {
                    FIX_ABSORB_1
                },
                F::ONE,
            );
            match word {
                MessageWord::Constant(value) => {
                    add(FIX_MESSAGE_CONST, F(value));
                }
                MessageWord::Witness(index) => {
                    add(FIX_MESSAGE_WITNESS_OFFSET + index, F::ONE);
                }
            }
        }
        ScheduleOp::FullRound { round } | ScheduleOp::PartialRound { round } => {
            add(
                if matches!(schedule_row.op, ScheduleOp::FullRound { .. }) {
                    FIX_FULL
                } else {
                    FIX_PARTIAL
                },
                F::ONE,
            );
            for index in 0..3 {
                add(
                    FIX_RC_OFFSET + index,
                    F(poseidon_manifest().round_constants()[round][index]),
                );
            }
        }
        ScheduleOp::Output { output_index } => {
            add(FIX_OUTPUT_OFFSET + output_index, F::ONE);
        }
    }
}

/// Evaluate all fixed schedule columns at one non-trace-domain point.
///
/// Verification needs only the transcript-selected query rows. Evaluating the
/// Lagrange basis here avoids allocating and FFT-expanding a 47-column,
/// 65,536-row fixed table for every admitted proof.
fn fixed_row_at_point(schedule: &[ScheduleRow], x: F) -> Result<Vec<F>, ZkAceStarkError> {
    if schedule.len() != TRACE_SIZE || x.pow(TRACE_SIZE as u128) == F::ONE {
        return Err(ZkAceStarkError::InternalInvariant(
            "fixed-row evaluation point has invalid shape/domain",
        ));
    }
    let trace_root = primitive_root(TRACE_LOG2)?;
    let mut trace_points = Vec::with_capacity(TRACE_SIZE);
    let mut denominators = Vec::with_capacity(TRACE_SIZE);
    let mut point = F::ONE;
    for _ in 0..TRACE_SIZE {
        trace_points.push(point);
        denominators.push(x.sub(point));
        point = point.mul(trace_root);
    }
    batch_invert(&mut denominators)?;
    let inverse_trace_size =
        F::reduce(TRACE_SIZE as u128)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "trace size must be invertible",
            ))?;
    let common = x
        .pow(TRACE_SIZE as u128)
        .sub(F::ONE)
        .mul(inverse_trace_size);
    let mut result = vec![F::ZERO; FIXED_WIDTH];
    for ((schedule_row, trace_point), inverse_denominator) in
        schedule.iter().copied().zip(trace_points).zip(denominators)
    {
        // Z_H'(h_i) = T / h_i, hence
        // L_i(x) = Z_H(x) * h_i / (T * (x - h_i)).
        let weight = common.mul(trace_point).mul(inverse_denominator);
        accumulate_fixed_row(&mut result, schedule_row, weight);
    }
    Ok(result)
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

const LOCAL_CONSTRAINT_COUNT: usize = 12 + LIMB_BITS + 1 + 1 + 8 + 3 * (LIMB_BITS - 32);
const TRANSITION_CONSTRAINT_COUNT: usize = 3 + PRIVATE_LIMBS;
const CONSTRAINT_COUNT: usize = LOCAL_CONSTRAINT_COUNT + TRANSITION_CONSTRAINT_COUNT;

fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn base_transcript_seed(public_digest: &[u8; 32], trace_root: &[u8; 32]) -> [u8; 32] {
    hash_parts(
        TRANSCRIPT_DOMAIN,
        &[
            &PROOF_VERSION.to_be_bytes(),
            &[TRACE_LOG2, BLOWUP_LOG2],
            &(QUERY_COUNT as u64).to_be_bytes(),
            &(SECURITY_LANES as u64).to_be_bytes(),
            &(MASK_DEGREE as u64).to_be_bytes(),
            public_digest,
            trace_root,
        ],
    )
}

fn challenge_field(seed: &[u8; 32], label: &[u8], lane: usize, index: usize) -> F {
    let digest = hash_parts(
        b"iroha:privacy:zk-ace:field-challenge:v1",
        &[
            seed,
            label,
            &(lane as u64).to_be_bytes(),
            &(index as u64).to_be_bytes(),
        ],
    );
    let value = u128::from_le_bytes(
        digest[..16]
            .try_into()
            .expect("SHA-256 prefix has sixteen bytes"),
    );
    let reduced = F::reduce(value);
    if reduced == F::ZERO { F::ONE } else { reduced }
}

fn challenge_vector(seed: &[u8; 32], label: &[u8], lane: usize, count: usize) -> Vec<F> {
    (0..count)
        .map(|index| challenge_field(seed, label, lane, index))
        .collect()
}

fn composition_seed(base_seed: &[u8; 32], composition_roots: &[[u8; 32]]) -> [u8; 32] {
    let mut encoded_roots = Vec::with_capacity(composition_roots.len() * 32);
    for root in composition_roots {
        encoded_roots.extend_from_slice(root);
    }
    hash_parts(
        b"iroha:privacy:zk-ace:composition-transcript:v1",
        &[base_seed, &encoded_roots],
    )
}

fn fri_lane_seed(composition_seed: &[u8; 32], lane: usize) -> [u8; 32] {
    hash_parts(
        b"iroha:privacy:zk-ace:fri-lane-transcript:v1",
        &[composition_seed, &(lane as u64).to_be_bytes()],
    )
}

fn fri_beta(lane_seed: &[u8; 32], lane: usize, round: usize, layer_root: &[u8; 32]) -> F {
    let seed = hash_parts(
        b"iroha:privacy:zk-ace:fri-round-transcript:v1",
        &[
            lane_seed,
            &(lane as u64).to_be_bytes(),
            &(round as u64).to_be_bytes(),
            layer_root,
        ],
    );
    challenge_field(&seed, b"fri-beta", lane, round)
}

fn query_seed_from_roots(composition_seed: &[u8; 32], lane_roots: &[Vec<[u8; 32]>]) -> [u8; 32] {
    let mut encoded_roots = Vec::new();
    for roots in lane_roots {
        for root in roots {
            encoded_roots.extend_from_slice(root);
        }
    }
    hash_parts(
        b"iroha:privacy:zk-ace:query-transcript:v1",
        &[composition_seed, &encoded_roots],
    )
}

fn derive_query_indices(seed: &[u8; 32]) -> Result<Vec<usize>, ZkAceStarkError> {
    let mut indices = Vec::with_capacity(QUERY_COUNT);
    let mut seen = BTreeSet::new();
    for counter in 0..MAX_QUERY_DERIVATION_ATTEMPTS {
        let digest = hash_parts(
            b"iroha:privacy:zk-ace:query-index:v1",
            &[seed, &(counter as u64).to_be_bytes()],
        );
        let raw = u64::from_le_bytes(
            digest[..8]
                .try_into()
                .expect("SHA-256 prefix has eight bytes"),
        );
        let index = (raw as usize) & (LDE_SIZE - 1);
        if seen.insert(index) {
            indices.push(index);
            if indices.len() == QUERY_COUNT {
                return Ok(indices);
            }
        }
    }
    Err(ZkAceStarkError::InternalInvariant(
        "query sampler exhausted its compiled attempt bound",
    ))
}

fn constraint_quotient_value(
    x: F,
    current: &[F],
    next: &[F],
    fixed: &[F],
    public_outputs: &[F; 8],
    alphas: &[F],
) -> Result<F, ZkAceStarkError> {
    let (inverse_trace_vanishing, transition_factor) = constraint_quotient_factors(x)?;
    constraint_quotient_value_with_factors(
        current,
        next,
        fixed,
        public_outputs,
        alphas,
        inverse_trace_vanishing,
        transition_factor,
    )
}

fn constraint_quotient_factors(x: F) -> Result<(F, F), ZkAceStarkError> {
    let z_h = x.pow(TRACE_SIZE as u128).sub(F::ONE);
    let inverse_trace_vanishing = z_h.inv().ok_or(ZkAceStarkError::InternalInvariant(
        "LDE point lies in the trace subgroup",
    ))?;
    let trace_root = primitive_root(TRACE_LOG2)?;
    let last_trace_point = trace_root.pow((TRACE_SIZE - 1) as u128);
    let transition_factor = x.sub(last_trace_point).mul(inverse_trace_vanishing);
    Ok((inverse_trace_vanishing, transition_factor))
}

fn constraint_quotient_value_with_factors(
    current: &[F],
    next: &[F],
    fixed: &[F],
    public_outputs: &[F; 8],
    alphas: &[F],
    inverse_trace_vanishing: F,
    transition_factor: F,
) -> Result<F, ZkAceStarkError> {
    if current.len() != TRACE_WIDTH
        || next.len() != TRACE_WIDTH
        || fixed.len() != FIXED_WIDTH
        || alphas.len() != CONSTRAINT_COUNT
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "constraint evaluation shape mismatch",
        ));
    }
    let mut alpha_index = 0usize;
    let mut result = F::ZERO;
    let mut absorb_local = |residue: F| {
        result = result.add(
            alphas[alpha_index]
                .mul(residue)
                .mul(inverse_trace_vanishing),
        );
        alpha_index += 1;
    };

    for word in 0..3 {
        absorb_local(
            current[A_OFFSET + word]
                .sub(current[STATE_OFFSET + word])
                .sub(fixed[FIX_RC_OFFSET + word]),
        );
        absorb_local(
            current[X2_OFFSET + word].sub(current[A_OFFSET + word].mul(current[A_OFFSET + word])),
        );
        absorb_local(
            current[X4_OFFSET + word].sub(current[X2_OFFSET + word].mul(current[X2_OFFSET + word])),
        );
        absorb_local(
            current[X5_OFFSET + word].sub(current[X4_OFFSET + word].mul(current[A_OFFSET + word])),
        );
    }
    for bit in 0..LIMB_BITS {
        let value = current[BIT_OFFSET + bit];
        absorb_local(value.mul(value.sub(F::ONE)));
    }
    let recomposed = (0..LIMB_BITS).fold(F::ZERO, |sum, bit| {
        sum.add(current[BIT_OFFSET + bit].mul(F::reduce(1_u128 << bit)))
    });
    absorb_local(current[LIMB_OFFSET].sub(recomposed));
    let mut expected_message = fixed[FIX_MESSAGE_CONST];
    for index in 0..PRIVATE_LIMBS {
        expected_message = expected_message
            .add(fixed[FIX_MESSAGE_WITNESS_OFFSET + index].mul(current[QUEUE_OFFSET + index]));
    }
    absorb_local(current[MESSAGE_OFFSET].sub(expected_message));
    for output in 0..8 {
        absorb_local(
            fixed[FIX_OUTPUT_OFFSET + output]
                .mul(current[STATE_OFFSET].sub(public_outputs[output])),
        );
    }
    for limb_index in [4usize, 9, 14] {
        for bit in 32..LIMB_BITS {
            absorb_local(fixed[FIX_LOAD_OFFSET + limb_index].mul(current[BIT_OFFSET + bit]));
        }
    }
    if alpha_index != LOCAL_CONSTRAINT_COUNT {
        return Err(ZkAceStarkError::InternalInvariant(
            "local constraint count drifted from the profile",
        ));
    }

    let full = fixed[FIX_FULL];
    let partial = fixed[FIX_PARTIAL];
    let absorb_0 = fixed[FIX_ABSORB_0];
    let absorb_1 = fixed[FIX_ABSORB_1];
    let reset = fixed[FIX_RESET];
    let hold = F::ONE
        .sub(full)
        .sub(partial)
        .sub(absorb_0)
        .sub(absorb_1)
        .sub(reset);
    let full_state = apply_mds([
        current[X5_OFFSET],
        current[X5_OFFSET + 1],
        current[X5_OFFSET + 2],
    ]);
    let partial_state = apply_mds([
        current[X5_OFFSET],
        current[A_OFFSET + 1],
        current[A_OFFSET + 2],
    ]);
    for word in 0..3 {
        let expected = full
            .mul(full_state[word])
            .add(partial.mul(partial_state[word]))
            .add(absorb_0.add(absorb_1).mul(current[STATE_OFFSET + word]))
            .add(if word == 0 {
                absorb_0.mul(current[MESSAGE_OFFSET])
            } else if word == 1 {
                absorb_1.mul(current[MESSAGE_OFFSET])
            } else {
                F::ZERO
            })
            .add(hold.mul(current[STATE_OFFSET + word]));
        let residue = next[STATE_OFFSET + word].sub(expected);
        result = result.add(alphas[alpha_index].mul(residue).mul(transition_factor));
        alpha_index += 1;
    }
    for index in 0..PRIVATE_LIMBS {
        let queue = current[QUEUE_OFFSET + index];
        let expected =
            queue.add(fixed[FIX_LOAD_OFFSET + index].mul(current[LIMB_OFFSET].sub(queue)));
        let residue = next[QUEUE_OFFSET + index].sub(expected);
        result = result.add(alphas[alpha_index].mul(residue).mul(transition_factor));
        alpha_index += 1;
    }
    if alpha_index != CONSTRAINT_COUNT {
        return Err(ZkAceStarkError::InternalInvariant(
            "transition constraint count drifted from the profile",
        ));
    }
    Ok(result)
}

fn trace_tree(trace_lde: &[Vec<F>]) -> Result<MerkleTree, ZkAceStarkError> {
    let leaves = (0..LDE_SIZE)
        .map(|index| row_at(trace_lde, index).map(|row| trace_leaf_hash(&row)))
        .collect::<Result<Vec<_>, _>>()?;
    MerkleTree::from_leaves(leaves)
}

fn composition_lanes(
    trace_lde: &[Vec<F>],
    fixed_lde: &[Vec<F>],
    public_outputs: &[F; 8],
    lane_alphas: &[Vec<F>],
) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    if lane_alphas.len() != SECURITY_LANES
        || lane_alphas
            .iter()
            .any(|alphas| alphas.len() != CONSTRAINT_COUNT)
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "composition lane challenge shape mismatch",
        ));
    }
    let lde_root = primitive_root(LDE_LOG2)?;
    let coset_shift = F(FIELD_GENERATOR);
    let trace_root = primitive_root(TRACE_LOG2)?;
    let last_trace_point = trace_root.pow((TRACE_SIZE - 1) as u128);
    // `x^TRACE_SIZE` repeats every blow-up factor along the LDE
    // domain, so only sixteen vanishing-polynomial inversions are needed.
    let mut inverse_vanishing_by_residue = Vec::with_capacity(TERMINAL_SIZE);
    let mut residue_point = coset_shift;
    for _ in 0..TERMINAL_SIZE {
        inverse_vanishing_by_residue.push(
            residue_point
                .pow(TRACE_SIZE as u128)
                .sub(F::ONE)
                .inv()
                .ok_or(ZkAceStarkError::InternalInvariant(
                    "LDE coset residue lies in the trace subgroup",
                ))?,
        );
        residue_point = residue_point.mul(lde_root);
    }
    let mut x = coset_shift;
    let mut lanes = (0..SECURITY_LANES)
        .map(|_| Vec::with_capacity(LDE_SIZE))
        .collect::<Vec<_>>();
    for index in 0..LDE_SIZE {
        let current = row_at(trace_lde, index)?;
        let next = row_at(trace_lde, (index + TERMINAL_SIZE) % LDE_SIZE)?;
        let fixed = row_at(fixed_lde, index)?;
        let inverse_trace_vanishing = inverse_vanishing_by_residue[index % TERMINAL_SIZE];
        let transition_factor = x.sub(last_trace_point).mul(inverse_trace_vanishing);
        for lane in 0..SECURITY_LANES {
            lanes[lane].push(constraint_quotient_value_with_factors(
                &current,
                &next,
                &fixed,
                public_outputs,
                &lane_alphas[lane],
                inverse_trace_vanishing,
                transition_factor,
            )?);
        }
        x = x.mul(lde_root);
    }
    Ok(lanes)
}

fn mix_fri_base(
    trace_lde: &[Vec<F>],
    composition: &[F],
    trace_mix: &[F],
    composition_mix: F,
) -> Result<Vec<F>, ZkAceStarkError> {
    if trace_lde.len() != TRACE_WIDTH
        || trace_mix.len() != TRACE_WIDTH
        || composition.len() != LDE_SIZE
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "FRI base mixing shape mismatch",
        ));
    }
    (0..LDE_SIZE)
        .map(|index| {
            let trace_value = trace_lde
                .iter()
                .zip(trace_mix)
                .fold(F::ZERO, |sum, (column, coefficient)| {
                    sum.add(column[index].mul(*coefficient))
                });
            Ok(trace_value.add(composition[index].mul(composition_mix)))
        })
        .collect()
}

fn fri_fold_pair(low: F, high: F, beta: F, x: F) -> Result<F, ZkAceStarkError> {
    let inverse_x = x.inv().ok_or(ZkAceStarkError::InternalInvariant(
        "FRI domain point must be invertible",
    ))?;
    fri_fold_pair_with_inverse_x(low, high, beta, inverse_x)
}

fn fri_fold_pair_with_inverse_x(
    low: F,
    high: F,
    beta: F,
    inverse_x: F,
) -> Result<F, ZkAceStarkError> {
    let two_inverse = F(2).inv().ok_or(ZkAceStarkError::InternalInvariant(
        "two must be invertible in Goldilocks",
    ))?;
    let even = low.add(high).mul(two_inverse);
    let odd = low.sub(high).mul(two_inverse).mul(inverse_x);
    Ok(even.add(beta.mul(odd)))
}

fn build_fri_lane(
    base_values: Vec<F>,
    lane_seed: &[u8; 32],
    lane: usize,
) -> Result<FriLaneMaterial, ZkAceStarkError> {
    if base_values.len() != LDE_SIZE {
        return Err(ZkAceStarkError::InternalInvariant(
            "FRI base vector length mismatch",
        ));
    }
    let mut layers = vec![base_values];
    let mut trees = Vec::with_capacity(FRI_ROUNDS + 1);
    let mut roots = Vec::with_capacity(FRI_ROUNDS + 1);
    let mut domain_shift = F(FIELD_GENERATOR);
    let mut domain_root = primitive_root(LDE_LOG2)?;

    for round in 0..FRI_ROUNDS {
        let current = layers
            .last()
            .expect("FRI starts with one base evaluation layer");
        let leaves = current
            .iter()
            .copied()
            .map(|value| fri_leaf_hash(lane, round, value))
            .collect();
        let tree = MerkleTree::from_leaves(leaves)?;
        let root = tree.root();
        // Each folding challenge is sampled only after the layer it
        // challenges has been committed.  Precomputing all betas before these
        // roots exist would let a malicious prover adapt the layer to its
        // challenge.
        let beta = fri_beta(lane_seed, lane, round, &root);
        let half = current.len() / 2;
        let mut next = Vec::with_capacity(half);
        let mut inverse_x = domain_shift
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "FRI domain shift must be invertible",
            ))?;
        let inverse_root = domain_root.inv().ok_or(ZkAceStarkError::InternalInvariant(
            "FRI domain root must be invertible",
        ))?;
        for index in 0..half {
            next.push(fri_fold_pair_with_inverse_x(
                current[index],
                current[index + half],
                beta,
                inverse_x,
            )?);
            inverse_x = inverse_x.mul(inverse_root);
        }
        trees.push(tree);
        roots.push(root);
        layers.push(next);
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    let terminal_values = layers
        .last()
        .ok_or(ZkAceStarkError::InternalInvariant(
            "FRI terminal layer is missing",
        ))?
        .clone();
    if terminal_values.len() != TERMINAL_SIZE {
        return Err(ZkAceStarkError::InternalInvariant(
            "FRI terminal layer has the wrong compiled size",
        ));
    }
    let terminal_tree = MerkleTree::from_leaves(
        terminal_values
            .iter()
            .copied()
            .map(|value| fri_leaf_hash(lane, FRI_ROUNDS, value))
            .collect(),
    )?;
    roots.push(terminal_tree.root());
    trees.push(terminal_tree);
    ensure_terminal_degree(&terminal_values)?;
    Ok(FriLaneMaterial {
        layers,
        trees,
        roots,
        terminal_values,
    })
}

fn ensure_terminal_degree(values: &[F]) -> Result<(), ZkAceStarkError> {
    if values.len() != TERMINAL_SIZE {
        return Err(ZkAceStarkError::FriDegree);
    }
    let root = primitive_root(BLOWUP_LOG2)?;
    let mut coefficients = values.to_vec();
    ifft(&mut coefficients, root)?;
    if coefficients[TERMINAL_DEGREE_BOUND + 1..]
        .iter()
        .any(|value| *value != F::ZERO)
    {
        return Err(ZkAceStarkError::FriDegree);
    }
    Ok(())
}

fn validate_relation_inputs(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<[F; 8], ZkAceStarkError> {
    if public_inputs.version != 1
        || public_inputs.domain_tag != ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG
        || public_inputs.action_class != ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER
        || public_inputs.verifier_key_id.backend.as_str() != ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND
        || public_inputs.verifier_key_id.name != ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID
        || public_inputs.amount == 0
        || public_inputs.policy_hash == [0; 32]
    {
        return Err(ZkAceStarkError::InvalidPublicInputs);
    }
    let expected_transfer_digest = derive_zk_ace_transfer_digest(
        &public_inputs.from,
        &public_inputs.to,
        &public_inputs.asset,
        public_inputs.amount,
        &public_inputs.chain_id,
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
        &public_inputs.policy_hash,
    );
    if public_inputs.tx_digest != expected_transfer_digest {
        return Err(ZkAceStarkError::InvalidPublicInputs);
    }
    public_output_words(public_inputs)
}

#[cfg(test)]
fn fixed_columns_for_public_inputs(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    let rows = build_schedule(public_inputs)?
        .into_iter()
        .map(fixed_row)
        .collect::<Vec<_>>();
    transpose_rows(&rows, FIXED_WIDTH)
}

fn composition_tree(lane: usize, values: &[F]) -> Result<MerkleTree, ZkAceStarkError> {
    if values.len() != LDE_SIZE {
        return Err(ZkAceStarkError::InternalInvariant(
            "composition vector length mismatch",
        ));
    }
    MerkleTree::from_leaves(
        values
            .iter()
            .copied()
            .map(|value| composition_leaf_hash(lane, value))
            .collect(),
    )
}

fn proof_query(
    index: usize,
    trace_lde: &[Vec<F>],
    trace_tree: &MerkleTree,
    compositions: &[Vec<F>],
    composition_trees: &[MerkleTree],
    fri_lanes: &[FriLaneMaterial],
) -> Result<ZkAceQueryProofV1, ZkAceStarkError> {
    let next_index = (index + TERMINAL_SIZE) % LDE_SIZE;
    let current_row = row_at(trace_lde, index)?;
    let next_row = row_at(trace_lde, next_index)?;
    let composition_values = compositions.iter().map(|values| values[index].0).collect();
    let composition_paths = composition_trees
        .iter()
        .map(|tree| tree.path(index))
        .collect::<Result<Vec<_>, _>>()?;
    let mut query_fri_lanes = Vec::with_capacity(SECURITY_LANES);
    for lane in fri_lanes {
        let mut layer_index = index;
        let mut rounds = Vec::with_capacity(FRI_ROUNDS);
        for round in 0..FRI_ROUNDS {
            let layer = &lane.layers[round];
            let half = layer.len() / 2;
            let low_index = layer_index % half;
            let high_index = low_index + half;
            rounds.push(ZkAceFriRoundOpeningV1 {
                low: layer[low_index].0,
                high: layer[high_index].0,
                low_path: lane.trees[round].path(low_index)?,
                high_path: lane.trees[round].path(high_index)?,
            });
            layer_index = low_index;
        }
        query_fri_lanes.push(ZkAceFriLaneQueryV1 { rounds });
    }
    Ok(ZkAceQueryProofV1 {
        index: u32::try_from(index).map_err(|_| {
            ZkAceStarkError::InternalInvariant("compiled query index does not fit u32")
        })?,
        current_row: current_row.into_iter().map(|value| value.0).collect(),
        next_row: next_row.into_iter().map(|value| value.0).collect(),
        current_row_path: trace_tree.path(index)?,
        next_row_path: trace_tree.path(next_index)?,
        composition_values,
        composition_paths,
        fri_lanes: query_fri_lanes,
    })
}

/// Construct a canonical masked proof using a caller-supplied fallible RNG.
///
/// The injected RNG exists for deterministic known-answer tests and explicit
/// entropy-failure tests. Product callers use [`rand::rngs::OsRng`].
pub(crate) fn prove_zk_ace_stark_v1_with_rng<R: TryRngCore>(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAceStarkError> {
    let _ = validate_relation_inputs(public_inputs)?;
    let public_digest = derive_zk_ace_air_public_digest(public_inputs)
        .map_err(|_| ZkAceStarkError::PublicInputEncoding)?;
    let trace_material = build_trace_material(public_inputs, witness)?;
    let trace_lde = masked_lde_columns(&trace_material.trace_columns, rng)?;
    let fixed_lde = fixed_lde_columns(&trace_material.fixed_columns)?;
    let trace_tree = trace_tree(&trace_lde)?;
    let trace_root = trace_tree.root();
    let base_seed = base_transcript_seed(&public_digest, &trace_root);

    let lane_alphas = (0..SECURITY_LANES)
        .map(|lane| challenge_vector(&base_seed, b"constraint-alpha", lane, CONSTRAINT_COUNT))
        .collect::<Vec<_>>();
    let compositions = composition_lanes(
        &trace_lde,
        &fixed_lde,
        &trace_material.public_outputs,
        &lane_alphas,
    )?;
    let mut composition_trees = Vec::with_capacity(SECURITY_LANES);
    let mut composition_roots = Vec::with_capacity(SECURITY_LANES);
    for (lane, values) in compositions.iter().enumerate() {
        let tree = composition_tree(lane, values)?;
        composition_roots.push(tree.root());
        composition_trees.push(tree);
    }

    let composition_seed = composition_seed(&base_seed, &composition_roots);
    let mut fri_material = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let trace_mix = challenge_vector(&composition_seed, b"trace-mix", lane, TRACE_WIDTH);
        let composition_mix = challenge_field(&composition_seed, b"composition-mix", lane, 0);
        let base_values =
            mix_fri_base(&trace_lde, &compositions[lane], &trace_mix, composition_mix)?;
        let lane_seed = fri_lane_seed(&composition_seed, lane);
        fri_material.push(build_fri_lane(base_values, &lane_seed, lane)?);
    }
    let fri_roots = fri_material
        .iter()
        .map(|lane| lane.roots.clone())
        .collect::<Vec<_>>();
    let query_seed = query_seed_from_roots(&composition_seed, &fri_roots);
    let query_indices = derive_query_indices(&query_seed)?;
    let queries = query_indices
        .into_iter()
        .map(|index| {
            proof_query(
                index,
                &trace_lde,
                &trace_tree,
                &compositions,
                &composition_trees,
                &fri_material,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let proof = ZkAceStarkProofV1 {
        version: PROOF_VERSION,
        trace_root,
        composition_roots,
        fri_lanes: fri_material
            .into_iter()
            .map(|lane| ZkAceFriLaneProofV1 {
                roots: lane.roots,
                terminal_values: lane
                    .terminal_values
                    .into_iter()
                    .map(|value| value.0)
                    .collect(),
            })
            .collect(),
        queries,
    };
    let encoded = norito::to_bytes(&proof).map_err(|_| ZkAceStarkError::MalformedProof)?;
    if encoded.len() > MAX_PROOF_BYTES {
        return Err(ZkAceStarkError::ProofTooLarge);
    }
    // Never return a prover artifact that the independently reconstructed
    // verifier view rejects.
    verify_zk_ace_stark_v1(public_inputs, &encoded)?;
    Ok(encoded)
}

fn canonical_fields(values: &[u64], expected: usize) -> Result<Vec<F>, ZkAceStarkError> {
    if values.len() != expected {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    values
        .iter()
        .copied()
        .map(|value| F::canonical(value).ok_or(ZkAceStarkError::NonCanonicalField))
        .collect()
}

fn validate_proof_shape(proof: &ZkAceStarkProofV1) -> Result<(), ZkAceStarkError> {
    if proof.version != PROOF_VERSION
        || proof.composition_roots.len() != SECURITY_LANES
        || proof.fri_lanes.len() != SECURITY_LANES
        || proof.queries.len() != QUERY_COUNT
    {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    for lane in &proof.fri_lanes {
        if lane.roots.len() != FRI_ROUNDS + 1 || lane.terminal_values.len() != TERMINAL_SIZE {
            return Err(ZkAceStarkError::ProfileMismatch);
        }
    }
    for query in &proof.queries {
        if query.current_row.len() != TRACE_WIDTH
            || query.next_row.len() != TRACE_WIDTH
            || query.current_row_path.len() != LDE_LOG2 as usize
            || query.next_row_path.len() != LDE_LOG2 as usize
            || query.composition_values.len() != SECURITY_LANES
            || query.composition_paths.len() != SECURITY_LANES
            || query
                .composition_paths
                .iter()
                .any(|path| path.len() != LDE_LOG2 as usize)
            || query.fri_lanes.len() != SECURITY_LANES
        {
            return Err(ZkAceStarkError::ProfileMismatch);
        }
        for lane in &query.fri_lanes {
            if lane.rounds.len() != FRI_ROUNDS {
                return Err(ZkAceStarkError::ProfileMismatch);
            }
            for (round, opening) in lane.rounds.iter().enumerate() {
                let expected_depth = LDE_LOG2 as usize - round;
                if opening.low_path.len() != expected_depth
                    || opening.high_path.len() != expected_depth
                {
                    return Err(ZkAceStarkError::ProfileMismatch);
                }
            }
        }
    }
    Ok(())
}

fn verify_fri_query(
    lane: usize,
    query_index: usize,
    expected_base_value: F,
    lane_proof: &ZkAceFriLaneProofV1,
    lane_query: &ZkAceFriLaneQueryV1,
    composition_seed: &[u8; 32],
    terminal_values: &[F],
) -> Result<(), ZkAceStarkError> {
    let lane_seed = fri_lane_seed(composition_seed, lane);
    let mut layer_index = query_index;
    let mut layer_size = LDE_SIZE;
    let mut domain_shift = F(FIELD_GENERATOR);
    let mut domain_root = primitive_root(LDE_LOG2)?;
    let mut expected = expected_base_value;

    for round in 0..FRI_ROUNDS {
        let opening = &lane_query.rounds[round];
        let low = F::canonical(opening.low).ok_or(ZkAceStarkError::NonCanonicalField)?;
        let high = F::canonical(opening.high).ok_or(ZkAceStarkError::NonCanonicalField)?;
        let half = layer_size / 2;
        let low_index = layer_index % half;
        let high_index = low_index + half;
        let depth = LDE_LOG2 as usize - round;
        if !verify_merkle_path(
            &lane_proof.roots[round],
            fri_leaf_hash(lane, round, low),
            low_index,
            &opening.low_path,
            depth,
        ) || !verify_merkle_path(
            &lane_proof.roots[round],
            fri_leaf_hash(lane, round, high),
            high_index,
            &opening.high_path,
            depth,
        ) {
            return Err(ZkAceStarkError::FriOpening);
        }
        let selected = if layer_index < half { low } else { high };
        if selected != expected {
            return Err(ZkAceStarkError::FriOpening);
        }
        let x = domain_shift.mul(domain_root.pow(low_index as u128));
        let beta = fri_beta(&lane_seed, lane, round, &lane_proof.roots[round]);
        expected = fri_fold_pair(low, high, beta, x)?;
        layer_index = low_index;
        layer_size = half;
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    if layer_size != TERMINAL_SIZE
        || terminal_values
            .get(layer_index)
            .copied()
            .ok_or(ZkAceStarkError::FriOpening)?
            != expected
    {
        return Err(ZkAceStarkError::FriOpening);
    }
    Ok(())
}

/// Verify the exact canonical dedicated ZK-ACE proof wire.
pub(crate) fn verify_zk_ace_stark_v1(
    public_inputs: &ZkAcePublicInputsV1,
    proof_bytes: &[u8],
) -> Result<(), ZkAceStarkError> {
    if proof_bytes.is_empty() {
        return Err(ZkAceStarkError::MalformedProof);
    }
    if proof_bytes.len() > MAX_PROOF_BYTES {
        return Err(ZkAceStarkError::ProofTooLarge);
    }
    let public_outputs = validate_relation_inputs(public_inputs)?;
    let public_digest = derive_zk_ace_air_public_digest(public_inputs)
        .map_err(|_| ZkAceStarkError::PublicInputEncoding)?;
    let proof: ZkAceStarkProofV1 =
        norito::decode_from_bytes_with_limits(proof_bytes, PROOF_DECODE_LIMITS)
            .map_err(|_| ZkAceStarkError::MalformedProof)?;
    let canonical = norito::to_bytes(&proof).map_err(|_| ZkAceStarkError::MalformedProof)?;
    if canonical.as_slice() != proof_bytes {
        return Err(ZkAceStarkError::NonCanonicalProof);
    }
    validate_proof_shape(&proof)?;

    let base_seed = base_transcript_seed(&public_digest, &proof.trace_root);
    let composition_seed = composition_seed(&base_seed, &proof.composition_roots);
    let lane_roots = proof
        .fri_lanes
        .iter()
        .map(|lane| lane.roots.clone())
        .collect::<Vec<_>>();
    let query_seed = query_seed_from_roots(&composition_seed, &lane_roots);
    let expected_indices = derive_query_indices(&query_seed)?;
    let fixed_schedule = build_schedule(public_inputs)?;
    let lde_root = primitive_root(LDE_LOG2)?;
    let mut terminal_fields = Vec::with_capacity(SECURITY_LANES);

    for (lane_index, lane) in proof.fri_lanes.iter().enumerate() {
        let terminal = canonical_fields(&lane.terminal_values, TERMINAL_SIZE)?;
        let terminal_tree = MerkleTree::from_leaves(
            terminal
                .iter()
                .copied()
                .map(|value| fri_leaf_hash(lane_index, FRI_ROUNDS, value))
                .collect(),
        )?;
        if terminal_tree.root() != lane.roots[FRI_ROUNDS] {
            return Err(ZkAceStarkError::FriOpening);
        }
        ensure_terminal_degree(&terminal)?;
        terminal_fields.push(terminal);
    }

    let alphas = (0..SECURITY_LANES)
        .map(|lane| challenge_vector(&base_seed, b"constraint-alpha", lane, CONSTRAINT_COUNT))
        .collect::<Vec<_>>();
    let trace_mix = (0..SECURITY_LANES)
        .map(|lane| challenge_vector(&composition_seed, b"trace-mix", lane, TRACE_WIDTH))
        .collect::<Vec<_>>();
    let composition_mix = (0..SECURITY_LANES)
        .map(|lane| challenge_field(&composition_seed, b"composition-mix", lane, 0))
        .collect::<Vec<_>>();

    for (query_position, query) in proof.queries.iter().enumerate() {
        let index =
            usize::try_from(query.index).map_err(|_| ZkAceStarkError::TranscriptMismatch)?;
        if index != expected_indices[query_position] || index >= LDE_SIZE {
            return Err(ZkAceStarkError::TranscriptMismatch);
        }
        let next_index = (index + TERMINAL_SIZE) % LDE_SIZE;
        let current = canonical_fields(&query.current_row, TRACE_WIDTH)?;
        let next = canonical_fields(&query.next_row, TRACE_WIDTH)?;
        if !verify_merkle_path(
            &proof.trace_root,
            trace_leaf_hash(&current),
            index,
            &query.current_row_path,
            LDE_LOG2 as usize,
        ) || !verify_merkle_path(
            &proof.trace_root,
            trace_leaf_hash(&next),
            next_index,
            &query.next_row_path,
            LDE_LOG2 as usize,
        ) {
            return Err(ZkAceStarkError::TraceOpening);
        }
        let x = F(FIELD_GENERATOR).mul(lde_root.pow(index as u128));
        let fixed = fixed_row_at_point(&fixed_schedule, x)?;
        let composition_values = canonical_fields(&query.composition_values, SECURITY_LANES)?;
        for lane in 0..SECURITY_LANES {
            if !verify_merkle_path(
                &proof.composition_roots[lane],
                composition_leaf_hash(lane, composition_values[lane]),
                index,
                &query.composition_paths[lane],
                LDE_LOG2 as usize,
            ) {
                return Err(ZkAceStarkError::ConstraintOpening);
            }
            let expected_composition = constraint_quotient_value(
                x,
                &current,
                &next,
                &fixed,
                &public_outputs,
                &alphas[lane],
            )?;
            if composition_values[lane] != expected_composition {
                return Err(ZkAceStarkError::ConstraintOpening);
            }
            let mixed_trace = current
                .iter()
                .zip(&trace_mix[lane])
                .fold(F::ZERO, |sum, (value, coefficient)| {
                    sum.add(value.mul(*coefficient))
                });
            let expected_base =
                mixed_trace.add(composition_values[lane].mul(composition_mix[lane]));
            verify_fri_query(
                lane,
                index,
                expected_base,
                &proof.fri_lanes[lane],
                &query.fri_lanes[lane],
                &composition_seed,
                &terminal_fields[lane],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr as _, sync::OnceLock};

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        proof::VerifyingKeyId,
        zk::{derive_zk_ace_identity_commitment, derive_zk_ace_replay_nullifier},
    };
    use rand::{RngCore, SeedableRng as _, rngs::StdRng};

    use super::*;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic ZK-ACE test account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("test domain"),
            Name::from_str("zkace").expect("test asset"),
        )
    }

    fn public_inputs_and_witness() -> (ZkAcePublicInputsV1, ZkAceWitnessV1) {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let chain_id = ChainId::from("taira-privacy-zk-ace-test");
        let source = account(1);
        let destination = account(2);
        let asset = asset();
        let policy_hash = [0x47; 32];
        let authorization_digest = [0xA6; 32];
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &source,
            &destination,
            &asset,
            19,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &authorization_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        (
            ZkAcePublicInputsV1::transparent_transfer(
                identity_commitment,
                tx_digest,
                authorization_digest,
                chain_id,
                replay_nullifier,
                policy_hash,
                source,
                destination,
                asset,
                19,
                VerifyingKeyId::new(
                    ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
                    ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
                ),
            ),
            witness,
        )
    }

    fn fixture() -> &'static (ZkAcePublicInputsV1, ZkAceWitnessV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkAcePublicInputsV1, ZkAceWitnessV1, Vec<u8>)> = OnceLock::new();
        let _guard = proof_test_guard();
        FIXTURE.get_or_init(|| {
            let (public_inputs, witness) = public_inputs_and_witness();
            let mut rng = StdRng::from_seed([0x5A; 32]);
            let proof = prove_zk_ace_stark_v1_with_rng(&public_inputs, &witness, &mut rng)
                .expect("construct sound deterministic fixture");
            (public_inputs, witness, proof)
        })
    }

    fn decode_fixture() -> ZkAceStarkProofV1 {
        norito::decode_from_bytes(&fixture().2).expect("decode canonical fixture")
    }

    fn assert_rejected(proof: &ZkAceStarkProofV1) {
        let bytes = norito::to_bytes(proof).expect("encode adversarial proof");
        assert!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes).is_err(),
            "adversarial proof must be rejected"
        );
    }

    #[test]
    fn goldilocks_fft_roundtrips_and_roots_have_exact_order() {
        for log_size in 1..=10 {
            let root = primitive_root(log_size).expect("compiled root");
            let size = 1usize << log_size;
            assert_eq!(root.pow(size as u128), F::ONE);
            assert_ne!(root.pow((size / 2) as u128), F::ONE);
            let mut values = (0..size)
                .map(|index| F::reduce((index as u128 + 1).pow(3)))
                .collect::<Vec<_>>();
            let expected = values.clone();
            fft(&mut values, root).expect("FFT");
            ifft(&mut values, root).expect("inverse FFT");
            assert_eq!(values, expected);
        }
    }

    #[test]
    fn complete_trace_matches_both_poseidon_relations() {
        let (public_inputs, witness) = public_inputs_and_witness();
        let material = build_trace_material(&public_inputs, &witness).expect("valid trace");
        assert_eq!(
            material.public_outputs,
            public_output_words(&public_inputs).expect("canonical public outputs")
        );
        assert_eq!(material.trace_columns.len(), TRACE_WIDTH);
        assert_eq!(material.fixed_columns.len(), FIXED_WIDTH);
        assert!(
            material
                .trace_columns
                .iter()
                .all(|column| column.len() == TRACE_SIZE)
        );
    }

    #[test]
    fn verifier_barycentric_fixed_rows_match_full_lde() {
        let (public_inputs, _) = public_inputs_and_witness();
        let schedule = build_schedule(&public_inputs).expect("compiled fixed schedule");
        let columns = fixed_columns_for_public_inputs(&public_inputs).expect("fixed base columns");
        let lde = fixed_lde_columns(&columns).expect("full fixed LDE");
        let root = primitive_root(LDE_LOG2).expect("LDE root");
        for index in [0usize, 1, 17, 31_337, LDE_SIZE - 1] {
            let x = F(FIELD_GENERATOR).mul(root.pow(index as u128));
            assert_eq!(
                fixed_row_at_point(&schedule, x).expect("barycentric fixed row"),
                row_at(&lde, index).expect("full-LDE fixed row"),
                "fixed interpolation drift at LDE index {index}"
            );
        }
    }

    #[test]
    fn every_private_witness_component_is_required() {
        let (public_inputs, witness) = public_inputs_and_witness();
        let mutations: [fn(&mut ZkAceWitnessV1); 3] = [
            |candidate: &mut ZkAceWitnessV1| candidate.identity_root[0] ^= 1,
            |candidate: &mut ZkAceWitnessV1| candidate.identity_blinding[0] ^= 1,
            |candidate: &mut ZkAceWitnessV1| candidate.replay_secret[0] ^= 1,
        ];
        for mutate in mutations {
            let mut changed = witness;
            mutate(&mut changed);
            assert!(matches!(
                build_trace_material(&public_inputs, &changed),
                Err(ZkAceStarkError::WitnessRelation)
            ));
        }
    }

    #[derive(Debug)]
    struct MaxValueRng;

    impl RngCore for MaxValueRng {
        fn next_u32(&mut self) -> u32 {
            u32::MAX
        }

        fn next_u64(&mut self) -> u64 {
            u64::MAX
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            destination.fill(0xFF);
        }
    }

    #[test]
    fn unavailable_canonical_entropy_fails_closed() {
        let (public_inputs, witness) = public_inputs_and_witness();
        assert!(matches!(
            prove_zk_ace_stark_v1_with_rng(&public_inputs, &witness, &mut MaxValueRng),
            Err(ZkAceStarkError::RandomnessUnavailable)
        ));
    }

    #[test]
    fn proof_roundtrips_under_exact_shape_and_byte_ceiling() {
        let (public_inputs, _, proof) = fixture();
        verify_zk_ace_stark_v1(public_inputs, proof).expect("proof verifies");
        assert!(!proof.is_empty());
        assert!(proof.len() <= MAX_PROOF_BYTES);
        let decoded = decode_fixture();
        assert_eq!(decoded.composition_roots.len(), SECURITY_LANES);
        assert_eq!(decoded.queries.len(), QUERY_COUNT);
        assert!(
            decoded
                .queries
                .iter()
                .all(|query| query.current_row.len() == TRACE_WIDTH)
        );
    }

    #[test]
    fn trace_masking_is_randomized_and_does_not_embed_raw_witness_bytes() {
        let (public_inputs, witness, first) = fixture();
        let _guard = proof_test_guard();
        let mut rng = StdRng::from_seed([0xA5; 32]);
        let second = prove_zk_ace_stark_v1_with_rng(public_inputs, witness, &mut rng)
            .expect("second masked proof");
        assert_ne!(first, &second);
        verify_zk_ace_stark_v1(public_inputs, &second).expect("second proof verifies");
        for marker in [
            witness.identity_root,
            witness.identity_blinding,
            witness.replay_secret,
        ] {
            assert!(
                !first
                    .windows(marker.len())
                    .any(|window| window == marker.as_slice())
            );
            assert!(
                !second
                    .windows(marker.len())
                    .any(|window| window == marker.as_slice())
            );
        }
    }

    #[test]
    fn every_public_relation_binding_rejects_replay() {
        let (public_inputs, _, proof) = fixture();
        let mutations: [(&str, fn(&mut ZkAcePublicInputsV1)); 14] = [
            ("version", |value| value.version ^= 1),
            ("identity", |value| value.identity_commitment[0] ^= 1),
            ("transfer", |value| value.tx_digest[0] ^= 1),
            ("authorization", |value| value.authorization_digest[0] ^= 1),
            ("chain", |value| value.chain_id = ChainId::from("foreign")),
            ("domain", |value| value.domain_tag.push('x')),
            ("action", |value| value.action_class.push('x')),
            ("nullifier", |value| value.replay_nullifier[0] ^= 1),
            ("policy", |value| value.policy_hash[0] ^= 1),
            ("source", |value| value.from = account(3)),
            ("destination", |value| value.to = account(4)),
            ("asset", |value| {
                value.asset = AssetDefinitionId::new(
                    DomainId::try_new("privacy", "universal").expect("test domain"),
                    Name::from_str("other").expect("other asset"),
                );
            }),
            ("amount", |value| value.amount += 1),
            ("verifier", |value| value.verifier_key_id.name.push('x')),
        ];
        for (label, mutate) in mutations {
            let mut changed = public_inputs.clone();
            mutate(&mut changed);
            assert!(
                verify_zk_ace_stark_v1(&changed, proof).is_err(),
                "{label} mutation must reject replay"
            );
        }
    }

    #[test]
    fn strict_wire_rejects_empty_oversized_truncated_and_trailing_data() {
        let (public_inputs, _, proof) = fixture();
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &[]),
            Err(ZkAceStarkError::MalformedProof)
        ));
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &vec![0; MAX_PROOF_BYTES + 1]),
            Err(ZkAceStarkError::ProofTooLarge)
        ));
        for length in [
            1,
            proof.len() / 3,
            proof.len() / 2,
            proof.len().saturating_sub(1),
        ] {
            assert!(verify_zk_ace_stark_v1(public_inputs, &proof[..length]).is_err());
        }
        let mut trailing = proof.clone();
        trailing.push(0);
        assert!(verify_zk_ace_stark_v1(public_inputs, &trailing).is_err());
    }

    #[test]
    fn malformed_shapes_noncanonical_fields_and_merkle_forgery_reject() {
        let mut changed = decode_fixture();
        changed.version ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries.pop();
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].current_row.pop();
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].current_row.push(0);
        let bytes = norito::to_bytes(&changed).expect("encode over-count row");
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes),
            Err(ZkAceStarkError::MalformedProof)
        ));

        changed = decode_fixture();
        changed.queries[0].current_row[0] = FIELD_MODULUS;
        let bytes = norito::to_bytes(&changed).expect("encode non-canonical field");
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes),
            Err(ZkAceStarkError::NonCanonicalField)
        ));

        changed = decode_fixture();
        changed.trace_root[0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].current_row_path[0][0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.composition_roots[0][0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].composition_values[0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_lanes[0].rounds[0].low ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_lanes[0].rounds[0].high_path[0][0] ^= 1;
        assert_rejected(&changed);
    }

    #[test]
    fn malicious_zero_composition_cannot_disconnect_private_trace() {
        let mut changed = decode_fixture();
        changed.composition_roots.fill([0; 32]);
        for query in &mut changed.queries {
            query.composition_values.fill(0);
            for path in &mut query.composition_paths {
                path.fill([0; 32]);
            }
        }
        assert_rejected(&changed);
    }

    #[test]
    fn terminal_root_cannot_hide_a_high_degree_polynomial() {
        let mut changed = decode_fixture();
        changed.fri_lanes[0].terminal_values[3] ^= 1;
        let terminal = canonical_fields(&changed.fri_lanes[0].terminal_values, TERMINAL_SIZE)
            .expect("mutated field remains canonical");
        let tree = MerkleTree::from_leaves(
            terminal
                .iter()
                .copied()
                .map(|value| fri_leaf_hash(0, FRI_ROUNDS, value))
                .collect(),
        )
        .expect("terminal tree");
        changed.fri_lanes[0].roots[FRI_ROUNDS] = tree.root();
        let bytes = norito::to_bytes(&changed).expect("encode high-degree terminal");
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes),
            Err(ZkAceStarkError::FriDegree)
        ));
    }
}
