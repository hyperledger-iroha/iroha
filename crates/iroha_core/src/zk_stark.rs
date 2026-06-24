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
/// Canonical ZK-ACE v0 STARK evaluation domain size (`2^n_log2`).
pub const ZK_ACE_STARK_FRI_V1_N_LOG2: u8 = 10;
/// Canonical ZK-ACE v0 FRI blowup factor (`2^blowup_log2`).
pub const ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2: u8 = 3;
/// Canonical ZK-ACE v0 verifier query count.
pub const ZK_ACE_STARK_FRI_V1_QUERIES: u16 = 24;
/// Canonical max proof size for hardened ZK-ACE v0 STARK/FRI proofs.
pub const ZK_ACE_STARK_FRI_V1_MAX_PROOF_BYTES: u32 = 1024 * 1024;
/// Minimum ZK-ACE STARK domain size accepted by ledger-grade admission.
pub const ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2: u8 = ZK_ACE_STARK_FRI_V1_N_LOG2;
/// Minimum ZK-ACE FRI blowup accepted by ledger-grade admission.
pub const ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2: u8 = ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2;
/// Minimum ZK-ACE FRI query count accepted by ledger-grade admission.
pub const ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES: u16 = ZK_ACE_STARK_FRI_V1_QUERIES;
/// Trace width of the generic OpenVerify binding AIR used by STARK/FRI v1 proofs.
pub const STARK_BINDING_AIR_TRACE_WIDTH_V1: u16 = 6;

const MAX_DOMAIN_LOG2: u8 = 24;
const MAX_FRI_LAYERS: usize = 32;
const MAX_FRI_QUERIES: usize = 32;
const MAX_MERKLE_DEPTH: usize = 32;
const MAX_AUX_TERMS: usize = 64;
const MAX_AIR_WIDTH: usize = 64;
const MAX_DOMAIN_TAG_LEN: usize = 64;
const MAX_TRANSCRIPT_LABEL_LEN: usize = 128;
const MAX_ENVELOPE_BYTES: usize = 1 << 20; // 1 MiB guard for decoded envelopes

pub(crate) const STARK_FRI_QUERY_INDEX_REPEATED_ERROR: &str = "FRI query index repeated";
const STARK_FRI_BOUNDED_QUERY_REJECTION_ATTEMPTS: usize = 8;
const BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS: u32 = 1024;
const GENERIC_STARK_AIR_BFV_FULL_BOOTSTRAP_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target the BFV full-bootstrap circuit; use the BFV full-bootstrap STARK prover";
const GENERIC_STARK_AIR_ZK_ACE_RESERVED_ERROR: &str =
    "generic STARK AIR prover cannot target the ZK-ACE circuit; use the ZK-ACE STARK prover";
