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
//! - one quartic-extension composition quotient shares the base-field trace
//!   commitment and is linked at a transcript-derived out-of-domain point;
//! - the FRI batch contains the DEEP evaluation quotients of every trace
//!   column and the composition quotient, rather than relying on ordinary ALI;
//! - FRI adds an independently committed full low-degree extension-field mask
//!   before its batching challenges are sampled, so folding cannot collapse
//!   the structured trace-mask entropy and reveal the witness;
//! - the verifier performs an actual low-degree FRI test, stopping at the compiled
//!   blow-up domain and checking the complete terminal polynomial has degree at
//!   most one;
//! - query openings bind the same masked trace rows, quotient values, and FRI
//!   base evaluations.
//!
//! No caller-selected parameter, transcript, proof shape, or backend is carried
//! by the wire value.  All dimensions below are compiled consensus constants.

use std::collections::BTreeSet;

use fastpq_prover::poseidon_manifest;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetDefinitionId,
    proof::VerifyingKeyId,
    zk::{
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        derive_zk_ace_transfer_digest, zk_ace_pack_bytes_to_field_limbs,
        zk_ace_poseidon2_domain_hash,
    },
};
use rand::{TryCryptoRng, TryRngCore};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

#[cfg(test)]
use super::transparent_stark::goldilocks_fft_v1;
use super::{
    prover_randomness::{HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1},
    transparent_stark::{
        ExactProofReaderV1, GOLDILOCKS_GENERATOR_V1, GoldilocksFieldV1 as F, GoldilocksFp4V1 as E,
        ReplayableTraceMaskV1, TransparentStarkErrorV1, append_goldilocks_fp4_v1,
        append_u16_v1 as append_u16, append_u32_v1 as append_u32, append_u64_v1 as append_u64,
        checked_transparent_stark_work_security_v1, ensure_fri_terminal_degree_fp4_v1,
        fri_fold_pair_fp4_v1, fri_fold_pair_with_inverse_x_fp4_v1, goldilocks_batch_invert_v1,
        goldilocks_evaluate_coset_v1, goldilocks_fp4_evaluate_coset_v1, goldilocks_fp4_ifft_v1,
        goldilocks_ifft_v1, goldilocks_primitive_root_v1, masked_trace_lde_column_with_mask_v1,
        random_goldilocks_fp4_v1, sample_trace_mask_v1, transparent_stark_zk_mask_geometry_v1,
    },
    zk_ace::ZkAcePrivacyWitnessV1,
};

/// Internal, fixed AIR projection of the typed privacy statement.
///
/// This type is deliberately not exported from `iroha_core`: callers submit
/// [`iroha_data_model::zk::ZkAcePrivacyPublicInputsV1`] through the typed
/// privacy envelope, while this exact projection exists only between the
/// dedicated prover and verifier.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ZkAceAirRelationInputsV1 {
    pub(super) version: u16,
    pub(super) identity_commitment: [u8; 32],
    pub(super) tx_digest: [u8; 32],
    pub(super) authorization_digest: [u8; 32],
    pub(super) chain_id: ChainId,
    pub(super) domain_tag: String,
    pub(super) action_class: String,
    pub(super) replay_nullifier: [u8; 32],
    pub(super) policy_hash: [u8; 32],
    pub(super) from: AccountId,
    pub(super) to: AccountId,
    pub(super) asset: AssetDefinitionId,
    pub(super) amount: u128,
    pub(super) verifier_key_id: VerifyingKeyId,
}

impl ZkAceAirRelationInputsV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn transparent_transfer(
        identity_commitment: [u8; 32],
        tx_digest: [u8; 32],
        authorization_digest: [u8; 32],
        chain_id: ChainId,
        replay_nullifier: [u8; 32],
        policy_hash: [u8; 32],
        from: AccountId,
        to: AccountId,
        asset: AssetDefinitionId,
        amount: u128,
    ) -> Self {
        Self {
            version: 1,
            identity_commitment,
            tx_digest,
            authorization_digest,
            chain_id,
            domain_tag: ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            action_class: ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            amount,
            verifier_key_id: VerifyingKeyId::new(
                ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
                ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
            ),
        }
    }
}

/// Exact, type-name-independent public transcript schema.
///
/// The schema descriptor is itself the first framed part. Every following part
/// is ordered and independently length-framed by
/// [`zk_ace_poseidon2_domain_hash`], whose byte packing is fixed to seven-byte
/// little-endian Goldilocks limbs.
pub(super) const AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1: &[u8] = b"framing=poseidon2-domain-words:domain-length-u64+7byte-le-limbs:part-count-u64:each-part-length-u64+7byte-le-limbs|part0=this-schema|part1=version:u16be|part2=identity-commitment:bytes32|part3=transfer-digest:bytes32|part4=authorization-digest:bytes32|part5=chain-id:utf8|part6=fixed-domain:utf8|part7=fixed-action:utf8|part8=replay-nullifier:bytes32|part9=policy-digest:bytes32|part10=source:account-canonical-hex-v1-utf8|part11=destination:account-canonical-hex-v1-utf8|part12=asset-definition-id:uuid-bytes16|part13=amount:u128be|part14=fixed-verifier-backend:utf8|part15=fixed-verifier-circuit:utf8";
const AIR_PUBLIC_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ace:air-public-digest:v1";

fn air_public_transcript_parts_v1(
    public_inputs: &ZkAceAirRelationInputsV1,
) -> Result<Vec<Vec<u8>>, ZkAceStarkError> {
    let source = public_inputs
        .from
        .to_canonical_hex()
        .map_err(|_| ZkAceStarkError::PublicInputEncoding)?
        .into_bytes();
    let destination = public_inputs
        .to
        .to_canonical_hex()
        .map_err(|_| ZkAceStarkError::PublicInputEncoding)?
        .into_bytes();
    Ok(vec![
        AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1.to_vec(),
        public_inputs.version.to_be_bytes().to_vec(),
        public_inputs.identity_commitment.to_vec(),
        public_inputs.tx_digest.to_vec(),
        public_inputs.authorization_digest.to_vec(),
        public_inputs.chain_id.as_str().as_bytes().to_vec(),
        public_inputs.domain_tag.as_bytes().to_vec(),
        public_inputs.action_class.as_bytes().to_vec(),
        public_inputs.replay_nullifier.to_vec(),
        public_inputs.policy_hash.to_vec(),
        source,
        destination,
        public_inputs.asset.aid_bytes().to_vec(),
        public_inputs.amount.to_be_bytes().to_vec(),
        public_inputs
            .verifier_key_id
            .backend
            .as_str()
            .as_bytes()
            .to_vec(),
        public_inputs.verifier_key_id.name.as_bytes().to_vec(),
    ])
}

fn hash_air_public_transcript_parts_v1(parts: &[Vec<u8>]) -> [u8; 32] {
    let parts = parts.iter().map(Vec::as_slice).collect::<Vec<_>>();
    zk_ace_poseidon2_domain_hash(AIR_PUBLIC_TRANSCRIPT_DOMAIN_V1, &parts)
}

fn derive_zk_ace_air_public_digest(
    public_inputs: &ZkAceAirRelationInputsV1,
) -> Result<[u8; 32], ZkAceStarkError> {
    air_public_transcript_parts_v1(public_inputs)
        .map(|parts| hash_air_public_transcript_parts_v1(&parts))
}

#[cfg(test)]
static PROOF_TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn proof_test_guard() -> std::sync::MutexGuard<'static, ()> {
    PROOF_TEST_MUTEX
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("ZK-ACE proof test mutex must not be poisoned")
}

#[cfg(test)]
const FIELD_MODULUS: u64 = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
const FIELD_GENERATOR: u64 = GOLDILOCKS_GENERATOR_V1;

// Symbolic degree ledger for every constraint family emitted by
// `constraint_quotient_value_with_factors`: affine state/limb equations,
// the three lowered Poseidon power equations, Booleanity, selector-gated
// local equations, state transitions, and queue transitions.
const AIR_CONSTRAINT_DEGREE_LEDGER_V1: [usize; 8] = [1, 2, 2, 2, 2, 2, 2, 2];

const fn maximum_air_constraint_degree_v1() -> usize {
    let mut maximum = 0;
    let mut index = 0;
    while index < AIR_CONSTRAINT_DEGREE_LEDGER_V1.len() {
        if AIR_CONSTRAINT_DEGREE_LEDGER_V1[index] > maximum {
            maximum = AIR_CONSTRAINT_DEGREE_LEDGER_V1[index];
        }
        index += 1;
    }
    maximum
}

/// Base execution trace has exactly 4,096 rows.
pub(crate) const TRACE_LOG2: u8 = 12;
/// The low-degree extension uses a compiled 16x blow-up.
pub(crate) const BLOWUP_LOG2: u8 = 4;
/// Number of distinct transcript-derived FRI query rounds.
///
/// The exact theorem-backed profile uses the list-decoding bound at rate 1/8,
/// a quartic Goldilocks extension, and 108 samples without replacement.
pub(crate) const QUERY_COUNT: usize = 108;
/// One genuine quartic-extension composition/FRI instance.
pub(crate) const SECURITY_LANES: usize = 1;
const PROOF_WIRE_MAGIC_V1: [u8; 4] = *b"ZKA1";
const HASH_BYTES: usize = 32;
const FIELD_BYTES: usize = 8;
const EXTENSION_FIELD_BYTES: usize = 4 * FIELD_BYTES;
const QUERY_INDEX_BYTES: usize = 4;
const PROOF_VERSION_BYTES: usize = 2;
const FRI_PATH_HASHES_PER_LANE_QUERY: usize = FRI_ROUNDS * (2 * LDE_LOG2 as usize - FRI_ROUNDS + 1);
const FRI_LANE_QUERY_BYTES: usize =
    FRI_ROUNDS * 2 * EXTENSION_FIELD_BYTES + FRI_PATH_HASHES_PER_LANE_QUERY * HASH_BYTES;
const FRI_MASK_OPENING_BYTES: usize = EXTENSION_FIELD_BYTES + LDE_LOG2 as usize * HASH_BYTES;
const QUERY_PROOF_BYTES: usize = QUERY_INDEX_BYTES
    + 2 * TRACE_WIDTH * FIELD_BYTES
    + 2 * LDE_LOG2 as usize * HASH_BYTES
    + SECURITY_LANES * EXTENSION_FIELD_BYTES
    + SECURITY_LANES * LDE_LOG2 as usize * HASH_BYTES
    + SECURITY_LANES * FRI_MASK_OPENING_BYTES
    + SECURITY_LANES * FRI_LANE_QUERY_BYTES;
const DEEP_OPENING_BYTES: usize = (2 * TRACE_WIDTH + SECURITY_LANES) * EXTENSION_FIELD_BYTES;
/// Exact length of the only admitted fixed-shape proof wire.
pub(crate) const CANONICAL_PROOF_BYTES_V1: usize = PROOF_WIRE_MAGIC_V1.len()
    + PROOF_VERSION_BYTES
    + HASH_BYTES
    + SECURITY_LANES * HASH_BYTES
    + SECURITY_LANES * HASH_BYTES
    + DEEP_OPENING_BYTES
    + SECURITY_LANES * ((FRI_ROUNDS + 1) * HASH_BYTES + TERMINAL_SIZE * EXTENSION_FIELD_BYTES)
    + QUERY_COUNT * QUERY_PROOF_BYTES;
/// Hard ceiling enforced before the fixed-shape parser allocates proof vectors.
pub(crate) const MAX_PROOF_BYTES: usize = CANONICAL_PROOF_BYTES_V1;
/// Work-normalized random-oracle security established by the compiled bound.
pub(crate) const PROVABLE_SOUNDNESS_BITS_V1: u16 = 128;
/// Largest adversarial random-oracle work budget covered by the compiled BCS
/// inequality.
pub(crate) const MAX_ROM_QUERY_LOG2_V1: u8 = 124;
/// Exact maximum total degree of any compiled AIR constraint.
pub(crate) const AIR_TOTAL_DEGREE_V1: usize = maximum_air_constraint_degree_v1();
/// `d_AIR - 1`, as used by the Protocol-3 masking theorem.
pub(crate) const REDUCED_AIR_DEGREE_V1: usize = AIR_TOTAL_DEGREE_V1 - 1;
const _: () = assert!(AIR_TOTAL_DEGREE_V1 == 2);
/// Complete consensus-relevant algebraic and commitment profile.
pub(crate) const COMPILED_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"version=1|base-field=goldilocks:0xffffffff00000001|challenge-field=goldilocks-fp4:w4=7:coefficients-c0-c3:u64be|generator=7|field_challenge=sha256:four-u64be-canonical-rejection:framed-u64be-attempt:max-attempts256:distinct270:max-hash-calls69120|poseidon2=width3:rate2:full8:partial57|air-public-transcript=poseidon2-domain-words:domain=iroha:privacy:zk-ace:air-public-digest:v1:schema=compiled-air-relation-schema-v1:parts16:ordered:length-framed:type-name-independent|air-total-degree=2|trace_rows=4096|trace_width=88|trace_mask_degree=511|trace_mask_coefficients=512|zk-bound=fft-decomposition:d-air2:d1:e4:n-deep1:n-fri108:formula=2d(e*n-deep+n-fri)+n-fri:required332:provided512|lde_rows=65536|blowup=16|constraint_lanes=1-fp4|deep-ali=one-point:z-uniform-outside-D-H-zero:excluded69633:sampling-cardinality=p^4-69633:trace-z-gz:composition-z:multi-point-trace-quotients|queries=108|query_schedule=sha256-prefix-u64be-mask16:unique-without-replacement:hypergeometric<=independent-power|merkle=sha256:binary:trace-u64be:fp4-c0-c3-u64be|fri=fp4-fold2:rounds12:terminal16:degree1:code-degree-exclusive8192:domain65536:rho=1/8:m=3:theta=1-7/(12sqrt2):unique-radius<theta<johnson-radius:gs-correlated-agreement:affine-oracles90:affine-random-coefficients89:fold-arities=12x2:sum-a=24|fri_mask=fp4:coefficients8191:degree-exclusive8191:protocol3-optimized-k5120:actual-rounded-k8192:max-structured-batch-degree5117:root-before-batch-challenges|soundness=haboeck-2022-1216-theorem2+theorem8:field-size=p^4:rho=1/8:m=3:D=65536:sum-a=24:fri-commit-1<2^-207:fri-commit-2<2^-230:fri-query=(7/(12sqrt2))^108<2^-135:deep-k-plus8194:deep-list-factor=(7/2)/sqrt(8194/65536):deep-constraint-count168:deep-identity-degree-bound24577:deep-weighted-numerator<245000:deep-denominator=p^4-69633:deep-link<2^-238:rbr-total<2^-134:rbr-certified-bits=129:block-et-al-2023-1071-bcs+block-tiwari-2024-1161:classical-rom:work-normalized:q-ro-max2^124:fs-over-q=eps-rbr+3(q+1/q)/2^256<2^-128:qrom-not-claimed:rom-bits-lower-bound128|wire=ZKA1:fixed-shape:big-endian:1341142|max_proof_bytes=1341142|domains=iroha:privacy:zk-ace:{transparent-stark,air-public-digest,trace-leaf,composition-leaf,fri-mask-leaf,batch-transcript,deep-transcript,fri-leaf,merkle-node,field-challenge,composition-transcript,fri-lane-transcript,fri-round-transcript,query-transcript,query-index}:v1";
/// Degree of the random trace masking polynomial.
const MASK_DEGREE: usize = 511;
const TRACE_MASK_COEFFICIENTS: usize = MASK_DEGREE + 1;
const CHALLENGE_EXTENSION_DEGREE: usize = 4;
const DEEP_QUERY_COUNT: usize = 1;
const FRI_MULTIPLICITY_PARAMETER: usize = 3;
const ROUND_BY_ROUND_SECURITY_BITS_V1: u16 = 129;
const RANDOM_ORACLE_BITS_V1: u16 = 256;
/// FRI stops on the complete compiled blow-up domain.
const TERMINAL_SIZE: usize = 1 << BLOWUP_LOG2;
/// Every folded terminal polynomial must be linear or constant.
const TERMINAL_DEGREE_BOUND: usize = 1;
const TRACE_SIZE: usize = 1 << TRACE_LOG2;
const LDE_LOG2: u8 = TRACE_LOG2 + BLOWUP_LOG2;
const LDE_SIZE: usize = 1 << LDE_LOG2;
const FRI_ROUNDS: usize = TRACE_LOG2 as usize;
const FRI_AFFINE_ORACLE_COUNT: usize = TRACE_WIDTH + 2 * SECURITY_LANES;
const FRI_AFFINE_RANDOM_COEFFICIENTS: usize = FRI_AFFINE_ORACLE_COUNT - 1;
const FRI_REDUCTION_ARITIES: [usize; FRI_ROUNDS] = [2; FRI_ROUNDS];
const DEEP_EXCLUDED_POINT_COUNT: usize = 1 + TRACE_SIZE + LDE_SIZE;
/// Exclusive degree bound of the code tested by FRI.
const FRI_CODE_DEGREE_BOUND_EXCLUSIVE: usize = (TERMINAL_DEGREE_BOUND + 1) << FRI_ROUNDS;
const DEEP_CANDIDATE_DEGREE_BOUND_EXCLUSIVE: usize = FRI_CODE_DEGREE_BOUND_EXCLUSIVE + 2;
const DEEP_IDENTITY_DEGREE_BOUND: usize = AIR_TOTAL_DEGREE_V1
    * (DEEP_CANDIDATE_DEGREE_BOUND_EXCLUSIVE - 1)
    + (FRI_CODE_DEGREE_BOUND_EXCLUSIVE - 1);
