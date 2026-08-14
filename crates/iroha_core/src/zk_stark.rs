//! Native STARK/FRI (binary folding) verifier used by the `stark/fri/*` backends.
//!
//! This module provides a deterministic verifier over the Goldilocks prime field.
//! It supports:
//! - SHA-256 transcripts + SHA-256 Merkle commitments (`stark/fri/sha256-goldilocks`), and
//! - Poseidon2 transcripts + Poseidon2 Merkle commitments (`stark/fri/poseidon2-goldilocks`).
//!
//! The verifier implements a multi-round binary FRI consistency check.
//!
//! The wire format is defined with Norito. The proof envelope carries params, Merkle
//! roots, and query decommitments. Verification replays the transcript and checks:
//! - Merkle openings for each queried value
//! - The domain-aware fold relation for `(x, -x)` openings in each round and query
//! - Distinct transcript-derived query positions
//! - Optional composition leaf constraints when `comp_root` is present
//!
//! Size and structural limits are enforced to reject oversized or malformed payloads
//! deterministically (see [`StarkVerifierLimits`]).
#![allow(clippy::needless_pass_by_value)]
use fastpq_prover::{hash_field_elements, pack_bytes};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use crate::json_macros::{JsonDeserialize, JsonSerialize};
/// Goldilocks prime modulus p = 2^64 - 2^32 + 1
const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;
const MOD_P_U64: u64 = MOD_P as u64;
const GOLDILOCKS_GENERATOR: u64 = 7;
/// Supported hash selector for the STARK envelope.
pub const STARK_HASH_SHA256_V1: u8 = 1;
/// Selector for a Poseidon2 transcript and Merkle commitments.
pub const STARK_HASH_POSEIDON2_V1: u8 = 2;
/// Minimum evaluation-domain exponent accepted by generic STARK admission.
pub const STARK_FRI_CONSENSUS_MIN_N_LOG2: u8 = 10;
/// Minimum FRI blowup exponent accepted by generic STARK admission.
pub const STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2: u8 = 3;
/// Minimum verifier query count accepted by generic STARK admission.
///
/// With the fixed 8x blowup, 48 independent queries provide a conservative
/// 144-bit proximity-sampling floor before accounting for the remaining FRI
/// terms. The earlier 24-query development profile provided only a 72-bit
/// floor and is not part of the first release.
pub const STARK_FRI_CONSENSUS_MIN_QUERIES: u16 = 48;
/// Trace width of the generic OpenVerify binding AIR used by STARK/FRI v1 proofs.
pub const STARK_BINDING_AIR_TRACE_WIDTH_V1: u16 = 6;
const MAX_DOMAIN_LOG2: u8 = 24;
const MAX_FRI_LAYERS: usize = 32;
const MAX_FRI_QUERIES: usize = 64;
const MAX_MERKLE_DEPTH: usize = 32;
const MAX_AUX_TERMS: usize = 64;
const MAX_AIR_WIDTH: usize = 64;
const MAX_DOMAIN_TAG_LEN: usize = 64;
const MAX_TRANSCRIPT_LABEL_LEN: usize = 128;
const MAX_ENVELOPE_BYTES: usize = 1 << 20; // 1 MiB guard for decoded envelopes
pub(crate) const STARK_FRI_QUERY_INDEX_REPEATED_ERROR: &str = "FRI query index repeated";
const STARK_FRI_BOUNDED_QUERY_REJECTION_ATTEMPTS: usize = 8;
#[cfg(test)]
const BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS: u32 = 1024;
const GENERIC_STARK_AIR_BFV_FULL_BOOTSTRAP_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target the BFV full-bootstrap circuit; use the BFV full-bootstrap STARK prover";
const GENERIC_STARK_AIR_ZK_ACE_RESERVED_ERROR: &str =
    "generic STARK AIR prover cannot target the retired ZK-ACE relation; use SubmitPrivacyProofV1";