const GENERIC_STARK_AIR_IVM_EXECUTION_RESERVED_ERROR: &str = "generic STARK AIR prover cannot target the IVM execution circuit; use the IVM execution STARK prover";

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
    use super::*;

    fn attach_valid_auxiliary_composition_values(envelope: &mut StarkVerifyEnvelopeV1) {
        let query_count = envelope.proof.queries.len();
        let leaf = 30_u64;
        let (comp_root, path) =
            stark_merkle_root_and_path_from_field_values_v1(&envelope.params, &[leaf], 0)
                .expect("derive auxiliary composition root");
        let comp_values = (0..query_count)
            .map(|_| StarkCompositionValueV1 {
                leaf,
                constant: 7,
                z_coeff: 0,
                aux_terms: vec![
                    StarkCompositionTermV1 {
                        wire_index: 1,
                        value: 5,
                        coeff: 3,
                    },
                    StarkCompositionTermV1 {
                        wire_index: 3,
                        value: 2,
                        coeff: 4,
                    },
                ],
                path: path.clone(),
            })
            .collect();
        envelope.proof.commits.comp_root = Some(comp_root);
        envelope.proof.comp_values = Some(comp_values);
    }

    #[test]
    fn fq_addition_wraps_correctly() {
        let a = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
        let b = Fq::one();
        assert_eq!(a.add(b), Fq::zero());
    }

    #[test]
    fn fq_subtraction_borrows_mod_prime() {
        let a = Fq::zero();
        let b = Fq::one();
        let expected = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
        assert_eq!(a.sub(b), expected);
    }

    #[test]
    fn fq_multiplication_reduces() {
        let a = Fq::from_canonical_u64(2).unwrap();
        let b = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
        let product = a.mul(b);
        let expected = Fq::from_canonical_u64(MOD_P_U64 - 2).unwrap();
        assert_eq!(product, expected);
    }

    #[test]
    fn fq_inverse_round_trip() {
        let element = Fq::from_canonical_u64(5).unwrap();
        let inv = element.inv().expect("invertible");
        assert_eq!(element.mul(inv), Fq::one());
    }

    #[test]
    fn fq_new_reduces_large_inputs() {
        let value = u64::MAX;
        let reduced = Fq::new(value);
        let expected = Fq::from_canonical_u64(((value as u128) % MOD_P) as u64).unwrap();
        assert_eq!(reduced, expected);
    }

    #[test]
    fn fq_from_canonical_rejects_out_of_range() {
        assert!(Fq::from_canonical_u64(MOD_P_U64).is_none());
    }

    #[test]
    fn zk_ace_stark_fri_verifying_key_payload_validation_fails_closed() {
        let valid = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
            n_log2: ZK_ACE_STARK_FRI_V1_N_LOG2,
            blowup_log2: ZK_ACE_STARK_FRI_V1_BLOWUP_LOG2,
            fold_arity: 2,
            queries: ZK_ACE_STARK_FRI_V1_QUERIES,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
        validate_zk_ace_stark_fri_verifying_key_payload(&valid)
            .expect("canonical ZK-ACE STARK/FRI payload is accepted");

        let mutations: [(&str, fn(&mut StarkFriVerifyingKeyV1)); 11] = [
            ("version", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.version = 2
            }),
            ("circuit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.circuit_id = "other".to_owned()
            }),
            ("hash", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.hash_fn = STARK_HASH_POSEIDON2_V1;
            }),
            ("fold", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.fold_arity = 4;
            }),
            ("merkle", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.merkle_arity = 4;
            }),
            ("n_log2_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.n_log2 = ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2 - 1;
            }),
            ("blowup_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.blowup_log2 = ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2 - 1;
            }),
            ("blowup_domain", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.blowup_log2 = payload.n_log2 + 1;
            }),
            ("queries_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.queries = ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES - 1;
            }),
            ("domain_limit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.n_log2 = MAX_DOMAIN_LOG2 + 1;
            }),
            ("query_limit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.queries = MAX_FRI_QUERIES as u16 + 1;
            }),
        ];
        for (label, mutate) in mutations {
            let mut invalid = valid.clone();
            mutate(&mut invalid);
            assert!(
                validate_zk_ace_stark_fri_verifying_key_payload(&invalid).is_err(),
                "{label} mutation must fail closed"
            );
        }
    }

    #[test]
    fn stark_fri_production_verifying_key_payload_validation_fails_closed() {
        const CIRCUIT_ID: &str = "soracloud:test-production-circuit";
        let valid = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: CIRCUIT_ID.to_owned(),
            n_log2: ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2,
            blowup_log2: ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
        validate_stark_fri_production_verifying_key_payload(&valid, CIRCUIT_ID, "test")
            .expect("production STARK/FRI payload is accepted");

        let mut poseidon = valid.clone();
        poseidon.hash_fn = STARK_HASH_POSEIDON2_V1;
        validate_stark_fri_production_verifying_key_payload(&poseidon, CIRCUIT_ID, "test")
            .expect("Poseidon2 STARK/FRI payload is accepted");

        let mutations: [(&str, fn(&mut StarkFriVerifyingKeyV1)); 11] = [
            ("version", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.version = 2
            }),
            ("circuit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.circuit_id = "other".to_owned()
            }),
            ("hash", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.hash_fn = 0xff;
            }),
            ("fold", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.fold_arity = 4;
            }),
            ("merkle", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.merkle_arity = 4;
            }),
            ("n_log2_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.n_log2 = ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2 - 1;
            }),
            ("blowup_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.blowup_log2 = ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2 - 1;
            }),
            ("blowup_domain", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.blowup_log2 = payload.n_log2 + 1;
            }),
            ("queries_floor", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.queries = ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES - 1;
            }),
            ("domain_limit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.n_log2 = MAX_DOMAIN_LOG2 + 1;
            }),
            ("query_limit", |payload: &mut StarkFriVerifyingKeyV1| {
                payload.queries = MAX_FRI_QUERIES as u16 + 1;
            }),
        ];
        for (label, mutate) in mutations {
            let mut invalid = valid.clone();
            mutate(&mut invalid);
            assert!(
                validate_stark_fri_production_verifying_key_payload(&invalid, CIRCUIT_ID, "test")
                    .is_err(),
                "{label} mutation must fail closed"
            );
        }

        let overlong_circuit_id = "soracloud:".to_owned() + &"x".repeat(MAX_TRANSCRIPT_LABEL_LEN);
        for circuit_id in [
            "",
            " ",
            "soracloud:test production-circuit",
            "soracloud:test\tproduction-circuit",
            overlong_circuit_id.as_str(),
        ] {
            let mut invalid = valid.clone();
            invalid.circuit_id = circuit_id.to_owned();
            assert!(
                validate_stark_fri_production_verifying_key_payload(&invalid, circuit_id, "test")
                    .is_err(),
                "matching noncanonical circuit id {circuit_id:?} must fail closed"
            );
        }
    }

    #[test]
    fn stark_verifier_limits_cannot_relax_canonical_structure_caps() {
        let valid = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:canonical-limit-cap".to_owned(),
        };
        assert_eq!(
            validate_params(&valid, 5, 2, &StarkVerifierLimits::default()),
            Some(4)
        );

        let mut relaxed = StarkVerifierLimits::default();
        relaxed.max_domain_log2 = MAX_DOMAIN_LOG2 + 1;
        relaxed.max_blowup_log2 = MAX_DOMAIN_LOG2 + 1;
        relaxed.max_queries = MAX_FRI_QUERIES + 1;
        relaxed.max_merkle_depth = MAX_MERKLE_DEPTH + 1;
        relaxed.max_aux_terms = MAX_AUX_TERMS + 1;
        relaxed.max_air_width = MAX_AIR_WIDTH + 1;
        relaxed.max_domain_tag_len = MAX_DOMAIN_TAG_LEN + 1;
        relaxed.max_transcript_label_len = MAX_TRANSCRIPT_LABEL_LEN + 1;
        relaxed.max_envelope_bytes = MAX_ENVELOPE_BYTES + 1;

        let mut oversized_domain = valid.clone();
        oversized_domain.n_log2 = MAX_DOMAIN_LOG2 + 1;
        oversized_domain.queries = 1;
        assert!(
            validate_params(
                &oversized_domain,
                usize::from(MAX_DOMAIN_LOG2) + 2,
                1,
                &relaxed
            )
            .is_none(),
            "caller limits must not relax canonical domain depth"
        );

        let mut oversized_blowup = valid.clone();
        oversized_blowup.blowup_log2 = MAX_DOMAIN_LOG2 + 1;
        assert!(
            validate_params(&oversized_blowup, 5, 2, &relaxed).is_none(),
            "caller limits must not relax canonical blowup depth"
        );
        let mut impossible_blowup = valid.clone();
        impossible_blowup.blowup_log2 = valid.n_log2 + 1;
        assert!(
            validate_params(&impossible_blowup, 5, 2, &relaxed).is_none(),
            "verifier must reject blowup depth greater than the evaluation domain"
        );

        let mut too_many_queries = valid.clone();
        too_many_queries.n_log2 = 6;
        too_many_queries.queries = (MAX_FRI_QUERIES + 1) as u16;
        assert!(
            validate_params(&too_many_queries, 7, MAX_FRI_QUERIES + 1, &relaxed).is_none(),
            "caller limits must not relax canonical query count"
        );

        let mut overlong_domain_tag = valid.clone();
        overlong_domain_tag.domain_tag = "d".repeat(MAX_DOMAIN_TAG_LEN + 1);
        assert!(
            validate_params(&overlong_domain_tag, 5, 2, &relaxed).is_none(),
            "caller limits must not relax canonical domain-tag length"
        );

        let too_deep_path = MerklePath {
            dirs: vec![0; (MAX_MERKLE_DEPTH + 8) / 8],
            siblings: vec![[0; 32]; MAX_MERKLE_DEPTH + 1],
        };
        assert!(
            !merkle_path_depth_ok(&too_deep_path, MAX_MERKLE_DEPTH + 1, &relaxed),
            "caller limits must not relax canonical Merkle depth"
        );

        let overlong_transcript_label = "T".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1);
        assert!(
            validate_stark_transcript_label(
                &overlong_transcript_label,
                effective_max_transcript_label_len(&relaxed),
            )
            .is_err(),
            "caller limits must not relax canonical transcript-label length"
        );
        assert_eq!(effective_max_aux_terms(&relaxed), MAX_AUX_TERMS);
        assert_eq!(effective_max_air_width(&relaxed), MAX_AIR_WIDTH);
        assert_eq!(effective_max_envelope_bytes(&relaxed), MAX_ENVELOPE_BYTES);
    }

    #[test]
    fn synthesized_envelope_verifies_sha256() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:sha256".to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-STARK".to_owned(),
            "stark/fri/sha256-goldilocks:test".to_owned(),
            [0x11; 32],
        )
        .expect("ok");
        assert!(verify_stark_fri_envelope(&bytes));
    }

    #[test]
    fn public_generic_air_provers_reject_bfv_full_bootstrap_circuit_aliases() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:reserved-bfv-generic-air".to_owned(),
        };
        let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
        let canonical = iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1;
        let circuit_ids = [
            canonical.to_owned(),
            format!("stark/fri/sha256-goldilocks:{canonical}"),
            format!("stark/fri/sha256-goldilocks/{canonical}"),
            format!("stark/fri/poseidon2-goldilocks:{canonical}"),
        ];

        for circuit_id in circuit_ids {
            let err = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-BFV-GENERIC-AIR".to_owned(),
                circuit_id.clone(),
                [0xB4; 32],
            )
            .expect_err("generic AIR prover must reject BFV full-bootstrap circuit aliases");
            assert!(
                err.contains("BFV full-bootstrap"),
                "unexpected generic AIR rejection for {circuit_id}: {err}"
            );

            let err = prove_stark_fri_zero_composition_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-BFV-ZERO-AIR".to_owned(),
                circuit_id.clone(),
                [0xB5; 32],
                rows.clone(),
            )
            .expect_err(
                "zero-composition AIR prover must reject BFV full-bootstrap circuit aliases",
            );
            assert!(
                err.contains("BFV full-bootstrap"),
                "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
            );
        }
    }

    #[test]
    fn public_generic_air_provers_reject_zk_ace_circuit_aliases() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:reserved-zk-ace-generic-air".to_owned(),
        };
        let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
        let canonical = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
        let circuit_ids = [
            canonical.to_owned(),
            format!("stark/fri/sha256-goldilocks:{canonical}"),
            format!("stark/fri/sha256-goldilocks/{canonical}"),
            format!("stark/fri/poseidon2-goldilocks:{canonical}"),
        ];

        for circuit_id in circuit_ids {
            let err = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-ZK-ACE-GENERIC-AIR".to_owned(),
                circuit_id.clone(),
                [0xC4; 32],
            )
            .expect_err("generic AIR prover must reject ZK-ACE circuit aliases");
            assert!(
                err.contains("ZK-ACE"),
                "unexpected generic AIR rejection for {circuit_id}: {err}"
            );

            let err = prove_stark_fri_zero_composition_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-ZK-ACE-ZERO-AIR".to_owned(),
                circuit_id.clone(),
                [0xC5; 32],
                rows.clone(),
            )
            .expect_err("zero-composition AIR prover must reject ZK-ACE circuit aliases");
            assert!(
                err.contains("ZK-ACE"),
                "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
            );
        }
    }

    #[test]
    fn public_generic_air_provers_reject_ivm_execution_circuit_aliases() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:reserved-ivm-generic-air".to_owned(),
        };
        let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
        let canonical = crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID;
        let circuit_ids = [
            canonical.to_owned(),
            format!("stark/fri/sha256-goldilocks:{canonical}"),
            format!("stark/fri/sha256-goldilocks/{canonical}"),
            format!("stark/fri/poseidon2-goldilocks:{canonical}"),
        ];

        for circuit_id in circuit_ids {
            let err = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-IVM-GENERIC-AIR".to_owned(),
                circuit_id.clone(),
                [0xD4; 32],
            )
            .expect_err("generic AIR prover must reject IVM execution circuit aliases");
            assert!(
                err.contains("IVM execution"),
                "unexpected generic AIR rejection for {circuit_id}: {err}"
            );

            let err = prove_stark_fri_zero_composition_air_envelope_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-IVM-ZERO-AIR".to_owned(),
                circuit_id.clone(),
                [0xD5; 32],
                rows.clone(),
            )
            .expect_err("zero-composition AIR prover must reject IVM execution circuit aliases");
            assert!(
                err.contains("IVM execution"),
                "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
            );
        }
    }

    #[test]
    fn synthesized_field_values_envelope_has_replayable_query_shape() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:field-values".to_owned(),
        };
        let values = vec![0; 1_usize << usize::from(params.n_log2)];
        let extra_query_roots = [[0xA1; 32], [0xA2; 32], [0xA3; 32]];
        let envelope = stark_synthesize_fri_envelope_from_field_values_v1(
            params.clone(),
            "IROHA-TEST-STARK".to_owned(),
            &values,
            &extra_query_roots,
        )
        .expect("synthesize field-value FRI envelope");
        let indices = validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
        )
        .expect("query shape replays");
        assert_eq!(indices.len(), usize::from(params.queries));
        assert!(
            indices
                .iter()
                .enumerate()
                .all(|(index, sampled)| !indices[..index].contains(sampled)),
            "query sampling must not repeat base indices"
        );

        let mut stale_merkle = envelope.clone();
        stale_merkle.proof.queries[0][0].path_y0.siblings[0][0] ^= 1;
        assert_eq!(
            validate_stark_fri_query_shape_and_indices_v1(
                &params,
                &stale_merkle.transcript_label,
                &stale_merkle.proof.commits.roots,
                &extra_query_roots,
                &stale_merkle.proof.queries,
            )
            .expect_err("stale FRI Merkle openings must be rejected"),
            "FRI query Merkle root mismatch"
        );

        let mut stale_folded_merkle = envelope.clone();
        stale_folded_merkle.proof.queries[0][0].path_z.siblings[0][0] ^= 1;
        assert_eq!(
            validate_stark_fri_query_shape_and_indices_v1(
                &params,
                &stale_folded_merkle.transcript_label,
                &stale_folded_merkle.proof.commits.roots,
                &extra_query_roots,
                &stale_folded_merkle.proof.queries,
            )
            .expect_err("stale folded FRI Merkle openings must be rejected"),
            "FRI query folded Merkle root mismatch"
        );

        let mut stale_fold = envelope;
        stale_fold.proof.queries[0][0].z = stale_fold.proof.queries[0][0].z.saturating_add(1);
        assert_eq!(
            validate_stark_fri_query_shape_and_indices_v1(
                &params,
                &stale_fold.transcript_label,
                &stale_fold.proof.commits.roots,
                &extra_query_roots,
                &stale_fold.proof.queries,
            )
            .expect_err("stale FRI fold values must be rejected"),
            "FRI query fold relation mismatch"
        );

        let nonzero_values = vec![1; 1_usize << usize::from(params.n_log2)];
        assert!(
            stark_synthesize_fri_envelope_from_field_values_v1(
                params.clone(),
                "IROHA-TEST-STARK".to_owned(),
                &nonzero_values,
                &extra_query_roots,
            )
            .is_none(),
            "prover must reject non-zero final FRI values before emitting proof bytes"
        );
        let public_err = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            params,
            "IROHA-TEST-STARK".to_owned(),
            "stark/fri/nonzero-final:test".to_owned(),
            [0xA5; 32],
            vec![vec![0]; 1_usize << 4],
            nonzero_values,
        )
        .expect_err("public AIR prover must reject non-zero final FRI values");
        assert_eq!(
            public_err, "STARK final FRI value must be zero",
            "non-zero final FRI values must fail during proof construction"
        );
    }

    #[test]
    fn without_replacement_query_schedule_uses_bound_specific_offsets() {
        assert_without_replacement_query_schedule_uses_bound_specific_offsets(
            STARK_HASH_SHA256_V1,
            "sha256",
        );
        assert_without_replacement_query_schedule_uses_bound_specific_offsets(
            STARK_HASH_POSEIDON2_V1,
            "poseidon2",
        );
    }

    fn assert_without_replacement_query_schedule_uses_bound_specific_offsets(
        hash_fn: u8,
        hash_label: &str,
    ) {
        let mut params = StarkFriParamsV1 {
            version: 1,
            n_log2: 3,
            blowup_log2: 1,
            fold_arity: 2,
            queries: 4,
            merkle_arity: 2,
            hash_fn,
            domain_tag: String::new(),
        };
        let label = "IROHA-TEST-BOUNDED-STARK-QUERY-OFFSET";
        let roots = [[0x42; 32], [0x24; 32]];
        let domain = 1_usize << usize::from(params.n_log2);
        let query_number = 1;
        let remaining = domain - query_number;

        for nonce in 0_u32..4096 {
            params.domain_tag = format!("iroha:test:{hash_label}:bounded-query-offset:{nonce}");
            let domain_remodulo = derive_query_index(label, &params, &roots, query_number)
                .expect("query index")
                % remaining;
            let bounded =
                derive_bounded_query_offset(label, &params, &roots, query_number, remaining)
                    .expect("bounded query offset");
            if domain_remodulo == bounded {
                continue;
            }

            let first_draw =
                derive_bounded_query_offset(label, &params, &roots, 0, domain).expect("first draw");
            let mut swaps = BTreeMap::new();
            let first_selected = first_draw;
            swaps.insert(first_draw, 0);
            let bounded_selected = swaps
                .get(&(query_number + bounded))
                .copied()
                .unwrap_or(query_number + bounded);
            let remodulo_selected = swaps
                .get(&(query_number + domain_remodulo))
                .copied()
                .unwrap_or(query_number + domain_remodulo);
            if bounded_selected == remodulo_selected {
                continue;
            }

            let indices =
                derive_query_indices_without_replacement(label, &params, &roots, 2, domain)
                    .expect("without-replacement schedule");
            assert_eq!(indices[0], first_selected);
            assert_eq!(indices[1], bounded_selected);
            assert_ne!(
                indices[1], remodulo_selected,
                "query schedule must use a bound-specific draw, not domain sample modulo remaining"
            );
            return;
        }

        panic!(
            "failed to find {hash_label} bounded-offset fixture that differs from domain remodulo"
        );
    }

    #[test]
    fn air_opening_first_fri_value_binding_uses_sampled_parity() {
        let empty_path = || MerklePath {
            dirs: Vec::new(),
            siblings: Vec::new(),
        };
        let opening = StarkAirOpeningV1 {
            index: 0,
            row: Vec::new(),
            next_row: Vec::new(),
            row_path: empty_path(),
            next_row_path: empty_path(),
            composition_value: 11,
            composition_path: empty_path(),
        };
        let decommit = FoldDecommitV1 {
            j: 0,
            y0: 11,
            y1: 17,
            path_y0: empty_path(),
            path_y1: empty_path(),
            z: 0,
            path_z: empty_path(),
        };
        validate_stark_air_opening_first_fri_value_v1(&opening, 0, &decommit)
            .expect("even sampled index binds y0");
        let mut odd_opening = opening.clone();
        odd_opening.index = 1;
        odd_opening.composition_value = 17;
        validate_stark_air_opening_first_fri_value_v1(&odd_opening, 1, &decommit)
            .expect("odd sampled index binds y1");
        assert_eq!(
            validate_stark_air_opening_first_fri_value_v1(&opening, 1, &decommit)
                .expect_err("mismatched sampled index must fail"),
            "AIR/FRI opening index mismatch"
        );
        let mut wrong_side_opening = odd_opening;
        wrong_side_opening.composition_value = 11;
        assert_eq!(
            validate_stark_air_opening_first_fri_value_v1(&wrong_side_opening, 1, &decommit)
                .expect_err("wrong FRI side must fail"),
            "AIR/FRI composition value mismatch"
        );
    }

    #[test]
    fn synthesized_envelope_verifies_poseidon2() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_POSEIDON2_V1,
            domain_tag: "iroha:test:poseidon2".to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-STARK".to_owned(),
            "stark/fri/poseidon2-goldilocks:test".to_owned(),
            [0x22; 32],
        )
        .expect("ok");
        assert!(verify_stark_fri_envelope(&bytes));
    }

    #[test]
    fn stark_fri_rejects_noncanonical_transcript_labels() {
        for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
            (
                STARK_HASH_SHA256_V1,
                "sha256",
                "iroha:test:canonical-transcript-label:sha256",
                "stark/fri/sha256-goldilocks:canonical-transcript-label",
                0x51_u8,
            ),
            (
                STARK_HASH_POSEIDON2_V1,
                "poseidon2",
                "iroha:test:canonical-transcript-label:poseidon2",
                "stark/fri/poseidon2-goldilocks:canonical-transcript-label",
                0x52_u8,
            ),
        ] {
            let params = StarkFriParamsV1 {
                version: 1,
                n_log2: 4,
                blowup_log2: 2,
                fold_arity: 2,
                queries: 2,
                merkle_arity: 2,
                hash_fn,
                domain_tag: domain_tag.to_owned(),
            };
            let bytes = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                format!("IROHA-TEST-STARK-CANONICAL-LABEL-{hash_label}"),
                circuit_id.to_owned(),
                [digest_byte; 32],
            )
            .expect("valid labeled STARK envelope");
            assert!(verify_stark_fri_envelope(&bytes));

            let mut envelope: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode labeled STARK envelope");
            let air = envelope.proof.air.as_ref().expect("AIR section");
            let extra_query_roots = [air.trace_root, air.composition_root, air.public_digest];
            let invalid_labels = [
                ("empty", String::new()),
                ("leading whitespace", " IROHA-TEST-STARK".to_owned()),
                ("embedded whitespace", "IROHA TEST STARK".to_owned()),
                ("control byte", "IROHA-TEST\nSTARK".to_owned()),
                ("non-ASCII", "IROHA-TEST-STARK-π".to_owned()),
                ("overlong", "A".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1)),
            ];
            for (label_case, invalid_label) in invalid_labels {
                let err = prove_stark_fri_air_envelope_bytes(
                    params.clone(),
                    invalid_label.clone(),
                    circuit_id.to_owned(),
                    [digest_byte; 32],
                )
                .expect_err(
                    "noncanonical STARK transcript labels must be rejected by proof construction",
                );
                assert!(
                    err.contains("transcript label"),
                    "{hash_label} {label_case} error should mention transcript labels, got: {err}"
                );

                assert_eq!(
                    validate_stark_fri_query_shape_and_indices_v1(
                        &params,
                        &invalid_label,
                        &envelope.proof.commits.roots,
                        &extra_query_roots,
                        &envelope.proof.queries,
                    )
                    .expect_err("query replay must reject noncanonical transcript labels"),
                    "FRI transcript label invalid"
                );

                envelope.transcript_label = invalid_label;
                let tampered =
                    norito::to_bytes(&envelope).expect("encode noncanonical-label STARK envelope");
                assert!(
                    !verify_stark_fri_envelope(&tampered),
                    "{hash_label} verifier must reject {label_case} transcript labels"
                );
            }
        }
    }

    #[test]
    fn stark_fri_rejects_noncanonical_circuit_ids() {
        for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
            (
                STARK_HASH_SHA256_V1,
                "sha256",
                "iroha:test:canonical-circuit-id:sha256",
                "stark/fri/sha256-goldilocks:canonical-circuit-id",
                0x61_u8,
            ),
            (
                STARK_HASH_POSEIDON2_V1,
                "poseidon2",
                "iroha:test:canonical-circuit-id:poseidon2",
                "stark/fri/poseidon2-goldilocks:canonical-circuit-id",
                0x62_u8,
            ),
        ] {
            let params = StarkFriParamsV1 {
                version: 1,
                n_log2: 4,
                blowup_log2: 2,
                fold_arity: 2,
                queries: 2,
                merkle_arity: 2,
                hash_fn,
                domain_tag: domain_tag.to_owned(),
            };
            let transcript_label = format!("IROHA-TEST-STARK-CANONICAL-CIRCUIT-{hash_label}");
            let bytes = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                transcript_label.clone(),
                circuit_id.to_owned(),
                [digest_byte; 32],
            )
            .expect("valid circuit-id STARK envelope");
            assert!(verify_stark_fri_envelope(&bytes));

            let mut envelope: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode circuit-id STARK envelope");
            let invalid_circuit_ids = [
                ("empty", String::new()),
                (
                    "leading whitespace",
                    " stark/fri/sha256-goldilocks:test".to_owned(),
                ),
                (
                    "embedded whitespace",
                    "stark/fri/sha256 goldilocks:test".to_owned(),
                ),
                (
                    "control byte",
                    "stark/fri/sha256-goldilocks:\ntest".to_owned(),
                ),
                ("non-ASCII", "stark/fri/sha256-goldilocks:π".to_owned()),
                ("overlong", "c".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1)),
            ];
            for (id_case, invalid_circuit_id) in invalid_circuit_ids {
                let err = prove_stark_fri_air_envelope_bytes(
                    params.clone(),
                    transcript_label.clone(),
                    invalid_circuit_id.clone(),
                    [digest_byte; 32],
                )
                .expect_err(
                    "noncanonical STARK circuit ids must be rejected by proof construction",
                );
                assert!(
                    err.contains("circuit_id"),
                    "{hash_label} {id_case} error should mention circuit_id, got: {err}"
                );

                envelope.proof.air.as_mut().expect("AIR section").circuit_id = invalid_circuit_id;
                let tampered = norito::to_bytes(&envelope)
                    .expect("encode noncanonical-circuit STARK envelope");
                assert!(
                    !verify_stark_fri_envelope(&tampered),
                    "{hash_label} verifier must reject {id_case} circuit ids"
                );
            }
        }
    }

    #[test]
    fn stark_fri_rejects_noncanonical_domain_tags() {
        for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
            (
                STARK_HASH_SHA256_V1,
                "sha256",
                "iroha:test:canonical-domain-tag:sha256",
                "stark/fri/sha256-goldilocks:canonical-domain-tag",
                0x71_u8,
            ),
            (
                STARK_HASH_POSEIDON2_V1,
                "poseidon2",
                "iroha:test:canonical-domain-tag:poseidon2",
                "stark/fri/poseidon2-goldilocks:canonical-domain-tag",
                0x72_u8,
            ),
        ] {
            let params = StarkFriParamsV1 {
                version: 1,
                n_log2: 4,
                blowup_log2: 2,
                fold_arity: 2,
                queries: 2,
                merkle_arity: 2,
                hash_fn,
                domain_tag: domain_tag.to_owned(),
            };
            let transcript_label = format!("IROHA-TEST-STARK-CANONICAL-DOMAIN-{hash_label}");
            let bytes = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                transcript_label.clone(),
                circuit_id.to_owned(),
                [digest_byte; 32],
            )
            .expect("valid domain-tag STARK envelope");
            assert!(verify_stark_fri_envelope(&bytes));

            let mut envelope: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode domain-tag STARK envelope");
            let invalid_domain_tags = [
                ("empty", String::new()),
                ("leading whitespace", " iroha:test:domain".to_owned()),
                ("embedded whitespace", "iroha:test domain".to_owned()),
                ("control byte", "iroha:test:\ndomain".to_owned()),
                ("non-ASCII", "iroha:test:π".to_owned()),
                ("overlong", "d".repeat(MAX_DOMAIN_TAG_LEN + 1)),
            ];
            for (tag_case, invalid_domain_tag) in invalid_domain_tags {
                let mut invalid_params = params.clone();
                invalid_params.domain_tag = invalid_domain_tag.clone();
                let err = prove_stark_fri_air_envelope_bytes(
                    invalid_params,
                    transcript_label.clone(),
                    circuit_id.to_owned(),
                    [digest_byte; 32],
                )
                .expect_err(
                    "noncanonical STARK domain tags must be rejected by proof construction",
                );
                assert!(
                    err.contains("domain tag"),
                    "{hash_label} {tag_case} error should mention domain tag, got: {err}"
                );

                envelope.params.domain_tag = invalid_domain_tag;
                let tampered =
                    norito::to_bytes(&envelope).expect("encode noncanonical-domain STARK envelope");
                assert!(
                    !verify_stark_fri_envelope(&tampered),
                    "{hash_label} verifier must reject {tag_case} domain tags"
                );
            }
        }
    }

    #[test]
    fn synthesized_envelope_without_air_is_rejected() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:missing-air".to_owned(),
        };
        let bytes =
            synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned()).expect("ok");
        assert!(!verify_stark_fri_envelope(&bytes));
    }

    #[test]
    fn air_envelope_verifies_and_rejects_tampered_opening() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:air".to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-STARK-AIR".to_owned(),
            "stark/fri/sha256-goldilocks:air-test".to_owned(),
            [0x42; 32],
        )
        .expect("air proof");
        assert!(verify_stark_fri_envelope(&bytes));

        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode air envelope");
        let air = envelope.proof.air.as_mut().expect("air section");
        air.openings[0].row[1] ^= 1;
        let tampered = norito::to_bytes(&envelope).expect("encode tampered air envelope");
        assert!(!verify_stark_fri_envelope(&tampered));
    }

    #[test]
    fn explicit_composition_air_envelope_binds_caller_rows_to_fri_queries() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:zero-composition-air".to_owned(),
        };
        let domain = 1_usize << usize::from(params.n_log2);
        let rows = (0..domain)
            .map(|index| {
                vec![
                    u64::try_from(index).expect("index fits u64"),
                    u64::try_from(index * 3 + 1).expect("sample value fits u64"),
                    7,
                ]
            })
            .collect::<Vec<_>>();
        let composition_values = vec![0; domain];
        let public_digest = [0x55; 32];
        let circuit_id = "stark/fri/custom-zero-air:test";
        let bytes = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            params.clone(),
            "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
            circuit_id.to_owned(),
            public_digest,
            rows.clone(),
            composition_values.clone(),
        )
        .expect("zero-composition AIR envelope");
        assert!(
            verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values,
            )
        );
        assert!(!verify_stark_fri_envelope(&bytes));

        let mut auxiliary_envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode zero-composition AIR envelope");
        attach_valid_auxiliary_composition_values(&mut auxiliary_envelope);
        let auxiliary_bytes =
            norito::to_bytes(&auxiliary_envelope).expect("encode auxiliary AIR envelope");
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &auxiliary_bytes,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values,
            ),
            "caller-owned explicit AIR must reject auxiliary generic composition commitments"
        );

        let mut drifted_rows = rows.clone();
        drifted_rows[0][0] ^= 1;
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &drifted_rows,
                &composition_values,
            )
        );
        let mut drifted_composition_values = composition_values.clone();
        drifted_composition_values[0] = 1;
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &rows,
                &drifted_composition_values,
            )
        );
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                "stark/fri/custom-zero-air:other",
                &public_digest,
                &rows,
                &composition_values,
            )
        );
        let wrong_public_digest = [0x56; 32];
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &wrong_public_digest,
                &rows,
                &composition_values,
            )
        );

        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode zero-composition AIR envelope");
        let air = envelope.proof.air.as_ref().expect("AIR section");
        assert_eq!(air.trace_width, 3);
        assert_eq!(air.public_digest, public_digest);
        assert_eq!(air.openings.len(), usize::from(params.queries));
        assert_eq!(
            envelope.proof.commits.roots.first(),
            Some(&air.composition_root)
        );
        let mut malformed_opening_params = envelope.params.clone();
        malformed_opening_params.domain_tag = "iroha:test:bad domain tag".to_owned();
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(
                &malformed_opening_params,
                air,
                air.openings.first().expect("AIR opening")
            )
            .expect_err("opening root replay must reject malformed STARK params"),
            "STARK opening commitment parameters invalid"
        );
        let mut impossible_opening_params = envelope.params.clone();
        impossible_opening_params.blowup_log2 = impossible_opening_params.n_log2 + 1;
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(
                &impossible_opening_params,
                air,
                air.openings.first().expect("AIR opening")
            )
            .expect_err("opening root replay must reject impossible FRI geometry"),
            "STARK opening commitment parameters invalid"
        );
        let extra_query_roots = [air.trace_root, air.composition_root, air.public_digest];
        let mut tight_opening_limits = StarkVerifierLimits::default();
        tight_opening_limits.max_domain_log2 = params.n_log2.saturating_sub(1);
        assert_eq!(
            validate_stark_air_opening_commitment_roots_with_limits_v1(
                &params,
                air,
                air.openings.first().expect("AIR opening"),
                &tight_opening_limits,
            )
            .expect_err("opening root replay must honor caller domain limits"),
            "STARK opening commitment parameters invalid"
        );

        let mut tight_query_limits = StarkVerifierLimits::default();
        tight_query_limits.max_queries = envelope.proof.queries.len().saturating_sub(1);
        assert_eq!(
            validate_stark_fri_query_shape_and_indices_with_limits_v1(
                &envelope.params,
                &envelope.transcript_label,
                &envelope.proof.commits.roots,
                &extra_query_roots,
                &envelope.proof.queries,
                &tight_query_limits,
            )
            .expect_err("FRI query replay must honor caller query limits"),
            "FRI parameter/root/query shape mismatch"
        );

        let indices = validate_stark_fri_query_shape_and_indices_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
        )
        .expect("query shape replays");
        let mut tight_base_index_limits = StarkVerifierLimits::default();
        tight_base_index_limits.max_queries = envelope.proof.queries.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
                &bytes,
                &tight_base_index_limits,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values,
                &indices,
            ),
            "explicit base-index AIR verifier must honor caller query limits"
        );
        for (opening_number, (opening, index)) in
            air.openings.iter().zip(indices.iter().copied()).enumerate()
        {
            assert_eq!(usize::try_from(opening.index).ok(), Some(index));
            assert_eq!(opening.row, rows[index]);
            assert_eq!(opening.next_row, rows[(index + 1) % domain]);
            assert_eq!(opening.composition_value, 0);
            validate_stark_air_opening_commitment_roots_v1(&params, air, opening)
                .expect("opening binds to trace and composition roots");
            validate_stark_air_opening_first_fri_value_v1(
                opening,
                index,
                envelope.proof.queries[opening_number]
                    .first()
                    .expect("query chain carries first decommitment"),
            )
            .expect("opening binds to first FRI layer");
        }

        let mut retargeted_opening_index = air.openings.first().expect("AIR opening").clone();
        let original_opening_index =
            usize::try_from(retargeted_opening_index.index).expect("opening index fits usize");
        retargeted_opening_index.index =
            u32::try_from((original_opening_index + 1) % domain).expect("domain fits u32");
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_opening_index)
                .expect_err("opening index retarget must fail before root replay"),
            "opening Merkle path index mismatch"
        );

        let mut retargeted_row_path = air.openings.first().expect("AIR opening").clone();
        retargeted_row_path.row_path.dirs[0] ^= 1;
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_row_path)
                .expect_err("row Merkle path retarget must fail before root replay"),
            "opening Merkle path index mismatch"
        );

        let mut retargeted_next_row_path = air.openings.first().expect("AIR opening").clone();
        retargeted_next_row_path.next_row_path.dirs[0] ^= 1;
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_next_row_path)
                .expect_err("next-row Merkle path retarget must fail before root replay"),
            "opening Merkle path index mismatch"
        );

        let mut retargeted_composition_path = air.openings.first().expect("AIR opening").clone();
        retargeted_composition_path.composition_path.dirs[0] ^= 1;
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(
                &params,
                air,
                &retargeted_composition_path
            )
            .expect_err("composition Merkle path retarget must fail before root replay"),
            "opening Merkle path index mismatch"
        );

        let mut tampered = envelope;
        let tampered_air = tampered.proof.air.as_mut().expect("AIR section");
        tampered_air.openings[0].row[0] ^= 1;
        let tampered_opening = tampered_air.openings[0].clone();
        assert_eq!(
            validate_stark_air_opening_commitment_roots_v1(
                &params,
                tampered_air,
                &tampered_opening
            )
            .expect_err("tampered caller row must not match trace root"),
            "row Merkle root mismatch"
        );

        let mut noncanonical_composition = vec![0; domain];
        noncanonical_composition[0] = MOD_P_U64;
        assert_eq!(
            prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
                params.clone(),
                "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
                "stark/fri/custom-zero-air:test".to_owned(),
                public_digest,
                rows.clone(),
                noncanonical_composition.clone(),
            )
            .expect_err("non-canonical composition values must be rejected"),
            "STARK AIR composition contains non-canonical field element"
        );
        assert!(
            stark_merkle_root_from_field_values_v1(&params, &noncanonical_composition).is_none(),
            "explicit AIR composition roots must reject non-canonical field elements"
        );
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &rows,
                &noncanonical_composition,
            ),
            "explicit AIR verification must reject non-canonical caller composition values"
        );

        let mut noncanonical_rows = rows;
        noncanonical_rows[domain - 1][1] = MOD_P_U64;
        assert_eq!(
            prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
                params.clone(),
                "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
                "stark/fri/custom-zero-air:test".to_owned(),
                public_digest,
                noncanonical_rows.clone(),
                composition_values.clone(),
            )
            .expect_err("non-canonical AIR rows must be rejected"),
            "STARK AIR row contains non-canonical field element"
        );
        assert!(
            stark_air_trace_root_from_rows_v1(&params, &noncanonical_rows).is_none(),
            "explicit AIR trace roots must reject non-canonical row field elements"
        );
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &noncanonical_rows,
                &composition_values,
            ),
            "explicit AIR verification must reject non-canonical caller rows"
        );
    }

    #[test]
    fn air_prover_rejects_more_queries_than_domain() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 1,
            blowup_log2: 1,
            fold_arity: 2,
            queries: 3,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:repeated-air-query".to_owned(),
        };
        let err = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-REPEATED-AIR-QUERY".to_owned(),
            "stark/fri/custom-repeated-query-air:test".to_owned(),
            [0x71; 32],
        )
        .expect_err("pigeonhole-small AIR query schedule must not emit proof bytes");
        assert!(
            err.contains("STARK query count exceeds domain size"),
            "unexpected impossible-query rejection: {err}"
        );
    }

    #[test]
    fn air_envelope_skips_repeated_transcript_query_indices() {
        let mut params = StarkFriParamsV1 {
            version: 1,
            n_log2: 3,
            blowup_log2: 1,
            fold_arity: 2,
            queries: 4,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: String::new(),
        };
        let domain = 1_usize << usize::from(params.n_log2);
        let values = vec![Fq::zero(); domain];

        for nonce in 0_u32..1024 {
            params.domain_tag = format!("iroha:test:skip-repeated-query:{nonce}");
            let envelope = synthesize_stark_fri_envelope_from_values(
                params.clone(),
                "IROHA-TEST-SKIP-REPEATED-AIR-QUERY".to_owned(),
                values.clone(),
                &[],
            )
            .expect("duplicate-free query schedule must synthesize");
            let raw_indices = (0..usize::from(params.queries))
                .map(|query_number| {
                    derive_query_index(
                        &envelope.transcript_label,
                        &params,
                        &envelope.proof.commits.roots,
                        query_number,
                    )
                    .expect("query index")
                        % domain
                })
                .collect::<Vec<_>>();
            if !raw_indices
                .iter()
                .enumerate()
                .any(|(index, sampled)| raw_indices[..index].contains(sampled))
            {
                continue;
            }

            let replayed_indices = validate_stark_fri_query_shape_and_indices_v1(
                &params,
                &envelope.transcript_label,
                &envelope.proof.commits.roots,
                &[],
                &envelope.proof.queries,
            )
            .expect("colliding raw transcript samples are mapped without replacement");
            assert_eq!(replayed_indices.len(), usize::from(params.queries));
            assert!(
                !replayed_indices
                    .iter()
                    .enumerate()
                    .any(|(index, sampled)| replayed_indices[..index].contains(sampled)),
                "replayed transcript query indices must be duplicate-free"
            );
            assert_ne!(
                raw_indices, replayed_indices,
                "fixture must exercise collision skipping"
            );

            let public_digest = [0x73; 32];
            let circuit_id = "stark/fri/custom-skip-repeated-query-air:test";
            let rows = (0..domain)
                .map(|index| {
                    stark_air_row(index, &public_digest).expect("build duplicate-skip AIR row")
                })
                .collect::<Vec<_>>();
            let composition_values = (0..domain)
                .map(|index| {
                    stark_air_composition_value(
                        index,
                        domain,
                        &public_digest,
                        &rows[index],
                        &rows[(index + 1) % domain],
                    )
                    .expect("build duplicate-skip AIR composition value")
                })
                .collect::<Vec<_>>();
            let composition_values_u64 = composition_values
                .iter()
                .map(|value| value.0)
                .collect::<Vec<_>>();
            let bytes = prove_stark_fri_air_envelope_from_rows_and_composition_values_fq_bytes(
                params.clone(),
                "IROHA-TEST-SKIP-REPEATED-AIR-QUERY".to_owned(),
                circuit_id.to_owned(),
                public_digest,
                rows.clone(),
                composition_values.clone(),
                None,
            )
            .expect("colliding raw samples must still produce a duplicate-free AIR envelope");
            assert!(
                verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &bytes,
                    circuit_id,
                    &public_digest,
                    &rows,
                    &composition_values_u64,
                )
            );

            let mut duplicate_opening: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode duplicate-free AIR envelope");
            duplicate_opening.proof.queries[1] = duplicate_opening.proof.queries[0].clone();
            let first_opening = duplicate_opening
                .proof
                .air
                .as_ref()
                .expect("duplicate-free AIR section")
                .openings[0]
                .clone();
            duplicate_opening
                .proof
                .air
                .as_mut()
                .expect("duplicate-free AIR section")
                .openings[1] = first_opening;
            let duplicate_opening_bytes =
                norito::to_bytes(&duplicate_opening).expect("encode duplicate AIR opening");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &duplicate_opening_bytes,
                    circuit_id,
                    &public_digest,
                    &rows,
                    &composition_values_u64,
                ),
                "duplicate raw query/opening replay must not satisfy skipped transcript samples"
            );
            return;
        }

        panic!("failed to find small-domain transcript query collision fixture");
    }

    fn sample_bfv_full_bootstrap_linear_transform_artifact_payload(
        params: &iroha_crypto::BfvParameters,
        role: iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1,
    ) -> Vec<u8> {
        let transform = iroha_crypto::BfvFullBootstrapLinearTransformV1 {
            input_slot_count: params.polynomial_degree,
            output_slot_count: params.polynomial_degree,
            diagonals: vec![iroha_crypto::BfvFullBootstrapLinearTransformDiagonalV1 {
                rotation_steps: 0,
                plaintext: iroha_crypto::encode_packed_plaintext_slots(
                    params,
                    &vec![1; usize::from(params.polynomial_degree)],
                )
                .expect("encode identity packed-slot mask"),
            }],
        };
        iroha_crypto::encode_bfv_full_bootstrap_linear_transform_artifact_v1(
            params, 1, role, &transform,
        )
        .expect("encode full-bootstrap linear transform artifact")
    }

    fn sample_bfv_full_bootstrap_artifacts_for_secret(
        params: &iroha_crypto::BfvParameters,
        secret_key: &iroha_crypto::BfvSecretKey,
    ) -> iroha_crypto::BfvFullBootstrapCircuitArtifactBundleV1 {
        let accumulator = iroha_crypto::BfvFullBootstrapAccumulatorV1 {
            slot_count: params.polynomial_degree,
            test_vector: iroha_crypto::encode_packed_plaintext_slots(
                params,
                &vec![1; usize::from(params.polynomial_degree)],
            )
            .expect("encode full-bootstrap accumulator"),
        };
        let accumulator = iroha_crypto::encode_bfv_full_bootstrap_accumulator_artifact_v1(
            params,
            1,
            &accumulator,
        )
        .expect("encode accumulator artifact");
        let accumulator_digest = iroha_crypto::Hash::new(&accumulator);
        let proof_public_input_schema =
            iroha_crypto::encode_bfv_full_bootstrap_proof_public_input_schema_artifact_v1(
                params,
                1,
                &iroha_crypto::bfv_full_bootstrap_proof_public_input_schema_v1(),
            )
            .expect("encode proof public-input schema artifact");
        let proof_public_input_schema_digest = iroha_crypto::Hash::new(&proof_public_input_schema);
        let arithmetic_air_constraint_system =
            iroha_crypto::encode_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_v1(
                params,
                1,
                &iroha_crypto::bfv_full_bootstrap_arithmetic_air_constraint_system_material_v1(),
            )
            .expect("encode arithmetic AIR artifact");
        let coefficient_to_slot_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
            params,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::CoefficientToSlotKey,
        );
        let slot_to_coefficient_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
            params,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::SlotToCoefficientKey,
        );
        let blind_rotation_key =
            iroha_crypto::bfv_full_bootstrap_blind_rotation_key_for_packed_left_rotation_v1(
                params,
                accumulator_digest,
                1,
            )
            .expect("build blind-rotation key");
        let blind_rotation_key =
            iroha_crypto::encode_bfv_full_bootstrap_blind_rotation_artifact_v1(
                params,
                1,
                &blind_rotation_key,
            )
            .expect("encode blind-rotation artifact");
        let sample_extraction = iroha_crypto::BfvFullBootstrapSampleExtractionV1 {
            source_slot_count: params.polynomial_degree,
            source_ciphertext_component_count: 2,
            extracted_coefficient_index: 0,
            output_ciphertext_component_count: 2,
        };
        let sample_extraction_key =
            iroha_crypto::bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
                params,
                secret_key,
                sample_extraction,
                b"zk-stark-bfv-full-bootstrap-sample-switch",
            )
            .expect("build sample-extraction switch key");
        let sample_extraction_key =
            iroha_crypto::encode_bfv_full_bootstrap_sample_extraction_switch_key_artifact_v1(
                params,
                1,
                &sample_extraction_key,
            )
            .expect("encode sample-extraction switch key artifact");
        let evaluator_artifact_set_digest =
            iroha_crypto::bfv_full_bootstrap_evaluator_artifact_set_digest_v1(
                params,
                1,
                &coefficient_to_slot_key,
                &slot_to_coefficient_key,
                &blind_rotation_key,
                &sample_extraction_key,
                &accumulator,
                &proof_public_input_schema,
                &arithmetic_air_constraint_system,
            )
            .expect("derive evaluator artifact-set digest");
        let prover_key_material =
            iroha_crypto::encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1(
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            )
            .expect("encode native prover material");
        let verifier_key_material =
            iroha_crypto::encode_bfv_full_bootstrap_native_stark_fri_verifier_key_material_v1(
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            )
            .expect("encode native verifier material");
        let (prover_key, verifier_key) =
            iroha_crypto::bfv_full_bootstrap_proof_key_pair_from_key_material_v1(
                params,
                1,
                proof_public_input_schema_digest,
                evaluator_artifact_set_digest,
                &prover_key_material,
                &verifier_key_material,
            )
            .expect("build native proof-key pair");
        let prover_key = iroha_crypto::encode_bfv_full_bootstrap_proof_key_artifact_v1(
            params,
            1,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            &prover_key,
        )
        .expect("encode prover-key artifact");
        let verifier_key = iroha_crypto::encode_bfv_full_bootstrap_proof_key_artifact_v1(
            params,
            1,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
            &verifier_key,
        )
        .expect("encode verifier-key artifact");
        iroha_crypto::BfvFullBootstrapCircuitArtifactBundleV1 {
            coefficient_to_slot_key,
            slot_to_coefficient_key,
            blind_rotation_key,
            sample_extraction_key,
            accumulator,
            proof_public_input_schema,
            arithmetic_air_constraint_system,
            prover_key,
            verifier_key,
        }
    }

    fn bfv_full_bootstrap_stark_test_prover_input_material()
    -> iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1 {
        bfv_full_bootstrap_stark_test_prover_input_material_for_slot(0)
    }

    fn bfv_full_bootstrap_stark_test_prover_input_material_for_slot(
        slot_index: u32,
    ) -> iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1 {
        let params = iroha_crypto::ram_lfe_bfv_parameters_v1();
        let (secret_key, public_key, _relinearization_key) =
            iroha_crypto::keygen_from_seed(&params, b"zk-stark-bfv-full-bootstrap-keygen")
                .expect("BFV keygen");
        let artifacts = sample_bfv_full_bootstrap_artifacts_for_secret(&params, &secret_key);
        let material = iroha_crypto::bfv_full_bootstrap_circuit_material_from_artifacts_v1(
            &params, 1, &artifacts,
        )
        .expect("derive governed full-bootstrap material");
        let blind_rotation = iroha_crypto::decode_bfv_full_bootstrap_blind_rotation_artifact_v1(
            &params,
            &material,
            &artifacts.blind_rotation_key,
        )
        .expect("decode blind-rotation artifact");
        let bootstrap_key = iroha_crypto::full_bootstrap_key_from_material_v1(
            &params,
            &public_key,
            "zk-stark-bfv-full-bootstrap-key",
            material.clone(),
        )
        .expect("full-bootstrap key");
        let plaintext = iroha_crypto::encode_packed_plaintext_slots(
            &params,
            &(0..usize::from(params.polynomial_degree))
                .map(|slot| u64::try_from((slot * 13 + 11) % 257).expect("slot fits"))
                .collect::<Vec<_>>(),
        )
        .expect("encode packed BFV plaintext");
        let input = iroha_crypto::encrypt_from_seed(
            &params,
            &public_key,
            &plaintext,
            b"zk-stark-bfv-full-bootstrap-input",
        )
        .expect("encrypt BFV input");
        let galois_keys = blind_rotation
            .steps
            .iter()
            .map(|step| {
                iroha_crypto::galois_key_from_seed(
                    &params,
                    &secret_key,
                    step.automorphism_power,
                    b"zk-stark-bfv-full-bootstrap-galois",
                )
                .expect("Galois key")
            })
            .collect::<Vec<_>>();
        let reviewer_key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![0xC3; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("fixture seed derives release reviewer keypair");
        let release_audit_package =
            iroha_crypto::bfv_full_bootstrap_release_audit_package_for_artifacts_v1(
                &params,
                &material,
                &artifacts,
                "sora-zk-audit-wg-2026",
                reviewer_key_pair.private_key(),
            )
            .expect("build canonical full-bootstrap release audit package from governed artifacts");
        let release_audit_package_digest =
            iroha_crypto::bfv_full_bootstrap_release_audit_package_digest_v1(
                &release_audit_package,
            )
            .expect("digest full-bootstrap release audit package");
        let output =
            iroha_crypto::full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                &input,
                &release_audit_package,
                release_audit_package_digest,
                "sora-zk-audit-wg-2026",
                reviewer_key_pair.public_key(),
            )
            .expect("release-audited artifact-aware full-bootstrap output");
        let input_bound = iroha_crypto::bfv_encrypted_zero_refresh_residual_multiple_bound(&params)
            .expect("input residual bound");
        let output_bound =
            iroha_crypto::bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                input_bound,
                &release_audit_package,
                release_audit_package_digest,
                "sora-zk-audit-wg-2026",
                reviewer_key_pair.public_key(),
            )
            .expect("release-audited artifact-aware full-bootstrap output bound");
        let claim = iroha_crypto::bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            slot_index,
            input,
            output,
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple,
            input_bound,
            output_bound,
        )
        .expect("derive execution proof claim");
        let witness_material =
            iroha_crypto::bfv_full_bootstrap_execution_witness_digest_material_v1(
                &params,
                &bootstrap_key,
                &artifacts,
                &galois_keys,
                &claim,
            )
            .expect("derive execution witness material");
        let proof_input = iroha_crypto::bfv_full_bootstrap_execution_proof_input_material_v1(
            &public_key,
            &witness_material,
        )
        .expect("build execution proof input material");
        let prover_key = iroha_crypto::decode_bfv_full_bootstrap_proof_key_artifact_v1(
            &params,
            &material,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
            &artifacts.prover_key,
        )
        .expect("decode prover key artifact");
        let verifier_key = iroha_crypto::decode_bfv_full_bootstrap_proof_key_artifact_v1(
            &params,
            &material,
            iroha_crypto::BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
            &artifacts.verifier_key,
        )
        .expect("decode verifier key artifact");
        iroha_crypto::bfv_full_bootstrap_execution_prover_input_material_v1(
            &proof_input,
            &prover_key,
            &verifier_key,
        )
        .expect("build BFV execution prover input material")
    }

    #[test]
    fn bfv_full_bootstrap_air_transcript_labels_are_canonical_retry_labels() {
        let base = iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1;
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            base
        ));
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            &format!("{base}:1")
        ));
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            &format!(
                "{}:{}",
                base,
                BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS - 1
            )
        ));

        for label in [
            format!("{base}:0"),
            format!("{base}:00"),
            format!("{base}:01"),
            format!("{base}:0001"),
            format!(
                "{}:{}",
                base, BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS
            ),
            format!("{base}:+1"),
            format!("{base}:1 "),
            format!("{base}:1:2"),
            format!("{base}-1"),
        ] {
            assert!(
                !bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(&label),
                "noncanonical BFV STARK transcript label must be rejected: {label:?}"
            );
        }
    }

    #[test]
    fn bfv_full_bootstrap_air_prover_binds_statement_and_public_openings() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let env: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        assert!(bfv_full_bootstrap_stark_air_transcript_label_allowed_v1(
            &env.transcript_label
        ));
        let expected_params =
            bfv_full_bootstrap_stark_air_params_v1(material.proof_input_material.statement_hash);
        assert!(bfv_full_bootstrap_stark_air_params_match_v1(
            &env.params,
            &expected_params
        ));
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        let witness = &material.proof_input_material.witness_material;
        assert!(
            verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must accept a generated native AIR envelope without private trace rows"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]),
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject zero statement hashes before envelope replay"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]),
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject zero trace-material digests before envelope replay"
        );
        let placeholder_statement_hash =
            iroha_crypto::Hash::new(b"pending BFV full-bootstrap execution witness digest");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                placeholder_statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject placeholder statement hashes before envelope replay"
        );
        let placeholder_trace_material_digest =
            iroha_crypto::Hash::new(b"pending BFV full-bootstrap execution witness digest");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                placeholder_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject placeholder trace-material digests before envelope replay"
        );
        let delayed_placeholder_statement_preimage = [
            b" \n\t".as_slice(),
            b"full-bootstrap material before placeholder: ".as_slice(),
            b"pending BFV full-bootstrap execution witness digest".as_slice(),
        ]
        .concat();
        let delayed_placeholder_statement_hash =
            iroha_crypto::Hash::new(&delayed_placeholder_statement_preimage);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                delayed_placeholder_statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject leading-whitespace delayed placeholder statement hashes before envelope replay"
        );
        let separator_spelled_statement_preimages = [
            b"p-e-n-d-i-n-g BFV full-bootstrap execution witness digest".as_slice(),
            b"p.e.n.d.i.n.g BFV full-bootstrap execution witness digest".as_slice(),
            b"p_e_n_d_i_n_g BFV full-bootstrap execution witness digest".as_slice(),
        ];
        let separator_spelled_statement_hashes =
            separator_spelled_statement_preimages.map(iroha_crypto::Hash::new);
        for separator_spelled_statement_hash in separator_spelled_statement_hashes.iter().copied() {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    separator_spelled_statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject separator-spelled placeholder statement hashes before envelope replay"
            );
        }
        let delayed_separator_spelled_statement_hashes =
            separator_spelled_statement_preimages.map(|preimage| {
                let delayed_preimage = [
                    b" \n\t".as_slice(),
                    b"full-bootstrap material before placeholder: ".as_slice(),
                    preimage,
                ]
                .concat();
                iroha_crypto::Hash::new(&delayed_preimage)
            });
        for delayed_separator_spelled_statement_hash in
            delayed_separator_spelled_statement_hashes.iter().copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    delayed_separator_spelled_statement_hash,
                    material.arithmetic_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject delayed separator-spelled placeholder statement hashes before envelope replay"
            );
        }
        let delayed_placeholder_trace_material_digest =
            iroha_crypto::Hash::new(&delayed_placeholder_statement_preimage);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                delayed_placeholder_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject leading-whitespace delayed placeholder trace-material digests before envelope replay"
        );
        let separator_spelled_trace_material_preimages = separator_spelled_statement_preimages;
        let separator_spelled_trace_material_digests =
            separator_spelled_trace_material_preimages.map(iroha_crypto::Hash::new);
        for separator_spelled_trace_material_digest in
            separator_spelled_trace_material_digests.iter().copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    material.proof_input_material.statement_hash,
                    separator_spelled_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject separator-spelled placeholder trace-material digests before envelope replay"
            );
        }
        let delayed_separator_spelled_trace_material_digests =
            separator_spelled_trace_material_preimages.map(|preimage| {
                let delayed_preimage = [
                    b" \n\t".as_slice(),
                    b"full-bootstrap material before placeholder: ".as_slice(),
                    preimage,
                ]
                .concat();
                iroha_crypto::Hash::new(&delayed_preimage)
            });
        for delayed_separator_spelled_trace_material_digest in
            delayed_separator_spelled_trace_material_digests
                .iter()
                .copied()
        {
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                    &bytes,
                    material.proof_input_material.statement_hash,
                    delayed_separator_spelled_trace_material_digest,
                    witness.slot_index,
                    witness.bound_mode,
                ),
                "public-padding BFV verifier must reject delayed separator-spelled placeholder trace-material digests before envelope replay"
            );
        }
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                u32::from(iroha_crypto::BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_PRIVATE_ROW_COUNT_V1),
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject out-of-range public slot headers before envelope replay"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                iroha_crypto::Hash::new(b"stale BFV full-bootstrap public statement hash"),
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must bind the statement hash"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index.saturating_add(1),
                witness.bound_mode,
            ),
            "public-padding BFV verifier must bind the public slot index"
        );
        let alternate_bound_mode = match witness.bound_mode {
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple => {
                iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::BoundedNoise
            }
            iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::BoundedNoise => {
                iroha_crypto::BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple
            }
        };
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                alternate_bound_mode,
            ),
            "public-padding BFV verifier must bind the public bound mode"
        );
        let air = env.proof.air.as_ref().expect("BFV AIR section");
        let domain_size = 1_usize << usize::from(env.params.n_log2);
        let public_padding_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &material.proof_input_material.statement_hash,
            trace_material_digest: &material.arithmetic_trace_material_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        assert!(
            stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                public_padding_context
            ),
            "private BFV public-padding context must accept the canonical STARK parameter profile"
        );
        let mut stale_public_padding_params = env.params.clone();
        stale_public_padding_params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap context statement"),
        )
        .domain_tag;
        assert!(
            !stark_air_context_matches_statement(
                &stale_public_padding_params,
                air,
                domain_size,
                public_padding_context,
            ),
            "private BFV public-padding context must reject statement-bound domain-tag drift"
        );
        let mut drifted_public_padding_params = env.params.clone();
        drifted_public_padding_params.hash_fn = STARK_HASH_POSEIDON2_V1;
        assert!(
            !stark_air_context_matches_statement(
                &drifted_public_padding_params,
                air,
                domain_size,
                public_padding_context,
            ),
            "private BFV public-padding context must reject canonical parameter-profile drift"
        );
        let zero_statement_hash = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
        let zero_public_digest = [0_u8; iroha_crypto::Hash::LENGTH];
        let zero_statement_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &zero_statement_hash,
            trace_material_digest: &material.arithmetic_trace_material_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        let mut zero_digest_air = air.clone();
        zero_digest_air.public_digest = zero_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &zero_digest_air,
                domain_size,
                zero_statement_context,
            ),
            "private BFV public-padding context must reject zero statement hashes even when the AIR digest matches"
        );
        let zero_trace_digest = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
        let zero_trace_context = StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
            statement_hash: &material.proof_input_material.statement_hash,
            trace_material_digest: &zero_trace_digest,
            slot_index: witness.slot_index,
            bound_mode: witness.bound_mode,
        };
        assert!(
            !stark_air_context_matches_statement(&env.params, air, domain_size, zero_trace_context,),
            "private BFV public-padding context must reject zero trace-material digests even when the AIR digest matches"
        );
        let first_public_opening = air.openings.first().expect("BFV AIR public opening");
        assert!(
            stark_air_composition_value_for_context(
                zero_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &zero_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a zero statement hash"
        );
        let placeholder_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
            placeholder_statement_hash.into();
        let placeholder_statement_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &placeholder_statement_hash,
                trace_material_digest: &material.arithmetic_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        let mut placeholder_digest_air = air.clone();
        placeholder_digest_air.public_digest = placeholder_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &placeholder_digest_air,
                domain_size,
                placeholder_statement_context,
            ),
            "private BFV public-padding context must reject placeholder statement hashes even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                placeholder_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &placeholder_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a placeholder statement hash"
        );
        let placeholder_trace_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &material.proof_input_material.statement_hash,
                trace_material_digest: &placeholder_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                placeholder_trace_context,
            ),
            "private BFV public-padding context must reject placeholder trace-material digests even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                placeholder_trace_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a placeholder trace-material digest"
        );
        let delayed_placeholder_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
            delayed_placeholder_statement_hash.into();
        let delayed_placeholder_statement_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &delayed_placeholder_statement_hash,
                trace_material_digest: &material.arithmetic_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        let mut delayed_placeholder_digest_air = air.clone();
        delayed_placeholder_digest_air.public_digest = delayed_placeholder_public_digest;
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                &delayed_placeholder_digest_air,
                domain_size,
                delayed_placeholder_statement_context,
            ),
            "private BFV public-padding context must reject leading-whitespace delayed placeholder statement hashes even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                delayed_placeholder_statement_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &delayed_placeholder_public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a leading-whitespace delayed placeholder statement hash"
        );
        for separator_spelled_statement_hash in separator_spelled_statement_hashes.iter().copied() {
            let separator_spelled_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
                separator_spelled_statement_hash.into();
            let separator_spelled_statement_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &separator_spelled_statement_hash,
                    trace_material_digest: &material.arithmetic_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            let mut separator_spelled_digest_air = air.clone();
            separator_spelled_digest_air.public_digest = separator_spelled_public_digest;
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    &separator_spelled_digest_air,
                    domain_size,
                    separator_spelled_statement_context,
                ),
                "private BFV public-padding context must reject separator-spelled placeholder statement hashes even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    separator_spelled_statement_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &separator_spelled_public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a separator-spelled placeholder statement hash"
            );
        }
        for delayed_separator_spelled_statement_hash in
            delayed_separator_spelled_statement_hashes.iter().copied()
        {
            let delayed_separator_spelled_public_digest: [u8; iroha_crypto::Hash::LENGTH] =
                delayed_separator_spelled_statement_hash.into();
            let delayed_separator_spelled_statement_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &delayed_separator_spelled_statement_hash,
                    trace_material_digest: &material.arithmetic_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            let mut delayed_separator_spelled_digest_air = air.clone();
            delayed_separator_spelled_digest_air.public_digest =
                delayed_separator_spelled_public_digest;
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    &delayed_separator_spelled_digest_air,
                    domain_size,
                    delayed_separator_spelled_statement_context,
                ),
                "private BFV public-padding context must reject delayed separator-spelled placeholder statement hashes even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    delayed_separator_spelled_statement_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &delayed_separator_spelled_public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a delayed separator-spelled placeholder statement hash"
            );
        }
        let delayed_placeholder_trace_context =
            StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                statement_hash: &material.proof_input_material.statement_hash,
                trace_material_digest: &delayed_placeholder_trace_material_digest,
                slot_index: witness.slot_index,
                bound_mode: witness.bound_mode,
            };
        assert!(
            !stark_air_context_matches_statement(
                &env.params,
                air,
                domain_size,
                delayed_placeholder_trace_context,
            ),
            "private BFV public-padding context must reject leading-whitespace delayed placeholder trace-material digests even when the AIR digest matches"
        );
        assert!(
            stark_air_composition_value_for_context(
                delayed_placeholder_trace_context,
                usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                domain_size,
                &public_digest,
                &first_public_opening.row,
                &first_public_opening.next_row,
            )
            .is_none(),
            "private BFV public-padding context must not replay openings under a leading-whitespace delayed placeholder trace-material digest"
        );
        for separator_spelled_trace_material_digest in
            separator_spelled_trace_material_digests.iter().copied()
        {
            let separator_spelled_trace_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &material.proof_input_material.statement_hash,
                    trace_material_digest: &separator_spelled_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    air,
                    domain_size,
                    separator_spelled_trace_context,
                ),
                "private BFV public-padding context must reject separator-spelled placeholder trace-material digests even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    separator_spelled_trace_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a separator-spelled placeholder trace-material digest"
            );
        }
        for delayed_separator_spelled_trace_material_digest in
            delayed_separator_spelled_trace_material_digests
                .iter()
                .copied()
        {
            let delayed_separator_spelled_trace_context =
                StarkAirVerificationContext::BfvFullBootstrapPublicPadding {
                    statement_hash: &material.proof_input_material.statement_hash,
                    trace_material_digest: &delayed_separator_spelled_trace_material_digest,
                    slot_index: witness.slot_index,
                    bound_mode: witness.bound_mode,
                };
            assert!(
                !stark_air_context_matches_statement(
                    &env.params,
                    air,
                    domain_size,
                    delayed_separator_spelled_trace_context,
                ),
                "private BFV public-padding context must reject delayed separator-spelled placeholder trace-material digests even when the AIR digest matches"
            );
            assert!(
                stark_air_composition_value_for_context(
                    delayed_separator_spelled_trace_context,
                    usize::try_from(first_public_opening.index).expect("opening index fits usize"),
                    domain_size,
                    &public_digest,
                    &first_public_opening.row,
                    &first_public_opening.next_row,
                )
                .is_none(),
                "private BFV public-padding context must not replay openings under a delayed separator-spelled placeholder trace-material digest"
            );
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
            material.proof_input_material.statement_hash,
            material.arithmetic_trace_material_digest,
            material.proof_input_material.witness_material.slot_index,
            material.proof_input_material.witness_material.bound_mode,
        )
        .expect("BFV transcript-bound public opening set");
        for opening in &air.openings {
            iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_public_padding_opening_v1(
                opening.index,
                &opening.row,
                &opening.next_row,
                material.proof_input_material.statement_hash,
                material.proof_input_material.witness_material.slot_index,
                material.proof_input_material.witness_material.bound_mode,
            )
            .expect("BFV sampled opening is a canonical public padding row");
        }

        let mut duplicate_opening_env = env.clone();
        {
            let duplicate_air = duplicate_opening_env
                .proof
                .air
                .as_mut()
                .expect("BFV AIR section");
            duplicate_air.openings[1] = duplicate_air.openings[0].clone();
        }
        let duplicate_opening_bytes =
            norito::to_bytes(&duplicate_opening_env).expect("encode duplicate BFV AIR opening");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &duplicate_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject duplicated sampled public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&duplicate_opening_bytes, &material),
            "artifact-bound BFV verifier must reject duplicated sampled public openings"
        );

        let mut reordered_opening_env = env.clone();
        reordered_opening_env
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings
            .swap(0, 1);
        let reordered_opening_bytes =
            norito::to_bytes(&reordered_opening_env).expect("encode reordered BFV AIR openings");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &reordered_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject reordered public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&reordered_opening_bytes, &material),
            "artifact-bound BFV verifier must reject reordered public openings"
        );

        let mut truncated_opening_env = env.clone();
        truncated_opening_env
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings
            .pop();
        let truncated_opening_bytes =
            norito::to_bytes(&truncated_opening_env).expect("encode truncated BFV AIR openings");
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &truncated_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject truncated sampled public openings"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&truncated_opening_bytes, &material),
            "artifact-bound BFV verifier must reject truncated sampled public openings"
        );

        let mut unsafe_generic_air = None;
        for attempt in 0..BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS {
            let transcript_label = bfv_full_bootstrap_stark_air_transcript_label_v1(attempt);
            let candidate_bytes =
                match prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
                    expected_params.clone(),
                    transcript_label.clone(),
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1.to_owned(),
                    public_digest,
                    material.arithmetic_trace_material.rows.clone(),
                    material
                        .arithmetic_air_evaluation_material
                        .composition_values
                        .clone(),
                ) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
            let candidate_env: StarkVerifyEnvelopeV1 =
                norito::decode_from_bytes(&candidate_bytes).expect("decode candidate BFV AIR");
            let candidate_air = candidate_env
                .proof
                .air
                .as_ref()
                .expect("candidate AIR section");
            let candidate_indices = candidate_air
                .openings
                .iter()
                .map(|opening| opening.index)
                .collect::<Vec<_>>();
            let Err(opening_err) =
                iroha_crypto::validate_bfv_full_bootstrap_arithmetic_trace_canonical_opening_indices_v1(
                    &candidate_indices,
                )
            else {
                continue;
            };
            unsafe_generic_air = Some((candidate_bytes, candidate_indices, opening_err));
            break;
        }
        let (unsafe_bytes, unsafe_indices, opening_err) = unsafe_generic_air
            .expect("find allowed BFV transcript label with private-row AIR openings");
        assert!(
            opening_err.to_string().contains("unmasked private row"),
            "unsafe generic AIR rejection must be privacy-related: {opening_err}"
        );
        assert!(
            verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &unsafe_bytes,
                iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                &public_digest,
                &material.arithmetic_trace_material.rows,
                &material
                    .arithmetic_air_evaluation_material
                    .composition_values,
            ),
            "unsafe candidate must remain a structurally valid generic AIR proof"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&unsafe_bytes, &material),
            "BFV native AIR verifier must reject generic proofs that open private rows: {unsafe_indices:?}"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &unsafe_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject generic proofs that open private rows: {unsafe_indices:?}"
        );

        let mut stale_domain = env.clone();
        stale_domain.params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap statement hash"),
        )
        .domain_tag;
        let stale_domain_bytes = norito::to_bytes(&stale_domain).expect("encode stale domain");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &stale_domain_bytes,
            &material
        ));

        let mut stale_material = material.clone();
        stale_material.proof_input_material.statement_hash =
            iroha_crypto::Hash::new(b"stale BFV full-bootstrap statement hash");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes,
            &stale_material
        ));

        let mut tampered_opening = env;
        tampered_opening
            .proof
            .air
            .as_mut()
            .expect("BFV AIR section")
            .openings[0]
            .row[9] = 1;
        let tampered_opening_bytes =
            norito::to_bytes(&tampered_opening).expect("encode tampered opening");
        assert!(!verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &tampered_opening_bytes,
            &material
        ));
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_public_padding_envelope(
                &tampered_opening_bytes,
                material.proof_input_material.statement_hash,
                material.arithmetic_trace_material_digest,
                witness.slot_index,
                witness.bound_mode,
            ),
            "public-padding BFV verifier must reject drifted public row openings"
        );
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_auxiliary_generic_composition_sidecars() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));

        let mut sidecar_envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        assert!(sidecar_envelope.proof.commits.comp_root.is_none());
        assert!(sidecar_envelope.proof.comp_values.is_none());
        attach_valid_auxiliary_composition_values(&mut sidecar_envelope);
        assert!(sidecar_envelope.proof.commits.comp_root.is_some());
        assert_eq!(
            sidecar_envelope.proof.comp_values.as_ref().map(Vec::len),
            Some(sidecar_envelope.proof.queries.len())
        );

        let mut comp_root_only = sidecar_envelope.clone();
        comp_root_only.proof.comp_values = None;
        let mut comp_values_only = sidecar_envelope.clone();
        comp_values_only.proof.commits.comp_root = None;
        let mut truncated_values = sidecar_envelope.clone();
        truncated_values
            .proof
            .comp_values
            .as_mut()
            .expect("composition values")
            .pop();

        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();
        for (case, envelope) in [
            ("paired auxiliary sidecars", sidecar_envelope),
            ("comp_root without comp_values", comp_root_only),
            ("comp_values without comp_root", comp_values_only),
            ("truncated comp_values", truncated_values),
        ] {
            let auxiliary_bytes =
                norito::to_bytes(&envelope).expect("encode auxiliary BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &auxiliary_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&auxiliary_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
        }
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_malformed_proof_and_air_bindings() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode malformed BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
        };

        let mut bad_proof_version = envelope.clone();
        bad_proof_version.proof.version = 2;
        assert_rejected("non-v1 proof version", &bad_proof_version);

        let mut bad_commit_version = envelope.clone();
        bad_commit_version.proof.commits.version = 2;
        assert_rejected("non-v1 commitment version", &bad_commit_version);

        let mut missing_air = envelope.clone();
        missing_air.proof.air = None;
        assert_rejected("missing AIR section", &missing_air);

        let mut foreign_air = envelope.clone();
        foreign_air
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .circuit_id = "stark/fri/sha256-goldilocks:foreign-bfv-air".to_owned();
        assert_rejected("foreign AIR circuit id", &foreign_air);

        let mut drifted_composition_root = envelope.clone();
        drifted_composition_root
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .composition_root[0] ^= 0x01;
        assert_rejected("AIR composition-root drift", &drifted_composition_root);

        let mut drifted_trace_root = envelope.clone();
        drifted_trace_root
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .trace_root[0] ^= 0x01;
        assert_rejected("AIR trace-root drift", &drifted_trace_root);

        let mut drifted_fri_root = envelope.clone();
        drifted_fri_root.proof.commits.roots[0][0] ^= 0x01;
        assert_rejected("FRI composition-root drift", &drifted_fri_root);

        let mut missing_query = envelope.clone();
        missing_query.proof.queries.pop();
        assert_rejected("missing FRI query chain", &missing_query);

        let mut missing_opening = envelope;
        missing_opening
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings
            .pop();
        assert_rejected("missing AIR opening", &missing_opening);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_opening_path_and_sample_drift() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode malformed BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
        };

        let mut wrong_opening_index = envelope.clone();
        let opening_index = wrong_opening_index
            .proof
            .air
            .as_ref()
            .expect("AIR section")
            .openings[0]
            .index;
        wrong_opening_index
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .index = opening_index.wrapping_add(1);
        assert_rejected("opening index drift", &wrong_opening_index);

        let mut swapped_row_paths = envelope.clone();
        let opening = &mut swapped_row_paths
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0];
        core::mem::swap(&mut opening.row_path, &mut opening.next_row_path);
        assert_rejected("swapped row and next-row paths", &swapped_row_paths);

        let mut tampered_row_path = envelope.clone();
        tampered_row_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .row_path
            .siblings
            .first_mut()
            .expect("row path sibling")[0] ^= 0x01;
        assert_rejected("row Merkle path drift", &tampered_row_path);

        let mut tampered_next_row_path = envelope.clone();
        tampered_next_row_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .next_row_path
            .siblings
            .first_mut()
            .expect("next-row path sibling")[0] ^= 0x01;
        assert_rejected("next-row Merkle path drift", &tampered_next_row_path);

        let mut tampered_composition_path = envelope.clone();
        tampered_composition_path
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .composition_path
            .siblings
            .first_mut()
            .expect("composition path sibling")[0] ^= 0x01;
        assert_rejected(
            "composition-value Merkle path drift",
            &tampered_composition_path,
        );

        let mut tampered_composition_value = envelope.clone();
        tampered_composition_value
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .openings[0]
            .composition_value ^= 0x01;
        assert_rejected(
            "opened composition-value drift",
            &tampered_composition_value,
        );

        let mut tampered_fri_base_value = envelope.clone();
        tampered_fri_base_value.proof.queries[0][0].y0 ^= 0x01;
        assert_rejected("FRI base value drift", &tampered_fri_base_value);

        let mut duplicated_opening = envelope;
        let air = duplicated_opening.proof.air.as_mut().expect("AIR section");
        assert!(air.openings.len() > 1, "BFV AIR test envelope has queries");
        air.openings[1] = air.openings[0].clone();
        assert_rejected("duplicated AIR opening", &duplicated_opening);
    }

    #[test]
    fn bfv_full_bootstrap_air_verifier_limits_are_enforced() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        assert!(
            verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &StarkVerifierLimits::default(),
                &material,
            )
        );

        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let air = envelope.proof.air.as_ref().expect("BFV AIR section");

        let mut tight_envelope_bytes = StarkVerifierLimits::default();
        tight_envelope_bytes.max_envelope_bytes = bytes.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_envelope_bytes,
                &material,
            ),
            "BFV native AIR verifier must honor envelope byte limits"
        );

        let mut tight_transcript_label = StarkVerifierLimits::default();
        tight_transcript_label.max_transcript_label_len =
            envelope.transcript_label.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_transcript_label,
                &material,
            ),
            "BFV native AIR verifier must honor transcript-label limits"
        );

        let mut tight_queries = StarkVerifierLimits::default();
        tight_queries.max_queries = envelope.proof.queries.len().saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_queries,
                &material,
            ),
            "BFV native AIR verifier must honor query-count limits"
        );

        let mut tight_air_width = StarkVerifierLimits::default();
        tight_air_width.max_air_width = usize::from(air.trace_width).saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_air_width,
                &material,
            ),
            "BFV native AIR verifier must honor AIR width limits"
        );

        let mut tight_merkle_depth = StarkVerifierLimits::default();
        tight_merkle_depth.max_merkle_depth = usize::from(envelope.params.n_log2).saturating_sub(1);
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope_with_limits(
                &bytes,
                &tight_merkle_depth,
                &material,
            ),
            "BFV native AIR verifier must honor Merkle-depth limits"
        );
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_parameter_profile_drift() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode BFV STARK envelope");
        let public_digest: [u8; 32] = material.proof_input_material.statement_hash.into();

        let assert_rejected = |case: &str, envelope: &StarkVerifyEnvelopeV1| {
            let malformed_bytes =
                norito::to_bytes(envelope).expect("encode parameter-drift BFV STARK envelope");
            assert!(
                !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                    &malformed_bytes,
                    iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
                    &public_digest,
                    &material.arithmetic_trace_material.rows,
                    &material
                        .arithmetic_air_evaluation_material
                        .composition_values,
                ),
                "caller-owned BFV explicit AIR must reject {case}"
            );
            assert!(
                !verify_stark_fri_bfv_full_bootstrap_air_envelope(&malformed_bytes, &material),
                "BFV native AIR verifier must reject {case}"
            );
        };

        let mut bad_version = envelope.clone();
        bad_version.params.version = 2;
        assert_rejected("STARK parameter version drift", &bad_version);

        let mut bad_domain_depth = envelope.clone();
        bad_domain_depth.params.n_log2 = bad_domain_depth.params.n_log2.saturating_add(1);
        assert_rejected("STARK domain-depth drift", &bad_domain_depth);

        let mut bad_blowup = envelope.clone();
        bad_blowup.params.blowup_log2 = bad_blowup.params.blowup_log2.saturating_add(1);
        assert_rejected("STARK blowup drift", &bad_blowup);

        let mut bad_fold_arity = envelope.clone();
        bad_fold_arity.params.fold_arity = 4;
        assert_rejected("STARK fold-arity drift", &bad_fold_arity);

        let mut bad_merkle_arity = envelope.clone();
        bad_merkle_arity.params.merkle_arity = 4;
        assert_rejected("STARK Merkle-arity drift", &bad_merkle_arity);

        let mut bad_hash_selector = envelope.clone();
        bad_hash_selector.params.hash_fn = STARK_HASH_POSEIDON2_V1;
        assert_rejected("STARK hash-selector drift", &bad_hash_selector);

        let mut bad_query_count = envelope.clone();
        bad_query_count.params.queries = bad_query_count.params.queries.saturating_sub(1);
        assert_rejected("STARK query-count header drift", &bad_query_count);

        let mut stale_domain = envelope;
        stale_domain.params.domain_tag = bfv_full_bootstrap_stark_air_params_v1(
            iroha_crypto::Hash::new(b"alternate BFV full-bootstrap parameter profile"),
        )
        .domain_tag;
        assert_rejected("statement-bound domain-tag drift", &stale_domain);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_stale_prover_input_material() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material();
        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));

        let assert_rejected_material =
            |case: &str, material: iroha_crypto::BfvFullBootstrapExecutionProverInputMaterialV1| {
                assert!(
                    iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(
                        &material
                    )
                    .is_err(),
                    "mutated BFV prover input material must fail validation: {case}"
                );
                assert!(
                    !verify_stark_fri_bfv_full_bootstrap_air_envelope(&bytes, &material),
                    "BFV native AIR verifier must reject {case}"
                );
                assert!(
                    prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material).is_err(),
                    "BFV native AIR prover must reject {case}"
                );
            };

        let mut stale_version = material.clone();
        stale_version.version = stale_version.version.saturating_add(1);
        assert_rejected_material("stale prover input material version", stale_version);

        let mut stale_field_count = material.clone();
        stale_field_count.field_count = stale_field_count.field_count.saturating_add(1);
        assert_rejected_material("stale prover input material field count", stale_field_count);

        let mut stale_proof_input_version = material.clone();
        stale_proof_input_version.proof_input_material.version = stale_proof_input_version
            .proof_input_material
            .version
            .saturating_add(1);
        assert_rejected_material(
            "stale proof-input material version",
            stale_proof_input_version,
        );

        let mut stale_trace_digest = material.clone();
        stale_trace_digest.arithmetic_trace_material_digest =
            iroha_crypto::Hash::new(b"stale BFV arithmetic trace material digest");
        assert_rejected_material("stale arithmetic trace material digest", stale_trace_digest);

        let mut stale_air_evaluation_digest = material.clone();
        stale_air_evaluation_digest.arithmetic_air_evaluation_material_digest =
            iroha_crypto::Hash::new(b"stale BFV arithmetic AIR evaluation material digest");
        assert_rejected_material(
            "stale arithmetic AIR evaluation material digest",
            stale_air_evaluation_digest,
        );

        let mut drifted_trace_rows = material.clone();
        drifted_trace_rows.arithmetic_trace_material.rows[0][0] ^= 0x01;
        assert_rejected_material("drifted arithmetic trace rows", drifted_trace_rows);

        let mut drifted_composition_values = material.clone();
        drifted_composition_values
            .arithmetic_air_evaluation_material
            .composition_values[0] ^= 0x01;
        assert_rejected_material(
            "drifted arithmetic AIR composition values",
            drifted_composition_values,
        );

        let mut swapped_proof_keys = material;
        core::mem::swap(
            &mut swapped_proof_keys.prover_key,
            &mut swapped_proof_keys.verifier_key,
        );
        assert_rejected_material("swapped BFV native proof keys", swapped_proof_keys);
    }

    #[test]
    fn bfv_full_bootstrap_air_rejects_valid_cross_statement_material_replay() {
        let material = bfv_full_bootstrap_stark_test_prover_input_material_for_slot(0);
        let alternate_material = bfv_full_bootstrap_stark_test_prover_input_material_for_slot(1);
        iroha_crypto::validate_bfv_full_bootstrap_execution_prover_input_material_v1(
            &alternate_material,
        )
        .expect("alternate BFV prover material is internally valid");
        assert_ne!(
            material.proof_input_material.statement_hash,
            alternate_material.proof_input_material.statement_hash,
            "alternate slot must bind a distinct BFV statement hash"
        );
        assert_ne!(
            material.arithmetic_trace_material_digest,
            alternate_material.arithmetic_trace_material_digest,
            "alternate slot must bind distinct trace material"
        );
        assert_ne!(
            material.arithmetic_air_evaluation_material_digest,
            alternate_material.arithmetic_air_evaluation_material_digest,
            "alternate slot must bind distinct AIR evaluation material"
        );

        let bytes = prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&material)
            .expect("BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &bytes, &material
        ));
        let alternate_bytes =
            prove_stark_fri_bfv_full_bootstrap_air_envelope_bytes(&alternate_material)
                .expect("alternate BFV full-bootstrap STARK AIR envelope");
        assert!(verify_stark_fri_bfv_full_bootstrap_air_envelope(
            &alternate_bytes,
            &alternate_material,
        ));
        assert_ne!(
            bytes, alternate_bytes,
            "statement-specific BFV native AIR envelopes must differ"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&bytes, &alternate_material),
            "valid BFV native AIR envelope must not replay against another valid statement package"
        );
        assert!(
            !verify_stark_fri_bfv_full_bootstrap_air_envelope(&alternate_bytes, &material),
            "alternate BFV native AIR envelope must not replay against the original statement package"
        );
    }

    fn zk_ace_test_account(seed: u8) -> iroha_data_model::account::AccountId {
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair");
        iroha_data_model::account::AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn zk_ace_test_account_uses_checked_seed_derivation() {
        assert_ne!(zk_ace_test_account(1), zk_ace_test_account(2));
        assert!(
            iroha_crypto::KeyPair::try_from_seed(vec![0; 32], iroha_crypto::Algorithm::Ed25519)
                .is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }

    fn zk_ace_test_asset_definition_id() -> iroha_data_model::asset::AssetDefinitionId {
        iroha_data_model::asset::AssetDefinitionId::new(
            iroha_data_model::domain::DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn zk_ace_test_public_inputs_and_witness() -> (
        iroha_data_model::zk::ZkAcePublicInputsV1,
        iroha_data_model::zk::ZkAceWitnessV1,
    ) {
        let witness = iroha_data_model::zk::ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let policy_hash = [0x44; 32];
        let chain_id: iroha_data_model::ChainId = "zk-ace-test-chain".parse().expect("chain id");
        let from = zk_ace_test_account(1);
        let to = zk_ace_test_account(2);
        let asset = zk_ace_test_asset_definition_id();
        let amount = 17;
        let verifier_key_id = iroha_data_model::proof::VerifyingKeyId::new(
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        );
        let identity_commitment = iroha_data_model::zk::derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = iroha_data_model::zk::derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            amount,
            &chain_id,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = iroha_data_model::zk::derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &tx_digest,
            &chain_id,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let public_inputs = iroha_data_model::zk::ZkAcePublicInputsV1::transparent_transfer(
            identity_commitment,
            tx_digest,
            chain_id,
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            amount,
            verifier_key_id,
        );
        (public_inputs, witness)
    }

    #[test]
    fn zk_ace_air_prover_self_verifies_generated_envelope() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:zk-ace-air-self-verify".to_owned(),
        };
        let (public_inputs, witness) = zk_ace_test_public_inputs_and_witness();
        let public_digest = iroha_data_model::zk::derive_zk_ace_air_public_digest(&public_inputs)
            .expect("ZK-ACE public AIR digest");
        let bytes = prove_stark_fri_zk_ace_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-ZK-ACE-AIR".to_owned(),
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
            public_digest,
            &public_inputs,
            &witness,
        )
        .expect("ZK-ACE AIR envelope");
        assert!(verify_stark_fri_zk_ace_envelope_with_limits(
            &bytes,
            &StarkVerifierLimits::default(),
            &public_inputs,
        ));
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode ZK-ACE AIR envelope");
        let air = envelope.proof.air.as_ref().expect("AIR section");
        let extra_query_roots = [air.trace_root, air.composition_root, air.public_digest];
        let indices = validate_stark_fri_query_shape_and_indices_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
        )
        .expect("ZK-ACE prover emits replayable duplicate-free queries");
        let domain = 1_usize << usize::from(params.n_log2);
        assert!(
            indices
                .iter()
                .all(|&index| zk_ace_air_opening_is_safe(index, domain)),
            "ZK-ACE prover must not open private witness rows"
        );
        let mut auxiliary_envelope = envelope.clone();
        attach_valid_auxiliary_composition_values(&mut auxiliary_envelope);
        let auxiliary_bytes =
            norito::to_bytes(&auxiliary_envelope).expect("encode auxiliary ZK-ACE AIR envelope");
        assert!(
            !verify_stark_fri_zk_ace_envelope_with_limits(
                &auxiliary_bytes,
                &StarkVerifierLimits::default(),
                &public_inputs,
            ),
            "ZK-ACE AIR must reject auxiliary generic composition commitments"
        );

        envelope.proof.air.as_mut().expect("AIR section").circuit_id =
            "stark/fri/zk-ace-pq-authorization-v0:wrong".to_owned();
        let wrong_circuit =
            norito::to_bytes(&envelope).expect("encode wrong-circuit ZK-ACE AIR envelope");
        assert!(
            !verify_stark_fri_zk_ace_envelope_with_limits(
                &wrong_circuit,
                &StarkVerifierLimits::default(),
                &public_inputs,
            ),
            "ZK-ACE AIR verification must bind the canonical circuit id"
        );

        let err = prove_stark_fri_zk_ace_air_envelope_bytes(
            params,
            "IROHA-TEST-ZK-ACE-AIR-WRONG-CIRCUIT".to_owned(),
            "stark/fri/zk-ace-pq-authorization-v0:wrong".to_owned(),
            public_digest,
            &public_inputs,
            &witness,
        )
        .expect_err("ZK-ACE AIR prover must reject wrong circuit ids");
        assert!(
            err.contains("circuit_id"),
            "wrong-circuit ZK-ACE AIR error should mention circuit_id, got: {err}"
        );
    }

    #[test]
    fn zk_ace_air_prover_rejects_repeated_query_schedule() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 2,
            blowup_log2: 1,
            fold_arity: 2,
            queries: 5,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:zk-ace-repeated-query".to_owned(),
        };
        let (public_inputs, witness) = zk_ace_test_public_inputs_and_witness();
        let public_digest = iroha_data_model::zk::derive_zk_ace_air_public_digest(&public_inputs)
            .expect("ZK-ACE public AIR digest");
        let err = prove_stark_fri_zk_ace_air_envelope_bytes(
            params,
            "IROHA-TEST-ZK-ACE-REPEATED-QUERY".to_owned(),
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.to_owned(),
            public_digest,
            &public_inputs,
            &witness,
        )
        .expect_err("pigeonhole-small ZK-ACE query schedule must not emit proof bytes");
        assert!(
            err.contains("STARK query count exceeds domain size"),
            "unexpected impossible-query rejection: {err}"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_unsupported_fold_arity() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 4,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:invalid".to_owned(),
        };
        let err = synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned())
            .expect_err("unsupported fold_arity must fail");
        assert!(
            err.contains("fold_arity"),
            "error should mention fold_arity, got: {err}"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_blowup_larger_than_domain() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 5,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:invalid-blowup-domain".to_owned(),
        };
        let err = synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned())
            .expect_err("blowup_log2 larger than n_log2 must fail");
        assert!(
            err.contains("blowup factor"),
            "error should mention blowup factor, got: {err}"
        );
    }

    #[test]
    fn synthesized_envelope_rejects_tampered_domain_tag() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:tamper".to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params,
            "IROHA-TEST-STARK".to_owned(),
            "stark/fri/sha256-goldilocks:tamper".to_owned(),
            [0x33; 32],
        )
        .expect("ok");
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode synthesized envelope");
        envelope.params.domain_tag.push_str(":mutated");
        let tampered = norito::to_bytes(&envelope).expect("encode mutated envelope");
        assert!(!verify_stark_fri_envelope(&tampered));
    }

    #[test]
    fn synthesized_envelope_rejects_malformed_payload() {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
            domain_tag: "iroha:test:malformed".to_owned(),
        };
        let mut bytes =
            synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned()).expect("ok");
        bytes.truncate(bytes.len().saturating_sub(1));
        assert!(!verify_stark_fri_envelope(&bytes));
    }
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