/// Dimension of each independent full-space FRI mask.
///
/// This is the `R(X) <- F[X]^{<d-1}` mask from the zero-knowledge FRI
/// batching construction, where `d` is the compiled code degree bound.
const FRI_MASK_COEFFICIENTS: usize = FRI_CODE_DEGREE_BOUND_EXCLUSIVE - 1;
/// Maximum degree of any masked execution-trace column.
const MASKED_TRACE_MAX_DEGREE: usize = TRACE_SIZE + MASK_DEGREE;
/// Maximum degree of the quadratic local-constraint quotient.
const COMPOSITION_MAX_DEGREE: usize = AIR_TOTAL_DEGREE_V1 * MASKED_TRACE_MAX_DEGREE - TRACE_SIZE;
/// Optimized Protocol-3 common bound before rounding to a binary FRI code.
const PROTOCOL3_OPTIMIZED_DEGREE_BOUND_EXCLUSIVE: usize =
    TRACE_SIZE + (AIR_TOTAL_DEGREE_V1 * TRACE_MASK_COEFFICIENTS);
/// Maximum degree of a structured evaluation quotient mixed into FRI.
const MAX_STRUCTURED_BATCH_DEGREE: usize = COMPOSITION_MAX_DEGREE - 1;
const PRIVATE_LIMBS: usize = 15;
const LIMB_BITS: usize = 56;
const POSEIDON_FULL_ROUNDS_HALF: usize = 4;
const POSEIDON_ROUNDS: usize = 65;
const PROOF_VERSION: u16 = 1;
const MAX_QUERY_DERIVATION_ATTEMPTS: usize = LDE_SIZE * 2;
/// Hard fail-closed bound for deriving one unbiased extension-field challenge.
///
/// Each of the four uniformly random 64-bit coefficients rejects exactly when
/// it is at least the Goldilocks modulus, a `2^-32` fraction.  Thus one tuple
/// rejects with probability below `2^-30`, and all 256 domain-separated
/// attempts fail with probability below `2^-7680` in the random-oracle model.
/// Zero is admitted here; the separate DEEP sampler additionally rejects zero
/// and every point in either evaluation domain.
const MAX_FIELD_CHALLENGE_DERIVATION_ATTEMPTS: usize = 256;

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
const FRI_MASK_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:fri-mask-leaf:v1";
const FRI_LEAF_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:fri-leaf:v1";
const MERKLE_NODE_DOMAIN: &[u8] = b"iroha:privacy:zk-ace:merkle-node:v1";

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

struct MaskedTraceMaterial {
    lde_columns: Vec<Vec<F>>,
    masks: Vec<ReplayableTraceMaskV1>,
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
    layers: Vec<Vec<E>>,
    trees: Vec<MerkleTree>,
    roots: Vec<[u8; 32]>,
    terminal_values: Vec<E>,
}

struct FriMaskMaterial {
    values: Vec<E>,
    tree: MerkleTree,
}

