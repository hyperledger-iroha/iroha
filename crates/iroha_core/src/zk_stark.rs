//! Native STARK/FRI (binary folding) verifier used by the `stark/fri-v1/*` backends.
//!
//! This module provides a deterministic, dependency-light verifier over the Goldilocks
//! prime field using SHA-256 for transcripts and Merkle commitments. It implements a
//! multi-round binary FRI consistency check.
//!
//! The wire format is defined with Norito. The proof envelope carries params, Merkle
//! roots, and query decommitments. Verification replays the transcript and checks:
//! - Merkle openings for each queried value
//! - The fold relation `z = y0 + r*y1` for each round and query
//! - Optional composition leaf constraints when `comp_root` is present
//!
//! Size and structural limits are enforced to reject oversized or malformed payloads
//! deterministically (see [`StarkVerifierLimits`]).

#![allow(clippy::needless_pass_by_value)]

use sha2::{Digest, Sha256};

/// Goldilocks prime modulus p = 2^64 - 2^32 + 1
const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;
const MOD_P_U64: u64 = MOD_P as u64;

/// Supported hash selector for the STARK envelope.
pub const STARK_HASH_SHA256_V1: u8 = 1;
/// Reserved selector for a Poseidon2 transcript; not yet supported by the native verifier.
pub const STARK_HASH_POSEIDON2_V1: u8 = 2;

const MAX_DOMAIN_LOG2: u8 = 24;
const MAX_FRI_LAYERS: usize = 32;
const MAX_FRI_QUERIES: usize = 32;
const MAX_MERKLE_DEPTH: usize = 32;
const MAX_AUX_TERMS: usize = 64;
const MAX_DOMAIN_TAG_LEN: usize = 64;
const MAX_TRANSCRIPT_LABEL_LEN: usize = 128;
const MAX_ENVELOPE_BYTES: usize = 1 << 20; // 1 MiB guard for decoded envelopes

/// Tunable limits applied during STARK envelope verification to prevent denial-of-service inputs.
#[derive(Clone, Copy, Debug)]
pub struct StarkVerifierLimits {
    /// Maximum supported domain log2.
    pub max_domain_log2: u8,
    /// Maximum supported blowup log2.
    pub max_blowup_log2: u8,
    /// Maximum fold arity.
    pub max_fold_arity: u8,
    /// Maximum number of queries.
    pub max_queries: usize,
    /// Maximum Merkle depth.
    pub max_merkle_depth: usize,
    /// Maximum auxiliary terms in composition leaf.
    pub max_aux_terms: usize,
    /// Maximum domain tag length.
    pub max_domain_tag_len: usize,
    /// Maximum transcript label length.
    pub max_transcript_label_len: usize,
    /// Maximum encoded envelope size in bytes (decoded input slice length).
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
            max_domain_tag_len: MAX_DOMAIN_TAG_LEN,
            max_transcript_label_len: MAX_TRANSCRIPT_LABEL_LEN,
            max_envelope_bytes: MAX_ENVELOPE_BYTES,
        }
    }
}

/// Goldilocks field element with canonical modular reduction.
///
/// This backend keeps values in the range `[0, MOD_P)` and implements the
/// minimal arithmetic needed by the test-only STARK verifier. Although kept
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

    #[cfg(test)]
    fn zero() -> Self {
        Self(0)
    }

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    #[cfg(test)]
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