const GENERIC_STARK_AIR_IVM_EXECUTION_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target the IVM execution circuit; use the IVM execution STARK prover";
const GENERIC_STARK_AIR_SORACLOUD_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target a Soracloud FHE relation; a dedicated typed Soracloud verifier is required";
const GENERIC_STARK_AIR_GOVERNANCE_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target a governance vote role; a dedicated semantic governance circuit is required";
fn validate_stark_transcript_label(label: &str, max_len: usize) -> Result<(), &'static str> {
    if label.is_empty() {
        return Err("transcript label must not be empty");
    }
    if label.len() > max_len {
        return Err("transcript label exceeds maximum length");
    }
    if !label.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("transcript label must contain only printable ASCII bytes without whitespace");
    }
    Ok(())
}
fn validate_stark_circuit_id(circuit_id: &str) -> Result<(), &'static str> {
    if circuit_id.is_empty() {
        return Err("circuit id must not be empty");
    }
    if circuit_id.len() > MAX_TRANSCRIPT_LABEL_LEN {
        return Err("circuit id exceeds maximum length");
    }
    if !circuit_id.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("circuit id must contain only printable ASCII bytes without whitespace");
    }
    Ok(())
}
fn stark_air_circuit_id_targets_reserved_circuit(circuit_id: &str, canonical: &str) -> bool {
    let trimmed = circuit_id.trim();
    trimmed == canonical
        || trimmed
            .strip_suffix(canonical)
            .is_some_and(|prefix| prefix.ends_with(':') || prefix.ends_with('/'))
}
fn stark_air_circuit_id_targets_bfv_full_bootstrap(circuit_id: &str) -> bool {
    stark_air_circuit_id_targets_reserved_circuit(
        circuit_id,
        iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
    )
}
fn stark_air_circuit_id_targets_zk_ace(circuit_id: &str) -> bool {
    stark_air_circuit_id_targets_reserved_circuit(
        circuit_id,
        iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
    )
}
fn stark_air_circuit_id_targets_ivm_execution(circuit_id: &str) -> bool {
    stark_air_circuit_id_targets_reserved_circuit(
        circuit_id,
        crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
    )
}
fn stark_air_circuit_id_targets_governance_vote_relation(circuit_id: &str) -> bool {
    [
        crate::zk::GOVERNANCE_BALLOT_CIRCUIT_ID_V1,
        crate::zk::GOVERNANCE_TALLY_CIRCUIT_ID_V1,
    ]
    .into_iter()
    .any(|canonical| stark_air_circuit_id_targets_reserved_circuit(circuit_id, canonical))
}
fn stark_air_circuit_id_targets_soracloud_fhe_relation(circuit_id: &str) -> bool {
    [
        iroha_data_model::soracloud::SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    ]
    .into_iter()
    .any(|canonical| stark_air_circuit_id_targets_reserved_circuit(circuit_id, canonical))
}
fn validate_generic_stark_air_circuit_id(circuit_id: &str) -> Result<(), String> {
    validate_stark_circuit_id(circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
    if stark_air_circuit_id_targets_bfv_full_bootstrap(circuit_id) {
        return Err(GENERIC_STARK_AIR_BFV_FULL_BOOTSTRAP_RESERVED_ERROR.to_owned());
    }
    if stark_air_circuit_id_targets_zk_ace(circuit_id) {
        return Err(GENERIC_STARK_AIR_ZK_ACE_RESERVED_ERROR.to_owned());
    }
    if stark_air_circuit_id_targets_ivm_execution(circuit_id) {
        return Err(GENERIC_STARK_AIR_IVM_EXECUTION_RESERVED_ERROR.to_owned());
    }
    if stark_air_circuit_id_targets_governance_vote_relation(circuit_id) {
        return Err(GENERIC_STARK_AIR_GOVERNANCE_RESERVED_ERROR.to_owned());
    }
    if stark_air_circuit_id_targets_soracloud_fhe_relation(circuit_id) {
        return Err(GENERIC_STARK_AIR_SORACLOUD_RESERVED_ERROR.to_owned());
    }
    Ok(())
}
fn validate_stark_domain_tag(domain_tag: &str, max_len: usize) -> Result<(), &'static str> {
    if domain_tag.is_empty() {
        return Err("domain tag must not be empty");
    }
    if domain_tag.len() > max_len {
        return Err("domain tag exceeds maximum length");
    }
    if !domain_tag.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("domain tag must contain only printable ASCII bytes without whitespace");
    }
    Ok(())
}
/// Tunable limits applied during STARK envelope verification to prevent denial-of-service inputs.
///
/// These values can tighten the built-in protocol caps for a caller, but they
/// cannot relax canonical verifier structure limits. Values above the native
/// caps are clamped internally.
#[derive(Clone, Copy, Debug)]
pub struct StarkVerifierLimits {
    /// Caller cap for domain log2, clamped to the native protocol maximum.
    pub max_domain_log2: u8,
    /// Caller cap for blowup log2, clamped to the native protocol maximum.
    pub max_blowup_log2: u8,
    /// Caller cap for fold arity, clamped to the native verifier maximum.
    pub max_fold_arity: u8,
    /// Caller cap for query count, clamped to the native protocol maximum.
    pub max_queries: usize,
    /// Caller cap for Merkle depth, clamped to the native verifier maximum.
    pub max_merkle_depth: usize,
    /// Caller cap for auxiliary terms in a composition leaf, clamped natively.
    pub max_aux_terms: usize,
    /// Caller cap for values in a sampled AIR trace row, clamped natively.
    pub max_air_width: usize,
    /// Caller cap for domain tag length, clamped to the canonical maximum.
    pub max_domain_tag_len: usize,
    /// Caller cap for transcript label length, clamped to the canonical maximum.
    pub max_transcript_label_len: usize,
    /// Caller cap for encoded envelope size, clamped to the native byte budget.
    pub max_envelope_bytes: usize,
}
impl Default for StarkVerifierLimits {
    fn default() -> Self {
        Self {
            max_domain_log2: MAX_DOMAIN_LOG2,
            max_blowup_log2: MAX_DOMAIN_LOG2,
            max_fold_arity: 1 << 5,
            max_queries: MAX_FRI_QUERIES,
            max_merkle_depth: MAX_MERKLE_DEPTH,
            max_aux_terms: MAX_AUX_TERMS,
            max_air_width: MAX_AIR_WIDTH,
            max_domain_tag_len: MAX_DOMAIN_TAG_LEN,
            max_transcript_label_len: MAX_TRANSCRIPT_LABEL_LEN,
            max_envelope_bytes: MAX_ENVELOPE_BYTES,
        }
    }
}
fn effective_max_domain_log2(limits: &StarkVerifierLimits) -> u8 {
    limits.max_domain_log2.min(MAX_DOMAIN_LOG2)
}
fn effective_max_blowup_log2(limits: &StarkVerifierLimits) -> u8 {
    limits.max_blowup_log2.min(MAX_DOMAIN_LOG2)
}
fn effective_max_fold_arity(limits: &StarkVerifierLimits) -> u8 {
    limits.max_fold_arity.min(1 << 5)
}
fn effective_max_queries(limits: &StarkVerifierLimits) -> usize {
    limits.max_queries.min(MAX_FRI_QUERIES)
}
fn effective_max_merkle_depth(limits: &StarkVerifierLimits) -> usize {
    limits.max_merkle_depth.min(MAX_MERKLE_DEPTH)
}
fn effective_max_aux_terms(limits: &StarkVerifierLimits) -> usize {
    limits.max_aux_terms.min(MAX_AUX_TERMS)
}
fn effective_max_air_width(limits: &StarkVerifierLimits) -> usize {
    limits.max_air_width.min(MAX_AIR_WIDTH)
}
fn effective_max_domain_tag_len(limits: &StarkVerifierLimits) -> usize {
    limits.max_domain_tag_len.min(MAX_DOMAIN_TAG_LEN)
}
fn effective_max_transcript_label_len(limits: &StarkVerifierLimits) -> usize {
    limits
        .max_transcript_label_len
        .min(MAX_TRANSCRIPT_LABEL_LEN)
}
fn effective_max_envelope_bytes(limits: &StarkVerifierLimits) -> usize {
    limits.max_envelope_bytes.min(MAX_ENVELOPE_BYTES)
}
/// Goldilocks field element with canonical modular reduction.
///
/// This backend keeps values in the range `[0, MOD_P)` and implements the
/// minimal arithmetic required by the native STARK verifier. Although kept
/// intentionally small, it now performs full modular reduction so that
/// callers do not need to pre-normalise inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fq(u64);
impl Fq {
    /// Construct an element from an arbitrary 64-bit integer by reducing it
    /// modulo `MOD_P`.
    fn new(v: u64) -> Self {
        Self::reduce(v as u128)
    }
    /// Construct from canonical representative. Returns `None` if the input is
    /// outside `[0, MOD_P)`.
    fn from_canonical_u64(v: u64) -> Option<Self> {
        if v >= MOD_P_U64 { None } else { Some(Self(v)) }
    }
    fn zero() -> Self {
        Self(0)
    }
    fn one() -> Self {
        Self(1)
    }
    fn add(self, rhs: Self) -> Self {
        let mut x = (self.0 as u128) + (rhs.0 as u128);
        if x >= MOD_P {
            x -= MOD_P;
        }
        Self(x as u64)
    }
    fn sub(self, rhs: Self) -> Self {
        let a = self.0 as u128;
        let b = rhs.0 as u128;
        let x = if a >= b { a - b } else { (a + MOD_P) - b };
        Self(x as u64)
    }
    fn mul(self, rhs: Self) -> Self {
        let x = (self.0 as u128) * (rhs.0 as u128);
        Self::reduce(x)
    }
    fn pow(self, mut e: u128) -> Self {
        let mut base = self;
        let mut acc = Self::one();
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(base);
            }
            base = base.mul(base);
            e >>= 1;
        }
        acc
    }
    fn inv(self) -> Option<Self> {
        if self.0 == 0 {
            return None;
        }
        // Fermat's little theorem: a^(p-2) mod p
        Some(self.pow((MOD_P - 2) as u128))
    }
    fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
    fn reduce(v: u128) -> Self {
        Self((v % MOD_P) as u64)
    }
}
fn two_inv() -> Fq {
    // (p + 1) / 2 for the odd Goldilocks prime.
    Fq(((MOD_P + 1) / 2) as u64)
}
fn domain_x_for_pair(layer_domain: usize, pair_index: usize) -> Option<Fq> {
    if layer_domain < 2 || !layer_domain.is_power_of_two() || pair_index >= layer_domain / 2 {
        return None;
    }
    let layer_domain = u128::try_from(layer_domain).ok()?;
    let exponent = (MOD_P - 1) / layer_domain;
    let root = Fq::new(GOLDILOCKS_GENERATOR).pow(exponent);
    Some(root.pow(pair_index as u128))
}
fn fri_fold_pair(y0: Fq, y1: Fq, beta: Fq, x: Fq) -> Option<Fq> {
    let inv_2x = x.mul(Fq::from_canonical_u64(2)?).inv()?;
    let even = y0.add(y1).mul(two_inv());
    let odd = y0.sub(y1).mul(inv_2x);
    Some(even.add(beta.mul(odd)))
}
fn u64_to_digest_le(val: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&val.to_le_bytes());
    out
}
fn digest_le_to_u64(bytes: &[u8; 32]) -> Option<u64> {
    if bytes[8..].iter().any(|b| *b != 0) {
        return None;
    }
    Some(u64::from_le_bytes(
        bytes[..8].try_into().expect("slice length"),
    ))
}
/// Transcript helper: derive a 64-bit field element challenge from label+bytes.
fn challenge(params: &StarkFriParamsV1, label: &str, bytes: &[u8]) -> Option<Fq> {
    match params.hash_fn {
        STARK_HASH_SHA256_V1 => {
            let mut h = Sha256::new();
            h.update(label.as_bytes());
            h.update(&[0u8]);
            h.update(bytes);
            let out = h.finalize();
            // Map to field by taking LE u64 and reducing
            let mut w = [0u8; 8];
            w.copy_from_slice(&out[..8]);
            let v = u64::from_le_bytes(w);
            Some(Fq::new((v as u128 % MOD_P) as u64))
        }
        STARK_HASH_POSEIDON2_V1 => {
            let mut preimage = Vec::with_capacity(label.len() + 1 + bytes.len());
            preimage.extend_from_slice(label.as_bytes());
            preimage.push(0);
            preimage.extend_from_slice(bytes);
            let packed = pack_bytes(&preimage);
            let len_field = u64::try_from(packed.length).ok()?;
            let mut limbs = Vec::with_capacity(packed.limbs.len() + 1);
            limbs.push(len_field);
            limbs.extend_from_slice(&packed.limbs);
            let v = hash_field_elements(&limbs);
            Fq::from_canonical_u64(v)
        }
        _ => None,
    }
}
/// Compute SHA-256 hash of a leaf value with domain separation.
fn leaf_hash(val: Fq) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"LEAF");
    h.update(&val.to_le_bytes());
    h.finalize().into()
}
/// Hash an internal node as SHA-256(left || right).
fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(left);
    h.update(right);
    h.finalize().into()
}
fn poseidon_domain_hash_u64(domain: &[u8], values: &[u64]) -> u64 {
    let packed = pack_bytes(domain);
    let len_field = u64::try_from(packed.length).unwrap_or(u64::MAX);
    let mut limbs = Vec::with_capacity(1 + packed.limbs.len() + values.len());
    limbs.push(len_field);
    limbs.extend_from_slice(&packed.limbs);
    limbs.extend_from_slice(values);
    hash_field_elements(&limbs)
}
fn poseidon_leaf_hash(val: Fq) -> [u8; 32] {
    // Domain-separated leaf hashing to avoid collisions with internal nodes.
    u64_to_digest_le(poseidon_domain_hash_u64(
        b"iroha:zk:stark:leaf:v1",
        &[val.0],
    ))
}
fn poseidon_node_hash(left: &[u8; 32], right: &[u8; 32]) -> Option<[u8; 32]> {
    let l = digest_le_to_u64(left)?;
    let r = digest_le_to_u64(right)?;
    Some(u64_to_digest_le(poseidon_domain_hash_u64(
        b"iroha:zk:stark:node:v1",
        &[l, r],
    )))
}
/// Verify a Merkle inclusion proof for a leaf value to `root`.
fn merkle_verify(params: &StarkFriParamsV1, root: &[u8; 32], leaf: Fq, path: &MerklePath) -> bool {
    let mut acc = match params.hash_fn {
        STARK_HASH_SHA256_V1 => leaf_hash(leaf),
        STARK_HASH_POSEIDON2_V1 => poseidon_leaf_hash(leaf),
        _ => return false,
    };
    for (i, sib) in path.siblings.iter().enumerate() {
        let byte = i / 8;
        if byte >= path.dirs.len() {
            return false;
        }
        let dir_bit = (path.dirs[byte] >> (i % 8)) & 1; // 0: leaf on left, 1: leaf on right
        acc = match params.hash_fn {
            STARK_HASH_SHA256_V1 => {
                if dir_bit == 0 {
                    node_hash(&acc, sib)
                } else {
                    node_hash(sib, &acc)
                }
            }
            STARK_HASH_POSEIDON2_V1 => {
                let next = if dir_bit == 0 {
                    poseidon_node_hash(&acc, sib)
                } else {
                    poseidon_node_hash(sib, &acc)
                };
                match next {
                    Some(v) => v,
                    None => return false,
                }
            }
            _ => return false,
        };
    }
    &acc == root
}
fn merkle_path_index(path: &MerklePath) -> Option<usize> {
    let depth = path.siblings.len();
    if depth == 0 {
        return Some(0);
    }
    if depth > usize::BITS as usize {
        return None;
    }
    let mut index = 0usize;
    for i in 0..depth {
        let byte = i / 8;
        if byte >= path.dirs.len() {
            return None;
        }
        let dir_bit = (path.dirs[byte] >> (i % 8)) & 1;
        index |= (dir_bit as usize) << i;
    }
    Some(index)
}
fn merkle_path_depth_ok(
    path: &MerklePath,
    expected_depth: usize,
    limits: &StarkVerifierLimits,
) -> bool {
    if expected_depth > effective_max_merkle_depth(limits) || path.siblings.len() != expected_depth
    {
        return false;
    }
    let required_dir_bytes = (expected_depth + 7) / 8;
    if path.dirs.len() != required_dir_bytes {
        return false;
    }
    if expected_depth % 8 != 0 {
        let used_bits = expected_depth % 8;
        if let Some(&last) = path.dirs.last() {
            let mask = !((1u8 << used_bits) - 1);
            if last & mask != 0 {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}
/// Verify a STARK FRI envelope under `zk-stark` with default limits.
pub fn verify_stark_fri_envelope(bytes: &[u8]) -> bool {
    verify_stark_fri_envelope_with_limits(bytes, &StarkVerifierLimits::default())
}
fn log2_usize(value: usize) -> Option<usize> {
    if value == 0 || !value.is_power_of_two() {
        return None;
    }
    Some(usize::BITS as usize - 1 - value.leading_zeros() as usize)
}
fn layers_required(params: &StarkFriParamsV1) -> Option<usize> {
    if params.fold_arity < 2 {
        return None;
    }
    let mut domain = 1usize << params.n_log2;
    let fold = params.fold_arity as usize;
    if !fold.is_power_of_two() {
        return None;
    }
    let mut layers = 0usize;
    while domain > 1 {
        if domain % fold != 0 {
            return None;
        }
        domain /= fold;
        layers += 1;
        if layers > MAX_FRI_LAYERS {
            return None;
        }
    }
    Some(layers)
}
fn validate_params(
    params: &StarkFriParamsV1,
    roots_len: usize,
    query_count: usize,
    limits: &StarkVerifierLimits,
) -> Option<usize> {
    if params.version != 1
        || params.n_log2 == 0
        || params.n_log2 > effective_max_domain_log2(limits)
    {
        return None;
    }
    if params.blowup_log2 == 0
        || params.blowup_log2 > effective_max_blowup_log2(limits)
        || params.blowup_log2 > params.n_log2
    {
        return None;
    }
    // The current wire format (`FoldDecommitV1`) carries a binary fold (y0,y1),
    // so only `fold_arity = 2` is supported by the native verifier.
    if params.fold_arity != 2 || params.fold_arity > effective_max_fold_arity(limits) {
        return None;
    }
    if params.merkle_arity != 2 {
        return None;
    }
    if params.hash_fn != STARK_HASH_SHA256_V1 && params.hash_fn != STARK_HASH_POSEIDON2_V1 {
        return None;
    }
    if validate_stark_domain_tag(&params.domain_tag, effective_max_domain_tag_len(limits)).is_err()
    {
        return None;
    }
    if params.queries == 0
        || params.queries as usize > effective_max_queries(limits)
        || params.queries as usize != query_count
    {
        return None;
    }
    let domain = 1usize.checked_shl(u32::from(params.n_log2))?;
    if query_count > domain {
        return None;
    }
    if roots_len == 0 || roots_len > effective_max_merkle_depth(limits) + 1 {
        return None;
    }
    let required_layers = layers_required(params)?;
    if roots_len != required_layers + 1 {
        return None;
    }
    Some(required_layers)
}
fn validate_stark_opening_commitment_params_with_limits_v1(
    params: &StarkFriParamsV1,
    limits: &StarkVerifierLimits,
) -> Result<(), &'static str> {
    let roots_len = layers_required(params)
        .and_then(|layers| layers.checked_add(1))
        .ok_or("STARK opening commitment parameters invalid")?;
    if validate_params(params, roots_len, usize::from(params.queries), limits).is_none() {
        return Err("STARK opening commitment parameters invalid");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    include!("zk_stark/tests.rs");
}
#[cfg(test)]
fn derive_query_index(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
) -> Option<usize> {
    if params.n_log2 as u32 >= usize::BITS {
        return None;
    }
    let domain = 1usize << params.n_log2;
    if domain == 0 {
        return None;
    }
    let (word, _) = derive_query_challenge_word(label, params, roots, query_idx, 0)?;
    Some((u128::from(word) % (domain as u128)) as usize)
}
fn extend_query_challenge_preimage(
    preimage: &mut Vec<u8>,
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
    rejection_attempt: usize,
) -> Option<()> {
    preimage.extend_from_slice(b"STARK:query-index");
    preimage.extend_from_slice(label.as_bytes());
    preimage.extend_from_slice(&params.version.to_le_bytes());
    preimage.extend_from_slice(&[
        params.n_log2,
        params.blowup_log2,
        params.fold_arity,
        params.merkle_arity,
        params.hash_fn,
    ]);
    preimage.extend_from_slice(&params.queries.to_le_bytes());
    preimage.extend_from_slice(&(params.domain_tag.len() as u32).to_le_bytes());
    preimage.extend_from_slice(params.domain_tag.as_bytes());
    preimage.extend_from_slice(&u64::try_from(query_idx).ok()?.to_le_bytes());
    if rejection_attempt != 0 {
        preimage.extend_from_slice(b"STARK:query-index:bounded-retry");
        preimage.extend_from_slice(&u64::try_from(rejection_attempt).ok()?.to_le_bytes());
    }
    for root in roots {
        preimage.extend_from_slice(root);
    }
    Some(())
}
fn derive_query_challenge_word(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
    rejection_attempt: usize,
) -> Option<(u64, u128)> {
    match params.hash_fn {
        STARK_HASH_SHA256_V1 => {
            let mut preimage = Vec::new();
            extend_query_challenge_preimage(
                &mut preimage,
                label,
                params,
                roots,
                query_idx,
                rejection_attempt,
            )?;
            let mut h = Sha256::new();
            h.update(&preimage);
            let digest = h.finalize();
            let mut w = [0u8; 8];
            w.copy_from_slice(&digest[..8]);
            Some((u64::from_le_bytes(w), 1_u128 << 64))
        }
        STARK_HASH_POSEIDON2_V1 => {
            let mut preimage = Vec::new();
            extend_query_challenge_preimage(
                &mut preimage,
                label,
                params,
                roots,
                query_idx,
                rejection_attempt,
            )?;
            let packed = pack_bytes(&preimage);
            let len_field = u64::try_from(packed.length).ok()?;
            let mut limbs = Vec::with_capacity(packed.limbs.len() + 1);
            limbs.push(len_field);
            limbs.extend_from_slice(&packed.limbs);
            let v = hash_field_elements(&limbs);
            Some((v, MOD_P))
        }
        _ => None,
    }
}
fn derive_bounded_query_offset(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
    bound: usize,
) -> Result<usize, &'static str> {
    if bound == 0 {
        return Err("FRI query index derivation failed");
    }
    let bound = bound as u128;
    for rejection_attempt in 0..STARK_FRI_BOUNDED_QUERY_REJECTION_ATTEMPTS {
        let (word, source_space) =
            derive_query_challenge_word(label, params, roots, query_idx, rejection_attempt)
                .ok_or("FRI query index derivation failed")?;
        let limit = source_space
            .checked_sub(source_space % bound)
            .ok_or("FRI query index derivation failed")?;
        let word = u128::from(word);
        if word < limit {
            return usize::try_from(word % bound).map_err(|_| "FRI query index derivation failed");
        }
    }
    Err("FRI query index derivation failed")
}
fn derive_query_indices_without_replacement(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_count: usize,
    domain: usize,
) -> Result<Vec<usize>, &'static str> {
    if domain == 0 {
        return Err("FRI domain size overflow");
    }
    if query_count > domain {
        return Err("FRI query count exceeds domain size");
    }
    let mut swaps = BTreeMap::new();
    let mut indices = Vec::with_capacity(query_count);
    for query_number in 0..query_count {
        let remaining = domain
            .checked_sub(query_number)
            .ok_or("FRI query index derivation failed")?;
        let offset = derive_bounded_query_offset(label, params, roots, query_number, remaining)?;
        let draw = query_number
            .checked_add(offset)
            .ok_or("FRI query index derivation failed")?;
        let selected = swaps.get(&draw).copied().unwrap_or(draw);
        if indices.contains(&selected) {
            return Err(STARK_FRI_QUERY_INDEX_REPEATED_ERROR);
        }
        let replacement = swaps.get(&query_number).copied().unwrap_or(query_number);
        swaps.insert(draw, replacement);
        indices.push(selected);
    }
    Ok(indices)
}
/// Norito-serializable Merkle path (dirs as bitset, siblings as hashes).
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct MerklePath {
    /// Direction bits per level: 0 => leaf/hash on left, 1 => on right
    pub dirs: Vec<u8>,
    /// Sibling hashes from leaf to root (one per level)
    pub siblings: Vec<[u8; 32]>,
}
/// Parameters for a binary multi-round FRI check.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkFriParamsV1 {
    /// Version tag for format evolution
    pub version: u16,
    /// Log2 of evaluation domain size (e.g., 3 for size 8)
    pub n_log2: u8,
    /// Log2 of the blowup factor applied before FRI folding (e.g., 3 for 8x)
    pub blowup_log2: u8,
    /// Arity of each FRI fold (must be a power of two; current backend supports 2)
    pub fold_arity: u8,
    /// Number of queries expected in the proof (must match `proof.queries.len()`)
    pub queries: u16,
    /// Merkle branching factor (current backend supports binary trees only)
    pub merkle_arity: u8,
    /// Hash function selector (`1 = SHA-256`, `2 = Poseidon2`)
    pub hash_fn: u8,
    /// Domain tag mixed into transcripts and query sampling
    pub domain_tag: String,
}
/// Minimal verifying-key payload for the `stark/fri/*` backends.
///
/// This is stored inside [`iroha_data_model::proof::VerifyingKeyBox::bytes`] and
/// pins the verifier parameters (hash function, domain size, query count, etc.).
///
/// Note: `domain_tag` is **not** part of the verifying key because it is instance-specific
/// and is derived from the outer [`iroha_data_model::zk::OpenVerifyEnvelope`] metadata.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkFriVerifyingKeyV1 {
    /// Version tag for format evolution.
    pub version: u16,
    /// Canonical circuit identifier string.
    pub circuit_id: String,
    /// Log2 of evaluation domain size.
    pub n_log2: u8,
    /// Log2 of the blowup factor applied before FRI folding.
    pub blowup_log2: u8,
    /// Arity of each FRI fold (current wire format supports 2).
    pub fold_arity: u8,
    /// Number of queries sampled by the verifier.
    pub queries: u16,
    /// Merkle branching factor (current backend supports binary trees only).
    pub merkle_arity: u8,
    /// Hash function selector (`1 = SHA-256`, `2 = Poseidon2`).
    pub hash_fn: u8,
}
/// Maximum canonical encoding accepted for a STARK/FRI V1 verifying key.
///
/// The payload contains one bounded circuit identifier and a fixed set of
/// scalar parameters, so 4 KiB leaves ample format headroom without allowing
/// registry input to inherit a caller-sized decode budget.
pub const STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES: usize = 4 * 1024;
const STARK_FRI_VERIFYING_KEY_V1_MAX_NESTING_DEPTH: usize = 8;
/// Decode one exact canonical STARK/FRI V1 verifying-key payload under a
/// schema-specific resource budget.
///
/// # Errors
///
/// Returns an error when the frame exceeds the V1 byte bound, advertises an
/// oversized field or allocation, is not canonical Norito, or does not decode
/// to [`StarkFriVerifyingKeyV1`].
pub fn decode_stark_fri_verifying_key_v1(bytes: &[u8]) -> Result<StarkFriVerifyingKeyV1, String> {
    if bytes.len() > STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES {
        return Err(format!(
            "STARK/FRI verifier key exceeds the {}-byte limit",
            STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES
        ));
    }
    let circuit_id_limit = MAX_TRANSCRIPT_LABEL_LEN;
    let limits = norito::DecodeLimits::new(
        circuit_id_limit,
        STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES,
        circuit_id_limit.saturating_add(16),
        STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES.saturating_mul(2),
        STARK_FRI_VERIFYING_KEY_V1_MAX_NESTING_DEPTH,
    );
    norito::decode_canonical_with_limits(bytes, limits)
        .map_err(|err| format!("invalid canonical STARK/FRI verifier key: {err}"))
}
/// Validate that a STARK/FRI verifier-key payload uses ledger-grade verifier parameters.
///
/// This is a control-plane floor for proof-system admission. It rejects historical
/// PoC-sized STARK/FRI parameters while still leaving circuit-specific algebraic
/// validation to the verifier for each proof.
pub fn validate_stark_fri_canonical_verifying_key_payload(
    payload: &StarkFriVerifyingKeyV1,
    circuit_id: &str,
    label: &str,
) -> Result<(), String> {
    validate_stark_circuit_id(circuit_id).map_err(|err| {
        format!("{label} STARK/FRI verifier key expected circuit id invalid: {err}")
    })?;
    if payload.version != 1 {
        return Err(format!("{label} STARK/FRI verifier key must use version 1"));
    }
    validate_stark_circuit_id(&payload.circuit_id)
        .map_err(|err| format!("{label} STARK/FRI verifier key circuit id invalid: {err}"))?;
    if payload.circuit_id != circuit_id {
        return Err(format!(
            "{label} STARK/FRI verifier key circuit id mismatch"
        ));
    }
    if payload.hash_fn != STARK_HASH_SHA256_V1 && payload.hash_fn != STARK_HASH_POSEIDON2_V1 {
        return Err(format!(
            "{label} STARK/FRI verifier key must use SHA-256 or Poseidon2"
        ));
    }
    if payload.fold_arity != 2 {
        return Err(format!(
            "{label} STARK/FRI verifier key must use binary FRI folding"
        ));
    }
    if payload.merkle_arity != 2 {
        return Err(format!(
            "{label} STARK/FRI verifier key must use binary Merkle paths"
        ));
    }
    if payload.n_log2 < STARK_FRI_CONSENSUS_MIN_N_LOG2 {
        return Err(format!(
            "{label} STARK/FRI n_log2 {} is below consensus floor {}",
            payload.n_log2, STARK_FRI_CONSENSUS_MIN_N_LOG2
        ));
    }
    if payload.blowup_log2 < STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2 {
        return Err(format!(
            "{label} STARK/FRI blowup_log2 {} is below consensus floor {}",
            payload.blowup_log2, STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2
        ));
    }
    if payload.blowup_log2 > payload.n_log2 {
        return Err(format!(
            "{label} STARK/FRI blowup_log2 {} exceeds n_log2 {}",
            payload.blowup_log2, payload.n_log2
        ));
    }
    if payload.queries < STARK_FRI_CONSENSUS_MIN_QUERIES {
        return Err(format!(
            "{label} STARK/FRI queries {} is below consensus floor {}",
            payload.queries, STARK_FRI_CONSENSUS_MIN_QUERIES
        ));
    }
    if payload.n_log2 > MAX_DOMAIN_LOG2
        || payload.blowup_log2 > MAX_DOMAIN_LOG2
        || usize::from(payload.queries) > MAX_FRI_QUERIES
    {
        return Err(format!(
            "{label} STARK/FRI verifier key exceeds native verifier limits"
        ));
    }
    Ok(())
}
#[cfg(test)]
mod verifying_key_decode_tests {
    use super::*;
    fn canonical_payload() -> StarkFriVerifyingKeyV1 {
        StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: "stark/fri/sha256-goldilocks:bounded-vk-test".to_owned(),
            n_log2: STARK_FRI_CONSENSUS_MIN_N_LOG2,
            blowup_log2: STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: STARK_FRI_CONSENSUS_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        }
    }
    #[test]
    fn bounded_verifying_key_decode_accepts_canonical_payload() {
        let payload = canonical_payload();
        let bytes = norito::encode_canonical(&payload).expect("encode canonical STARK key");
        let decoded = decode_stark_fri_verifying_key_v1(&bytes)
            .expect("bounded decoder must accept canonical STARK key");
        assert_eq!(decoded.circuit_id, payload.circuit_id);
        assert_eq!(decoded.queries, payload.queries);
        assert_eq!(decoded.hash_fn, payload.hash_fn);
    }
    #[test]
    fn bounded_verifying_key_decode_rejects_alternate_layout() {
        let payload = canonical_payload();
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&payload).expect("encode alternate-layout STARK key")
        };
        norito::decode_from_bytes::<StarkFriVerifyingKeyV1>(&alternate)
            .expect("ordinary decoder accepts the advertised alternate layout");
        assert!(
            decode_stark_fri_verifying_key_v1(&alternate).is_err(),
            "registry decoder must accept only the canonical layout"
        );
    }
    #[test]
    fn bounded_verifying_key_decode_rejects_huge_declared_inner_length() {
        let payload = canonical_payload();
        let mut bytes = norito::encode_canonical(&payload).expect("encode canonical STARK key");
        let circuit = payload.circuit_id.as_bytes();
        let circuit_offset = bytes
            .windows(circuit.len())
            .position(|window| window == circuit)
            .expect("encoded circuit id");
        assert!(circuit_offset > 0, "circuit id must carry a length prefix");
        bytes[circuit_offset - 1] = u8::MAX;
        assert!(
            decode_stark_fri_verifying_key_v1(&bytes).is_err(),
            "a tiny frame must not allocate from an attacker-declared inner length"
        );
    }
    #[test]
    fn bounded_verifying_key_decode_rejects_oversized_frame() {
        let bytes = vec![0_u8; STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES + 1];
        let error = decode_stark_fri_verifying_key_v1(&bytes)
            .expect_err("oversized verifier-key frame must fail before decoding");
        assert!(error.contains("exceeds"), "unexpected error: {error}");
    }
}
/// Commitments for multiple layers and optional composition root.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkCommitmentsV1 {
    /// Version tag for format evolution
    pub version: u16,
    /// Merkle roots per layer, from layer 0 (original evaluations) to layer L (final folded layer)
    pub roots: Vec<[u8; 32]>,
    /// Optional composition polynomial root over the final layer domain (length n >> L)
    pub comp_root: Option<[u8; 32]>,
}
/// Auxiliary term contributing to the composition polynomial evaluation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
)]
pub struct StarkCompositionTermV1 {
    /// Canonical wire index for this auxiliary value (monotonic, caller-defined ordering)
    pub wire_index: u32,
    /// Value contributed by this wire
    pub value: u64,
    /// Coefficient multiplied with the value
    pub coeff: u64,
}
/// Composition leaf data stored under `comp_root`.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkCompositionValueV1 {
    /// Merkle leaf value recorded under `comp_root`
    pub leaf: u64,
    /// Constant term added to the composition result
    pub constant: u64,
    /// Coefficient applied to the final folded `z` value
    pub z_coeff: u64,
    /// Additional auxiliary wire contributions
    pub aux_terms: Vec<StarkCompositionTermV1>,
    /// Inclusion path for the leaf under `comp_root`
    pub path: MerklePath,
}
/// Sampled AIR trace and composition opening for one verifier query.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkAirOpeningV1 {
    /// Evaluation-domain index sampled by the verifier transcript.
    pub index: u32,
    /// AIR trace row at `index`.
    pub row: Vec<u64>,
    /// AIR trace row at `(index + 1) mod domain_size`.
    pub next_row: Vec<u64>,
    /// Inclusion path for `row` under [`StarkAirProofV1::trace_root`].
    pub row_path: MerklePath,
    /// Inclusion path for `next_row` under [`StarkAirProofV1::trace_root`].
    pub next_row_path: MerklePath,
    /// AIR composition evaluation at `index`; this is the FRI base-layer value.
    pub composition_value: u64,
    /// Inclusion path for `composition_value` under [`StarkAirProofV1::composition_root`].
    pub composition_path: MerklePath,
}
/// Verifier-owned AIR statement carried by V1 STARK proofs.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkAirProofV1 {
    /// Version tag.
    pub version: u16,
    /// Canonical circuit identifier for the public statement.
    pub circuit_id: String,
    /// Digest of the public statement reconstructed by the caller.
    pub public_digest: [u8; 32],
    /// Merkle root over row-major AIR trace rows.
    pub trace_root: [u8; 32],
    /// Merkle root over AIR composition evaluations; must equal FRI layer root 0.
    pub composition_root: [u8; 32],
    /// Number of field values in each AIR trace row.
    pub trace_width: u16,
    /// Sampled AIR openings, one per FRI query.
    pub openings: Vec<StarkAirOpeningV1>,
}
/// Decommitment for one fold step at layer `k`.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct FoldDecommitV1 {
    /// Index j at this layer (so layer k reads positions 2*j and 2*j+1 from layer k)
    pub j: u32,
    /// Two values from layer k: y0 = f(2*j), y1 = f(2*j+1)
    pub y0: u64,
    /// Right branch value at this layer (position 2*j+1)
    pub y1: u64,
    /// Merkle paths for y0 and y1 in layer k
    pub path_y0: MerklePath,
    /// Merkle path for y1 in layer k
    pub path_y1: MerklePath,
    /// Folded value at layer k+1, with Merkle path into root[k+1].
    ///
    /// Current V1 semantics interpret the two adjacent openings as evaluations at
    /// `(x, -x)` and require
    /// `z = (y0 + y1) / 2 + r_k * (y0 - y1) / (2x)`.
    pub z: u64,
    /// Merkle path for the folded value z in the next layer (k+1)
    pub path_z: MerklePath,
}
/// STARK proof carrying commitments and query decommitments.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkProofV1 {
    /// Version tag
    pub version: u16,
    /// Commitment roots
    pub commits: StarkCommitmentsV1,
    /// Query decommitments: one chain of folds per query
    pub queries: Vec<Vec<FoldDecommitV1>>,
    /// Optional composition leaf, auxiliary inputs, and path at final layer per query.
    ///
    /// When present, the expected composition leaf is
    /// `constant + z_coeff*z_final + sum_i coeff_i * value_i`.
    pub comp_values: Option<Vec<StarkCompositionValueV1>>,
    /// V1 AIR section binding sampled trace and composition openings to FRI.
    pub air: Option<StarkAirProofV1>,
}
/// Verification envelope for STARK FRI multi-round (binary) proofs.
#[derive(
    Debug, Clone, JsonSerialize, JsonDeserialize, norito::NoritoSerialize, norito::NoritoDeserialize,
)]
pub struct StarkVerifyEnvelopeV1 {
    /// Parameters used by the prover
    pub params: StarkFriParamsV1,
    /// Proof object
    pub proof: StarkProofV1,
    /// Transcript label to domain-separate instances
    pub transcript_label: String,
}
fn merkle_levels_from_values(
    params: &StarkFriParamsV1,
    values: &[Fq],
) -> Option<Vec<Vec<[u8; 32]>>> {
    if values.is_empty() {
        return None;
    }
    let mut current = values
        .iter()
        .map(|&value| match params.hash_fn {
            STARK_HASH_SHA256_V1 => Some(leaf_hash(value)),
            STARK_HASH_POSEIDON2_V1 => Some(poseidon_leaf_hash(value)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let mut levels = Vec::new();
    loop {
        levels.push(current.clone());
        if current.len() == 1 {
            break;
        }
        if current.len() % 2 == 1 {
            let last = *current.last()?;
            current.push(last);
        }
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks_exact(2) {
            next.push(match params.hash_fn {
                STARK_HASH_SHA256_V1 => node_hash(&pair[0], &pair[1]),
                STARK_HASH_POSEIDON2_V1 => poseidon_node_hash(&pair[0], &pair[1])?,
                _ => return None,
            });
        }
        current = next;
    }
    Some(levels)
}
fn merkle_path_from_levels(index: usize, levels: &[Vec<[u8; 32]>]) -> Option<MerklePath> {
    let leaf_level = levels.first()?;
    if index >= leaf_level.len() {
        return None;
    }
    let depth = levels.len().checked_sub(1)?;
    if depth > usize::BITS as usize {
        return None;
    }
    let mut dirs = vec![0u8; (depth + 7) / 8];
    let mut siblings = Vec::with_capacity(depth);
    let mut current_index = index;
    for (level_idx, level) in levels.iter().take(depth).enumerate() {
        if current_index >= level.len() {
            return None;
        }
        let sibling_idx = if current_index.is_multiple_of(2) {
            current_index + 1
        } else {
            current_index.saturating_sub(1)
        };
        let sibling = level
            .get(sibling_idx)
            .copied()
            .unwrap_or_else(|| level[current_index]);
        if current_index % 2 == 1 {
            dirs[level_idx / 8] |= 1u8 << (level_idx % 8);
        }
        siblings.push(sibling);
        current_index /= 2;
    }
    Some(MerklePath { dirs, siblings })
}
fn merkle_root_from_levels(levels: &[Vec<[u8; 32]>]) -> Option<[u8; 32]> {
    levels.last()?.first().copied()
}
fn merkle_levels_from_hashes(
    params: &StarkFriParamsV1,
    leaves: Vec<[u8; 32]>,
) -> Option<Vec<Vec<[u8; 32]>>> {
    if leaves.is_empty() {
        return None;
    }
    let mut current = leaves;
    let mut levels = Vec::new();
    loop {
        levels.push(current.clone());
        if current.len() == 1 {
            break;
        }
        if current.len() % 2 == 1 {
            let last = *current.last()?;
            current.push(last);
        }
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks_exact(2) {
            next.push(match params.hash_fn {
                STARK_HASH_SHA256_V1 => node_hash(&pair[0], &pair[1]),
                STARK_HASH_POSEIDON2_V1 => poseidon_node_hash(&pair[0], &pair[1])?,
                _ => return None,
            });
        }
        current = next;
    }
    Some(levels)
}
fn merkle_verify_hash(
    params: &StarkFriParamsV1,
    root: &[u8; 32],
    leaf: &[u8; 32],
    path: &MerklePath,
) -> bool {
    let mut acc = *leaf;
    for (i, sib) in path.siblings.iter().enumerate() {
        let byte = i / 8;
        if byte >= path.dirs.len() {
            return false;
        }
        let dir_bit = (path.dirs[byte] >> (i % 8)) & 1;
        acc = match params.hash_fn {
            STARK_HASH_SHA256_V1 => {
                if dir_bit == 0 {
                    node_hash(&acc, sib)
                } else {
                    node_hash(sib, &acc)
                }
            }
            STARK_HASH_POSEIDON2_V1 => {
                let next = if dir_bit == 0 {
                    poseidon_node_hash(&acc, sib)
                } else {
                    poseidon_node_hash(sib, &acc)
                };
                match next {
                    Some(value) => value,
                    None => return false,
                }
            }
            _ => return false,
        };
    }
    &acc == root
}
/// Build a v1 STARK Merkle root from canonical field values.
pub(crate) fn stark_merkle_root_from_field_values_v1(
    params: &StarkFriParamsV1,
    values: &[u64],
) -> Option<[u8; 32]> {
    let values = values
        .iter()
        .copied()
        .map(Fq::from_canonical_u64)
        .collect::<Option<Vec<_>>>()?;
    let levels = merkle_levels_from_values(params, &values)?;
    merkle_root_from_levels(&levels)
}
/// Build a v1 STARK AIR trace Merkle root from row-major trace values.
pub(crate) fn stark_air_trace_root_from_rows_v1(
    params: &StarkFriParamsV1,
    rows: &[Vec<u64>],
) -> Option<[u8; 32]> {
    let trace_leaves = rows
        .iter()
        .map(|row| stark_air_trace_leaf_hash(params, row))
        .collect::<Option<Vec<_>>>()?;
    let levels = merkle_levels_from_hashes(params, trace_leaves)?;
    merkle_root_from_levels(&levels)
}
/// Build a v1 STARK Merkle root and path from canonical field values.
#[cfg(test)]
pub(crate) fn stark_merkle_root_and_path_from_field_values_v1(
    params: &StarkFriParamsV1,
    values: &[u64],
    index: usize,
) -> Option<([u8; 32], MerklePath)> {
    let values = values
        .iter()
        .copied()
        .map(Fq::from_canonical_u64)
        .collect::<Option<Vec<_>>>()?;
    let levels = merkle_levels_from_values(params, &values)?;
    Some((
        merkle_root_from_levels(&levels)?,
        merkle_path_from_levels(index, &levels)?,
    ))
}
/// Build a deterministic V1 STARK/FRI envelope from canonical field values.
#[cfg(test)]
pub(crate) fn stark_synthesize_fri_envelope_from_field_values_v1(
    params: StarkFriParamsV1,
    transcript_label: String,
    values: &[u64],
    extra_query_roots: &[[u8; 32]],
) -> Option<StarkVerifyEnvelopeV1> {
    let values = values
        .iter()
        .copied()
        .map(Fq::from_canonical_u64)
        .collect::<Option<Vec<_>>>()?;
    synthesize_stark_fri_envelope_from_values(params, transcript_label, values, extra_query_roots)
        .ok()
}
/// Verify that one native AIR opening is bound to its Merkle path indices and roots.
#[cfg(test)]
pub(crate) fn validate_stark_air_opening_commitment_roots_v1(
    params: &StarkFriParamsV1,
    air: &StarkAirProofV1,
    opening: &StarkAirOpeningV1,
) -> Result<(), &'static str> {
    validate_stark_air_opening_commitment_roots_with_limits_v1(
        params,
        air,
        opening,
        &StarkVerifierLimits::default(),
    )
}
/// Verify that one native AIR opening is bound to its Merkle path indices and roots.
pub(crate) fn validate_stark_air_opening_commitment_roots_with_limits_v1(
    params: &StarkFriParamsV1,
    air: &StarkAirProofV1,
    opening: &StarkAirOpeningV1,
    limits: &StarkVerifierLimits,
) -> Result<(), &'static str> {
    validate_stark_opening_commitment_params_with_limits_v1(params, limits)?;
    let domain_size = 1_usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or("opening domain size overflow")?;
    let opening_index = usize::try_from(opening.index).map_err(|_| "opening index out of range")?;
    if opening_index >= domain_size {
        return Err("opening index out of range");
    }
    let expected_depth = log2_usize(domain_size).ok_or("opening Merkle path depth mismatch")?;
    if !merkle_path_depth_ok(&opening.row_path, expected_depth, limits)
        || !merkle_path_depth_ok(&opening.next_row_path, expected_depth, limits)
        || !merkle_path_depth_ok(&opening.composition_path, expected_depth, limits)
    {
        return Err("opening Merkle path depth mismatch");
    }
    let next_index = (opening_index + 1) % domain_size;
    if merkle_path_index(&opening.row_path) != Some(opening_index)
        || merkle_path_index(&opening.next_row_path) != Some(next_index)
        || merkle_path_index(&opening.composition_path) != Some(opening_index)
    {
        return Err("opening Merkle path index mismatch");
    }
    let row_leaf = stark_air_trace_leaf_hash(params, &opening.row).ok_or("row leaf hash failed")?;
    if !merkle_verify_hash(params, &air.trace_root, &row_leaf, &opening.row_path) {
        return Err("row Merkle root mismatch");
    }
    let next_row_leaf =
        stark_air_trace_leaf_hash(params, &opening.next_row).ok_or("next-row leaf hash failed")?;
    if !merkle_verify_hash(
        params,
        &air.trace_root,
        &next_row_leaf,
        &opening.next_row_path,
    ) {
        return Err("next-row Merkle root mismatch");
    }
    let composition =
        Fq::from_canonical_u64(opening.composition_value).ok_or("composition field element")?;
    if !merkle_verify(
        params,
        &air.composition_root,
        composition,
        &opening.composition_path,
    ) {
        return Err("composition Merkle root mismatch");
    }
    Ok(())
}
/// Validate FRI query-chain shape, commitments, folds, and return transcript-derived base indices.
pub(crate) fn validate_stark_fri_query_shape_and_indices_v1(
    params: &StarkFriParamsV1,
    transcript_label: &str,
    roots: &[[u8; 32]],
    extra_query_roots: &[[u8; 32]],
    queries: &[Vec<FoldDecommitV1>],
) -> Result<Vec<usize>, &'static str> {
    validate_stark_fri_query_shape_and_indices_with_limits_v1(
        params,
        transcript_label,
        roots,
        extra_query_roots,
        queries,
        &StarkVerifierLimits::default(),
    )
}
pub(crate) fn validate_stark_fri_query_shape_and_indices_with_limits_v1(
    params: &StarkFriParamsV1,
    transcript_label: &str,
    roots: &[[u8; 32]],
    extra_query_roots: &[[u8; 32]],
    queries: &[Vec<FoldDecommitV1>],
    limits: &StarkVerifierLimits,
) -> Result<Vec<usize>, &'static str> {
    validate_stark_transcript_label(transcript_label, effective_max_transcript_label_len(limits))
        .map_err(|_| "FRI transcript label invalid")?;
    let expected_chain_len = validate_params(params, roots.len(), queries.len(), limits)
        .ok_or("FRI parameter/root/query shape mismatch")?;
    let total_domain = 1_usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or("FRI domain size overflow")?;
    let fold_arity = usize::from(params.fold_arity);
    let mut query_roots = roots.to_vec();
    query_roots.extend_from_slice(extra_query_roots);
    let base_indices = derive_query_indices_without_replacement(
        transcript_label,
        params,
        &query_roots,
        queries.len(),
        total_domain,
    )?;
    for (chain, mut idx_layer) in queries.iter().zip(base_indices.iter().copied()) {
        if chain.len() != expected_chain_len {
            return Err("FRI query chain length mismatch");
        }
        let mut layer_domain = total_domain;
        for (round, decommit) in chain.iter().enumerate() {
            if layer_domain < fold_arity {
                return Err("FRI query layer domain underflow");
            }
            let expected_pairs = layer_domain / fold_arity;
            let expected_j = idx_layer / fold_arity;
            if expected_j >= expected_pairs || usize::try_from(decommit.j).ok() != Some(expected_j)
            {
                return Err("FRI query fold index mismatch");
            }
            let depth_current =
                log2_usize(layer_domain).ok_or("FRI query current layer depth mismatch")?;
            let depth_next = log2_usize(layer_domain / fold_arity)
                .ok_or("FRI query next layer depth mismatch")?;
            if !merkle_path_depth_ok(&decommit.path_y0, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_y1, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_z, depth_next, limits)
            {
                return Err("FRI query Merkle path depth mismatch");
            }
            let expected_y0 = expected_j
                .checked_mul(fold_arity)
                .ok_or("FRI query y0 index overflow")?;
            let expected_y1 = expected_y0
                .checked_add(1)
                .ok_or("FRI query y1 index overflow")?;
            if merkle_path_index(&decommit.path_y0) != Some(expected_y0)
                || merkle_path_index(&decommit.path_y1) != Some(expected_y1)
                || merkle_path_index(&decommit.path_z) != Some(expected_j)
            {
                return Err("FRI query Merkle path index mismatch");
            }
            let y0 = Fq::from_canonical_u64(decommit.y0).ok_or("FRI query y0 field element")?;
            let y1 = Fq::from_canonical_u64(decommit.y1).ok_or("FRI query y1 field element")?;
            let z = Fq::from_canonical_u64(decommit.z).ok_or("FRI query z field element")?;
            let current_root = roots.get(round).ok_or("FRI query current root missing")?;
            let next_root = roots.get(round + 1).ok_or("FRI query next root missing")?;
            if !merkle_verify(params, current_root, y0, &decommit.path_y0)
                || !merkle_verify(params, current_root, y1, &decommit.path_y1)
            {
                return Err("FRI query Merkle root mismatch");
            }
            let beta = fri_round_challenge(params, transcript_label, current_root)
                .ok_or("FRI query challenge derivation failed")?;
            let x = domain_x_for_pair(layer_domain, expected_j)
                .ok_or("FRI query domain element derivation failed")?;
            if fri_fold_pair(y0, y1, beta, x) != Some(z) {
                return Err("FRI query fold relation mismatch");
            }
            if !merkle_verify(params, next_root, z, &decommit.path_z) {
                return Err("FRI query folded Merkle root mismatch");
            }
            layer_domain /= fold_arity;
            idx_layer = expected_j;
        }
        if layer_domain != 1 || idx_layer != 0 {
            return Err("FRI query does not collapse to final layer");
        }
        let final_z = chain
            .last()
            .and_then(|decommit| Fq::from_canonical_u64(decommit.z))
            .ok_or("FRI query final field element")?;
        if final_z != Fq::zero() {
            return Err("FRI query final value mismatch");
        }
    }
    Ok(base_indices)
}
pub(crate) fn validate_stark_fri_query_shape_for_base_indices_v1(
    params: &StarkFriParamsV1,
    transcript_label: &str,
    roots: &[[u8; 32]],
    queries: &[Vec<FoldDecommitV1>],
    base_indices: &[usize],
) -> Result<Vec<usize>, &'static str> {
    validate_stark_fri_query_shape_for_base_indices_with_limits_v1(
        params,
        transcript_label,
        roots,
        queries,
        base_indices,
        &StarkVerifierLimits::default(),
    )
}
pub(crate) fn validate_stark_fri_query_shape_for_base_indices_with_limits_v1(
    params: &StarkFriParamsV1,
    transcript_label: &str,
    roots: &[[u8; 32]],
    queries: &[Vec<FoldDecommitV1>],
    base_indices: &[usize],
    limits: &StarkVerifierLimits,
) -> Result<Vec<usize>, &'static str> {
    validate_stark_transcript_label(transcript_label, effective_max_transcript_label_len(limits))
        .map_err(|_| "FRI transcript label invalid")?;
    let expected_chain_len = validate_params(params, roots.len(), queries.len(), limits)
        .ok_or("FRI parameter/root/query shape mismatch")?;
    let total_domain = 1_usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or("FRI domain size overflow")?;
    if base_indices.len() != queries.len() {
        return Err("FRI query base index count mismatch");
    }
    if base_indices.iter().any(|&index| index >= total_domain) {
        return Err("FRI query base index exceeds domain");
    }
    let mut seen_indices = BTreeSet::new();
    if base_indices
        .iter()
        .copied()
        .any(|index| !seen_indices.insert(index))
    {
        return Err(STARK_FRI_QUERY_INDEX_REPEATED_ERROR);
    }
    let fold_arity = usize::from(params.fold_arity);
    for (chain, mut idx_layer) in queries.iter().zip(base_indices.iter().copied()) {
        if chain.len() != expected_chain_len {
            return Err("FRI query chain length mismatch");
        }
        let mut layer_domain = total_domain;
        for (round, decommit) in chain.iter().enumerate() {
            if layer_domain < fold_arity {
                return Err("FRI query layer domain underflow");
            }
            let expected_pairs = layer_domain / fold_arity;
            let expected_j = idx_layer / fold_arity;
            if expected_j >= expected_pairs || usize::try_from(decommit.j).ok() != Some(expected_j)
            {
                return Err("FRI query fold index mismatch");
            }
            let depth_current =
                log2_usize(layer_domain).ok_or("FRI query current layer depth mismatch")?;
            let depth_next = log2_usize(layer_domain / fold_arity)
                .ok_or("FRI query next layer depth mismatch")?;
            if !merkle_path_depth_ok(&decommit.path_y0, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_y1, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_z, depth_next, limits)
            {
                return Err("FRI query Merkle path depth mismatch");
            }
            let expected_y0 = expected_j
                .checked_mul(fold_arity)
                .ok_or("FRI query y0 index overflow")?;
            let expected_y1 = expected_y0
                .checked_add(1)
                .ok_or("FRI query y1 index overflow")?;
            if merkle_path_index(&decommit.path_y0) != Some(expected_y0)
                || merkle_path_index(&decommit.path_y1) != Some(expected_y1)
                || merkle_path_index(&decommit.path_z) != Some(expected_j)
            {
                return Err("FRI query Merkle path index mismatch");
            }
            let y0 = Fq::from_canonical_u64(decommit.y0).ok_or("FRI query y0 field element")?;
            let y1 = Fq::from_canonical_u64(decommit.y1).ok_or("FRI query y1 field element")?;
            let z = Fq::from_canonical_u64(decommit.z).ok_or("FRI query z field element")?;
            let current_root = roots.get(round).ok_or("FRI query current root missing")?;
            let next_root = roots.get(round + 1).ok_or("FRI query next root missing")?;
            if !merkle_verify(params, current_root, y0, &decommit.path_y0)
                || !merkle_verify(params, current_root, y1, &decommit.path_y1)
            {
                return Err("FRI query Merkle root mismatch");
            }
            let beta = fri_round_challenge(params, transcript_label, current_root)
                .ok_or("FRI query challenge derivation failed")?;
            let x = domain_x_for_pair(layer_domain, expected_j)
                .ok_or("FRI query domain element derivation failed")?;
            if fri_fold_pair(y0, y1, beta, x) != Some(z) {
                return Err("FRI query fold relation mismatch");
            }
            if !merkle_verify(params, next_root, z, &decommit.path_z) {
                return Err("FRI query folded Merkle root mismatch");
            }
            layer_domain /= fold_arity;
            idx_layer = expected_j;
        }
        if layer_domain != 1 || idx_layer != 0 {
            return Err("FRI query does not collapse to final layer");
        }
        let final_z = chain
            .last()
            .and_then(|decommit| Fq::from_canonical_u64(decommit.z))
            .ok_or("FRI query final field element")?;
        if final_z != Fq::zero() {
            return Err("FRI query final value mismatch");
        }
    }
    Ok(base_indices.to_vec())
}
/// Verify that an AIR opening is the value opened by the first FRI layer.
pub(crate) fn validate_stark_air_opening_first_fri_value_v1(
    opening: &StarkAirOpeningV1,
    base_index: usize,
    first_decommit: &FoldDecommitV1,
) -> Result<(), &'static str> {
    if usize::try_from(opening.index).ok() != Some(base_index) {
        return Err("AIR/FRI opening index mismatch");
    }
    let opened_fri_value = if base_index.is_multiple_of(2) {
        first_decommit.y0
    } else {
        first_decommit.y1
    };
    if opened_fri_value != opening.composition_value {
        return Err("AIR/FRI composition value mismatch");
    }
    Ok(())
}
fn stark_air_trace_width() -> usize {
    usize::from(STARK_BINDING_AIR_TRACE_WIDTH_V1)
}
#[derive(Clone, Copy)]
struct StarkAirExplicitVerificationContext<'a> {
    circuit_id: &'a str,
    public_digest: &'a [u8; 32],
    rows: &'a [Vec<u64>],
    composition_values: &'a [u64],
    base_indices: Option<&'a [usize]>,
}
#[derive(Clone, Copy)]
enum StarkAirVerificationContext<'a> {
    Binding,
    BfvFullBootstrapPublicPadding {
        statement_hash: &'a iroha_crypto::Hash,
        trace_material_digest: &'a iroha_crypto::Hash,
        slot_index: u32,
        bound_mode: iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1,
    },
    Explicit(&'a StarkAirExplicitVerificationContext<'a>),
}
impl StarkAirVerificationContext<'_> {
    fn allows_auxiliary_composition(self) -> bool {
        matches!(self, Self::Binding)
    }
    fn trace_width(self) -> usize {
        match self {
            Self::Binding => stark_air_trace_width(),
            Self::BfvFullBootstrapPublicPadding { .. } => {
                usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1)
            }
            Self::Explicit(explicit) => explicit.rows.first().map(Vec::len).unwrap_or(usize::MAX),
        }
    }
}
fn stark_air_digest_limbs(public_digest: &[u8; 32]) -> [u64; 4] {
    let mut limbs = [0u64; 4];
    for (idx, chunk) in public_digest.chunks_exact(8).enumerate() {
        let mut word = [0u8; 8];
        word.copy_from_slice(chunk);
        limbs[idx] = (u128::from(u64::from_le_bytes(word)) % MOD_P) as u64;
    }
    limbs
}
fn stark_air_row(index: usize, public_digest: &[u8; 32]) -> Option<Vec<u64>> {
    let index = u64::try_from(index).ok()?;
    let index = (u128::from(index) % MOD_P) as u64;
    let limbs = stark_air_digest_limbs(public_digest);
    let width = u64::try_from(stark_air_trace_width()).ok()?;
    Some(vec![index, limbs[0], limbs[1], limbs[2], limbs[3], width])
}
fn stark_air_trace_leaf_hash(params: &StarkFriParamsV1, row: &[u64]) -> Option<[u8; 32]> {
    if row
        .iter()
        .copied()
        .any(|value| Fq::from_canonical_u64(value).is_none())
    {
        return None;
    }
    match params.hash_fn {
        STARK_HASH_SHA256_V1 => {
            let mut h = Sha256::new();
            h.update(b"STARK:AIR:TRACE:ROW:V1");
            h.update(&(row.len() as u64).to_le_bytes());
            for value in row {
                h.update(&value.to_le_bytes());
            }
            Some(h.finalize().into())
        }
        STARK_HASH_POSEIDON2_V1 => Some(u64_to_digest_le(poseidon_domain_hash_u64(
            b"iroha:zk:stark:air:trace-row:v1",
            row,
        ))),
        _ => None,
    }
}
fn stark_air_composition_value(
    index: usize,
    domain_size: usize,
    public_digest: &[u8; 32],
    row: &[u64],
    next_row: &[u64],
) -> Option<Fq> {
    let width = stark_air_trace_width();
    if domain_size == 0 || row.len() != width || next_row.len() != width {
        return None;
    }
    let expected = stark_air_row(index, public_digest)?;
    let expected_next = stark_air_row((index + 1) % domain_size, public_digest)?;
    let mut acc = Fq::zero();
    let mut coeff = Fq::from_canonical_u64(3)?;
    for (actual, expected) in row.iter().zip(expected.iter()) {
        let residue = Fq::from_canonical_u64(*actual)?.sub(Fq::from_canonical_u64(*expected)?);
        acc = acc.add(coeff.mul(residue));
        coeff = coeff.add(Fq::from_canonical_u64(2)?);
    }
    for (actual, expected) in next_row.iter().zip(expected_next.iter()) {
        let residue = Fq::from_canonical_u64(*actual)?.sub(Fq::from_canonical_u64(*expected)?);
        acc = acc.add(coeff.mul(residue));
        coeff = coeff.add(Fq::from_canonical_u64(2)?);
    }
    Some(acc)
}
fn stark_air_composition_value_for_context(
    context: StarkAirVerificationContext<'_>,
    index: usize,
    domain_size: usize,
    public_digest: &[u8; 32],
    row: &[u64],
    next_row: &[u64],
) -> Option<Fq> {
    match context {
        StarkAirVerificationContext::Binding => {
            stark_air_composition_value(index, domain_size, public_digest, row, next_row)
        }
        StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash,
            trace_material_digest,
            slot_index,
            bound_mode,
        } => {
            if !bfv_full_bootstrap_public_padding_inputs_are_admissible(
                *statement_hash,
                *trace_material_digest,
                slot_index,
                bound_mode,
            ) {
                return None;
            }
            if *public_digest != <[u8; iroha_crypto::Hash::LENGTH]>::from(*statement_hash) {
                return None;
            }
            let opening_index = u32::try_from(index).ok()?;
            iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_public_padding_opening_v1(
                opening_index,
                row,
                next_row,
                *statement_hash,
                slot_index,
                bound_mode,
            )
            .ok()?;
            Some(Fq::zero())
        }
        StarkAirVerificationContext::Explicit(explicit) => {
            if domain_size == 0
                || *public_digest != *explicit.public_digest
                || explicit.rows.len() != domain_size
                || explicit.composition_values.len() != domain_size
                || index >= domain_size
            {
                return None;
            }
            if row != explicit.rows.get(index)?
                || next_row != explicit.rows.get((index + 1) % domain_size)?
            {
                return None;
            }
            Fq::from_canonical_u64(*explicit.composition_values.get(index)?)
        }
    }
}
fn stark_air_context_matches_statement(
    params: &StarkFriParamsV1,
    air: &StarkAirProofV1,
    total_domain: usize,
    context: StarkAirVerificationContext<'_>,
) -> bool {
    match context {
        StarkAirVerificationContext::Binding => {
            !stark_air_circuit_id_targets_governance_vote_relation(&air.circuit_id)
                && !stark_air_circuit_id_targets_soracloud_fhe_relation(&air.circuit_id)
        }
        StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash,
            trace_material_digest,
            slot_index,
            bound_mode,
        } => {
            let expected_params = bfv_full_bootstrap_stark_air_params_v1(*statement_hash);
            bfv_full_bootstrap_public_padding_inputs_are_admissible(
                *statement_hash,
                *trace_material_digest,
                slot_index,
                bound_mode,
            ) && bfv_full_bootstrap_stark_air_params_match_v1(params, &expected_params)
                && air.circuit_id == iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1
                && air.public_digest == <[u8; iroha_crypto::Hash::LENGTH]>::from(*statement_hash)
        }
        StarkAirVerificationContext::Explicit(explicit) => {
            if air.circuit_id != explicit.circuit_id
                || air.public_digest != *explicit.public_digest
                || explicit.rows.len() != total_domain
                || explicit.composition_values.len() != total_domain
            {
                return false;
            }
            let Some(trace_width) = explicit.rows.first().map(Vec::len) else {
                return false;
            };
            if trace_width == 0 || trace_width != usize::from(air.trace_width) {
                return false;
            }
            if explicit.rows.iter().any(|row| row.len() != trace_width) {
                return false;
            }
            let Some(trace_root) = stark_air_trace_root_from_rows_v1(params, explicit.rows) else {
                return false;
            };
            if air.trace_root != trace_root {
                return false;
            }
            let Some(composition_root) =
                stark_merkle_root_from_field_values_v1(params, explicit.composition_values)
            else {
                return false;
            };
            air.composition_root == composition_root
        }
    }
}
fn bfv_full_bootstrap_statement_hash_is_admissible(
    statement_hash: iroha_crypto::Hash,
    slot_index: u32,
    bound_mode: iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1,
) -> bool {
    iroha_crypto::bfv_full_bootstrap_arithmetic_trace_public_padding_row_v1(
        u32::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_COUNT_V1),
        statement_hash,
        slot_index,
        bound_mode,
    )
    .is_ok()
}
fn bfv_full_bootstrap_public_padding_inputs_are_admissible(
    statement_hash: iroha_crypto::Hash,
    trace_material_digest: iroha_crypto::Hash,
    slot_index: u32,
    bound_mode: iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1,
) -> bool {
    bfv_full_bootstrap_statement_hash_is_admissible(statement_hash, slot_index, bound_mode)
        && iroha_crypto::bfv_full_bootstrap_arithmetic_trace_canonical_opening_indices_from_transcript_v1(
            statement_hash,
            trace_material_digest,
        )
        .is_ok()
}
fn bfv_full_bootstrap_expected_base_indices_v1(
    statement_hash: iroha_crypto::Hash,
    trace_material_digest: iroha_crypto::Hash,
    query_count: usize,
    total_domain: usize,
) -> Result<Vec<usize>, &'static str> {
    let indices =
        iroha_crypto::bfv_full_bootstrap_arithmetic_trace_canonical_opening_indices_from_transcript_v1(
            statement_hash,
            trace_material_digest,
        )
        .map_err(|_| "BFV full-bootstrap opening transcript derivation failed")?;
    if indices.len() != query_count {
        return Err("BFV full-bootstrap opening schedule count mismatch");
    }
    let mut seen_indices = BTreeSet::new();
    indices
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index)
                .map_err(|_| "BFV full-bootstrap opening index exceeds usize")?;
            if index >= total_domain {
                return Err("BFV full-bootstrap opening index exceeds STARK domain");
            }
            if !seen_indices.insert(index) {
                return Err(STARK_FRI_QUERY_INDEX_REPEATED_ERROR);
            }
            Ok(index)
        })
        .collect()
}
fn stark_air_query_roots(roots: &[[u8; 32]], air: Option<&StarkAirProofV1>) -> Vec<[u8; 32]> {
    let mut query_roots = roots.to_vec();
    if let Some(air) = air {
        query_roots.push(air.trace_root);
        query_roots.push(air.composition_root);
        query_roots.push(air.public_digest);
    }
    query_roots
}
fn bfv_full_bootstrap_stark_air_params_v1(statement_hash: iroha_crypto::Hash) -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_N_LOG2_V1,
        blowup_log2: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_BLOWUP_LOG2_V1,
        fold_arity: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_FOLD_ARITY_V1,
        queries: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1,
        merkle_arity: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_MERKLE_ARITY_V1,
        hash_fn: iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_HASH_SHA256_V1,
        domain_tag: iroha_crypto::bfv_full_bootstrap_native_stark_air_domain_tag_v1(statement_hash),
    }
}
fn bfv_full_bootstrap_stark_air_params_match_v1(
    actual: &StarkFriParamsV1,
    expected: &StarkFriParamsV1,
) -> bool {
    actual.version == expected.version
        && actual.n_log2 == expected.n_log2
        && actual.blowup_log2 == expected.blowup_log2
        && actual.fold_arity == expected.fold_arity
        && actual.queries == expected.queries
        && actual.merkle_arity == expected.merkle_arity
        && actual.hash_fn == expected.hash_fn
        && actual.domain_tag == expected.domain_tag
}
fn bfv_full_bootstrap_stark_air_transcript_label_v1(attempt: u32) -> String {
    if attempt == 0 {
        return iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1.to_owned();
    }
    format!(
        "{}:{attempt}",
        iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
    )
}
fn bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(label: &str) -> bool {
    label == iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1
}
fn validate_stark_prover_params(
    params: &StarkFriParamsV1,
    transcript_label: &str,
) -> Result<usize, String> {
    if params.version != 1 {
        return Err("unsupported STARK params version".to_owned());
    }
    if params.n_log2 == 0 || params.n_log2 > MAX_DOMAIN_LOG2 {
        return Err("unsupported STARK domain size".to_owned());
    }
    if params.blowup_log2 == 0
        || params.blowup_log2 > MAX_DOMAIN_LOG2
        || params.blowup_log2 > params.n_log2
    {
        return Err("unsupported STARK blowup factor".to_owned());
    }
    if params.fold_arity != 2 {
        return Err("unsupported STARK fold_arity (expected 2)".to_owned());
    }
    if params.merkle_arity != 2 {
        return Err("unsupported STARK merkle_arity (expected 2)".to_owned());
    }
    if params.hash_fn != STARK_HASH_SHA256_V1 && params.hash_fn != STARK_HASH_POSEIDON2_V1 {
        return Err("unsupported STARK hash_fn".to_owned());
    }
    validate_stark_domain_tag(&params.domain_tag, MAX_DOMAIN_TAG_LEN)
        .map_err(|err| format!("invalid STARK domain tag: {err}"))?;
    let query_count = params.queries as usize;
    if query_count == 0 || query_count > MAX_FRI_QUERIES {
        return Err("invalid STARK query count".to_owned());
    }
    validate_stark_transcript_label(transcript_label, MAX_TRANSCRIPT_LABEL_LEN)
        .map_err(str::to_owned)?;
    let n_log2 = params.n_log2 as usize;
    if n_log2 > MAX_MERKLE_DEPTH {
        return Err("STARK domain depth exceeds verifier limits".to_owned());
    }
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    if query_count > domain {
        return Err("STARK query count exceeds domain size".to_owned());
    }
    layers_required(params).ok_or_else(|| "invalid STARK folding parameters".to_owned())
}
fn fri_round_challenge(
    params: &StarkFriParamsV1,
    transcript_label: &str,
    root: &[u8; 32],
) -> Option<Fq> {
    let mut tb = Vec::new();
    tb.extend_from_slice(transcript_label.as_bytes());
    tb.extend_from_slice(&params.version.to_le_bytes());
    tb.extend_from_slice(&[
        params.n_log2,
        params.blowup_log2,
        params.fold_arity,
        params.merkle_arity,
        params.hash_fn,
    ]);
    tb.extend_from_slice(&params.queries.to_le_bytes());
    tb.extend_from_slice(&(params.domain_tag.len() as u32).to_le_bytes());
    tb.extend_from_slice(params.domain_tag.as_bytes());
    tb.extend_from_slice(root);
    challenge(params, "stark:fri:r:k", &tb)
}
#[cfg(test)]
fn synthesize_stark_fri_envelope_from_values(
    params: StarkFriParamsV1,
    transcript_label: String,
    base_values: Vec<Fq>,
    extra_query_roots: &[[u8; 32]],
) -> Result<StarkVerifyEnvelopeV1, String> {
    synthesize_stark_fri_envelope_from_values_with_base_indices(
        params,
        transcript_label,
        base_values,
        extra_query_roots,
        None,
    )
}
fn synthesize_stark_fri_envelope_from_values_with_base_indices(
    params: StarkFriParamsV1,
    transcript_label: String,
    base_values: Vec<Fq>,
    extra_query_roots: &[[u8; 32]],
    base_indices: Option<&[usize]>,
) -> Result<StarkVerifyEnvelopeV1, String> {
    let required_layers = validate_stark_prover_params(&params, &transcript_label)?;
    let total_domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    if base_values.len() != total_domain {
        return Err("STARK base evaluations do not match domain size".to_owned());
    }
    let fold_arity = params.fold_arity as usize;
    let mut layer_values = Vec::with_capacity(required_layers + 1);
    let mut layer_merkle = Vec::with_capacity(required_layers + 1);
    let mut roots = Vec::with_capacity(required_layers + 1);
    layer_values.push(base_values);
    for round in 0..required_layers {
        let current = layer_values
            .get(round)
            .ok_or_else(|| "missing STARK FRI layer".to_owned())?;
        let levels = merkle_levels_from_values(&params, current)
            .ok_or_else(|| "failed to build STARK FRI Merkle layer".to_owned())?;
        let root = merkle_root_from_levels(&levels)
            .ok_or_else(|| "failed to derive STARK FRI root".to_owned())?;
        let beta = fri_round_challenge(&params, &transcript_label, &root)
            .ok_or_else(|| "failed to derive STARK FRI challenge".to_owned())?;
        roots.push(root);
        layer_merkle.push(levels);
        let mut next = Vec::with_capacity(current.len() / fold_arity);
        for (pair_index, pair) in current.chunks_exact(fold_arity).enumerate() {
            let x = domain_x_for_pair(current.len(), pair_index)
                .ok_or_else(|| "failed to derive STARK FRI domain element".to_owned())?;
            let folded = fri_fold_pair(pair[0], pair[1], beta, x)
                .ok_or_else(|| "failed to fold STARK FRI pair".to_owned())?;
            next.push(folded);
        }
        layer_values.push(next);
    }
    let final_values = layer_values
        .last()
        .ok_or_else(|| "missing STARK final FRI layer".to_owned())?;
    if final_values.len() != 1 || final_values.first().copied() != Some(Fq::zero()) {
        return Err("STARK final FRI value must be zero".to_owned());
    }
    let final_levels = merkle_levels_from_values(&params, final_values)
        .ok_or_else(|| "failed to build STARK final FRI Merkle layer".to_owned())?;
    let final_root = merkle_root_from_levels(&final_levels)
        .ok_or_else(|| "failed to derive STARK final FRI root".to_owned())?;
    roots.push(final_root);
    layer_merkle.push(final_levels);
    let mut query_roots = roots.clone();
    query_roots.extend_from_slice(extra_query_roots);
    let query_count = params.queries as usize;
    let query_indices = if let Some(base_indices) = base_indices {
        if base_indices.len() != query_count {
            return Err("STARK explicit query schedule count mismatch".to_owned());
        }
        if base_indices.iter().any(|&index| index >= total_domain) {
            return Err("STARK explicit query schedule index exceeds domain".to_owned());
        }
        let mut seen_indices = BTreeSet::new();
        if base_indices
            .iter()
            .copied()
            .any(|index| !seen_indices.insert(index))
        {
            return Err(STARK_FRI_QUERY_INDEX_REPEATED_ERROR.to_owned());
        }
        base_indices.to_vec()
    } else {
        derive_query_indices_without_replacement(
            &transcript_label,
            &params,
            &query_roots,
            query_count,
            total_domain,
        )
        .map_err(|err| format!("failed to derive STARK query schedule: {err}"))?
    };
    let mut queries = Vec::with_capacity(query_count);
    for mut idx_layer in query_indices {
        let mut chain = Vec::with_capacity(required_layers);
        for k in 0..required_layers {
            let j = idx_layer / 2;
            let y0_idx = j
                .checked_mul(2)
                .ok_or_else(|| "query index overflow".to_owned())?;
            let y1_idx = y0_idx
                .checked_add(1)
                .ok_or_else(|| "query index overflow".to_owned())?;
            let path_y0 = merkle_path_from_levels(y0_idx, &layer_merkle[k])
                .ok_or_else(|| "failed to build y0 path".to_owned())?;
            let path_y1 = merkle_path_from_levels(y1_idx, &layer_merkle[k])
                .ok_or_else(|| "failed to build y1 path".to_owned())?;
            let path_z = merkle_path_from_levels(j, &layer_merkle[k + 1])
                .ok_or_else(|| "failed to build z path".to_owned())?;
            let j_u32 = u32::try_from(j).map_err(|_| "query index does not fit u32".to_owned())?;
            let y0 = layer_values
                .get(k)
                .and_then(|values| values.get(y0_idx))
                .copied()
                .ok_or_else(|| "query y0 is out of range".to_owned())?;
            let y1 = layer_values
                .get(k)
                .and_then(|values| values.get(y1_idx))
                .copied()
                .ok_or_else(|| "query y1 is out of range".to_owned())?;
            let z = layer_values
                .get(k + 1)
                .and_then(|values| values.get(j))
                .copied()
                .ok_or_else(|| "query z is out of range".to_owned())?;
            chain.push(FoldDecommitV1 {
                j: j_u32,
                y0: y0.0,
                y1: y1.0,
                path_y0,
                path_y1,
                z: z.0,
                path_z,
            });
            idx_layer = j;
        }
        if idx_layer != 0 {
            return Err("final query index must collapse to zero".to_owned());
        }
        queries.push(chain);
    }
    let envelope = StarkVerifyEnvelopeV1 {
        params,
        proof: StarkProofV1 {
            version: 1,
            commits: StarkCommitmentsV1 {
                version: 1,
                roots,
                comp_root: None,
            },
            queries,
            comp_values: None,
            air: None,
        },
        transcript_label,
    };
    Ok(envelope)
}
/// Build a deterministic STARK FRI proof envelope that passes native verification.
///
/// The generated witness uses all-zero layer evaluations and deterministic Merkle openings for
/// transcript-derived query indices. This keeps proving deterministic and avoids trusted setup.
///
/// Returns Norito-encoded [`StarkVerifyEnvelopeV1`] bytes.
#[cfg(test)]
fn synthesize_stark_fri_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
) -> Result<Vec<u8>, String> {
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    let values = vec![Fq::zero(); domain];
    let envelope =
        synthesize_stark_fri_envelope_from_values(params, transcript_label, values, &[])?;
    norito::to_bytes(&envelope).map_err(|err| format!("failed to encode STARK envelope: {err}"))
}
fn validate_stark_composition_terms(
    constant: u64,
    z_coeff: u64,
    aux_terms: &[StarkCompositionTermV1],
) -> Result<(), String> {
    if aux_terms.len() > MAX_AUX_TERMS {
        return Err("too many STARK composition auxiliary terms".to_owned());
    }
    if Fq::from_canonical_u64(constant).is_none() {
        return Err("invalid STARK constant".to_owned());
    }
    if Fq::from_canonical_u64(z_coeff).is_none() {
        return Err("invalid STARK z coefficient".to_owned());
    }
    let mut last_wire = None;
    for term in aux_terms {
        if Fq::from_canonical_u64(term.value).is_none()
            || Fq::from_canonical_u64(term.coeff).is_none()
        {
            return Err("invalid STARK composition auxiliary field element".to_owned());
        }
        if let Some(prev) = last_wire
            && term.wire_index <= prev
        {
            return Err("STARK composition auxiliary wires must be strictly ordered".to_owned());
        }
        last_wire = Some(term.wire_index);
    }
    Ok(())
}
/// Build the V1 public AIR statement digest for composition terms.
pub fn stark_air_public_digest_from_composition(
    constant: u64,
    z_coeff: u64,
    aux_terms: &[StarkCompositionTermV1],
) -> Result<[u8; 32], String> {
    validate_stark_composition_terms(constant, z_coeff, aux_terms)?;
    let mut h = Sha256::new();
    h.update(b"iroha:zk:stark:air-public-digest:v1");
    h.update(&constant.to_le_bytes());
    h.update(&z_coeff.to_le_bytes());
    h.update(&(aux_terms.len() as u64).to_le_bytes());
    for term in aux_terms {
        h.update(&term.wire_index.to_le_bytes());
        h.update(&term.value.to_le_bytes());
        h.update(&term.coeff.to_le_bytes());
    }
    Ok(h.finalize().into())
}
/// Build a V1 STARK/FRI AIR envelope from caller-validated rows and composition values.
///
/// Domain-specific AIR callers remain responsible for proving that the supplied rows and
/// composition evaluations are the result of their arithmetic constraint system. This helper commits
/// those already-evaluated vectors and emits transcript-derived AIR openings bound to the same FRI
/// query roots replayed by the verifier.
#[cfg(test)]
pub(crate) fn prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<u64>,
) -> Result<Vec<u8>, String> {
    validate_generic_stark_air_circuit_id(&circuit_id)?;
    prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values,
        None,
    )
}
#[cfg(test)]
pub(crate) fn prove_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<u64>,
    base_indices: &[usize],
) -> Result<Vec<u8>, String> {
    validate_generic_stark_air_circuit_id(&circuit_id)?;
    prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values,
        Some(base_indices),
    )
}
/// Build a reserved-circuit AIR envelope with an explicit query schedule.
pub(crate) fn prove_stark_fri_reserved_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<u64>,
    base_indices: &[usize],
) -> Result<Vec<u8>, String> {
    validate_stark_circuit_id(&circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
    prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values,
        Some(base_indices),
    )
}
fn prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes_for_validated_circuit(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<u64>,
    base_indices: Option<&[usize]>,
) -> Result<Vec<u8>, String> {
    let composition_values = composition_values
        .into_iter()
        .map(|value| {
            Fq::from_canonical_u64(value).ok_or_else(|| {
                "STARK AIR composition contains non-canonical field element".to_owned()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    prove_stark_fri_air_envelope_from_rows_and_composition_values_fq_bytes(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values,
        base_indices,
    )
}
fn prove_stark_fri_air_envelope_from_rows_and_composition_values_fq_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<Fq>,
    base_indices: Option<&[usize]>,
) -> Result<Vec<u8>, String> {
    validate_stark_circuit_id(&circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
    validate_stark_prover_params(&params, &transcript_label)?;
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    if rows.len() != domain {
        return Err("STARK AIR row count does not match domain size".to_owned());
    }
    if composition_values.len() != domain {
        return Err("STARK AIR composition count does not match domain size".to_owned());
    }
    let trace_width = rows
        .first()
        .map(Vec::len)
        .ok_or_else(|| "STARK AIR rows must not be empty".to_owned())?;
    if trace_width == 0 || trace_width > MAX_AIR_WIDTH {
        return Err("invalid STARK AIR trace width".to_owned());
    }
    let trace_width = u16::try_from(trace_width)
        .map_err(|_| "STARK AIR trace width does not fit u16".to_owned())?;
    for row in &rows {
        if row.len() != usize::from(trace_width) {
            return Err("STARK AIR rows must have uniform width".to_owned());
        }
        if row
            .iter()
            .copied()
            .any(|value| Fq::from_canonical_u64(value).is_none())
        {
            return Err("STARK AIR row contains non-canonical field element".to_owned());
        }
    }
    let trace_leaves = rows
        .iter()
        .map(|row| {
            stark_air_trace_leaf_hash(&params, row)
                .ok_or_else(|| "failed to hash STARK AIR row".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let trace_levels = merkle_levels_from_hashes(&params, trace_leaves)
        .ok_or_else(|| "failed to build STARK AIR trace commitment".to_owned())?;
    let trace_root = merkle_root_from_levels(&trace_levels)
        .ok_or_else(|| "failed to derive STARK AIR trace root".to_owned())?;
    let composition_levels = merkle_levels_from_values(&params, &composition_values)
        .ok_or_else(|| "failed to build STARK AIR composition commitment".to_owned())?;
    let composition_root = merkle_root_from_levels(&composition_levels)
        .ok_or_else(|| "failed to derive STARK AIR composition root".to_owned())?;
    let extra_query_roots = [trace_root, composition_root, public_digest];
    let mut envelope = synthesize_stark_fri_envelope_from_values_with_base_indices(
        params,
        transcript_label,
        composition_values.clone(),
        &extra_query_roots,
        base_indices,
    )?;
    if envelope.proof.commits.roots.first().copied() != Some(composition_root) {
        return Err("STARK AIR composition root does not match FRI base root".to_owned());
    }
    let query_indices = if let Some(base_indices) = base_indices {
        validate_stark_fri_query_shape_for_base_indices_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &envelope.proof.queries,
            base_indices,
        )
    } else {
        validate_stark_fri_query_shape_and_indices_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
        )
    }
    .map_err(|err| format!("STARK AIR FRI query shape failed validation: {err}"))?;
    let mut openings = Vec::with_capacity(envelope.proof.queries.len());
    for index in query_indices {
        let next_index = (index + 1) % domain;
        let row_path = merkle_path_from_levels(index, &trace_levels)
            .ok_or_else(|| "failed to open STARK AIR row".to_owned())?;
        let next_row_path = merkle_path_from_levels(next_index, &trace_levels)
            .ok_or_else(|| "failed to open next STARK AIR row".to_owned())?;
        let composition_path = merkle_path_from_levels(index, &composition_levels)
            .ok_or_else(|| "failed to open STARK AIR composition".to_owned())?;
        let composition_value = composition_values
            .get(index)
            .copied()
            .ok_or_else(|| "STARK AIR composition index is out of range".to_owned())?;
        openings.push(StarkAirOpeningV1 {
            index: u32::try_from(index)
                .map_err(|_| "STARK AIR query index does not fit u32".to_owned())?,
            row: rows[index].clone(),
            next_row: rows[next_index].clone(),
            row_path,
            next_row_path,
            composition_value: composition_value.0,
            composition_path,
        });
    }
    let composition_values_u64 = composition_values
        .iter()
        .map(|value| value.0)
        .collect::<Vec<_>>();
    envelope.proof.air = Some(StarkAirProofV1 {
        version: 1,
        circuit_id: circuit_id.clone(),
        public_digest,
        trace_root,
        composition_root,
        trace_width,
        openings,
    });
    let bytes = ivm::codec::encode_canonical_norito(&envelope)
        .map_err(|err| format!("failed to encode STARK envelope: {err}"))?;
    let mut limits = StarkVerifierLimits::default();
    limits.max_envelope_bytes = usize::MAX;
    let self_verified = if let Some(base_indices) = base_indices {
        verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
            &bytes,
            &limits,
            &circuit_id,
            &public_digest,
            &rows,
            &composition_values_u64,
            base_indices,
        )
    } else {
        verify_stark_fri_air_envelope_from_rows_and_composition_values_with_limits(
            &bytes,
            &limits,
            &circuit_id,
            &public_digest,
            &rows,
            &composition_values_u64,
        )
    };
    if !self_verified {
        return Err("STARK AIR envelope self-verification failed".to_owned());
    }
    Ok(bytes)
}
/// Build a deterministic V1 STARK/FRI envelope with an explicit verifier-owned AIR section.
pub fn prove_stark_fri_air_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
) -> Result<Vec<u8>, String> {
    validate_generic_stark_air_circuit_id(&circuit_id)?;
    prove_stark_fri_air_envelope_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
    )
}
/// Build an AIR envelope for crate-owned reserved circuits after caller-side family checks.
pub(crate) fn prove_stark_fri_reserved_air_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
) -> Result<Vec<u8>, String> {
    validate_stark_circuit_id(&circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
    prove_stark_fri_air_envelope_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
    )
}
fn prove_stark_fri_air_envelope_bytes_for_validated_circuit(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
) -> Result<Vec<u8>, String> {
    validate_stark_prover_params(&params, &transcript_label)?;
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    let rows = (0..domain)
        .map(|index| {
            stark_air_row(index, &public_digest)
                .ok_or_else(|| "failed to build STARK AIR row".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let composition_values = (0..domain)
        .map(|index| {
            stark_air_composition_value(
                index,
                domain,
                &public_digest,
                &rows[index],
                &rows[(index + 1) % domain],
            )
            .ok_or_else(|| "failed to evaluate STARK AIR composition".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes_for_validated_circuit(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values
            .into_iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        None,
    )
}
/// Build a V1 STARK/FRI AIR envelope from caller-validated rows and zero composition values.
///
/// This helper only constructs commitments and transcript-derived openings. Domain-specific AIR
/// callers remain responsible for proving that the supplied rows satisfy their arithmetic
/// constraints; this function records the already-zero composition vector used by those constraints.
pub fn prove_stark_fri_zero_composition_air_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
) -> Result<Vec<u8>, String> {
    validate_generic_stark_air_circuit_id(&circuit_id)?;
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    let composition_values = vec![Fq::zero(); domain];
    prove_stark_fri_air_envelope_from_rows_and_composition_values_fq_bytes(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values,
        None,
    )
}
/// Build a canonical BFV full-bootstrap native STARK/FRI AIR proof envelope for Core internals.
///
/// The proof is synthesized from validated
/// [`iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1`]. The
/// wrapper binds the native STARK domain tag to the BFV execution statement
/// hash and uses the crypto-derived explicit public-padding opening schedule.
/// BFV execution AIR proofs are emitted under the canonical base transcript
/// label only, so equivalent suffixed-label proof bytes cannot become
/// alternate encodings of the same governed statement. Public native proof
/// generation must use the Soracloud release/audit-aware entry points, which
/// validate caller-owned governed artifacts before reaching this crate-scoped
/// helper.
///
/// # Errors
///
/// Returns an error when the BFV prover input material is invalid, the STARK
/// envelope cannot be built, or no deterministic transcript label yields the
/// canonical duplicate-free public opening set.
pub(crate) fn prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(
    material: &iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1,
) -> Result<Vec<u8>, String> {
    iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(material)
        .map_err(|err| format!("BFV full-bootstrap prover input material invalid: {err}"))?;
    let statement_hash = material.proof_input_material.statement_hash;
    let public_digest: [u8; 32] = statement_hash.into();
    let params = bfv_full_bootstrap_stark_air_params_v1(statement_hash);
    let rows = material.arithmetic_trace_material.rows.clone();
    let composition_values = material
        .arithmetic_air_evaluation_material
        .composition_values
        .clone();
    let expected_base_indices = bfv_full_bootstrap_expected_base_indices_v1(
        statement_hash,
        material.arithmetic_trace_material_digest,
        usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1),
        1_usize
            .checked_shl(u32::from(params.n_log2))
            .ok_or_else(|| "BFV full-bootstrap STARK domain size overflow".to_owned())?,
    )
    .map_err(|err| format!("BFV full-bootstrap opening schedule invalid: {err}"))?;
    let bytes =
        prove_stark_fri_reserved_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
            params,
            bfv_full_bootstrap_stark_air_transcript_label_v1(0),
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1.to_owned(),
            public_digest,
            rows,
            composition_values,
            &expected_base_indices,
        )?;
    let mut limits = StarkVerifierLimits::default();
    limits.max_envelope_bytes = usize::MAX;
    if verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(&bytes, &limits, material) {
        Ok(bytes)
    } else {
        Err(
            "BFV full-bootstrap STARK/FRI envelope failed transcript-bound self-verification"
                .to_owned(),
        )
    }
}
/// Verify a canonical BFV full-bootstrap native STARK/FRI AIR proof envelope.
///
/// This is the default-limit companion to
/// [`verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits`].
#[must_use]
pub fn verify_stark_fri_bfv_full_bootstrap_air_envelope(
    bytes: &[u8],
    material: &iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1,
) -> bool {
    verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
        bytes,
        &StarkVerifierLimits::default(),
        material,
    )
}
/// Verify a BFV full-bootstrap native STARK/FRI AIR proof from public padding data.
///
/// This verifier-facing check does not require the private row-major trace. It
/// validates the STARK/FRI envelope, the statement-bound BFV domain tag, the
/// canonical BFV circuit/profile, duplicate-free public padding openings, and
/// zero public-padding composition samples. Callers that hold governed trace
/// material should still use [`verify_stark_fri_bfv_full_bootstrap_air_envelope`]
/// for the stronger artifact-bound replay.
#[must_use]
pub fn verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
    bytes: &[u8],
    statement_hash: iroha_crypto::Hash,
    trace_material_digest: iroha_crypto::Hash,
    slot_index: u32,
    bound_mode: iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1,
) -> bool {
    verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
        bytes,
        &StarkVerifierLimits::default(),
        statement_hash,
        trace_material_digest,
        slot_index,
        bound_mode,
    )
}
/// Verify a BFV full-bootstrap native STARK/FRI AIR proof from public padding data with limits.
#[must_use]
pub fn verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    statement_hash: iroha_crypto::Hash,
    trace_material_digest: iroha_crypto::Hash,
    slot_index: u32,
    bound_mode: iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1,
) -> bool {
    if !bfv_full_bootstrap_public_padding_inputs_are_admissible(
        statement_hash,
        trace_material_digest,
        slot_index,
        bound_mode,
    ) {
        return false;
    }
    if !verify_stark_fri_envelope_with_context(
        bytes,
        limits,
        StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &statement_hash,
            trace_material_digest: &trace_material_digest,
            slot_index,
            bound_mode,
        },
    ) {
        return false;
    }
    let env: StarkVerifyEnvelopeV1 = match norito::decode_from_bytes(bytes) {
        Ok(env) => env,
        Err(_) => return false,
    };
    if !bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(&env.transcript_label) {
        return false;
    }
    let expected_params = bfv_full_bootstrap_stark_air_params_v1(statement_hash);
    if !bfv_full_bootstrap_stark_air_params_match_v1(&env.params, &expected_params) {
        return false;
    }
    let public_digest: [u8; 32] = statement_hash.into();
    let Some(air) = env.proof.air.as_ref() else {
        return false;
    };
    if air.circuit_id != iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1
        || air.public_digest != public_digest
        || usize::from(air.trace_width)
            != usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1)
        || air.openings.len()
            != usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1)
    {
        return false;
    }
    let opening_indices = air
        .openings
        .iter()
        .map(|opening| opening.index)
        .collect::<Vec<_>>();
    let opened_rows = air
        .openings
        .iter()
        .map(|opening| opening.row.clone())
        .collect::<Vec<_>>();
    let opened_next_rows = air
        .openings
        .iter()
        .map(|opening| opening.next_row.clone())
        .collect::<Vec<_>>();
    if iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_transcript_public_padding_openings_v1(
        &opening_indices,
        &opened_rows,
        &opened_next_rows,
        statement_hash,
        trace_material_digest,
        slot_index,
        bound_mode,
    )
    .is_err()
    {
        return false;
    }
    air.openings
        .iter()
        .all(|opening| opening.composition_value == 0)
}
fn bfv_full_bootstrap_air_openings_match_public_opening_material_v1(
    air: &StarkAirProofV1,
    material: &iroha_crypto::BfvFullBootstrapArithmeticTracePublicOpeningMaterialV1,
) -> bool {
    if air.openings.len() != material.opening_indices.len()
        || air.openings.len() != material.opened_rows.len()
        || air.openings.len() != material.opened_next_rows.len()
    {
        return false;
    }
    air.openings.iter().enumerate().all(|(position, opening)| {
        material.opening_indices.get(position) == Some(&opening.index)
            && material.opened_rows.get(position).map(Vec::as_slice) == Some(opening.row.as_slice())
            && material.opened_next_rows.get(position).map(Vec::as_slice)
                == Some(opening.next_row.as_slice())
    })
}
/// Verify a BFV full-bootstrap native STARK/FRI AIR proof envelope with limits.
///
/// Generic STARK verification is only the first stage. This BFV wrapper also
/// requires the exact first-release BFV STARK parameters, the statement-bound
/// domain tag, the canonical BFV circuit id, the statement hash as the public
/// digest, the canonical opening count, sampled public-padding rows that match
/// the BFV statement header, and the typed public-opening material carried by
/// the governed execution prover-input package.
#[must_use]
pub fn verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    material: &iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1,
) -> bool {
    if iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(material)
        .is_err()
    {
        return false;
    }
    let statement_hash = material.proof_input_material.statement_hash;
    let public_digest: [u8; 32] = statement_hash.into();
    let witness = &material.proof_input_material.witness_material;
    if !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope_with_limits(
        bytes,
        limits,
        statement_hash,
        material.arithmetic_trace_material_digest,
        witness.slot_index,
        witness.bound_mode,
    ) {
        return false;
    }
    let expected_base_indices = match bfv_full_bootstrap_expected_base_indices_v1(
        statement_hash,
        material.arithmetic_trace_material_digest,
        usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1),
        material.arithmetic_trace_material.rows.len(),
    ) {
        Ok(indices) => indices,
        Err(_) => return false,
    };
    if !verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
        bytes,
        limits,
        iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
        &public_digest,
        &material.arithmetic_trace_material.rows,
        &material
            .arithmetic_air_evaluation_material
            .composition_values,
        &expected_base_indices,
    ) {
        return false;
    }
    let env: StarkVerifyEnvelopeV1 = match norito::decode_from_bytes(bytes) {
        Ok(env) => env,
        Err(_) => return false,
    };
    if !bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(&env.transcript_label) {
        return false;
    }
    let expected_params = bfv_full_bootstrap_stark_air_params_v1(statement_hash);
    if !bfv_full_bootstrap_stark_air_params_match_v1(&env.params, &expected_params) {
        return false;
    }
    let Some(air) = env.proof.air.as_ref() else {
        return false;
    };
    if air.circuit_id != iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1
        || air.public_digest != public_digest
        || usize::from(air.trace_width)
            != usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_ROW_WIDTH_V1)
        || air.openings.len()
            != usize::from(iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1)
    {
        return false;
    }
    if !bfv_full_bootstrap_air_openings_match_public_opening_material_v1(
        air,
        &material.public_opening_material,
    ) {
        return false;
    }
    let opening_indices = air
        .openings
        .iter()
        .map(|opening| opening.index)
        .collect::<Vec<_>>();
    let opened_rows = air
        .openings
        .iter()
        .map(|opening| opening.row.clone())
        .collect::<Vec<_>>();
    let opened_next_rows = air
        .openings
        .iter()
        .map(|opening| opening.next_row.clone())
        .collect::<Vec<_>>();
    iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_transcript_public_padding_openings_v1(
        &opening_indices,
        &opened_rows,
        &opened_next_rows,
        statement_hash,
        material.arithmetic_trace_material_digest,
        witness.slot_index,
        witness.bound_mode,
    )
    .is_ok()
}
/// Build a deterministic V1 STARK/FRI envelope with verifier-owned composition terms.
pub fn prove_stark_fri_composition_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    constant: u64,
    z_coeff: u64,
    aux_terms: Vec<StarkCompositionTermV1>,
) -> Result<Vec<u8>, String> {
    validate_stark_composition_terms(constant, z_coeff, &aux_terms)?;
    let constant_f =
        Fq::from_canonical_u64(constant).ok_or_else(|| "invalid STARK constant".to_owned())?;
    let z_coeff_f =
        Fq::from_canonical_u64(z_coeff).ok_or_else(|| "invalid STARK z coefficient".to_owned())?;
    let public_digest = stark_air_public_digest_from_composition(constant, z_coeff, &aux_terms)?;
    let bytes = prove_stark_fri_air_envelope_bytes(
        params,
        transcript_label,
        "composition-v1".to_owned(),
        public_digest,
    )?;
    let mut envelope: StarkVerifyEnvelopeV1 = norito::decode_from_bytes(&bytes)
        .map_err(|err| format!("failed to decode STARK AIR envelope: {err}"))?;
    let z_final = Fq::from_canonical_u64(
        envelope
            .proof
            .queries
            .first()
            .and_then(|chain| chain.last())
            .map(|decommit| decommit.z)
            .unwrap_or(0),
    )
    .ok_or_else(|| "invalid STARK final folded value".to_owned())?;
    let mut expected = constant_f.add(z_coeff_f.mul(z_final));
    for term in &aux_terms {
        let coeff = Fq::from_canonical_u64(term.coeff)
            .ok_or_else(|| "invalid STARK composition coefficient".to_owned())?;
        let value = Fq::from_canonical_u64(term.value)
            .ok_or_else(|| "invalid STARK composition value".to_owned())?;
        expected = expected.add(coeff.mul(value));
    }
    let comp_levels = merkle_levels_from_values(&envelope.params, &[expected])
        .ok_or_else(|| "failed to build STARK composition commitment".to_owned())?;
    let comp_root = merkle_root_from_levels(&comp_levels)
        .ok_or_else(|| "failed to derive STARK composition root".to_owned())?;
    let comp_path = merkle_path_from_levels(0, &comp_levels)
        .ok_or_else(|| "failed to derive STARK composition path".to_owned())?;
    let comp_value = StarkCompositionValueV1 {
        leaf: expected.0,
        constant,
        z_coeff,
        aux_terms,
        path: comp_path,
    };
    let comp_values = vec![comp_value; envelope.proof.queries.len()];
    envelope.proof.commits.comp_root = Some(comp_root);
    envelope.proof.comp_values = Some(comp_values);
    ivm::codec::encode_canonical_norito(&envelope)
        .map_err(|err| format!("failed to encode STARK envelope: {err}"))
}
fn verify_stark_air_opening(
    params: &StarkFriParamsV1,
    air: &StarkAirProofV1,
    opening: &StarkAirOpeningV1,
    base_index: usize,
    total_domain: usize,
    first_decommit: &FoldDecommitV1,
    limits: &StarkVerifierLimits,
    context: StarkAirVerificationContext<'_>,
) -> bool {
    if usize::try_from(opening.index).ok() != Some(base_index)
        || opening.row.len() != air.trace_width as usize
        || opening.next_row.len() != air.trace_width as usize
        || opening.row.len() > effective_max_air_width(limits)
        || opening.next_row.len() > effective_max_air_width(limits)
    {
        return false;
    }
    let depth = match log2_usize(total_domain) {
        Some(value) => value,
        None => return false,
    };
    if !merkle_path_depth_ok(&opening.row_path, depth, limits)
        || !merkle_path_depth_ok(&opening.next_row_path, depth, limits)
        || !merkle_path_depth_ok(&opening.composition_path, depth, limits)
    {
        return false;
    }
    let next_index = (base_index + 1) % total_domain;
    if merkle_path_index(&opening.row_path) != Some(base_index)
        || merkle_path_index(&opening.next_row_path) != Some(next_index)
        || merkle_path_index(&opening.composition_path) != Some(base_index)
    {
        return false;
    }
    let row_leaf = match stark_air_trace_leaf_hash(params, &opening.row) {
        Some(value) => value,
        None => return false,
    };
    if !merkle_verify_hash(params, &air.trace_root, &row_leaf, &opening.row_path) {
        return false;
    }
    let next_row_leaf = match stark_air_trace_leaf_hash(params, &opening.next_row) {
        Some(value) => value,
        None => return false,
    };
    if !merkle_verify_hash(
        params,
        &air.trace_root,
        &next_row_leaf,
        &opening.next_row_path,
    ) {
        return false;
    }
    let composition = match Fq::from_canonical_u64(opening.composition_value) {
        Some(value) => value,
        None => return false,
    };
    if !merkle_verify(
        params,
        &air.composition_root,
        composition,
        &opening.composition_path,
    ) {
        return false;
    }
    let expected = match stark_air_composition_value_for_context(
        context,
        base_index,
        total_domain,
        &air.public_digest,
        &opening.row,
        &opening.next_row,
    ) {
        Some(value) => value,
        None => return false,
    };
    if expected != composition {
        return false;
    }
    let opened_fri_value = if base_index.is_multiple_of(2) {
        first_decommit.y0
    } else {
        first_decommit.y1
    };
    if opened_fri_value != opening.composition_value {
        return false;
    }
    true
}
/// Verify a STARK FRI envelope under `zk-stark` with caller-provided limits.
pub fn verify_stark_fri_envelope_with_limits(bytes: &[u8], limits: &StarkVerifierLimits) -> bool {
    verify_stark_fri_envelope_with_context(bytes, limits, StarkAirVerificationContext::Binding)
}
/// Verify a STARK FRI AIR envelope against caller-provided trace rows and composition values.
#[cfg(test)]
pub(crate) fn verify_stark_fri_air_envelope_from_rows_and_composition_values(
    bytes: &[u8],
    circuit_id: &str,
    public_digest: &[u8; 32],
    rows: &[Vec<u64>],
    composition_values: &[u64],
) -> bool {
    verify_stark_fri_air_envelope_from_rows_and_composition_values_with_limits(
        bytes,
        &StarkVerifierLimits::default(),
        circuit_id,
        public_digest,
        rows,
        composition_values,
    )
}
/// Verify a STARK FRI AIR envelope against caller-provided trace rows and composition values.
pub(crate) fn verify_stark_fri_air_envelope_from_rows_and_composition_values_with_limits(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    circuit_id: &str,
    public_digest: &[u8; 32],
    rows: &[Vec<u64>],
    composition_values: &[u64],
) -> bool {
    let explicit = StarkAirExplicitVerificationContext {
        circuit_id,
        public_digest,
        rows,
        composition_values,
        base_indices: None,
    };
    verify_stark_fri_envelope_with_context(
        bytes,
        limits,
        StarkAirVerificationContext::Explicit(&explicit),
    )
}
pub(crate) fn verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    circuit_id: &str,
    public_digest: &[u8; 32],
    rows: &[Vec<u64>],
    composition_values: &[u64],
    base_indices: &[usize],
) -> bool {
    let explicit = StarkAirExplicitVerificationContext {
        circuit_id,
        public_digest,
        rows,
        composition_values,
        base_indices: Some(base_indices),
    };
    verify_stark_fri_envelope_with_context(
        bytes,
        limits,
        StarkAirVerificationContext::Explicit(&explicit),
    )
}
fn verify_stark_fri_envelope_with_context(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    context: StarkAirVerificationContext<'_>,
) -> bool {
    if bytes.len() > effective_max_envelope_bytes(limits) {
        return false;
    }
    // Decode the single canonical V1 representation.
    let env: StarkVerifyEnvelopeV1 = match ivm::codec::decode_canonical_norito(bytes) {
        Ok(e) => e,
        Err(_) => return false,
    };
    if validate_stark_transcript_label(
        &env.transcript_label,
        effective_max_transcript_label_len(limits),
    )
    .is_err()
    {
        return false;
    }
    if env.proof.version != 1 || env.proof.commits.version != 1 {
        return false;
    }
    let roots = &env.proof.commits.roots;
    let query_count = env.proof.queries.len();
    let expected_chain_len = match validate_params(&env.params, roots.len(), query_count, limits) {
        Some(v) => v,
        None => return false,
    };
    if env.proof.commits.comp_root.is_some() != env.proof.comp_values.is_some() {
        return false;
    }
    if !context.allows_auxiliary_composition()
        && (env.proof.commits.comp_root.is_some() || env.proof.comp_values.is_some())
    {
        return false;
    }
    if let Some(values) = env.proof.comp_values.as_ref() {
        if values.len() != query_count {
            return false;
        }
    }
    let Some(air) = env.proof.air.as_ref() else {
        return false;
    };
    if air.version != 1
        || validate_stark_circuit_id(&air.circuit_id).is_err()
        || air.trace_width as usize != context.trace_width()
        || air.trace_width as usize > effective_max_air_width(limits)
        || air.openings.len() != query_count
        || roots.first().copied() != Some(air.composition_root)
    {
        return false;
    }
    let total_domain = match 1usize.checked_shl(u32::from(env.params.n_log2)) {
        Some(value) if value != 0 => value,
        _ => return false,
    };
    if !stark_air_context_matches_statement(&env.params, air, total_domain, context) {
        return false;
    }
    let query_roots = stark_air_query_roots(roots, Some(air));
    let fold_arity = env.params.fold_arity as usize;
    let sampled_base_indices = match context {
        StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash,
            trace_material_digest,
            ..
        } => match bfv_full_bootstrap_expected_base_indices_v1(
            *statement_hash,
            *trace_material_digest,
            query_count,
            total_domain,
        ) {
            Ok(indices) => indices,
            Err(_) => return false,
        },
        StarkAirVerificationContext::Explicit(explicit) => {
            if let Some(base_indices) = explicit.base_indices {
                match validate_stark_fri_query_shape_for_base_indices_with_limits_v1(
                    &env.params,
                    &env.transcript_label,
                    roots,
                    &env.proof.queries,
                    base_indices,
                    limits,
                ) {
                    Ok(indices) => indices,
                    Err(_) => return false,
                }
            } else {
                match derive_query_indices_without_replacement(
                    &env.transcript_label,
                    &env.params,
                    &query_roots,
                    query_count,
                    total_domain,
                ) {
                    Ok(indices) => indices,
                    Err(_) => return false,
                }
            }
        }
        _ => match derive_query_indices_without_replacement(
            &env.transcript_label,
            &env.params,
            &query_roots,
            query_count,
            total_domain,
        ) {
            Ok(indices) => indices,
            Err(_) => return false,
        },
    };
    for (qi, (chain, base_index)) in env
        .proof
        .queries
        .iter()
        .zip(sampled_base_indices.iter().copied())
        .enumerate()
    {
        if chain.len() != expected_chain_len {
            return false;
        }
        let Some(opening) = air.openings.get(qi) else {
            return false;
        };
        let Some(first_decommit) = chain.first() else {
            return false;
        };
        if !verify_stark_air_opening(
            &env.params,
            air,
            opening,
            base_index,
            total_domain,
            first_decommit,
            limits,
            context,
        ) {
            return false;
        }
        let mut idx_layer = base_index;
        let mut layer_domain = total_domain;
        let mut last_z: Option<Fq> = None;
        for (k, decommit) in chain.iter().enumerate() {
            if layer_domain < fold_arity {
                return false;
            }
            let expected_pairs = layer_domain / fold_arity;
            let expected_j = idx_layer / fold_arity;
            if expected_j >= expected_pairs || decommit.j as usize != expected_j {
                return false;
            }
            let depth_current = match log2_usize(layer_domain) {
                Some(v) => v,
                None => return false,
            };
            let depth_next = match log2_usize(layer_domain / fold_arity) {
                Some(v) => v,
                None => return false,
            };
            if !merkle_path_depth_ok(&decommit.path_y0, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_y1, depth_current, limits)
                || !merkle_path_depth_ok(&decommit.path_z, depth_next, limits)
            {
                return false;
            }
            // Bind Merkle openings to the expected indices for this fold. Without this, a prover
            // can mix-and-match openings from arbitrary positions while still satisfying the
            // fold relation and Merkle roots, which breaks soundness.
            let idx_y0 = match merkle_path_index(&decommit.path_y0) {
                Some(v) => v,
                None => return false,
            };
            let idx_y1 = match merkle_path_index(&decommit.path_y1) {
                Some(v) => v,
                None => return false,
            };
            let idx_z = match merkle_path_index(&decommit.path_z) {
                Some(v) => v,
                None => return false,
            };
            let expected_y0 = match expected_j.checked_mul(fold_arity) {
                Some(v) => v,
                None => return false,
            };
            let expected_y1 = match expected_y0.checked_add(1) {
                Some(v) => v,
                None => return false,
            };
            if idx_y0 != expected_y0 || idx_y1 != expected_y1 || idx_z != expected_j {
                return false;
            }
            let mut tb = Vec::new();
            tb.extend_from_slice(env.transcript_label.as_bytes());
            tb.extend_from_slice(&env.params.version.to_le_bytes());
            tb.extend_from_slice(&[
                env.params.n_log2,
                env.params.blowup_log2,
                env.params.fold_arity,
                env.params.merkle_arity,
                env.params.hash_fn,
            ]);
            tb.extend_from_slice(&env.params.queries.to_le_bytes());
            tb.extend_from_slice(&(env.params.domain_tag.len() as u32).to_le_bytes());
            tb.extend_from_slice(env.params.domain_tag.as_bytes());
            tb.extend_from_slice(&roots[k]);
            let r_k = match challenge(&env.params, "stark:fri:r:k", &tb) {
                Some(v) => v,
                None => return false,
            };
            let y0 = match Fq::from_canonical_u64(decommit.y0) {
                Some(v) => v,
                None => return false,
            };
            let y1 = match Fq::from_canonical_u64(decommit.y1) {
                Some(v) => v,
                None => return false,
            };
            if !merkle_verify(&env.params, &roots[k], y0, &decommit.path_y0) {
                return false;
            }
            if !merkle_verify(&env.params, &roots[k], y1, &decommit.path_y1) {
                return false;
            }
            let z = match Fq::from_canonical_u64(decommit.z) {
                Some(v) => v,
                None => return false,
            };
            let x = match domain_x_for_pair(layer_domain, expected_j) {
                Some(v) => v,
                None => return false,
            };
            let zr = match fri_fold_pair(y0, y1, r_k, x) {
                Some(v) => v,
                None => return false,
            };
            if zr != z {
                return false;
            }
            if !merkle_verify(&env.params, &roots[k + 1], z, &decommit.path_z) {
                return false;
            }
            last_z = Some(z);
            layer_domain /= fold_arity;
            idx_layer = expected_j;
        }
        if last_z != Some(Fq::zero()) {
            return false;
        }
        if let (Some(comp_root), Some(cv_all)) =
            (env.proof.commits.comp_root, env.proof.comp_values.as_ref())
        {
            if qi >= cv_all.len() {
                return false;
            }
            let comp_entry = &cv_all[qi];
            if comp_entry.aux_terms.len() > effective_max_aux_terms(limits) {
                return false;
            }
            let depth_comp = match log2_usize(layer_domain) {
                Some(v) => v,
                None => return false,
            };
            if !merkle_path_depth_ok(&comp_entry.path, depth_comp, limits) {
                return false;
            }
            let cv_f = match Fq::from_canonical_u64(comp_entry.leaf) {
                Some(v) => v,
                None => return false,
            };
            if !merkle_verify(&env.params, &comp_root, cv_f, &comp_entry.path) {
                return false;
            }
            let constant = match Fq::from_canonical_u64(comp_entry.constant) {
                Some(v) => v,
                None => return false,
            };
            let z_coeff = match Fq::from_canonical_u64(comp_entry.z_coeff) {
                Some(v) => v,
                None => return false,
            };
            let expected_public_digest = match stark_air_public_digest_from_composition(
                comp_entry.constant,
                comp_entry.z_coeff,
                &comp_entry.aux_terms,
            ) {
                Ok(digest) => digest,
                Err(_) => return false,
            };
            if air.public_digest != expected_public_digest {
                return false;
            }
            let mut expected = constant;
            if let Some(zf) = last_z {
                if comp_entry.z_coeff != 0 {
                    expected = expected.add(z_coeff.mul(zf));
                }
            } else if comp_entry.z_coeff != 0 {
                return false;
            }
            let mut last_wire: Option<u32> = None;
            for term in &comp_entry.aux_terms {
                if let Some(prev) = last_wire {
                    if term.wire_index <= prev {
                        return false;
                    }
                }
                last_wire = Some(term.wire_index);
                let coeff = match Fq::from_canonical_u64(term.coeff) {
                    Some(v) => v,
                    None => return false,
                };
                let value = match Fq::from_canonical_u64(term.value) {
                    Some(v) => v,
                    None => return false,
                };
                expected = expected.add(coeff.mul(value));
            }
            if cv_f != expected {
                return false;
            }
        }
        if layer_domain != 1 || idx_layer != 0 {
            return false;
        }
    }
    true
}