impl Drop for FriMaskMaterial {
    fn drop(&mut self) {
        self.values.fill(E::ZERO);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZkAceStarkProofV1 {
    version: u16,
    trace_root: [u8; 32],
    composition_roots: Vec<[u8; 32]>,
    fri_mask_roots: Vec<[u8; 32]>,
    deep_trace_current: Vec<E>,
    deep_trace_next: Vec<E>,
    deep_composition_values: Vec<E>,
    fri_lanes: Vec<ZkAceFriLaneProofV1>,
    queries: Vec<ZkAceQueryProofV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZkAceFriLaneProofV1 {
    roots: Vec<[u8; 32]>,
    terminal_values: Vec<E>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZkAceQueryProofV1 {
    index: u32,
    current_row: Vec<u64>,
    next_row: Vec<u64>,
    current_row_path: Vec<[u8; 32]>,
    next_row_path: Vec<[u8; 32]>,
    composition_values: Vec<E>,
    composition_paths: Vec<Vec<[u8; 32]>>,
    fri_mask_values: Vec<E>,
    fri_mask_paths: Vec<Vec<[u8; 32]>>,
    fri_lanes: Vec<ZkAceFriLaneQueryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZkAceFriLaneQueryV1 {
    rounds: Vec<ZkAceFriRoundOpeningV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ZkAceFriRoundOpeningV1 {
    low: E,
    high: E,
    low_path: Vec<[u8; 32]>,
    high_path: Vec<[u8; 32]>,
}

/// Failure returned by the dedicated ZK-ACE STARK.
#[derive(Debug, Error)]
pub(crate) enum ZkAceStarkError {
    #[error("ZK-ACE public inputs do not match the compiled transfer relation")]
    InvalidPublicInputs,
    #[error("ZK-ACE public account identifiers cannot be encoded canonically")]
    PublicInputEncoding,
    #[error("ZK-ACE public digest is not a canonical Goldilocks field encoding")]
    NonCanonicalPublicDigest,
    #[error("ZK-ACE witness cannot be packed into the compiled 32-byte limb layout")]
    WitnessPacking,
    #[error("ZK-ACE witness does not satisfy the public commitment/nullifier relation")]
    WitnessRelation,
    #[error("cryptographic randomness is unavailable for ZK-ACE trace masking")]
    RandomnessUnavailable,
    #[error("ZK-ACE prover randomness failed its catastrophic-prefix health check")]
    RandomnessUnhealthy,
    #[error("ZK-ACE proof exceeds the compiled byte ceiling")]
    ProofTooLarge,
    #[error("ZK-ACE proof is malformed")]
    MalformedProof,
    #[error("memory for the exact bounded ZK-ACE proof shape is unavailable")]
    ProofAllocationUnavailable,
    #[error("ZK-ACE proof shape does not match the compiled profile")]
    ProfileMismatch,
    #[error("ZK-ACE proof contains a non-canonical field element")]
    NonCanonicalField,
    #[error("ZK-ACE proof transcript or query schedule is inconsistent")]
    TranscriptMismatch,
    #[error("ZK-ACE field-challenge rejection sampler exhausted its compiled attempt bound")]
    ChallengeDerivationExhausted,
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

fn exact_vec<T>(capacity: usize) -> Result<Vec<T>, ZkAceStarkError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ZkAceStarkError::ProofAllocationUnavailable)?;
    Ok(values)
}

#[derive(Clone, Copy)]
struct FriTheoremProfileV1 {
    multiplicity_parameter: usize,
    code_degree_bound_exclusive: usize,
    domain_size: usize,
    agreement_squared_numerator: usize,
    agreement_squared_denominator: usize,
    query_count: usize,
    query_sampling_without_replacement: bool,
    deep_candidate_degree_bound_exclusive: usize,
    deep_identity_degree_bound: usize,
    deep_constraint_count: usize,
    deep_excluded_point_count: usize,
    fold_count: usize,
    terminal_size: usize,
    terminal_degree_bound: usize,
    affine_commit_factor_numerator: usize,
    affine_commit_factor_denominator: usize,
    affine_oracle_count: usize,
    affine_random_coefficients: usize,
    reduction_arities: [usize; FRI_ROUNDS],
    fri_mask_coefficients: usize,
    protocol3_optimized_degree_bound_exclusive: usize,
    maximum_structured_batch_degree: usize,
    domains_are_smooth_and_disjoint: bool,
}

fn compiled_fri_theorem_profile_v1() -> Result<FriTheoremProfileV1, ZkAceStarkError> {
    let lde_root = primitive_root(LDE_LOG2)?;
    let trace_root = primitive_root(TRACE_LOG2)?;
    let domains_are_smooth_and_disjoint = LDE_SIZE % TRACE_SIZE == 0
        && lde_root.pow((LDE_SIZE / TRACE_SIZE) as u128) == trace_root
        && F(FIELD_GENERATOR).pow(LDE_SIZE as u128) != F::ONE;
    Ok(FriTheoremProfileV1 {
        multiplicity_parameter: FRI_MULTIPLICITY_PARAMETER,
        code_degree_bound_exclusive: FRI_CODE_DEGREE_BOUND_EXCLUSIVE,
        domain_size: LDE_SIZE,
        agreement_squared_numerator: 49,
        agreement_squared_denominator: 288,
        query_count: QUERY_COUNT,
        query_sampling_without_replacement: true,
        deep_candidate_degree_bound_exclusive: DEEP_CANDIDATE_DEGREE_BOUND_EXCLUSIVE,
        deep_identity_degree_bound: DEEP_IDENTITY_DEGREE_BOUND,
        deep_constraint_count: CONSTRAINT_COUNT,
        deep_excluded_point_count: DEEP_EXCLUDED_POINT_COUNT,
        fold_count: FRI_ROUNDS,
        terminal_size: TERMINAL_SIZE,
        terminal_degree_bound: TERMINAL_DEGREE_BOUND,
        affine_commit_factor_numerator: 3,
        affine_commit_factor_denominator: 2,
        affine_oracle_count: FRI_AFFINE_ORACLE_COUNT,
        affine_random_coefficients: FRI_AFFINE_RANDOM_COEFFICIENTS,
        reduction_arities: FRI_REDUCTION_ARITIES,
        fri_mask_coefficients: FRI_MASK_COEFFICIENTS,
        protocol3_optimized_degree_bound_exclusive: PROTOCOL3_OPTIMIZED_DEGREE_BOUND_EXCLUSIVE,
        maximum_structured_batch_degree: MAX_STRUCTURED_BATCH_DEGREE,
        domains_are_smooth_and_disjoint,
    })
}

fn validate_fri_theorem_profile_v1(profile: FriTheoremProfileV1) -> Result<(), ZkAceStarkError> {
    let reduced_domain_size = profile
        .domain_size
        .checked_shr(
            u32::try_from(profile.fold_count).map_err(|_| ZkAceStarkError::ProfileMismatch)?,
        )
        .ok_or(ZkAceStarkError::ProfileMismatch)?;
    let reduced_code_degree = profile
        .code_degree_bound_exclusive
        .checked_shr(
            u32::try_from(profile.fold_count).map_err(|_| ZkAceStarkError::ProfileMismatch)?,
        )
        .ok_or(ZkAceStarkError::ProfileMismatch)?;
    let reduction_arity_sum = profile
        .reduction_arities
        .iter()
        .try_fold(0_usize, |sum, arity| sum.checked_add(*arity))
        .ok_or(ZkAceStarkError::ProfileMismatch)?;
    let terminal_code_dimension = profile
        .terminal_degree_bound
        .checked_add(1)
        .ok_or(ZkAceStarkError::ProfileMismatch)?;

    // For rho=1/8 and m=3, alpha=1-theta=7/(12*sqrt(2)),
    // hence alpha^2=49/288.  The exact integer comparisons below establish:
    //   (1-rho)/2 < theta < 1-sqrt(rho), and
    //   alpha > sqrt((1+1/m)*rho), the Guruswami--Sudan agreement threshold.
    let agreement_numerator = profile.agreement_squared_numerator as u128;
    let agreement_denominator = profile.agreement_squared_denominator as u128;
    let theta_above_unique_radius = agreement_numerator * 256 < 81 * agreement_denominator;
    let theta_below_johnson_radius = agreement_numerator * 8 > agreement_denominator;
    let correlated_agreement_is_admissible = agreement_numerator * 6 > agreement_denominator;

    if profile.multiplicity_parameter != FRI_MULTIPLICITY_PARAMETER
        || profile.multiplicity_parameter < 3
        || profile.code_degree_bound_exclusive != FRI_CODE_DEGREE_BOUND_EXCLUSIVE
        || profile.domain_size != LDE_SIZE
        || !profile.code_degree_bound_exclusive.is_power_of_two()
        || !profile.domain_size.is_power_of_two()
        || profile.domain_size % profile.code_degree_bound_exclusive != 0
        || profile.domain_size / profile.code_degree_bound_exclusive != 8
        || profile.agreement_squared_numerator != 49
        || profile.agreement_squared_denominator != 288
        || profile.agreement_squared_denominator == 0
        || profile.query_count != QUERY_COUNT
        || profile.query_count == 0
        || profile.query_count > profile.domain_size
        || !profile.query_sampling_without_replacement
        || profile.deep_candidate_degree_bound_exclusive != profile.code_degree_bound_exclusive + 2
        || profile.deep_candidate_degree_bound_exclusive != DEEP_CANDIDATE_DEGREE_BOUND_EXCLUSIVE
        || profile.deep_identity_degree_bound != DEEP_IDENTITY_DEGREE_BOUND
        || profile.deep_identity_degree_bound
            != AIR_TOTAL_DEGREE_V1 * (profile.deep_candidate_degree_bound_exclusive - 1)
                + (profile.code_degree_bound_exclusive - 1)
        || profile.deep_constraint_count != CONSTRAINT_COUNT
        || profile.deep_excluded_point_count != DEEP_EXCLUDED_POINT_COUNT
        || profile.deep_excluded_point_count != 1 + TRACE_SIZE + profile.domain_size
        || profile.fold_count != FRI_ROUNDS
        || reduced_domain_size != profile.terminal_size
        || reduced_code_degree != terminal_code_dimension
        || profile.terminal_size != TERMINAL_SIZE
        || profile.terminal_degree_bound != TERMINAL_DEGREE_BOUND
        || profile.affine_commit_factor_numerator != 3
        || profile.affine_commit_factor_denominator != 2
        || profile.affine_oracle_count != FRI_AFFINE_ORACLE_COUNT
        || profile.affine_random_coefficients != FRI_AFFINE_RANDOM_COEFFICIENTS
        || profile.affine_random_coefficients + 1 != profile.affine_oracle_count
        || profile.reduction_arities != FRI_REDUCTION_ARITIES
        || reduction_arity_sum != 24
        || profile.fri_mask_coefficients != FRI_MASK_COEFFICIENTS
        || profile.fri_mask_coefficients + 1 != profile.code_degree_bound_exclusive
        || profile.protocol3_optimized_degree_bound_exclusive
            != PROTOCOL3_OPTIMIZED_DEGREE_BOUND_EXCLUSIVE
        || profile.protocol3_optimized_degree_bound_exclusive != 5_120
        || profile.protocol3_optimized_degree_bound_exclusive > profile.code_degree_bound_exclusive
        || profile.maximum_structured_batch_degree != MAX_STRUCTURED_BATCH_DEGREE
        || profile.maximum_structured_batch_degree != 5_117
        || profile.maximum_structured_batch_degree >= profile.fri_mask_coefficients
        || !profile.domains_are_smooth_and_disjoint
        || !theta_above_unique_radius
        || !theta_below_johnson_radius
        || !correlated_agreement_is_admissible
    {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_security_profile_geometry_v1(
    air_total_degree: usize,
    reduced_air_degree: usize,
    trace_mask_coefficients: usize,
    target_bits: u16,
    round_by_round_bits: u16,
    random_oracle_bits: u16,
    max_random_oracle_query_log2: u16,
) -> Result<(), ZkAceStarkError> {
    if air_total_degree != AIR_TOTAL_DEGREE_V1
        || air_total_degree.checked_sub(1) != Some(reduced_air_degree)
        || reduced_air_degree != REDUCED_AIR_DEGREE_V1
        || trace_mask_coefficients != TRACE_MASK_COEFFICIENTS
        || target_bits != PROVABLE_SOUNDNESS_BITS_V1
        || round_by_round_bits != ROUND_BY_ROUND_SECURITY_BITS_V1
        || random_oracle_bits != RANDOM_ORACLE_BITS_V1
        || max_random_oracle_query_log2 != u16::from(MAX_ROM_QUERY_LOG2_V1)
    {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    let geometry = transparent_stark_zk_mask_geometry_v1(
        reduced_air_degree,
        CHALLENGE_EXTENSION_DEGREE,
        DEEP_QUERY_COUNT,
        QUERY_COUNT,
    )
    .map_err(|_| ZkAceStarkError::ProfileMismatch)?;
    if geometry.minimum_mask_coefficients != 332
        || geometry.minimum_mask_degree != 331
        || trace_mask_coefficients < geometry.minimum_mask_coefficients
    {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    checked_transparent_stark_work_security_v1(
        target_bits,
        round_by_round_bits,
        random_oracle_bits,
        max_random_oracle_query_log2,
    )
    .map_err(|_| ZkAceStarkError::ProfileMismatch)?;
    validate_fri_theorem_profile_v1(compiled_fri_theorem_profile_v1()?)?;
    Ok(())
}

fn validate_compiled_security_profile_v1() -> Result<(), ZkAceStarkError> {
    validate_security_profile_geometry_v1(
        AIR_TOTAL_DEGREE_V1,
        REDUCED_AIR_DEGREE_V1,
        TRACE_MASK_COEFFICIENTS,
        PROVABLE_SOUNDNESS_BITS_V1,
        ROUND_BY_ROUND_SECURITY_BITS_V1,
        RANDOM_ORACLE_BITS_V1,
        u16::from(MAX_ROM_QUERY_LOG2_V1),
    )
}

struct ProofReaderV1<'a> {
    inner: ExactProofReaderV1<'a>,
}

impl<'a> ProofReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            inner: ExactProofReaderV1::new(bytes),
        }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], ZkAceStarkError> {
        self.inner
            .take()
            .map_err(|_| ZkAceStarkError::MalformedProof)
    }

    fn u16(&mut self) -> Result<u16, ZkAceStarkError> {
        self.inner
            .u16()
            .map_err(|_| ZkAceStarkError::MalformedProof)
    }

    fn u32(&mut self) -> Result<u32, ZkAceStarkError> {
        self.inner
            .u32()
            .map_err(|_| ZkAceStarkError::MalformedProof)
    }

    fn u64(&mut self) -> Result<u64, ZkAceStarkError> {
        self.inner
            .u64()
            .map_err(|_| ZkAceStarkError::MalformedProof)
    }

    fn hashes(&mut self, count: usize) -> Result<Vec<[u8; 32]>, ZkAceStarkError> {
        let mut hashes = exact_vec(count)?;
        for _ in 0..count {
            hashes.push(self.take()?);
        }
        Ok(hashes)
    }

    fn fields(&mut self, count: usize) -> Result<Vec<u64>, ZkAceStarkError> {
        let mut fields = exact_vec(count)?;
        for _ in 0..count {
            fields.push(self.u64()?);
        }
        Ok(fields)
    }

    fn fp4s(&mut self, count: usize) -> Result<Vec<E>, ZkAceStarkError> {
        let mut fields = exact_vec(count)?;
        for _ in 0..count {
            fields.push(self.inner.fp4().map_err(|error| match error {
                TransparentStarkErrorV1::NonCanonicalField => ZkAceStarkError::NonCanonicalField,
                _ => ZkAceStarkError::MalformedProof,
            })?);
        }
        Ok(fields)
    }

    fn finish(self) -> Result<(), ZkAceStarkError> {
        self.inner
            .finish()
            .map_err(|_| ZkAceStarkError::MalformedProof)
    }
}

fn append_hashes(bytes: &mut Vec<u8>, hashes: &[[u8; 32]]) {
    for hash in hashes {
        bytes.extend_from_slice(hash);
    }
}

fn append_fields(bytes: &mut Vec<u8>, fields: &[u64]) {
    for field in fields {
        append_u64(bytes, *field);
    }
}

fn append_fp4s(bytes: &mut Vec<u8>, fields: &[E]) {
    for field in fields {
        append_goldilocks_fp4_v1(bytes, *field);
    }
}

fn encode_zk_ace_stark_proof_v1(proof: &ZkAceStarkProofV1) -> Result<Vec<u8>, ZkAceStarkError> {
    validate_proof_shape(proof)?;
    let mut bytes = exact_vec(CANONICAL_PROOF_BYTES_V1)?;
    bytes.extend_from_slice(&PROOF_WIRE_MAGIC_V1);
    append_u16(&mut bytes, proof.version);
    bytes.extend_from_slice(&proof.trace_root);
    append_hashes(&mut bytes, &proof.composition_roots);
    append_hashes(&mut bytes, &proof.fri_mask_roots);
    append_fp4s(&mut bytes, &proof.deep_trace_current);
    append_fp4s(&mut bytes, &proof.deep_trace_next);
    append_fp4s(&mut bytes, &proof.deep_composition_values);
    for lane in &proof.fri_lanes {
        append_hashes(&mut bytes, &lane.roots);
        append_fp4s(&mut bytes, &lane.terminal_values);
    }
    for query in &proof.queries {
        append_u32(&mut bytes, query.index);
        append_fields(&mut bytes, &query.current_row);
        append_fields(&mut bytes, &query.next_row);
        append_hashes(&mut bytes, &query.current_row_path);
        append_hashes(&mut bytes, &query.next_row_path);
        append_fp4s(&mut bytes, &query.composition_values);
        for path in &query.composition_paths {
            append_hashes(&mut bytes, path);
        }
        append_fp4s(&mut bytes, &query.fri_mask_values);
        for path in &query.fri_mask_paths {
            append_hashes(&mut bytes, path);
        }
        for lane in &query.fri_lanes {
            for opening in &lane.rounds {
                append_goldilocks_fp4_v1(&mut bytes, opening.low);
                append_goldilocks_fp4_v1(&mut bytes, opening.high);
                append_hashes(&mut bytes, &opening.low_path);
                append_hashes(&mut bytes, &opening.high_path);
            }
        }
    }
    if bytes.len() != CANONICAL_PROOF_BYTES_V1 {
        return Err(ZkAceStarkError::InternalInvariant(
            "fixed-shape proof encoder length mismatch",
        ));
    }
    Ok(bytes)
}

fn decode_zk_ace_stark_proof_v1(proof_bytes: &[u8]) -> Result<ZkAceStarkProofV1, ZkAceStarkError> {
    if proof_bytes.len() != CANONICAL_PROOF_BYTES_V1 {
        return Err(ZkAceStarkError::MalformedProof);
    }
    let mut reader = ProofReaderV1::new(proof_bytes);
    if reader.take::<4>()? != PROOF_WIRE_MAGIC_V1 {
        return Err(ZkAceStarkError::MalformedProof);
    }
    let version = reader.u16()?;
    if version != PROOF_VERSION {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    let trace_root = reader.take()?;
    let composition_roots = reader.hashes(SECURITY_LANES)?;
    let fri_mask_roots = reader.hashes(SECURITY_LANES)?;
    let deep_trace_current = reader.fp4s(TRACE_WIDTH)?;
    let deep_trace_next = reader.fp4s(TRACE_WIDTH)?;
    let deep_composition_values = reader.fp4s(SECURITY_LANES)?;
    let mut fri_lanes = exact_vec(SECURITY_LANES)?;
    for _ in 0..SECURITY_LANES {
        fri_lanes.push(ZkAceFriLaneProofV1 {
            roots: reader.hashes(FRI_ROUNDS + 1)?,
            terminal_values: reader.fp4s(TERMINAL_SIZE)?,
        });
    }
    let mut queries = exact_vec(QUERY_COUNT)?;
    for _ in 0..QUERY_COUNT {
        let index = reader.u32()?;
        let current_row = reader.fields(TRACE_WIDTH)?;
        let next_row = reader.fields(TRACE_WIDTH)?;
        let current_row_path = reader.hashes(LDE_LOG2 as usize)?;
        let next_row_path = reader.hashes(LDE_LOG2 as usize)?;
        let composition_values = reader.fp4s(SECURITY_LANES)?;
        let mut composition_paths = exact_vec(SECURITY_LANES)?;
        for _ in 0..SECURITY_LANES {
            composition_paths.push(reader.hashes(LDE_LOG2 as usize)?);
        }
        let fri_mask_values = reader.fp4s(SECURITY_LANES)?;
        let mut fri_mask_paths = exact_vec(SECURITY_LANES)?;
        for _ in 0..SECURITY_LANES {
            fri_mask_paths.push(reader.hashes(LDE_LOG2 as usize)?);
        }
        let mut query_fri_lanes = exact_vec(SECURITY_LANES)?;
        for _ in 0..SECURITY_LANES {
            let mut rounds = exact_vec(FRI_ROUNDS)?;
            for round in 0..FRI_ROUNDS {
                let depth = LDE_LOG2 as usize - round;
                rounds.push(ZkAceFriRoundOpeningV1 {
                    low: reader.inner.fp4().map_err(|error| match error {
                        TransparentStarkErrorV1::NonCanonicalField => {
                            ZkAceStarkError::NonCanonicalField
                        }
                        _ => ZkAceStarkError::MalformedProof,
                    })?,
                    high: reader.inner.fp4().map_err(|error| match error {
                        TransparentStarkErrorV1::NonCanonicalField => {
                            ZkAceStarkError::NonCanonicalField
                        }
                        _ => ZkAceStarkError::MalformedProof,
                    })?,
                    low_path: reader.hashes(depth)?,
                    high_path: reader.hashes(depth)?,
                });
            }
            query_fri_lanes.push(ZkAceFriLaneQueryV1 { rounds });
        }
        queries.push(ZkAceQueryProofV1 {
            index,
            current_row,
            next_row,
            current_row_path,
            next_row_path,
            composition_values,
            composition_paths,
            fri_mask_values,
            fri_mask_paths,
            fri_lanes: query_fri_lanes,
        });
    }
    reader.finish()?;
    let proof = ZkAceStarkProofV1 {
        version,
        trace_root,
        composition_roots,
        fri_mask_roots,
        deep_trace_current,
        deep_trace_next,
        deep_composition_values,
        fri_lanes,
        queries,
    };
    validate_proof_shape(&proof)?;
    Ok(proof)
}

fn trace_leaf_hash(row: &[F]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRACE_LEAF_DOMAIN);
    hasher.update((row.len() as u64).to_be_bytes());
    for value in row {
        hasher.update(value.0.to_be_bytes());
    }
    hasher.finalize().into()
}

fn composition_leaf_hash(lane: usize, value: E) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(COMPOSITION_LEAF_DOMAIN);
    hasher.update((lane as u64).to_be_bytes());
    hasher.update(value.to_be_bytes());
    hasher.finalize().into()
}

fn fri_mask_leaf_hash(lane: usize, value: E) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FRI_MASK_LEAF_DOMAIN);
    hasher.update((lane as u64).to_be_bytes());
    hasher.update(value.to_be_bytes());
    hasher.finalize().into()
}

fn fri_leaf_hash(lane: usize, round: usize, value: E) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FRI_LEAF_DOMAIN);
    hasher.update((lane as u64).to_be_bytes());
    hasher.update((round as u64).to_be_bytes());
    hasher.update(value.to_be_bytes());
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
    goldilocks_primitive_root_v1(log_size).map_err(|_| {
        ZkAceStarkError::InternalInvariant(
            "requested FFT domain must have a primitive Goldilocks root",
        )
    })
}