/// Transcript helper: derive a 64-bit field element challenge from label+bytes.
fn challenge(params: &StarkFriParamsV1, label: &str, bytes: &[u8]) -> Option<Fq> {
    if params.hash_fn != STARK_HASH_SHA256_V1 {
        return None;
    }
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

/// Verify a Merkle inclusion proof for a leaf value to `root`.
fn merkle_verify(root: &[u8; 32], leaf: Fq, path: &MerklePath) -> bool {
    let mut acc = leaf_hash(leaf);
    for (i, sib) in path.siblings.iter().enumerate() {
        let byte = i / 8;
        if byte >= path.dirs.len() {
            return false;
        }
        let dir_bit = (path.dirs[byte] >> (i % 8)) & 1; // 0: leaf on left, 1: leaf on right
        acc = if dir_bit == 0 {
            node_hash(&acc, sib)
        } else {
            node_hash(sib, &acc)
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
    if expected_depth > limits.max_merkle_depth || path.siblings.len() != expected_depth {
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
    if params.version != 1 || params.n_log2 == 0 || params.n_log2 > limits.max_domain_log2 {
        return None;
    }
    if params.blowup_log2 == 0 || params.blowup_log2 > limits.max_blowup_log2 {
        return None;
    }
    // The current wire format (`FoldDecommitV1`) carries a binary fold (y0,y1),
    // so only `fold_arity = 2` is supported by the native verifier.
    if params.fold_arity != 2 || params.fold_arity > limits.max_fold_arity {
        return None;
    }
    if params.merkle_arity != 2 || params.hash_fn != STARK_HASH_SHA256_V1 {
        return None;
    }
    if params.domain_tag.is_empty() || params.domain_tag.len() > limits.max_domain_tag_len {
        return None;
    }
    if params.queries == 0
        || params.queries as usize > limits.max_queries
        || params.queries as usize != query_count
    {
        return None;
    }
    if roots_len == 0 || roots_len > limits.max_merkle_depth + 1 {
        return None;
    }
    let required_layers = layers_required(params)?;
    if roots_len != required_layers + 1 {
        return None;
    }
    Some(required_layers)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

fn derive_query_index(
    label: &str,
    params: &StarkFriParamsV1,
    roots: &[[u8; 32]],
    query_idx: usize,
) -> Option<usize> {
    if params.hash_fn != STARK_HASH_SHA256_V1 || params.n_log2 as u32 >= usize::BITS {
        return None;
    }
    let domain = 1usize << params.n_log2;
    if domain == 0 {
        return None;
    }
    let mut h = Sha256::new();
    h.update(b"STARK:query-index");
    h.update(label.as_bytes());
    h.update(&params.version.to_le_bytes());
    h.update(&[
        params.n_log2,
        params.blowup_log2,
        params.fold_arity,
        params.merkle_arity,
        params.hash_fn,
    ]);
    h.update(&params.queries.to_le_bytes());
    h.update(&(params.domain_tag.len() as u32).to_le_bytes());
    h.update(params.domain_tag.as_bytes());
    h.update(&(query_idx as u64).to_le_bytes());
    for root in roots {
        h.update(root);
    }
    let digest = h.finalize();
    let mut w = [0u8; 8];
    w.copy_from_slice(&digest[..8]);
    let idx = (u64::from_le_bytes(w) % (domain as u64)) as usize;
    Some(idx)
}

/// Norito-serializable Merkle path (dirs as bitset, siblings as hashes).
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct MerklePath {
    /// Direction bits per level: 0 => leaf/hash on left, 1 => on right
    pub dirs: Vec<u8>,
    /// Sibling hashes from leaf to root (one per level)
    pub siblings: Vec<[u8; 32]>,
}

/// Parameters for a binary multi-round FRI check.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
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
    /// Hash function selector (`1 = SHA-256`, `2 = Poseidon2` reserved)
    pub hash_fn: u8,
    /// Domain tag mixed into transcripts and query sampling
    pub domain_tag: String,
}

/// Commitments for multiple layers and optional composition root.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct StarkCommitmentsV1 {
    /// Version tag for format evolution
    pub version: u16,
    /// Merkle roots per layer, from layer 0 (original evaluations) to layer L (final folded layer)
    pub roots: Vec<[u8; 32]>,
    /// Optional composition polynomial root over the final layer domain (length n >> L)
    pub comp_root: Option<[u8; 32]>,
}

/// Auxiliary term contributing to the composition polynomial evaluation.
#[derive(Debug, Clone, Copy, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct StarkCompositionTermV1 {
    /// Canonical wire index for this auxiliary value (monotonic, caller-defined ordering)
    pub wire_index: u32,
    /// Value contributed by this wire
    pub value: u64,
    /// Coefficient multiplied with the value
    pub coeff: u64,
}

/// Composition leaf data stored under `comp_root`.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
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

/// Decommitment for one fold step at layer `k`.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
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
    /// Folded value at layer k+1: z = y0 + r_k * y1, with Merkle path into root[k+1]
    pub z: u64,
    /// Merkle path for the folded value z in the next layer (k+1)
    pub path_z: MerklePath,
}

/// STARK proof carrying commitments and query decommitments.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
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
}

/// Verification envelope for STARK FRI multi-round (binary) proofs.
#[derive(Debug, Clone, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct StarkVerifyEnvelopeV1 {
    /// Parameters used by the prover
    pub params: StarkFriParamsV1,
    /// Proof object
    pub proof: StarkProofV1,
    /// Transcript label to domain-separate instances
    pub transcript_label: String,
}

/// Verify a STARK FRI envelope under `zk-stark` with caller-provided limits.
pub fn verify_stark_fri_envelope_with_limits(bytes: &[u8], limits: &StarkVerifierLimits) -> bool {
    if bytes.len() > limits.max_envelope_bytes {
        return false;
    }
    // Decode envelope
    let env: StarkVerifyEnvelopeV1 = match norito::decode_from_bytes(bytes) {
        Ok(e) => e,
        Err(_) => return false,
    };
    if env.transcript_label.len() > limits.max_transcript_label_len {
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
    if let Some(values) = env.proof.comp_values.as_ref() {
        if values.len() != query_count {
            return false;
        }
    }
    let total_domain = 1usize << env.params.n_log2;
    if total_domain == 0 || env.params.hash_fn != STARK_HASH_SHA256_V1 {
        return false;
    }
    let fold_arity = env.params.fold_arity as usize;

    for (qi, chain) in env.proof.queries.iter().enumerate() {
        if chain.len() != expected_chain_len {
            return false;
        }
        let base_index = match derive_query_index(&env.transcript_label, &env.params, roots, qi) {
            Some(idx) => idx % total_domain,
            None => return false,
        };
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
            if !merkle_verify(&roots[k], y0, &decommit.path_y0) {
                return false;
            }
            if !merkle_verify(&roots[k], y1, &decommit.path_y1) {
                return false;
            }
            let z = match Fq::from_canonical_u64(decommit.z) {
                Some(v) => v,
                None => return false,
            };
            let zr = y0.add(r_k.mul(y1));
            if zr != z {
                return false;
            }
            if !merkle_verify(&roots[k + 1], z, &decommit.path_z) {
                return false;
            }
            last_z = Some(z);
            layer_domain /= fold_arity;
            idx_layer = expected_j;
        }

        if let (Some(comp_root), Some(cv_all)) =
            (env.proof.commits.comp_root, env.proof.comp_values.as_ref())
        {
            if qi >= cv_all.len() {
                return false;
            }
            let comp_entry = &cv_all[qi];
            if comp_entry.aux_terms.len() > limits.max_aux_terms {
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
            if !merkle_verify(&comp_root, cv_f, &comp_entry.path) {
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