/// Validate that a STARK/FRI verifier-key payload uses ledger-grade verifier parameters.
///
/// This is a control-plane floor for proof-system admission. It rejects historical
/// PoC-sized STARK/FRI parameters while still leaving circuit-specific algebraic
/// validation to the verifier for each proof.
pub fn validate_stark_fri_production_verifying_key_payload(
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
    if payload.n_log2 < ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2 {
        return Err(format!(
            "{label} STARK/FRI n_log2 {} is below production floor {}",
            payload.n_log2, ZK_ACE_STARK_FRI_PRODUCTION_MIN_N_LOG2
        ));
    }
    if payload.blowup_log2 < ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2 {
        return Err(format!(
            "{label} STARK/FRI blowup_log2 {} is below production floor {}",
            payload.blowup_log2, ZK_ACE_STARK_FRI_PRODUCTION_MIN_BLOWUP_LOG2
        ));
    }
    if payload.blowup_log2 > payload.n_log2 {
        return Err(format!(
            "{label} STARK/FRI blowup_log2 {} exceeds n_log2 {}",
            payload.blowup_log2, payload.n_log2
        ));
    }
    if payload.queries < ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES {
        return Err(format!(
            "{label} STARK/FRI queries {} is below production floor {}",
            payload.queries, ZK_ACE_STARK_FRI_PRODUCTION_MIN_QUERIES
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

/// Validate that a STARK/FRI verifier-key payload is acceptable for ledger-grade ZK-ACE.
///
/// This is a control-plane floor, not a full algebraic soundness proof. It prevents
/// governance or recovery paths from activating the historical PoC-sized ZK-ACE
/// parameters for fresh ledger admission.
pub fn validate_zk_ace_stark_fri_verifying_key_payload(
    payload: &StarkFriVerifyingKeyV1,
) -> Result<(), String> {
    validate_stark_fri_production_verifying_key_payload(
        payload,
        iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        "ZK-ACE",
    )?;
    if payload.hash_fn != STARK_HASH_SHA256_V1 {
        return Err("ZK-ACE STARK/FRI verifier key must use SHA-256".to_owned());
    }
    Ok(())
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

fn zk_ace_air_trace_width() -> usize {
    31
}

const ZK_ACE_AIR_PRIVATE_ROW_INDEX: usize = 0;
const ZK_ACE_AIR_SAFE_ROW_KIND: u64 = 0;
const ZK_ACE_AIR_PRIVATE_ROW_KIND: u64 = 1;
const ZK_ACE_AIR_PUBLIC_DIGEST_OFFSET: usize = 2;
const ZK_ACE_AIR_WIDTH_OFFSET: usize = 6;
const ZK_ACE_AIR_WITNESS_OFFSET: usize = 7;
const ZK_ACE_AIR_WITNESS_LIMBS: usize = 15;
const ZK_ACE_AIR_MAX_BLINDING_ATTEMPTS: u64 = 256;

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
    ZkAce {
        public_inputs: &'a iroha_data_model::zk::ZkAcePublicInputsV1,
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
            Self::ZkAce { .. } => zk_ace_air_trace_width(),
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

fn zk_ace_bytes_to_limbs(bytes: &[u8; 32]) -> Option<[u64; 5]> {
    let packed = iroha_data_model::zk::zk_ace_pack_bytes_to_field_limbs(bytes);
    let limbs: [u64; 5] = packed.limbs.try_into().ok()?;
    if limbs[..4].iter().any(|limb| *limb >= (1u64 << 56)) || limbs[4] >= (1u64 << 32) {
        return None;
    }
    Some(limbs)
}

fn zk_ace_limbs_to_bytes(limbs: &[u64]) -> Option<[u8; 32]> {
    let limbs: &[u64; 5] = limbs.try_into().ok()?;
    if limbs[..4].iter().any(|limb| *limb >= (1u64 << 56)) || limbs[4] >= (1u64 << 32) {
        return None;
    }
    let mut out = [0u8; 32];
    let mut offset = 0usize;
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = limb.to_le_bytes();
        let take = if index == 4 { 4 } else { 7 };
        out[offset..offset + take].copy_from_slice(&bytes[..take]);
        offset += take;
    }
    Some(out)
}

fn zk_ace_witness_limbs(witness: &iroha_data_model::zk::ZkAceWitnessV1) -> Option<[u64; 15]> {
    let root = zk_ace_bytes_to_limbs(&witness.identity_root)?;
    let blinding = zk_ace_bytes_to_limbs(&witness.identity_blinding)?;
    let replay = zk_ace_bytes_to_limbs(&witness.replay_secret)?;
    let mut limbs = [0u64; 15];
    limbs[..5].copy_from_slice(&root);
    limbs[5..10].copy_from_slice(&blinding);
    limbs[10..].copy_from_slice(&replay);
    Some(limbs)
}

fn zk_ace_air_blinding(
    statement_digest: &[u8; 32],
    index: usize,
    column_index: usize,
    blinding_attempt: u64,
) -> Option<Fq> {
    let mut h = Sha256::new();
    h.update(b"iroha:zk-ace:air-blinding:v2");
    h.update(statement_digest);
    h.update(&(index as u64).to_le_bytes());
    h.update(&(column_index as u64).to_le_bytes());
    h.update(&blinding_attempt.to_le_bytes());
    let out = h.finalize();
    let mut word = [0u8; 8];
    word.copy_from_slice(&out[..8]);
    Some(Fq::new(u64::from_le_bytes(word)))
}

fn zk_ace_air_row(
    index: usize,
    witness_limbs: &[u64; 15],
    public_digest: &[u8; 32],
    statement_digest: &[u8; 32],
    blinding_attempt: u64,
) -> Option<Vec<u64>> {
    let width = zk_ace_air_trace_width();
    let mut row = Vec::with_capacity(width);
    row.push((u128::from(u64::try_from(index).ok()?) % MOD_P) as u64);
    row.push(if index == ZK_ACE_AIR_PRIVATE_ROW_INDEX {
        ZK_ACE_AIR_PRIVATE_ROW_KIND
    } else {
        ZK_ACE_AIR_SAFE_ROW_KIND
    });
    row.extend_from_slice(&stark_air_digest_limbs(public_digest));
    row.push(u64::try_from(width).ok()?);
    if index == ZK_ACE_AIR_PRIVATE_ROW_INDEX {
        row.extend_from_slice(witness_limbs);
    }
    while row.len() < width {
        let value = zk_ace_air_blinding(statement_digest, index, row.len(), blinding_attempt)?;
        row.push(value.0);
    }
    if row.len() != width {
        return None;
    }
    Some(row)
}

fn zk_ace_air_row_residue(
    index: usize,
    public_digest: &[u8; 32],
    public_inputs: &iroha_data_model::zk::ZkAcePublicInputsV1,
    row: &[u64],
) -> Option<Fq> {
    if row.len() != zk_ace_air_trace_width() {
        return None;
    }
    for value in row {
        Fq::from_canonical_u64(*value)?;
    }
    let mut acc = Fq::zero();
    let mut coeff = Fq::from_canonical_u64(3)?;
    let add_residue = |acc: &mut Fq, coeff: &mut Fq, actual: u64, expected: u64| -> Option<()> {
        let residue = Fq::from_canonical_u64(actual)?.sub(Fq::from_canonical_u64(expected)?);
        *acc = acc.add((*coeff).mul(residue));
        *coeff = coeff.add(Fq::from_canonical_u64(2)?);
        Some(())
    };

    let index = (u128::from(u64::try_from(index).ok()?) % MOD_P) as u64;
    add_residue(&mut acc, &mut coeff, row[0], index)?;
    let expected_kind = if usize::try_from(index).ok()? == ZK_ACE_AIR_PRIVATE_ROW_INDEX {
        ZK_ACE_AIR_PRIVATE_ROW_KIND
    } else {
        ZK_ACE_AIR_SAFE_ROW_KIND
    };
    add_residue(&mut acc, &mut coeff, row[1], expected_kind)?;
    for (offset, expected) in stark_air_digest_limbs(public_digest).iter().enumerate() {
        add_residue(
            &mut acc,
            &mut coeff,
            row[ZK_ACE_AIR_PUBLIC_DIGEST_OFFSET + offset],
            *expected,
        )?;
    }
    add_residue(
        &mut acc,
        &mut coeff,
        row[ZK_ACE_AIR_WIDTH_OFFSET],
        u64::try_from(zk_ace_air_trace_width()).ok()?,
    )?;

    let expected_air_digest =
        iroha_data_model::zk::derive_zk_ace_air_public_digest(public_inputs).ok()?;
    for (actual, expected) in public_digest.iter().zip(expected_air_digest.iter()) {
        add_residue(
            &mut acc,
            &mut coeff,
            u64::from(*actual),
            u64::from(*expected),
        )?;
    }

    if expected_kind == ZK_ACE_AIR_SAFE_ROW_KIND {
        return Some(acc);
    }

    let witness_end = ZK_ACE_AIR_WITNESS_OFFSET + ZK_ACE_AIR_WITNESS_LIMBS;
    let limbs: [u64; ZK_ACE_AIR_WITNESS_LIMBS] = row[ZK_ACE_AIR_WITNESS_OFFSET..witness_end]
        .try_into()
        .ok()?;
    let identity_root = zk_ace_limbs_to_bytes(&limbs[..5])?;
    let identity_blinding = zk_ace_limbs_to_bytes(&limbs[5..10])?;
    let replay_secret = zk_ace_limbs_to_bytes(&limbs[10..15])?;
    for witness_bytes in [&identity_root, &identity_blinding, &replay_secret] {
        if witness_bytes == &[0u8; 32] {
            acc = acc.add(coeff);
        }
        coeff = coeff.add(Fq::from_canonical_u64(2)?);
    }

    let identity_commitment = iroha_data_model::zk::derive_zk_ace_identity_commitment(
        &identity_root,
        &identity_blinding,
        &public_inputs.domain_tag,
    );
    for (actual, expected) in identity_commitment
        .iter()
        .zip(public_inputs.identity_commitment.iter())
    {
        add_residue(
            &mut acc,
            &mut coeff,
            u64::from(*actual),
            u64::from(*expected),
        )?;
    }

    let replay_nullifier = iroha_data_model::zk::derive_zk_ace_replay_nullifier(
        &replay_secret,
        &public_inputs.tx_digest,
        &public_inputs.chain_id,
        &public_inputs.action_class,
        &public_inputs.domain_tag,
    );
    for (actual, expected) in replay_nullifier
        .iter()
        .zip(public_inputs.replay_nullifier.iter())
    {
        add_residue(
            &mut acc,
            &mut coeff,
            u64::from(*actual),
            u64::from(*expected),
        )?;
    }

    let tx_digest = iroha_data_model::zk::derive_zk_ace_transfer_digest(
        &public_inputs.from,
        &public_inputs.to,
        &public_inputs.asset,
        public_inputs.amount,
        &public_inputs.chain_id,
        public_inputs.action_class.trim(),
        &public_inputs.policy_hash,
    );
    for (actual, expected) in tx_digest.iter().zip(public_inputs.tx_digest.iter()) {
        add_residue(
            &mut acc,
            &mut coeff,
            u64::from(*actual),
            u64::from(*expected),
        )?;
    }

    if public_inputs.identity_commitment == [0u8; 32]
        || public_inputs.replay_nullifier == [0u8; 32]
        || public_inputs.policy_hash == [0u8; 32]
        || public_inputs.domain_tag.trim().is_empty()
        || public_inputs.action_class.trim().is_empty()
    {
        acc = acc.add(coeff);
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
        StarkAirVerificationContext::ZkAce { public_inputs } => {
            if domain_size == 0
                || row.len() != zk_ace_air_trace_width()
                || next_row.len() != zk_ace_air_trace_width()
            {
                return None;
            }
            let current = zk_ace_air_row_residue(index, public_digest, public_inputs, row)?;
            let next = zk_ace_air_row_residue(
                (index + 1) % domain_size,
                public_digest,
                public_inputs,
                next_row,
            )?;
            Some(current.add(Fq::from_canonical_u64(17)?.mul(next)))
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
        StarkAirVerificationContext::Binding => true,
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
        StarkAirVerificationContext::ZkAce { .. } => {
            air.circuit_id == iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID
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

fn zk_ace_air_opening_is_private(index: usize, domain_size: usize) -> bool {
    domain_size != 0 && index % domain_size == ZK_ACE_AIR_PRIVATE_ROW_INDEX
}

fn zk_ace_air_opening_is_safe(index: usize, domain_size: usize) -> bool {
    if domain_size == 0 {
        return false;
    }
    !zk_ace_air_opening_is_private(index, domain_size)
        && !zk_ace_air_opening_is_private((index + 1) % domain_size, domain_size)
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
    let base = iroha_crypto::BFV_FULL_BOOTSTRAP_NATIVE_STARK_AIR_TRANSCRIPT_LABEL_V1;
    if label == base {
        return true;
    }
    let Some(suffix) = label
        .strip_prefix(base)
        .and_then(|value| value.strip_prefix(':'))
    else {
        return false;
    };
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    suffix.parse::<u32>().is_ok_and(|attempt| {
        attempt > 0
            && attempt < BFV_FULL_BOOTSTRAP_STARK_AIR_TRANSCRIPT_LABEL_ATTEMPTS
            && suffix == attempt.to_string()
    })
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
pub(crate) fn prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    rows: Vec<Vec<u64>>,
    composition_values: Vec<u64>,
) -> Result<Vec<u8>, String> {
    validate_stark_circuit_id(&circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
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
        None,
    )
}

pub(crate) fn prove_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
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
        Some(base_indices),
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
    let bytes = norito::to_bytes(&envelope)
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
    prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
        params,
        transcript_label,
        circuit_id,
        public_digest,
        rows,
        composition_values
            .into_iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
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
/// hash and retries deterministic transcript labels until all sampled openings
/// are public padding rows. Public production proof generation must use the
/// Soracloud release/audit-aware entry points, which validate caller-owned
/// governed artifacts before reaching this crate-scoped helper.
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
        prove_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
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

/// Verify a BFV full-bootstrap native STARK/FRI AIR proof envelope with limits.
///
/// Generic STARK verification is only the first stage. This BFV wrapper also
/// requires the exact first-release BFV STARK parameters, the statement-bound
/// domain tag, the canonical BFV circuit id, the statement hash as the public
/// digest, the canonical opening count, and sampled public-padding rows that
/// match the BFV statement header.
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

/// Build a deterministic V1 STARK/FRI envelope for the ZK-ACE authorization AIR.
pub fn prove_stark_fri_zk_ace_air_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
    public_inputs: &iroha_data_model::zk::ZkAcePublicInputsV1,
    witness: &iroha_data_model::zk::ZkAceWitnessV1,
) -> Result<Vec<u8>, String> {
    validate_stark_circuit_id(&circuit_id)
        .map_err(|err| format!("invalid STARK AIR circuit_id: {err}"))?;
    if circuit_id != iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID {
        return Err("ZK-ACE AIR circuit_id mismatch".to_owned());
    }
    validate_stark_prover_params(&params, &transcript_label)?;
    let expected_public_digest =
        iroha_data_model::zk::derive_zk_ace_air_public_digest(public_inputs)
            .map_err(|err| format!("failed to derive ZK-ACE AIR public digest: {err}"))?;
    if public_digest != expected_public_digest {
        return Err("ZK-ACE AIR public digest mismatch".to_owned());
    }
    let statement_digest =
        iroha_data_model::zk::derive_zk_ace_air_statement_digest(public_inputs, witness)
            .map_err(|err| format!("failed to derive ZK-ACE AIR statement digest: {err}"))?;
    let witness_limbs = zk_ace_witness_limbs(witness)
        .ok_or_else(|| "failed to pack ZK-ACE witness limbs".to_owned())?;
    let domain = 1usize
        .checked_shl(u32::from(params.n_log2))
        .ok_or_else(|| "STARK domain size overflow".to_owned())?;
    let context = StarkAirVerificationContext::ZkAce { public_inputs };

    for blinding_attempt in 0..ZK_ACE_AIR_MAX_BLINDING_ATTEMPTS {
        let rows = (0..domain)
            .map(|index| {
                zk_ace_air_row(
                    index,
                    &witness_limbs,
                    &public_digest,
                    &statement_digest,
                    blinding_attempt,
                )
                .ok_or_else(|| "failed to build ZK-ACE AIR row".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let trace_leaves = rows
            .iter()
            .map(|row| {
                stark_air_trace_leaf_hash(&params, row)
                    .ok_or_else(|| "failed to hash ZK-ACE AIR row".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let trace_levels = merkle_levels_from_hashes(&params, trace_leaves)
            .ok_or_else(|| "failed to build ZK-ACE AIR trace commitment".to_owned())?;
        let trace_root = merkle_root_from_levels(&trace_levels)
            .ok_or_else(|| "failed to derive ZK-ACE AIR trace root".to_owned())?;

        let composition_values = (0..domain)
            .map(|index| {
                stark_air_composition_value_for_context(
                    context,
                    index,
                    domain,
                    &public_digest,
                    &rows[index],
                    &rows[(index + 1) % domain],
                )
                .ok_or_else(|| "failed to evaluate ZK-ACE AIR composition".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let composition_levels = merkle_levels_from_values(&params, &composition_values)
            .ok_or_else(|| "failed to build ZK-ACE AIR composition commitment".to_owned())?;
        let composition_root = merkle_root_from_levels(&composition_levels)
            .ok_or_else(|| "failed to derive ZK-ACE AIR composition root".to_owned())?;
        let extra_query_roots = [trace_root, composition_root, public_digest];

        let mut envelope = match synthesize_stark_fri_envelope_from_values(
            params.clone(),
            transcript_label.clone(),
            composition_values,
            &extra_query_roots,
        ) {
            Ok(envelope) => envelope,
            Err(err) if err == STARK_FRI_QUERY_INDEX_REPEATED_ERROR => continue,
            Err(err) => return Err(err),
        };
        if envelope.proof.commits.roots.first().copied() != Some(composition_root) {
            return Err("ZK-ACE AIR composition root does not match FRI base root".to_owned());
        }

        let query_indices = match validate_stark_fri_query_shape_and_indices_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
        ) {
            Ok(indices) => indices,
            Err(STARK_FRI_QUERY_INDEX_REPEATED_ERROR) => continue,
            Err(err) => {
                return Err(format!(
                    "ZK-ACE AIR FRI query shape failed validation: {err}"
                ));
            }
        };
        if query_indices
            .iter()
            .any(|&index| !zk_ace_air_opening_is_safe(index, domain))
        {
            continue;
        }

        let mut openings = Vec::with_capacity(envelope.proof.queries.len());
        for index in query_indices {
            let next_index = (index + 1) % domain;
            let row_path = merkle_path_from_levels(index, &trace_levels)
                .ok_or_else(|| "failed to open ZK-ACE AIR row".to_owned())?;
            let next_row_path = merkle_path_from_levels(next_index, &trace_levels)
                .ok_or_else(|| "failed to open next ZK-ACE AIR row".to_owned())?;
            let composition_path = merkle_path_from_levels(index, &composition_levels)
                .ok_or_else(|| "failed to open ZK-ACE AIR composition".to_owned())?;
            let composition_value = stark_air_composition_value_for_context(
                context,
                index,
                domain,
                &public_digest,
                &rows[index],
                &rows[next_index],
            )
            .ok_or_else(|| "failed to evaluate opened ZK-ACE AIR composition".to_owned())?;
            openings.push(StarkAirOpeningV1 {
                index: u32::try_from(index)
                    .map_err(|_| "ZK-ACE AIR query index does not fit u32".to_owned())?,
                row: rows[index].clone(),
                next_row: rows[next_index].clone(),
                row_path,
                next_row_path,
                composition_value: composition_value.0,
                composition_path,
            });
        }
        envelope.proof.air = Some(StarkAirProofV1 {
            version: 1,
            circuit_id: circuit_id.clone(),
            public_digest,
            trace_root,
            composition_root,
            trace_width: u16::try_from(zk_ace_air_trace_width())
                .map_err(|_| "ZK-ACE AIR trace width does not fit u16".to_owned())?,
            openings,
        });
        let bytes = norito::to_bytes(&envelope)
            .map_err(|err| format!("failed to encode STARK envelope: {err}"))?;
        let mut limits = StarkVerifierLimits::default();
        limits.max_envelope_bytes = usize::MAX;
        if !verify_stark_fri_zk_ace_envelope_with_limits(&bytes, &limits, public_inputs) {
            return Err("ZK-ACE AIR envelope self-verification failed".to_owned());
        }
        return Ok(bytes);
    }

    Err("failed to derive ZK-ACE AIR blinding with duplicate-free public query openings".to_owned())
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
    norito::to_bytes(&envelope).map_err(|err| format!("failed to encode STARK envelope: {err}"))
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

/// Verify a ZK-ACE STARK FRI envelope under `zk-stark` with caller-provided limits.
pub fn verify_stark_fri_zk_ace_envelope_with_limits(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    public_inputs: &iroha_data_model::zk::ZkAcePublicInputsV1,
) -> bool {
    verify_stark_fri_envelope_with_context(
        bytes,
        limits,
        StarkAirVerificationContext::ZkAce { public_inputs },
    )
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
    // Decode envelope
    let env: StarkVerifyEnvelopeV1 = match norito::decode_from_bytes(bytes) {
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