#[cfg(test)]
fn fft(values: &mut [F], root: F) -> Result<(), ZkAceStarkError> {
    goldilocks_fft_v1(values, root).map_err(|_| {
        ZkAceStarkError::InternalInvariant("FFT requires an exact power-of-two Goldilocks domain")
    })
}

fn ifft(values: &mut [F], root: F) -> Result<(), ZkAceStarkError> {
    goldilocks_ifft_v1(values, root).map_err(|_| {
        ZkAceStarkError::InternalInvariant(
            "inverse FFT requires an exact power-of-two Goldilocks domain",
        )
    })
}

fn evaluate_coefficients_on_coset(
    coefficients: &[F],
    size: usize,
    root: F,
    shift: F,
) -> Result<Vec<F>, ZkAceStarkError> {
    goldilocks_evaluate_coset_v1(coefficients, size, root, shift).map_err(|_| {
        ZkAceStarkError::InternalInvariant("invalid coefficient/coset evaluation shape")
    })
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

fn identity_message_words(public_inputs: &ZkAceAirRelationInputsV1) -> Vec<MessageWord> {
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

fn replay_message_words(public_inputs: &ZkAceAirRelationInputsV1) -> Vec<MessageWord> {
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
    public_inputs: &ZkAceAirRelationInputsV1,
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

fn witness_limbs(witness: &ZkAcePrivacyWitnessV1) -> Result<[F; PRIVATE_LIMBS], ZkAceStarkError> {
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

fn public_output_words(
    public_inputs: &ZkAceAirRelationInputsV1,
) -> Result<[F; 8], ZkAceStarkError> {
    let mut words = [F::ZERO; 8];
    for (word_index, chunk) in public_inputs
        .identity_commitment
        .chunks_exact(8)
        .chain(public_inputs.replay_nullifier.chunks_exact(8))
        .enumerate()
    {
        // Data-model Poseidon2 digests are four canonical Goldilocks residues
        // encoded little-endian. Proof-field elements use big-endian on the
        // outer ZKA1 wire, but changing byte order at this relation boundary
        // would compare the AIR against different field elements.
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

fn apply_mds_extension(state: [E; 3]) -> [E; 3] {
    let mds = poseidon_manifest().mds();
    let mut result = [E::ZERO; 3];
    for row in 0..3 {
        for (column, value) in state.iter().copied().enumerate() {
            result[row] = result[row].add(value.mul_base(F(mds[row][column])));
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
    public_inputs: &ZkAceAirRelationInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
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

fn masked_lde_columns<R: TryRngCore>(
    base_columns: &[Vec<F>],
    rng: &mut R,
) -> Result<MaskedTraceMaterial, ZkAceStarkError> {
    let map_error = |error| match error {
        TransparentStarkErrorV1::RandomnessUnavailable => ZkAceStarkError::RandomnessUnavailable,
        TransparentStarkErrorV1::AllocationFailure => ZkAceStarkError::ProofAllocationUnavailable,
        _ => ZkAceStarkError::InternalInvariant("compiled trace-masking LDE shape is invalid"),
    };
    let mut lde_columns = Vec::with_capacity(base_columns.len());
    let mut masks = Vec::with_capacity(base_columns.len());
    for column in base_columns {
        let mask = sample_trace_mask_v1(MASK_DEGREE, rng).map_err(map_error)?;
        let lde =
            masked_trace_lde_column_with_mask_v1(column, TRACE_LOG2, LDE_LOG2, mask.coefficients())
                .map_err(map_error)?;
        lde_columns.push(lde);
        masks.push(mask);
    }
    Ok(MaskedTraceMaterial { lde_columns, masks })
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
    goldilocks_batch_invert_v1(values).map_err(|error| match error {
        TransparentStarkErrorV1::AllocationFailure => ZkAceStarkError::ProofAllocationUnavailable,
        _ => ZkAceStarkError::InternalInvariant("batch inversion input must be non-zero"),
    })
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

fn evaluate_base_coefficients_at_extension(coefficients: &[F], point: E) -> E {
    coefficients
        .iter()
        .rev()
        .fold(E::ZERO, |value, coefficient| {
            value.mul(point).add(E::from_base(*coefficient))
        })
}

fn evaluate_extension_coefficients(coefficients: &[E], point: E) -> E {
    coefficients
        .iter()
        .rev()
        .fold(E::ZERO, |value, coefficient| {
            value.mul(point).add(*coefficient)
        })
}

fn masked_trace_column_at_extension(
    base_column: &[F],
    mask: &ReplayableTraceMaskV1,
    point: E,
) -> Result<E, ZkAceStarkError> {
    if base_column.len() != TRACE_SIZE || mask.coefficients().len() != MASK_DEGREE + 1 {
        return Err(ZkAceStarkError::InternalInvariant(
            "masked extension evaluation shape mismatch",
        ));
    }
    let mut coefficients = base_column.to_vec();
    ifft(&mut coefficients, primitive_root(TRACE_LOG2)?)?;
    let base_value = evaluate_base_coefficients_at_extension(&coefficients, point);
    coefficients.fill(F::ZERO);
    let mask_value = evaluate_base_coefficients_at_extension(mask.coefficients(), point);
    Ok(base_value.add(point.pow(TRACE_SIZE as u128).sub(E::ONE).mul(mask_value)))
}

fn masked_trace_rows_at_deep_point(
    base_columns: &[Vec<F>],
    masks: &[ReplayableTraceMaskV1],
    point: E,
) -> Result<(Vec<E>, Vec<E>), ZkAceStarkError> {
    if base_columns.len() != TRACE_WIDTH || masks.len() != TRACE_WIDTH {
        return Err(ZkAceStarkError::InternalInvariant(
            "masked DEEP trace shape mismatch",
        ));
    }
    let trace_step = E::from_base(primitive_root(TRACE_LOG2)?);
    let next_point = point.mul(trace_step);
    let current = base_columns
        .iter()
        .zip(masks)
        .map(|(column, mask)| masked_trace_column_at_extension(column, mask, point))
        .collect::<Result<Vec<_>, _>>()?;
    let next = base_columns
        .iter()
        .zip(masks)
        .map(|(column, mask)| masked_trace_column_at_extension(column, mask, next_point))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((current, next))
}

fn fixed_row_at_extension_point(
    schedule: &[ScheduleRow],
    point: E,
) -> Result<Vec<E>, ZkAceStarkError> {
    if schedule.len() != TRACE_SIZE || point.pow(TRACE_SIZE as u128) == E::ONE {
        return Err(ZkAceStarkError::InternalInvariant(
            "fixed-row DEEP evaluation point has invalid shape/domain",
        ));
    }
    let trace_root = primitive_root(TRACE_LOG2)?;
    let mut trace_point = F::ONE;
    let mut trace_points = Vec::with_capacity(TRACE_SIZE);
    let mut inverse_denominators = Vec::with_capacity(TRACE_SIZE);
    for _ in 0..TRACE_SIZE {
        trace_points.push(trace_point);
        inverse_denominators.push(point.sub(E::from_base(trace_point)));
        trace_point = trace_point.mul(trace_root);
    }
    batch_invert_extension(&mut inverse_denominators)?;
    let mut result = vec![E::ZERO; FIXED_WIDTH];
    let inverse_trace_size =
        F::reduce(TRACE_SIZE as u128)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "trace size must be invertible",
            ))?;
    let common = point
        .pow(TRACE_SIZE as u128)
        .sub(E::ONE)
        .mul_base(inverse_trace_size);
    for ((schedule_row, trace_point), inverse) in
        schedule.iter().zip(trace_points).zip(inverse_denominators)
    {
        let weight = common.mul_base(trace_point).mul(inverse);
        accumulate_fixed_row_extension(&mut result, *schedule_row, weight);
    }
    Ok(result)
}

fn accumulate_fixed_row_extension(result: &mut [E], schedule_row: ScheduleRow, weight: E) {
    let mut add = |column: usize, value: F| {
        result[column] = result[column].add(weight.mul_base(value));
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
                MessageWord::Constant(value) => add(FIX_MESSAGE_CONST, F(value)),
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
        ScheduleOp::Output { output_index } => add(FIX_OUTPUT_OFFSET + output_index, F::ONE),
    }
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
/// Number of distinct quartic-extension challenges in one proof transcript.
#[cfg(test)]
const DISTINCT_FIELD_CHALLENGE_COUNT: usize =
    1 + SECURITY_LANES * (CONSTRAINT_COUNT + TRACE_WIDTH + 1 + FRI_ROUNDS);
/// Absolute SHA-256 call budget for all field-challenge rejection samplers.
#[cfg(test)]
const MAX_FIELD_CHALLENGE_HASH_CALLS: usize =
    DISTINCT_FIELD_CHALLENGE_COUNT * MAX_FIELD_CHALLENGE_DERIVATION_ATTEMPTS;

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
            &(FRI_MASK_COEFFICIENTS as u64).to_be_bytes(),
            public_digest,
            trace_root,
        ],
    )
}

fn challenge_from_digest_stream(
    mut digest_for_attempt: impl FnMut(u64) -> [u8; 32],
) -> Result<E, ZkAceStarkError> {
    for attempt in 0..MAX_FIELD_CHALLENGE_DERIVATION_ATTEMPTS {
        let digest = digest_for_attempt(attempt as u64);
        if let Some(challenge) = E::canonical_be_bytes(digest) {
            return Ok(challenge);
        }
    }
    Err(ZkAceStarkError::ChallengeDerivationExhausted)
}

fn challenge_field(
    seed: &[u8; 32],
    label: &[u8],
    lane: usize,
    index: usize,
) -> Result<E, ZkAceStarkError> {
    challenge_from_digest_stream(|attempt| {
        hash_parts(
            b"iroha:privacy:zk-ace:field-challenge:v1",
            &[
                seed,
                label,
                &(lane as u64).to_be_bytes(),
                &(index as u64).to_be_bytes(),
                &attempt.to_be_bytes(),
            ],
        )
    })
}

fn challenge_vector(
    seed: &[u8; 32],
    label: &[u8],
    lane: usize,
    count: usize,
) -> Result<Vec<E>, ZkAceStarkError> {
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

fn batch_seed(
    composition_seed: &[u8; 32],
    deep_trace_current: &[E],
    deep_trace_next: &[E],
    deep_composition_values: &[E],
    fri_mask_roots: &[[u8; 32]],
) -> Result<[u8; 32], ZkAceStarkError> {
    if deep_trace_current.len() != TRACE_WIDTH
        || deep_trace_next.len() != TRACE_WIDTH
        || deep_composition_values.len() != SECURITY_LANES
        || fri_mask_roots.len() != SECURITY_LANES
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "DEEP/batch transcript shape does not match the compiled profile",
        ));
    }
    let mut encoded_deep =
        Vec::with_capacity((2 * TRACE_WIDTH + SECURITY_LANES) * EXTENSION_FIELD_BYTES);
    for value in deep_trace_current
        .iter()
        .chain(deep_trace_next)
        .chain(deep_composition_values)
    {
        encoded_deep.extend_from_slice(&value.to_be_bytes());
    }
    let mut encoded_roots = Vec::with_capacity(fri_mask_roots.len() * HASH_BYTES);
    for root in fri_mask_roots {
        encoded_roots.extend_from_slice(root);
    }
    Ok(hash_parts(
        b"iroha:privacy:zk-ace:batch-transcript:v1",
        &[composition_seed, &encoded_deep, &encoded_roots],
    ))
}

fn fri_lane_seed(batch_seed: &[u8; 32], lane: usize) -> [u8; 32] {
    hash_parts(
        b"iroha:privacy:zk-ace:fri-lane-transcript:v1",
        &[batch_seed, &(lane as u64).to_be_bytes()],
    )
}

fn fri_beta(
    lane_seed: &[u8; 32],
    lane: usize,
    round: usize,
    layer_root: &[u8; 32],
) -> Result<E, ZkAceStarkError> {
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

fn deep_seed(composition_seed: &[u8; 32]) -> [u8; 32] {
    hash_parts(
        b"iroha:privacy:zk-ace:deep-transcript:v1",
        &[composition_seed],
    )
}

fn is_base_domain_point(value: E) -> Result<bool, ZkAceStarkError> {
    let coefficients = value.coefficients();
    if coefficients[1..]
        .iter()
        .any(|coefficient| *coefficient != F::ZERO)
    {
        return Ok(false);
    }
    let point = coefficients[0];
    if point == F::ZERO || point.pow(TRACE_SIZE as u128) == F::ONE {
        return Ok(true);
    }
    let generator_inverse = F(FIELD_GENERATOR)
        .inv()
        .ok_or(ZkAceStarkError::InternalInvariant(
            "compiled coset generator must be invertible",
        ))?;
    Ok(point.mul(generator_inverse).pow(LDE_SIZE as u128) == F::ONE)
}

fn challenge_deep_point(seed: &[u8; 32]) -> Result<E, ZkAceStarkError> {
    for attempt in 0..MAX_FIELD_CHALLENGE_DERIVATION_ATTEMPTS {
        let digest = hash_parts(
            b"iroha:privacy:zk-ace:field-challenge:v1",
            &[
                seed,
                b"deep-point",
                &0_u64.to_be_bytes(),
                &0_u64.to_be_bytes(),
                &(attempt as u64).to_be_bytes(),
            ],
        );
        if let Some(candidate) = E::canonical_be_bytes(digest)
            && candidate != E::ZERO
            && !is_base_domain_point(candidate)?
        {
            return Ok(candidate);
        }
    }
    Err(ZkAceStarkError::ChallengeDerivationExhausted)
}

fn query_seed_from_roots(batch_seed: &[u8; 32], lane_roots: &[Vec<[u8; 32]>]) -> [u8; 32] {
    let mut encoded_roots = Vec::new();
    for roots in lane_roots {
        for root in roots {
            encoded_roots.extend_from_slice(root);
        }
    }
    hash_parts(
        b"iroha:privacy:zk-ace:query-transcript:v1",
        &[batch_seed, &encoded_roots],
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
        let raw = u64::from_be_bytes(
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
    x: E,
    current: &[E],
    next: &[E],
    fixed: &[E],
    public_outputs: &[E; 8],
    alphas: &[E],
) -> Result<E, ZkAceStarkError> {
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

fn constraint_quotient_factors(x: E) -> Result<(E, E), ZkAceStarkError> {
    let z_h = x.pow(TRACE_SIZE as u128).sub(E::ONE);
    let inverse_trace_vanishing = z_h.inv().ok_or(ZkAceStarkError::InternalInvariant(
        "LDE point lies in the trace subgroup",
    ))?;
    let trace_root = primitive_root(TRACE_LOG2)?;
    let last_trace_point = trace_root.pow((TRACE_SIZE - 1) as u128);
    let transition_factor = x
        .sub(E::from_base(last_trace_point))
        .mul(inverse_trace_vanishing);
    Ok((inverse_trace_vanishing, transition_factor))
}

fn constraint_quotient_value_with_factors(
    current: &[E],
    next: &[E],
    fixed: &[E],
    public_outputs: &[E; 8],
    alphas: &[E],
    inverse_trace_vanishing: E,
    transition_factor: E,
) -> Result<E, ZkAceStarkError> {
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
    let mut result = E::ZERO;
    let mut absorb_local = |residue: E| {
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
        absorb_local(value.mul(value.sub(E::ONE)));
    }
    let recomposed = (0..LIMB_BITS).fold(E::ZERO, |sum, bit| {
        sum.add(current[BIT_OFFSET + bit].mul_base(F::reduce(1_u128 << bit)))
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
    let hold = E::ONE
        .sub(full)
        .sub(partial)
        .sub(absorb_0)
        .sub(absorb_1)
        .sub(reset);
    let full_state = apply_mds_extension([
        current[X5_OFFSET],
        current[X5_OFFSET + 1],
        current[X5_OFFSET + 2],
    ]);
    let partial_state = apply_mds_extension([
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
                E::ZERO
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
    lane_alphas: &[Vec<E>],
) -> Result<Vec<Vec<E>>, ZkAceStarkError> {
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
    let public_outputs = public_outputs.map(E::from_base);
    let mut lanes = (0..SECURITY_LANES)
        .map(|_| Vec::with_capacity(LDE_SIZE))
        .collect::<Vec<_>>();
    for index in 0..LDE_SIZE {
        let current = row_at(trace_lde, index)?
            .into_iter()
            .map(E::from_base)
            .collect::<Vec<_>>();
        let next = row_at(trace_lde, (index + TERMINAL_SIZE) % LDE_SIZE)?
            .into_iter()
            .map(E::from_base)
            .collect::<Vec<_>>();
        let fixed = row_at(fixed_lde, index)?
            .into_iter()
            .map(E::from_base)
            .collect::<Vec<_>>();
        let inverse_trace_vanishing =
            E::from_base(inverse_vanishing_by_residue[index % TERMINAL_SIZE]);
        let transition_factor = E::from_base(
            x.sub(last_trace_point)
                .mul(inverse_vanishing_by_residue[index % TERMINAL_SIZE]),
        );
        for lane in 0..SECURITY_LANES {
            lanes[lane].push(constraint_quotient_value_with_factors(
                &current,
                &next,
                &fixed,
                &public_outputs,
                &lane_alphas[lane],
                inverse_trace_vanishing,
                transition_factor,
            )?);
        }
        x = x.mul(lde_root);
    }
    Ok(lanes)
}

fn extension_coset_coefficients(values: &[E]) -> Result<Vec<E>, ZkAceStarkError> {
    if values.len() != LDE_SIZE {
        return Err(ZkAceStarkError::InternalInvariant(
            "Fp4 coset interpolation shape mismatch",
        ));
    }
    let mut coefficients = values.to_vec();
    goldilocks_fp4_ifft_v1(&mut coefficients, primitive_root(LDE_LOG2)?).map_err(|_| {
        ZkAceStarkError::InternalInvariant("Fp4 inverse FFT failed on the compiled LDE domain")
    })?;
    let inverse_shift = F(FIELD_GENERATOR)
        .inv()
        .ok_or(ZkAceStarkError::InternalInvariant(
            "compiled coset generator must be invertible",
        ))?;
    let mut inverse_shift_power = F::ONE;
    for coefficient in &mut coefficients {
        *coefficient = coefficient.mul_base(inverse_shift_power);
        inverse_shift_power = inverse_shift_power.mul(inverse_shift);
    }
    Ok(coefficients)
}

fn composition_values_at_deep_point(
    compositions: &[Vec<E>],
    deep_point: E,
) -> Result<Vec<E>, ZkAceStarkError> {
    compositions
        .iter()
        .map(|values| {
            let mut coefficients = extension_coset_coefficients(values)?;
            let value = evaluate_extension_coefficients(&coefficients, deep_point);
            coefficients.fill(E::ZERO);
            Ok(value)
        })
        .collect()
}

fn batch_invert_extension(values: &mut [E]) -> Result<(), ZkAceStarkError> {
    let mut prefixes = Vec::with_capacity(values.len());
    let mut product = E::ONE;
    for value in values.iter().copied() {
        if value == E::ZERO {
            return Err(ZkAceStarkError::InternalInvariant(
                "DEEP denominator must be nonzero",
            ));
        }
        prefixes.push(product);
        product = product.mul(value);
    }
    let mut inverse = product.inv().ok_or(ZkAceStarkError::InternalInvariant(
        "DEEP denominator product must be invertible",
    ))?;
    for index in (0..values.len()).rev() {
        let value = values[index];
        values[index] = inverse.mul(prefixes[index]);
        inverse = inverse.mul(value);
    }
    Ok(())
}

fn deep_trace_interpolant(
    x: E,
    deep_point: E,
    deep_next_point: E,
    inverse_point_delta: E,
    current: E,
    next: E,
) -> E {
    current
        .mul(x.sub(deep_next_point))
        .mul(inverse_point_delta)
        .add(next.mul(x.sub(deep_point)).mul(inverse_point_delta.neg()))
}

fn mix_deep_fri_opening(
    x: F,
    trace_row: &[F],
    composition: E,
    fri_mask: E,
    deep_point: E,
    deep_trace_current: &[E],
    deep_trace_next: &[E],
    deep_composition: E,
    trace_mix: &[E],
    composition_mix: E,
) -> Result<E, ZkAceStarkError> {
    if trace_row.len() != TRACE_WIDTH
        || deep_trace_current.len() != TRACE_WIDTH
        || deep_trace_next.len() != TRACE_WIDTH
        || trace_mix.len() != TRACE_WIDTH
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "DEEP FRI opening shape mismatch",
        ));
    }
    let x = E::from_base(x);
    let deep_next_point = deep_point.mul(E::from_base(primitive_root(TRACE_LOG2)?));
    let inverse_point_delta =
        deep_point
            .sub(deep_next_point)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "DEEP point and its trace translate must differ",
            ))?;
    let inverse_x_minus_deep =
        x.sub(deep_point)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "DEEP point must be outside the FRI domain",
            ))?;
    let inverse_x_minus_next =
        x.sub(deep_next_point)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "translated DEEP point must be outside the FRI domain",
            ))?;
    let inverse_trace_denominator = inverse_x_minus_deep.mul(inverse_x_minus_next);
    let trace_value = trace_row
        .iter()
        .copied()
        .zip(deep_trace_current)
        .zip(deep_trace_next)
        .zip(trace_mix)
        .fold(
            E::ZERO,
            |sum, (((opened, deep_current), deep_next), coefficient)| {
                let interpolant = deep_trace_interpolant(
                    x,
                    deep_point,
                    deep_next_point,
                    inverse_point_delta,
                    *deep_current,
                    *deep_next,
                );
                let quotient = E::from_base(opened)
                    .sub(interpolant)
                    .mul(inverse_trace_denominator);
                sum.add(quotient.mul(*coefficient))
            },
        );
    let composition_quotient = composition.sub(deep_composition).mul(inverse_x_minus_deep);
    Ok(fri_mask
        .add(trace_value)
        .add(composition_quotient.mul(composition_mix)))
}

fn mix_fri_base(
    fri_mask: &[E],
    trace_lde: &[Vec<F>],
    composition: &[E],
    deep_point: E,
    deep_trace_current: &[E],
    deep_trace_next: &[E],
    deep_composition: E,
    trace_mix: &[E],
    composition_mix: E,
) -> Result<Vec<E>, ZkAceStarkError> {
    if fri_mask.len() != LDE_SIZE
        || trace_lde.len() != TRACE_WIDTH
        || trace_lde.iter().any(|column| column.len() != LDE_SIZE)
        || deep_trace_current.len() != TRACE_WIDTH
        || deep_trace_next.len() != TRACE_WIDTH
        || trace_mix.len() != TRACE_WIDTH
        || composition.len() != LDE_SIZE
    {
        return Err(ZkAceStarkError::InternalInvariant(
            "FRI base mixing shape mismatch",
        ));
    }
    let lde_root = primitive_root(LDE_LOG2)?;
    let deep_next_point = deep_point.mul(E::from_base(primitive_root(TRACE_LOG2)?));
    let inverse_point_delta =
        deep_point
            .sub(deep_next_point)
            .inv()
            .ok_or(ZkAceStarkError::InternalInvariant(
                "DEEP point and its trace translate must differ",
            ))?;
    let mut inverse_x_minus_deep = Vec::with_capacity(LDE_SIZE);
    let mut inverse_x_minus_next = Vec::with_capacity(LDE_SIZE);
    let mut x = F(FIELD_GENERATOR);
    for _ in 0..LDE_SIZE {
        let extension_x = E::from_base(x);
        inverse_x_minus_deep.push(extension_x.sub(deep_point));
        inverse_x_minus_next.push(extension_x.sub(deep_next_point));
        x = x.mul(lde_root);
    }
    batch_invert_extension(&mut inverse_x_minus_deep)?;
    batch_invert_extension(&mut inverse_x_minus_next)?;

    let mut result = Vec::with_capacity(LDE_SIZE);
    let mut x = F(FIELD_GENERATOR);
    for index in 0..LDE_SIZE {
        let extension_x = E::from_base(x);
        let inverse_trace_denominator =
            inverse_x_minus_deep[index].mul(inverse_x_minus_next[index]);
        let trace_value = trace_lde
            .iter()
            .zip(deep_trace_current)
            .zip(deep_trace_next)
            .zip(trace_mix)
            .fold(
                E::ZERO,
                |sum, (((column, deep_current), deep_next), coefficient)| {
                    let interpolant = deep_trace_interpolant(
                        extension_x,
                        deep_point,
                        deep_next_point,
                        inverse_point_delta,
                        *deep_current,
                        *deep_next,
                    );
                    let quotient = E::from_base(column[index])
                        .sub(interpolant)
                        .mul(inverse_trace_denominator);
                    sum.add(quotient.mul(*coefficient))
                },
            );
        let composition_quotient = composition[index]
            .sub(deep_composition)
            .mul(inverse_x_minus_deep[index]);
        result.push(
            fri_mask[index]
                .add(trace_value)
                .add(composition_quotient.mul(composition_mix)),
        );
        x = x.mul(lde_root);
    }
    inverse_x_minus_deep.fill(E::ZERO);
    inverse_x_minus_next.fill(E::ZERO);
    Ok(result)
}

fn fri_fold_pair(low: E, high: E, beta: E, x: F) -> Result<E, ZkAceStarkError> {
    fri_fold_pair_fp4_v1(low, high, beta, x)
        .map_err(|_| ZkAceStarkError::InternalInvariant("FRI domain point must be invertible"))
}

fn fri_fold_pair_with_inverse_x(
    low: E,
    high: E,
    beta: E,
    inverse_x: F,
) -> Result<E, ZkAceStarkError> {
    fri_fold_pair_with_inverse_x_fp4_v1(low, high, beta, inverse_x)
        .map_err(|_| ZkAceStarkError::InternalInvariant("two must be invertible in Goldilocks"))
}

fn build_fri_lane(
    base_values: Vec<E>,
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
        let beta = fri_beta(lane_seed, lane, round, &root)?;
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

fn ensure_terminal_degree(values: &[E]) -> Result<(), ZkAceStarkError> {
    ensure_fri_terminal_degree_fp4_v1(values, BLOWUP_LOG2, TERMINAL_DEGREE_BOUND)
        .map_err(|_| ZkAceStarkError::FriDegree)
}

fn validate_relation_inputs(
    public_inputs: &ZkAceAirRelationInputsV1,
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
    )
    .map_err(|_| ZkAceStarkError::InvalidPublicInputs)?;
    if public_inputs.tx_digest != expected_transfer_digest {
        return Err(ZkAceStarkError::InvalidPublicInputs);
    }
    public_output_words(public_inputs)
}

#[cfg(test)]
fn fixed_columns_for_public_inputs(
    public_inputs: &ZkAceAirRelationInputsV1,
) -> Result<Vec<Vec<F>>, ZkAceStarkError> {
    let rows = build_schedule(public_inputs)?
        .into_iter()
        .map(fixed_row)
        .collect::<Vec<_>>();
    transpose_rows(&rows, FIXED_WIDTH)
}

fn composition_tree(lane: usize, values: &[E]) -> Result<MerkleTree, ZkAceStarkError> {
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

fn validate_fri_mask_coefficients_v1(coefficients: &[E]) -> Result<(), ZkAceStarkError> {
    if coefficients.len() != FRI_MASK_COEFFICIENTS
        || coefficients
            .iter()
            .any(|coefficient| !coefficient.is_canonical())
    {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    Ok(())
}

fn fri_mask_material<R: TryRngCore>(
    lane: usize,
    rng: &mut R,
) -> Result<FriMaskMaterial, ZkAceStarkError> {
    let mut coefficients = exact_vec(LDE_SIZE)?;
    for _ in 0..FRI_MASK_COEFFICIENTS {
        coefficients.push(random_goldilocks_fp4_v1(rng).map_err(|error| match error {
            TransparentStarkErrorV1::RandomnessUnavailable => {
                ZkAceStarkError::RandomnessUnavailable
            }
            TransparentStarkErrorV1::AllocationFailure => {
                ZkAceStarkError::ProofAllocationUnavailable
            }
            _ => {
                ZkAceStarkError::InternalInvariant("compiled FRI-mask coefficient sampling failed")
            }
        })?);
    }
    validate_fri_mask_coefficients_v1(&coefficients)?;
    coefficients.resize(LDE_SIZE, E::ZERO);
    let lde_root = primitive_root(LDE_LOG2)?;
    let values =
        goldilocks_fp4_evaluate_coset_v1(&coefficients, LDE_SIZE, lde_root, F(FIELD_GENERATOR))
            .map_err(|_| {
                ZkAceStarkError::InternalInvariant("invalid Fp4 FRI-mask coefficient/coset shape")
            })?;
    coefficients.fill(E::ZERO);
    let tree = MerkleTree::from_leaves(
        values
            .iter()
            .copied()
            .map(|value| fri_mask_leaf_hash(lane, value))
            .collect(),
    )?;
    Ok(FriMaskMaterial { values, tree })
}

fn proof_query(
    index: usize,
    trace_lde: &[Vec<F>],
    trace_tree: &MerkleTree,
    compositions: &[Vec<E>],
    composition_trees: &[MerkleTree],
    fri_masks: &[FriMaskMaterial],
    fri_lanes: &[FriLaneMaterial],
) -> Result<ZkAceQueryProofV1, ZkAceStarkError> {
    let next_index = (index + TERMINAL_SIZE) % LDE_SIZE;
    let current_row = row_at(trace_lde, index)?;
    let next_row = row_at(trace_lde, next_index)?;
    let composition_values = compositions.iter().map(|values| values[index]).collect();
    let composition_paths = composition_trees
        .iter()
        .map(|tree| tree.path(index))
        .collect::<Result<Vec<_>, _>>()?;
    let fri_mask_values = fri_masks.iter().map(|mask| mask.values[index]).collect();
    let fri_mask_paths = fri_masks
        .iter()
        .map(|mask| mask.tree.path(index))
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
                low: layer[low_index],
                high: layer[high_index],
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
        fri_mask_values,
        fri_mask_paths,
        fri_lanes: query_fri_lanes,
    })
}

/// Construct a canonical masked proof using a caller-supplied fallible RNG.
///
/// The injected RNG exists for deterministic known-answer tests and explicit
/// entropy-failure tests. Product callers use [`rand::rngs::OsRng`]. All
/// deterministic profile, public-input, and witness checks complete before one
/// shared checked entropy session is opened for both masking phases.
pub(super) fn prove_zk_ace_stark_v1_with_rng<R: TryCryptoRng + ?Sized>(
    public_inputs: &ZkAceAirRelationInputsV1,
    witness: &ZkAcePrivacyWitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkAceStarkError> {
    validate_compiled_security_profile_v1()?;
    let _ = validate_relation_inputs(public_inputs)?;
    let public_digest = derive_zk_ace_air_public_digest(public_inputs)?;
    let trace_material = build_trace_material(public_inputs, witness)?;
    let mut checked_rng = HealthCheckedTryCryptoRngV1::new(rng).map_err(|error| match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => ZkAceStarkError::RandomnessUnavailable,
        TryCryptoProverRandomnessErrorV1::Unhealthy => ZkAceStarkError::RandomnessUnhealthy,
    })?;
    let masked_trace = masked_lde_columns(&trace_material.trace_columns, &mut checked_rng)?;
    let trace_lde = &masked_trace.lde_columns;
    let fixed_lde = fixed_lde_columns(&trace_material.fixed_columns)?;
    let trace_tree = trace_tree(trace_lde)?;
    let trace_root = trace_tree.root();
    let base_seed = base_transcript_seed(&public_digest, &trace_root);

    let lane_alphas = (0..SECURITY_LANES)
        .map(|lane| challenge_vector(&base_seed, b"constraint-alpha", lane, CONSTRAINT_COUNT))
        .collect::<Result<Vec<_>, _>>()?;
    let compositions = composition_lanes(
        trace_lde,
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
    let deep_point = challenge_deep_point(&deep_seed(&composition_seed))?;
    let (deep_trace_current, deep_trace_next) = masked_trace_rows_at_deep_point(
        &trace_material.trace_columns,
        &masked_trace.masks,
        deep_point,
    )?;
    let deep_composition_values = composition_values_at_deep_point(&compositions, deep_point)?;
    let schedule = build_schedule(public_inputs)?;
    let deep_fixed = fixed_row_at_extension_point(&schedule, deep_point)?;
    let deep_public_outputs = trace_material.public_outputs.map(E::from_base);
    for lane in 0..SECURITY_LANES {
        let expected = constraint_quotient_value(
            deep_point,
            &deep_trace_current,
            &deep_trace_next,
            &deep_fixed,
            &deep_public_outputs,
            &lane_alphas[lane],
        )?;
        if expected != deep_composition_values[lane] {
            return Err(ZkAceStarkError::InternalInvariant(
                "honest DEEP composition evaluation does not match the AIR identity",
            ));
        }
    }
    // Each mask root is committed before any challenge that weights the
    // corresponding trace/composition lane.  This ordering is consensus
    // critical: sampling a batching challenge first would let a malicious
    // prover choose `R` to cancel a high-degree lane.
    let mut fri_masks = Vec::with_capacity(SECURITY_LANES);
    let mut fri_mask_roots = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let mask = fri_mask_material(lane, &mut checked_rng)?;
        fri_mask_roots.push(mask.tree.root());
        fri_masks.push(mask);
    }
    let batch_seed = batch_seed(
        &composition_seed,
        &deep_trace_current,
        &deep_trace_next,
        &deep_composition_values,
        &fri_mask_roots,
    )?;
    let mut fri_material = Vec::with_capacity(SECURITY_LANES);
    for lane in 0..SECURITY_LANES {
        let trace_mix = challenge_vector(&batch_seed, b"trace-mix", lane, TRACE_WIDTH)?;
        let composition_mix = challenge_field(&batch_seed, b"composition-mix", lane, 0)?;
        let base_values = mix_fri_base(
            &fri_masks[lane].values,
            trace_lde,
            &compositions[lane],
            deep_point,
            &deep_trace_current,
            &deep_trace_next,
            deep_composition_values[lane],
            &trace_mix,
            composition_mix,
        )?;
        let lane_seed = fri_lane_seed(&batch_seed, lane);
        fri_material.push(build_fri_lane(base_values, &lane_seed, lane)?);
    }
    let fri_roots = fri_material
        .iter()
        .map(|lane| lane.roots.clone())
        .collect::<Vec<_>>();
    let query_seed = query_seed_from_roots(&batch_seed, &fri_roots);
    let query_indices = derive_query_indices(&query_seed)?;
    let queries = query_indices
        .into_iter()
        .map(|index| {
            proof_query(
                index,
                trace_lde,
                &trace_tree,
                &compositions,
                &composition_trees,
                &fri_masks,
                &fri_material,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let proof = ZkAceStarkProofV1 {
        version: PROOF_VERSION,
        trace_root,
        composition_roots,
        fri_mask_roots,
        deep_trace_current,
        deep_trace_next,
        deep_composition_values,
        fri_lanes: fri_material
            .into_iter()
            .map(|lane| ZkAceFriLaneProofV1 {
                roots: lane.roots,
                terminal_values: lane.terminal_values,
            })
            .collect(),
        queries,
    };
    let encoded = encode_zk_ace_stark_proof_v1(&proof)?;
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
        || proof.fri_mask_roots.len() != SECURITY_LANES
        || proof.deep_trace_current.len() != TRACE_WIDTH
        || proof.deep_trace_next.len() != TRACE_WIDTH
        || proof.deep_composition_values.len() != SECURITY_LANES
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
            || query.fri_mask_values.len() != SECURITY_LANES
            || query.fri_mask_paths.len() != SECURITY_LANES
            || query
                .fri_mask_paths
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
    expected_base_value: E,
    lane_proof: &ZkAceFriLaneProofV1,
    lane_query: &ZkAceFriLaneQueryV1,
    lane_betas: &[E],
    terminal_values: &[E],
) -> Result<(), ZkAceStarkError> {
    if lane_betas.len() != FRI_ROUNDS {
        return Err(ZkAceStarkError::ProfileMismatch);
    }
    let mut layer_index = query_index;
    let mut layer_size = LDE_SIZE;
    let mut domain_shift = F(FIELD_GENERATOR);
    let mut domain_root = primitive_root(LDE_LOG2)?;
    let mut expected = expected_base_value;

    for round in 0..FRI_ROUNDS {
        let opening = &lane_query.rounds[round];
        let low = opening.low;
        let high = opening.high;
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
        let beta = lane_betas[round];
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
pub(super) fn verify_zk_ace_stark_v1(
    public_inputs: &ZkAceAirRelationInputsV1,
    proof_bytes: &[u8],
) -> Result<(), ZkAceStarkError> {
    validate_compiled_security_profile_v1()?;
    if proof_bytes.is_empty() {
        return Err(ZkAceStarkError::MalformedProof);
    }
    if proof_bytes.len() > MAX_PROOF_BYTES {
        return Err(ZkAceStarkError::ProofTooLarge);
    }
    let public_outputs = validate_relation_inputs(public_inputs)?;
    let public_digest = derive_zk_ace_air_public_digest(public_inputs)?;
    let proof = decode_zk_ace_stark_proof_v1(proof_bytes)?;

    let base_seed = base_transcript_seed(&public_digest, &proof.trace_root);
    let composition_seed = composition_seed(&base_seed, &proof.composition_roots);
    let deep_point = challenge_deep_point(&deep_seed(&composition_seed))?;
    let batch_seed = batch_seed(
        &composition_seed,
        &proof.deep_trace_current,
        &proof.deep_trace_next,
        &proof.deep_composition_values,
        &proof.fri_mask_roots,
    )?;
    let lane_roots = proof
        .fri_lanes
        .iter()
        .map(|lane| lane.roots.clone())
        .collect::<Vec<_>>();
    let query_seed = query_seed_from_roots(&batch_seed, &lane_roots);
    let expected_indices = derive_query_indices(&query_seed)?;
    let fixed_schedule = build_schedule(public_inputs)?;
    let lde_root = primitive_root(LDE_LOG2)?;
    let mut terminal_fields = Vec::with_capacity(SECURITY_LANES);

    for (lane_index, lane) in proof.fri_lanes.iter().enumerate() {
        let terminal = lane.terminal_values.clone();
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
        .collect::<Result<Vec<_>, _>>()?;
    let deep_fixed = fixed_row_at_extension_point(&fixed_schedule, deep_point)?;
    let extension_public_outputs = public_outputs.map(E::from_base);
    for lane in 0..SECURITY_LANES {
        let expected_deep_composition = constraint_quotient_value(
            deep_point,
            &proof.deep_trace_current,
            &proof.deep_trace_next,
            &deep_fixed,
            &extension_public_outputs,
            &alphas[lane],
        )?;
        if proof.deep_composition_values[lane] != expected_deep_composition {
            return Err(ZkAceStarkError::ConstraintOpening);
        }
    }
    let trace_mix = (0..SECURITY_LANES)
        .map(|lane| challenge_vector(&batch_seed, b"trace-mix", lane, TRACE_WIDTH))
        .collect::<Result<Vec<_>, _>>()?;
    let composition_mix = (0..SECURITY_LANES)
        .map(|lane| challenge_field(&batch_seed, b"composition-mix", lane, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let fri_betas = proof
        .fri_lanes
        .iter()
        .enumerate()
        .map(|(lane, lane_proof)| {
            let lane_seed = fri_lane_seed(&batch_seed, lane);
            (0..FRI_ROUNDS)
                .map(|round| fri_beta(&lane_seed, lane, round, &lane_proof.roots[round]))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;

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
        let composition_values = &query.composition_values;
        let fri_mask_values = &query.fri_mask_values;
        let extension_current = current
            .iter()
            .copied()
            .map(E::from_base)
            .collect::<Vec<_>>();
        let extension_next = next.iter().copied().map(E::from_base).collect::<Vec<_>>();
        let extension_fixed = fixed.iter().copied().map(E::from_base).collect::<Vec<_>>();
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
                E::from_base(x),
                &extension_current,
                &extension_next,
                &extension_fixed,
                &extension_public_outputs,
                &alphas[lane],
            )?;
            if composition_values[lane] != expected_composition {
                return Err(ZkAceStarkError::ConstraintOpening);
            }
            if !verify_merkle_path(
                &proof.fri_mask_roots[lane],
                fri_mask_leaf_hash(lane, fri_mask_values[lane]),
                index,
                &query.fri_mask_paths[lane],
                LDE_LOG2 as usize,
            ) {
                return Err(ZkAceStarkError::FriOpening);
            }
            let expected_base = mix_deep_fri_opening(
                x,
                &current,
                composition_values[lane],
                fri_mask_values[lane],
                deep_point,
                &proof.deep_trace_current,
                &proof.deep_trace_next,
                proof.deep_composition_values[lane],
                &trace_mix[lane],
                composition_mix[lane],
            )?;
            verify_fri_query(
                lane,
                index,
                expected_base,
                &proof.fri_lanes[lane],
                &query.fri_lanes[lane],
                &fri_betas[lane],
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

    fn public_inputs_and_witness() -> (ZkAceAirRelationInputsV1, ZkAcePrivacyWitnessV1) {
        let witness = ZkAcePrivacyWitnessV1 {
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
        )
        .expect("canonical test accounts have domainless encodings");
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &authorization_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        (
            ZkAceAirRelationInputsV1::transparent_transfer(
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
            ),
            witness,
        )
    }

    fn fixture() -> &'static (ZkAceAirRelationInputsV1, Vec<u8>) {
        static FIXTURE: OnceLock<(ZkAceAirRelationInputsV1, Vec<u8>)> = OnceLock::new();
        let _guard = proof_test_guard();
        FIXTURE.get_or_init(|| {
            let (public_inputs, witness) = public_inputs_and_witness();
            let mut rng = StdRng::from_seed([0x5A; 32]);
            let proof = prove_zk_ace_stark_v1_with_rng(&public_inputs, &witness, &mut rng)
                .expect("construct sound deterministic fixture");
            (public_inputs, proof)
        })
    }

    fn decode_fixture() -> ZkAceStarkProofV1 {
        decode_zk_ace_stark_proof_v1(&fixture().1).expect("decode canonical fixture")
    }

    fn assert_rejected(proof: &ZkAceStarkProofV1) {
        match encode_zk_ace_stark_proof_v1(proof) {
            Ok(bytes) => assert!(
                verify_zk_ace_stark_v1(&fixture().0, &bytes).is_err(),
                "adversarial proof must be rejected"
            ),
            Err(ZkAceStarkError::ProfileMismatch) => {}
            Err(error) => panic!("unexpected adversarial encoding failure: {error}"),
        }
    }

    #[test]
    fn air_public_transcript_has_fixed_vector_and_unambiguous_framing() {
        let parts = vec![
            AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1.to_vec(),
            1_u16.to_be_bytes().to_vec(),
            vec![0x11; 32],
            vec![0x22; 32],
            vec![0x33; 32],
            b"taira-transcript-kat".to_vec(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.as_bytes().to_vec(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER
                .as_bytes()
                .to_vec(),
            vec![0x44; 32],
            vec![0x55; 32],
            b"0x012345".to_vec(),
            b"0xabcdef".to_vec(),
            vec![0x66; 16],
            19_u128.to_be_bytes().to_vec(),
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND.as_bytes().to_vec(),
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.as_bytes().to_vec(),
        ];
        let expected = hash_air_public_transcript_parts_v1(&parts);
        assert_eq!(
            expected,
            [
                0xec, 0x26, 0x70, 0x2c, 0xee, 0xb4, 0xf0, 0x37, 0x8f, 0x00, 0x67, 0x62, 0x10, 0x5e,
                0xdf, 0x1f, 0x12, 0x9b, 0x24, 0x38, 0x65, 0xea, 0x5a, 0xcc, 0x6a, 0xd9, 0x38, 0x99,
                0x38, 0xab, 0x17, 0x38,
            ]
        );

        let mut permuted = parts.clone();
        permuted.swap(2, 3);
        assert_ne!(
            hash_air_public_transcript_parts_v1(&permuted),
            expected,
            "same-length field permutation must change the transcript"
        );

        let mut truncated = parts.clone();
        truncated.pop();
        assert_ne!(
            hash_air_public_transcript_parts_v1(&truncated),
            expected,
            "truncating the final field must change the framed part count"
        );

        let mut schema_drift = parts.clone();
        schema_drift[0].push(b'2');
        assert_ne!(
            hash_air_public_transcript_parts_v1(&schema_drift),
            expected,
            "changing only the explicit schema marker must change the transcript"
        );

        assert_ne!(
            hash_air_public_transcript_parts_v1(&[b"ab".to_vec(), b"c".to_vec()]),
            hash_air_public_transcript_parts_v1(&[b"a".to_vec(), b"bc".to_vec()]),
            "per-part lengths must prevent concatenation ambiguity"
        );
    }

    #[test]
    fn air_public_transcript_projection_uses_only_explicit_canonical_fields() {
        let (public_inputs, _) = public_inputs_and_witness();
        let parts =
            air_public_transcript_parts_v1(&public_inputs).expect("canonical account identifiers");
        assert_eq!(parts.len(), 16);
        assert_eq!(parts[0], AIR_PUBLIC_TRANSCRIPT_SCHEMA_V1);
        assert_eq!(parts[1], public_inputs.version.to_be_bytes());
        assert_eq!(parts[2], public_inputs.identity_commitment);
        assert_eq!(parts[3], public_inputs.tx_digest);
        assert_eq!(parts[4], public_inputs.authorization_digest);
        assert_eq!(parts[5], public_inputs.chain_id.as_str().as_bytes());
        assert_eq!(parts[6], public_inputs.domain_tag.as_bytes());
        assert_eq!(parts[7], public_inputs.action_class.as_bytes());
        assert_eq!(parts[8], public_inputs.replay_nullifier);
        assert_eq!(parts[9], public_inputs.policy_hash);
        assert_eq!(
            parts[10],
            public_inputs
                .from
                .to_canonical_hex()
                .expect("canonical source")
                .as_bytes()
        );
        assert_eq!(
            parts[11],
            public_inputs
                .to
                .to_canonical_hex()
                .expect("canonical destination")
                .as_bytes()
        );
        assert_eq!(parts[12], public_inputs.asset.aid_bytes());
        assert_eq!(parts[13], public_inputs.amount.to_be_bytes());
        assert_eq!(
            parts[14],
            public_inputs.verifier_key_id.backend.as_str().as_bytes()
        );
        assert_eq!(parts[15], public_inputs.verifier_key_id.name.as_bytes());
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
        let expected_public_outputs = public_inputs
            .identity_commitment
            .chunks_exact(8)
            .chain(public_inputs.replay_nullifier.chunks_exact(8))
            .map(|chunk| {
                F::canonical(u64::from_le_bytes(
                    chunk
                        .try_into()
                        .expect("Poseidon digest has exact eight-byte residues"),
                ))
                .expect("Poseidon squeeze emits canonical Goldilocks residues")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            material.public_outputs.as_slice(),
            expected_public_outputs.as_slice(),
            "AIR outputs must preserve the data-model Poseidon little-endian residue encoding"
        );
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
        let (public_inputs, _) = public_inputs_and_witness();
        for component in 0..3 {
            let (_, mut changed) = public_inputs_and_witness();
            match component {
                0 => changed.identity_root[0] ^= 1,
                1 => changed.identity_blinding[0] ^= 1,
                2 => changed.replay_secret[0] ^= 1,
                _ => unreachable!("closed witness component range"),
            }
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

    impl rand::CryptoRng for MaxValueRng {}

    #[test]
    fn constant_entropy_fails_the_shared_health_preflight() {
        let (public_inputs, witness) = public_inputs_and_witness();
        assert!(matches!(
            prove_zk_ace_stark_v1_with_rng(&public_inputs, &witness, &mut MaxValueRng),
            Err(ZkAceStarkError::RandomnessUnhealthy)
        ));
    }

    #[test]
    fn proof_roundtrips_under_exact_shape_and_byte_ceiling() {
        let (public_inputs, proof) = fixture();
        verify_zk_ace_stark_v1(public_inputs, proof).expect("proof verifies");
        assert_eq!(proof.len(), CANONICAL_PROOF_BYTES_V1);
        assert_eq!(proof.len(), MAX_PROOF_BYTES);
        assert_eq!(&proof[..PROOF_WIRE_MAGIC_V1.len()], &PROOF_WIRE_MAGIC_V1);
        let decoded = decode_fixture();
        let reencoded = encode_zk_ace_stark_proof_v1(&decoded).expect("canonical re-encode");
        assert_eq!(reencoded.as_slice(), proof.as_slice());
        assert_eq!(decoded.composition_roots.len(), SECURITY_LANES);
        assert_eq!(decoded.fri_mask_roots.len(), SECURITY_LANES);
        assert_eq!(decoded.queries.len(), QUERY_COUNT);
        assert!(
            decoded
                .queries
                .iter()
                .all(|query| query.current_row.len() == TRACE_WIDTH)
        );
        assert_eq!(CANONICAL_PROOF_BYTES_V1, 1_341_142);
        assert!(CANONICAL_PROOF_BYTES_V1 < 8 * 1024 * 1024);
    }

    #[test]
    fn zero_knowledge_and_fri_degree_budgets_are_machine_checked() {
        assert_eq!(AIR_CONSTRAINT_DEGREE_LEDGER_V1, [1, 2, 2, 2, 2, 2, 2, 2]);
        assert_eq!(AIR_TOTAL_DEGREE_V1, 2);
        assert_eq!(REDUCED_AIR_DEGREE_V1, 1);
        // A query opens both the current and next trace row.  The structured
        // trace mask therefore has strictly more independent coefficients than
        // the maximum number of direct evaluations of any private column.
        assert!(TRACE_MASK_COEFFICIENTS > 2 * QUERY_COUNT);
        let geometry = transparent_stark_zk_mask_geometry_v1(
            REDUCED_AIR_DEGREE_V1,
            CHALLENGE_EXTENSION_DEGREE,
            DEEP_QUERY_COUNT,
            QUERY_COUNT,
        )
        .expect("quadratic ZK-ACE AIR has valid Protocol-3 geometry");
        assert_eq!(geometry.minimum_mask_coefficients, 332);
        assert_eq!(geometry.minimum_mask_degree, 331);
        assert!(TRACE_MASK_COEFFICIENTS >= geometry.minimum_mask_coefficients);
        let cubic_geometry = transparent_stark_zk_mask_geometry_v1(
            2,
            CHALLENGE_EXTENSION_DEGREE,
            DEEP_QUERY_COUNT,
            QUERY_COUNT,
        )
        .expect("cubic counterfactual has valid arithmetic");
        assert_eq!(cubic_geometry.minimum_mask_coefficients, 556);
        assert!(TRACE_MASK_COEFFICIENTS < cubic_geometry.minimum_mask_coefficients);
        // The independent FRI mask occupies the complete code space except
        // for the single top coefficient reserved by the strict degree bound.
        assert_eq!(FRI_MASK_COEFFICIENTS + 1, FRI_CODE_DEGREE_BOUND_EXCLUSIVE);
        assert_eq!(FRI_CODE_DEGREE_BOUND_EXCLUSIVE, 8_192);
        assert_eq!(
            FRI_CODE_DEGREE_BOUND_EXCLUSIVE,
            (TERMINAL_DEGREE_BOUND + 1) << FRI_ROUNDS
        );
        assert_eq!(MASKED_TRACE_MAX_DEGREE, 4_607);
        assert_eq!(COMPOSITION_MAX_DEGREE, 5_118);
        assert_eq!(PROTOCOL3_OPTIMIZED_DEGREE_BOUND_EXCLUSIVE, 5_120);
        assert_eq!(MAX_STRUCTURED_BATCH_DEGREE, 5_117);
        // Every structured component mixed into FRI is strictly inside the
        // independent R-mask coefficient space.  Consequently R can
        // statistically decouple the complete folded transcript, not merely
        // the selected round-zero openings.
        assert!(MASKED_TRACE_MAX_DEGREE < FRI_MASK_COEFFICIENTS);
        assert!(COMPOSITION_MAX_DEGREE < FRI_MASK_COEFFICIENTS);
        assert!(MASKED_TRACE_MAX_DEGREE < FRI_CODE_DEGREE_BOUND_EXCLUSIVE);
        assert!(COMPOSITION_MAX_DEGREE < FRI_CODE_DEGREE_BOUND_EXCLUSIVE);
        assert!(FRI_MASK_COEFFICIENTS < FRI_CODE_DEGREE_BOUND_EXCLUSIVE);
    }

    #[test]
    fn air_degree_mask_and_work_security_substitution_fail_closed() {
        assert!(validate_compiled_security_profile_v1().is_ok());
        let cases = [
            // A zero/linear reduced AIR degree is outside the compiled
            // quadratic relation and invalid under the masking theorem.
            (0, 0, TRACE_MASK_COEFFICIENTS, 128, 129, 256, 124),
            (1, 0, TRACE_MASK_COEFFICIENTS, 128, 129, 256, 124),
            // Claiming a different reduced degree, including the cubic
            // counterfactual whose 556-coefficient minimum exceeds h=512.
            (2, 0, TRACE_MASK_COEFFICIENTS, 128, 129, 256, 124),
            (3, 2, TRACE_MASK_COEFFICIENTS, 128, 129, 256, 124),
            // Degree/mask off-by-one and theorem-parameter substitutions.
            (2, 1, 511, 128, 129, 256, 124),
            (2, 1, 513, 128, 129, 256, 124),
            (2, 1, TRACE_MASK_COEFFICIENTS, 129, 129, 256, 124),
            (2, 1, TRACE_MASK_COEFFICIENTS, 128, 128, 256, 124),
            (2, 1, TRACE_MASK_COEFFICIENTS, 128, 129, 255, 124),
            (2, 1, TRACE_MASK_COEFFICIENTS, 128, 129, 256, 125),
        ];
        for (
            air_total_degree,
            reduced_air_degree,
            trace_mask_coefficients,
            target_bits,
            round_by_round_bits,
            random_oracle_bits,
            max_query_log2,
        ) in cases
        {
            assert!(matches!(
                validate_security_profile_geometry_v1(
                    air_total_degree,
                    reduced_air_degree,
                    trace_mask_coefficients,
                    target_bits,
                    round_by_round_bits,
                    random_oracle_bits,
                    max_query_log2,
                ),
                Err(ZkAceStarkError::ProfileMismatch)
            ));
        }
    }

    #[test]
    fn fri_theorem_precondition_substitutions_fail_closed() {
        let profile = compiled_fri_theorem_profile_v1().expect("compiled roots and cosets");
        validate_fri_theorem_profile_v1(profile).expect("compiled FRI theorem profile");

        let mut mutations = Vec::new();
        let mut changed = profile;
        changed.multiplicity_parameter = 2;
        mutations.push(changed);
        changed = profile;
        changed.code_degree_bound_exclusive = 4_096;
        mutations.push(changed);
        changed = profile;
        changed.domain_size = 32_768;
        mutations.push(changed);
        changed = profile;
        changed.agreement_squared_numerator = 1;
        changed.agreement_squared_denominator = 8;
        mutations.push(changed);
        changed = profile;
        changed.query_count -= 1;
        mutations.push(changed);
        changed = profile;
        changed.query_sampling_without_replacement = false;
        mutations.push(changed);
        changed = profile;
        changed.deep_candidate_degree_bound_exclusive -= 1;
        mutations.push(changed);
        changed = profile;
        changed.deep_identity_degree_bound -= 1;
        mutations.push(changed);
        changed = profile;
        changed.deep_constraint_count -= 1;
        mutations.push(changed);
        changed = profile;
        changed.deep_excluded_point_count -= 1;
        mutations.push(changed);
        changed = profile;
        changed.fold_count = 11;
        mutations.push(changed);
        changed = profile;
        changed.terminal_size = 32;
        mutations.push(changed);
        changed = profile;
        changed.terminal_degree_bound = 2;
        mutations.push(changed);
        changed = profile;
        changed.affine_commit_factor_numerator = 2;
        mutations.push(changed);
        changed = profile;
        changed.affine_commit_factor_denominator = 3;
        mutations.push(changed);
        changed = profile;
        changed.affine_oracle_count += 1;
        mutations.push(changed);
        changed = profile;
        changed.affine_random_coefficients -= 1;
        mutations.push(changed);
        changed = profile;
        changed.reduction_arities[0] = 3;
        mutations.push(changed);
        changed = profile;
        changed.fri_mask_coefficients += 1;
        mutations.push(changed);
        changed = profile;
        changed.protocol3_optimized_degree_bound_exclusive -= 1;
        mutations.push(changed);
        changed = profile;
        changed.maximum_structured_batch_degree += 1;
        mutations.push(changed);
        changed = profile;
        changed.domains_are_smooth_and_disjoint = false;
        mutations.push(changed);

        for mutation in mutations {
            assert!(matches!(
                validate_fri_theorem_profile_v1(mutation),
                Err(ZkAceStarkError::ProfileMismatch)
            ));
        }
    }

    #[test]
    fn unique_query_schedule_preserves_theorem_two_power_bound() {
        let indices =
            derive_query_indices(&[0x6D; 32]).expect("bounded deterministic query schedule");
        assert_eq!(indices.len(), QUERY_COUNT);
        assert_eq!(
            &indices[..12],
            &[
                9_908, 49_349, 28_803, 45_802, 29_489, 14_356, 39_949, 25_477, 58_343, 19_542,
                34_221, 58_879,
            ]
        );
        assert!(indices.iter().all(|index| *index < LDE_SIZE));
        assert_eq!(
            indices.iter().copied().collect::<BTreeSet<_>>().len(),
            QUERY_COUNT
        );

        // For a fixed bad set of size b in a domain of size n, sampling s
        // distinct indices accepts only inside that set with probability
        //   product_{j=0}^{s-1} (b-j)/(n-j).
        // Each factor is <=b/n because n(b-j)<=b(n-j) iff j*b<=j*n.
        // Thus the hypergeometric probability is no larger than the
        // with-replacement `(b/n)^s` term used by Theorem 2.  These boundary
        // representatives machine-check the cross-multiplied inequality; the
        // displayed equivalence proves it for every 0<=b<=n.
        let n = LDE_SIZE as u128;
        for bad_count in [
            0_usize,
            1,
            LDE_SIZE / 8,
            LDE_SIZE / 2,
            LDE_SIZE - 1,
            LDE_SIZE,
        ] {
            let b = bad_count as u128;
            for draw in 0..QUERY_COUNT.min(bad_count) {
                let j = draw as u128;
                assert!(n * (b - j) <= b * (n - j));
            }
        }
    }

    #[test]
    fn fri_mask_rejects_cap_plus_one_and_nonzero_high_coefficient() {
        let mut coefficients = vec![E::ZERO; FRI_MASK_COEFFICIENTS];
        coefficients[FRI_MASK_COEFFICIENTS - 1] = E::ONE;
        validate_fri_mask_coefficients_v1(&coefficients)
            .expect("highest admitted coefficient is canonical");

        coefficients.push(E::ZERO);
        assert!(matches!(
            validate_fri_mask_coefficients_v1(&coefficients),
            Err(ZkAceStarkError::ProfileMismatch)
        ));
        *coefficients
            .last_mut()
            .expect("cap-plus-one vector has a high coefficient") = E::ONE;
        assert!(matches!(
            validate_fri_mask_coefficients_v1(&coefficients),
            Err(ZkAceStarkError::ProfileMismatch)
        ));
    }

    #[test]
    fn theorem_backed_fp4_soundness_budget_clears_128_bits() {
        // Haböck 2022/1216, Theorem 2, with rho=1/8, m=3 and
        // affine batching. Since 7/(12*sqrt(2)) < 53/128 and
        // (53/128)^12 < 2^-15, 108 queries contribute <2^-135.
        assert!(7_u128.pow(2) * 128_u128.pow(2) < 53_u128.pow(2) * 288_u128);
        assert!(53_u128.pow(12) * (1_u128 << 15) < 128_u128.pow(12));
        assert_eq!(QUERY_COUNT / 12, 9);

        // p > (31/32)*2^64. This lower bound makes all remaining
        // comparisons fit exact small integers:
        //   affine FRI commit term < 7^7*2^29/p^4 < 2^-207;
        //   reduction-round term < 7*(D+1)*24*3/p^4 < 2^-230.
        assert!(u128::from(FIELD_MODULUS) > 31_u128 * (1_u128 << 59));
        assert!(7_u128.pow(7) < 31_u128.pow(4));
        assert!(
            7_u128 * (u128::from(LDE_SIZE as u64) + 1) * 24 * 3 < 31_u128.pow(4) * (1_u128 << 6)
        );

        // Haböck Theorem 8 uses k+=k+2=8,194 and
        // L+=(7/2)/sqrt(k+/|D|).  For the quadratic AIR its root-count term is
        // 2*(k+-1)+(k-1)=24,577, while constraint batching contributes C=168.
        // The exact comparisons prove L+*(168+24,577)<245,000.
        assert_eq!(DEEP_CANDIDATE_DEGREE_BOUND_EXCLUSIVE, 8_194);
        assert_eq!(DEEP_IDENTITY_DEGREE_BOUND, 24_577);
        assert_eq!(CONSTRAINT_COUNT, 168);
        assert!(802_816_u128 * 24_745_u128.pow(2) < 8_194_u128 * 245_000_u128.pow(2));

        // z is uniform over Fp4 \ (D union H union {0}).  D is a disjoint
        // 65,536-point coset and H has 4,096 points, so the exact denominator
        // is p^4-69,633.  Since p>63*2^58 and the excluded set is <2^232,
        // p^4-69,633 >(63^4-1)*2^232 >245,000*2^238.
        assert_eq!(DEEP_EXCLUDED_POINT_COUNT, 69_633);
        assert_ne!(
            F(FIELD_GENERATOR).pow(LDE_SIZE as u128),
            F::ONE,
            "FRI coset must be disjoint from its underlying subgroup"
        );
        assert!(u128::from(FIELD_MODULUS) > 63_u128 * (1_u128 << 58));
        assert!(u128::from(DEEP_EXCLUDED_POINT_COUNT as u64) < 1_u128 << 127);
        assert!(63_u128.pow(4) - 1 > 245_000_u128 * 64);

        // Block et al. 2023/1071 BCS, in the work-normalized concrete
        // security definition used by Block--Tiwari 2024/1161:
        // epsilon/Q <= epsilon_rbr + 3*(Q+1/Q)/2^256.
        // For Q <= 2^124 the hash term is <2^-129; adding the <2^-134
        // complete algebraic/FRI union remains strictly below 2^-128.
        assert!(3_u128 < 1_u128 << (256 - 125 - 129));
        assert!(
            checked_transparent_stark_work_security_v1(
                PROVABLE_SOUNDNESS_BITS_V1,
                ROUND_BY_ROUND_SECURITY_BITS_V1,
                RANDOM_ORACLE_BITS_V1,
                u16::from(MAX_ROM_QUERY_LOG2_V1),
            )
            .is_ok()
        );
        assert_eq!(MAX_ROM_QUERY_LOG2_V1, 124);
        assert_eq!(PROVABLE_SOUNDNESS_BITS_V1, 128);
    }

    #[test]
    fn field_challenge_rejection_is_bounded_uniform_and_replayable() {
        assert_eq!(CONSTRAINT_COUNT, 168);
        assert_eq!(DISTINCT_FIELD_CHALLENGE_COUNT, 270);
        assert_eq!(MAX_FIELD_CHALLENGE_HASH_CALLS, 69_120);
        // Each coefficient rejects exactly the `2^64 - p = 2^32 - 1`
        // non-canonical encodings. The rejection probability is therefore
        // strictly less than `2^-32`, not equal to it. Zero is a valid
        // unbiased challenge outside the special DEEP-point sampler.
        let rejected_coefficient_encodings = (1_u128 << 64) - u128::from(FIELD_MODULUS);
        assert_eq!(rejected_coefficient_encodings, (1_u128 << 32) - 1);
        assert!(rejected_coefficient_encodings < 1_u128 << 32);

        let digest_with_coefficients = |coefficients: [u64; 4]| {
            let mut digest = [0_u8; 32];
            for (index, coefficient) in coefficients.into_iter().enumerate() {
                digest[index * FIELD_BYTES..(index + 1) * FIELD_BYTES]
                    .copy_from_slice(&coefficient.to_be_bytes());
            }
            digest
        };
        assert_eq!(
            challenge_from_digest_stream(|_| {
                digest_with_coefficients([FIELD_MODULUS - 1, 0, 0, 0])
            })
            .expect("p - 1 is the largest canonical coefficient"),
            E::canonical([FIELD_MODULUS - 1, 0, 0, 0]).expect("p - 1 is a canonical coefficient")
        );
        for coefficient_index in 0..4 {
            for first_rejected in [FIELD_MODULUS, u64::MAX] {
                let mut coefficients = [0_u64; 4];
                coefficients[coefficient_index] = first_rejected;
                assert!(matches!(
                    challenge_from_digest_stream(|_| digest_with_coefficients(coefficients)),
                    Err(ZkAceStarkError::ChallengeDerivationExhausted)
                ));
            }
        }

        let mut attempted = Vec::new();
        let challenge = challenge_from_digest_stream(|attempt| {
            attempted.push(attempt);
            let coefficients = if attempt == 0 {
                [FIELD_MODULUS, 0, 0, 0]
            } else {
                [7, 11, 13, 17]
            };
            digest_with_coefficients(coefficients)
        })
        .expect("second tuple is canonical");
        assert_eq!(
            challenge,
            E::canonical([7, 11, 13, 17]).expect("small Fp4 element")
        );
        assert_eq!(attempted, vec![0, 1]);

        assert!(matches!(
            challenge_from_digest_stream(|_| {
                let mut digest = [0_u8; 32];
                digest[..FIELD_BYTES].copy_from_slice(&FIELD_MODULUS.to_be_bytes());
                digest
            }),
            Err(ZkAceStarkError::ChallengeDerivationExhausted)
        ));
        assert_eq!(
            challenge_from_digest_stream(|_| [0_u8; 32]).expect("zero is uniformly admitted"),
            E::ZERO
        );

        let seed = [0xA5; 32];
        let first =
            challenge_field(&seed, b"replay-kat", 2, 17).expect("compiled rejection sampler");
        let replay =
            challenge_field(&seed, b"replay-kat", 2, 17).expect("compiled rejection sampler");
        assert_eq!(first, replay);

        assert!(is_base_domain_point(E::ZERO).expect("domain predicate"));
        assert!(is_base_domain_point(E::ONE).expect("trace-domain predicate"));
        assert!(
            is_base_domain_point(E::from_base(F(FIELD_GENERATOR))).expect("LDE-domain predicate")
        );
        assert!(
            !is_base_domain_point(E::canonical([0, 1, 0, 0]).expect("extension generator"))
                .expect("extension-domain predicate")
        );
        let deep = challenge_deep_point(&seed).expect("bounded exact DEEP sampler");
        assert_ne!(deep, E::ZERO);
        assert!(!is_base_domain_point(deep).expect("DEEP point domain exclusion"));
    }

    #[test]
    fn trace_masking_is_randomized_and_does_not_embed_raw_witness_bytes() {
        let (public_inputs, first) = fixture();
        let (_, witness) = public_inputs_and_witness();
        let _guard = proof_test_guard();
        let mut rng = StdRng::from_seed([0xA5; 32]);
        let second = prove_zk_ace_stark_v1_with_rng(public_inputs, &witness, &mut rng)
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
        let (public_inputs, proof) = fixture();
        let mutations: [(&str, fn(&mut ZkAceAirRelationInputsV1)); 14] = [
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
        let (public_inputs, proof) = fixture();
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
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &trailing),
            Err(ZkAceStarkError::ProofTooLarge)
        ));

        let mut wrong_magic = proof.clone();
        wrong_magic[0] ^= 1;
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &wrong_magic),
            Err(ZkAceStarkError::MalformedProof)
        ));

        let mut wrong_version = proof.clone();
        wrong_version[PROOF_WIRE_MAGIC_V1.len() + 1] ^= 1;
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &wrong_version),
            Err(ZkAceStarkError::ProfileMismatch)
        ));

        let exact_length_garbage = vec![0; CANONICAL_PROOF_BYTES_V1];
        assert!(matches!(
            verify_zk_ace_stark_v1(public_inputs, &exact_length_garbage),
            Err(ZkAceStarkError::MalformedProof)
        ));
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
        assert!(matches!(
            encode_zk_ace_stark_proof_v1(&changed),
            Err(ZkAceStarkError::ProfileMismatch)
        ));

        changed = decode_fixture();
        changed.queries[0].current_row[0] = FIELD_MODULUS;
        let bytes =
            encode_zk_ace_stark_proof_v1(&changed).expect("encode non-canonical field value");
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes),
            Err(ZkAceStarkError::NonCanonicalField)
        ));

        let mut noncanonical_extension = fixture().1.clone();
        let deep_offset = PROOF_WIRE_MAGIC_V1.len()
            + PROOF_VERSION_BYTES
            + HASH_BYTES
            + 2 * SECURITY_LANES * HASH_BYTES;
        noncanonical_extension[deep_offset..deep_offset + FIELD_BYTES]
            .copy_from_slice(&FIELD_MODULUS.to_be_bytes());
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &noncanonical_extension),
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
        changed.fri_mask_roots[0][0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.deep_trace_current[0] = changed.deep_trace_current[0].add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.deep_trace_next[0] = changed.deep_trace_next[0].add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.deep_composition_values[0] = changed.deep_composition_values[0].add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].composition_values[0] =
            changed.queries[0].composition_values[0].add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_mask_values[0] = changed.queries[0].fri_mask_values[0].add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_mask_paths[0][0][0] ^= 1;
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_lanes[0].rounds[0].low =
            changed.queries[0].fri_lanes[0].rounds[0].low.add(E::ONE);
        assert_rejected(&changed);

        changed = decode_fixture();
        changed.queries[0].fri_lanes[0].rounds[0].high_path[0][0] ^= 1;
        assert_rejected(&changed);
    }

    #[test]
    fn fri_mask_commitment_precedes_and_changes_every_batch_challenge() {
        let base = [0x11; 32];
        let composition_roots = [[0x22; 32]; SECURITY_LANES];
        let composition = composition_seed(&base, &composition_roots);
        let first_roots = [[0x33; 32]; SECURITY_LANES];
        let mut second_roots = first_roots;
        second_roots[0][0] ^= 1;
        let deep_trace_current = vec![E::ONE; TRACE_WIDTH];
        let deep_trace_next = vec![E::ZERO; TRACE_WIDTH];
        let deep_composition = vec![E::ONE; SECURITY_LANES];
        let first = batch_seed(
            &composition,
            &deep_trace_current,
            &deep_trace_next,
            &deep_composition,
            &first_roots,
        )
        .expect("compiled root count");
        let second = batch_seed(
            &composition,
            &deep_trace_current,
            &deep_trace_next,
            &deep_composition,
            &second_roots,
        )
        .expect("compiled root count");
        assert_ne!(first, second);
        for lane in 0..SECURITY_LANES {
            assert_ne!(
                challenge_vector(&first, b"trace-mix", lane, TRACE_WIDTH)
                    .expect("compiled rejection sampler"),
                challenge_vector(&second, b"trace-mix", lane, TRACE_WIDTH)
                    .expect("compiled rejection sampler")
            );
            assert_ne!(
                challenge_field(&first, b"composition-mix", lane, 0)
                    .expect("compiled rejection sampler"),
                challenge_field(&second, b"composition-mix", lane, 0)
                    .expect("compiled rejection sampler")
            );
        }
    }

    #[test]
    fn malicious_zero_composition_cannot_disconnect_private_trace() {
        let mut changed = decode_fixture();
        changed.composition_roots.fill([0; 32]);
        for query in &mut changed.queries {
            query.composition_values.fill(E::ZERO);
            for path in &mut query.composition_paths {
                path.fill([0; 32]);
            }
        }
        assert_rejected(&changed);
    }

    #[test]
    fn terminal_root_cannot_hide_a_high_degree_polynomial() {
        let mut changed = decode_fixture();
        changed.fri_lanes[0].terminal_values[3] =
            changed.fri_lanes[0].terminal_values[3].add(E::ONE);
        let terminal = changed.fri_lanes[0].terminal_values.clone();
        let tree = MerkleTree::from_leaves(
            terminal
                .iter()
                .copied()
                .map(|value| fri_leaf_hash(0, FRI_ROUNDS, value))
                .collect(),
        )
        .expect("terminal tree");
        changed.fri_lanes[0].roots[FRI_ROUNDS] = tree.root();
        let bytes = encode_zk_ace_stark_proof_v1(&changed).expect("encode high-degree terminal");
        assert!(matches!(
            verify_zk_ace_stark_v1(&fixture().0, &bytes),
            Err(ZkAceStarkError::FriDegree)
        ));
    }
}
