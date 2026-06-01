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
//! - Optional composition leaf constraints when `comp_root` is present
//!
//! Size and structural limits are enforced to reject oversized or malformed payloads
//! deterministically (see [`StarkVerifierLimits`]).

#![allow(clippy::needless_pass_by_value)]

use fastpq_prover::{hash_field_elements, pack_bytes};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::json_macros::{JsonDeserialize, JsonSerialize};

/// Goldilocks prime modulus p = 2^64 - 2^32 + 1
const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;
const MOD_P_U64: u64 = MOD_P as u64;
const GOLDILOCKS_GENERATOR: u64 = 7;

/// Supported hash selector for the STARK envelope.
pub const STARK_HASH_SHA256_V1: u8 = 1;
/// Selector for a Poseidon2 transcript and Merkle commitments.
pub const STARK_HASH_POSEIDON2_V1: u8 = 2;

const MAX_DOMAIN_LOG2: u8 = 24;
const MAX_FRI_LAYERS: usize = 32;
const MAX_FRI_QUERIES: usize = 32;
const MAX_MERKLE_DEPTH: usize = 32;
const MAX_AUX_TERMS: usize = 64;
const MAX_AIR_WIDTH: usize = 64;
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
    /// Maximum values in a sampled AIR trace row.
    pub max_air_width: usize,
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
            max_air_width: MAX_AIR_WIDTH,
            max_domain_tag_len: MAX_DOMAIN_TAG_LEN,
            max_transcript_label_len: MAX_TRANSCRIPT_LABEL_LEN,
            max_envelope_bytes: MAX_ENVELOPE_BYTES,
        }
    }
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
    if params.merkle_arity != 2 {
        return None;
    }
    if params.hash_fn != STARK_HASH_SHA256_V1 && params.hash_fn != STARK_HASH_POSEIDON2_V1 {
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
    match params.hash_fn {
        STARK_HASH_SHA256_V1 => {
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
            Some((u64::from_le_bytes(w) % (domain as u64)) as usize)
        }
        STARK_HASH_POSEIDON2_V1 => {
            let mut preimage = Vec::new();
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
            preimage.extend_from_slice(&(query_idx as u64).to_le_bytes());
            for root in roots {
                preimage.extend_from_slice(root);
            }
            let packed = pack_bytes(&preimage);
            let len_field = u64::try_from(packed.length).ok()?;
            let mut limbs = Vec::with_capacity(packed.limbs.len() + 1);
            limbs.push(len_field);
            limbs.extend_from_slice(&packed.limbs);
            let v = hash_field_elements(&limbs);
            Some((v % (domain as u64)) as usize)
        }
        _ => None,
    }
}

/// Norito-serializable Merkle path (dirs as bitset, siblings as hashes).
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
)]
pub struct MerklePath {
    /// Direction bits per level: 0 => leaf/hash on left, 1 => on right
    pub dirs: Vec<u8>,
    /// Sibling hashes from leaf to root (one per level)
    pub siblings: Vec<[u8; 32]>,
}

/// Parameters for a binary multi-round FRI check.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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

/// Commitments for multiple layers and optional composition root.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Serialize,
    Deserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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
    Debug,
    Clone,
    Serialize,
    Deserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
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

fn stark_air_trace_width() -> usize {
    6
}

fn zk_ace_air_trace_width() -> usize {
    31
}

#[derive(Clone, Copy)]
enum StarkAirVerificationContext<'a> {
    Binding,
    ZkAce {
        public_inputs: &'a iroha_data_model::zk::ZkAcePublicInputsV1,
    },
}

impl StarkAirVerificationContext<'_> {
    fn trace_width(self) -> usize {
        match self {
            Self::Binding => stark_air_trace_width(),
            Self::ZkAce { .. } => zk_ace_air_trace_width(),
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

fn zk_ace_air_mask(statement_digest: &[u8; 32], index: usize, limb_index: usize) -> Option<Fq> {
    let mut h = Sha256::new();
    h.update(b"iroha:zk-ace:air-mask:v1");
    h.update(statement_digest);
    h.update(&(index as u64).to_le_bytes());
    h.update(&(limb_index as u64).to_le_bytes());
    let out = h.finalize();
    let mut word = [0u8; 8];
    word.copy_from_slice(&out[..8]);
    let mask = u64::from_le_bytes(word);
    let mask = (u128::from(mask) % (MOD_P - 1)) as u64 + 1;
    Fq::from_canonical_u64(mask)
}

fn zk_ace_air_row(
    index: usize,
    witness_limbs: &[u64; 15],
    statement_digest: &[u8; 32],
) -> Option<Vec<u64>> {
    let mut row = Vec::with_capacity(zk_ace_air_trace_width());
    row.push((u128::from(u64::try_from(index).ok()?) % MOD_P) as u64);
    for (limb_index, limb) in witness_limbs.iter().enumerate() {
        let limb = Fq::from_canonical_u64(*limb)?;
        let mask = zk_ace_air_mask(statement_digest, index, limb_index)?;
        row.push(limb.add(mask).0);
    }
    for limb_index in 0..witness_limbs.len() {
        row.push(zk_ace_air_mask(statement_digest, index, limb_index)?.0);
    }
    Some(row)
}

fn zk_ace_unmask_witness_limbs(row: &[u64]) -> Option<[u64; 15]> {
    if row.len() != zk_ace_air_trace_width() {
        return None;
    }
    let mut limbs = [0u64; 15];
    for limb_index in 0..15 {
        let masked = Fq::from_canonical_u64(row[1 + limb_index])?;
        let mask = Fq::from_canonical_u64(row[16 + limb_index])?;
        if mask == Fq::zero() {
            return None;
        }
        limbs[limb_index] = masked.sub(mask).0;
    }
    Some(limbs)
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
    for limb_index in 0..15 {
        let mask = Fq::from_canonical_u64(row[16 + limb_index])?;
        if mask == Fq::zero() {
            acc = acc.add(coeff);
        }
        coeff = coeff.add(Fq::from_canonical_u64(2)?);
    }

    let limbs = zk_ace_unmask_witness_limbs(row)?;
    let identity_root = zk_ace_limbs_to_bytes(&limbs[..5])?;
    let identity_blinding = zk_ace_limbs_to_bytes(&limbs[5..10])?;
    let replay_secret = zk_ace_limbs_to_bytes(&limbs[10..15])?;
    for witness_bytes in [&identity_root, &identity_blinding, &replay_secret] {
        if witness_bytes == &[0u8; 32] {
            acc = acc.add(coeff);
        }
        coeff = coeff.add(Fq::from_canonical_u64(2)?);
    }

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
    }
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
    if params.blowup_log2 == 0 || params.blowup_log2 > MAX_DOMAIN_LOG2 {
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
    if params.domain_tag.is_empty() || params.domain_tag.len() > MAX_DOMAIN_TAG_LEN {
        return Err("invalid STARK domain tag".to_owned());
    }
    let query_count = params.queries as usize;
    if query_count == 0 || query_count > MAX_FRI_QUERIES {
        return Err("invalid STARK query count".to_owned());
    }
    if transcript_label.len() > MAX_TRANSCRIPT_LABEL_LEN {
        return Err("transcript label exceeds maximum length".to_owned());
    }
    let n_log2 = params.n_log2 as usize;
    if n_log2 > MAX_MERKLE_DEPTH {
        return Err("STARK domain depth exceeds verifier limits".to_owned());
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
    let final_levels = merkle_levels_from_values(&params, final_values)
        .ok_or_else(|| "failed to build STARK final FRI Merkle layer".to_owned())?;
    let final_root = merkle_root_from_levels(&final_levels)
        .ok_or_else(|| "failed to derive STARK final FRI root".to_owned())?;
    roots.push(final_root);
    layer_merkle.push(final_levels);

    let mut query_roots = roots.clone();
    query_roots.extend_from_slice(extra_query_roots);

    let query_count = params.queries as usize;
    let mut queries = Vec::with_capacity(query_count);
    for qi in 0..query_count {
        let mut idx_layer = derive_query_index(&transcript_label, &params, &query_roots, qi)
            .ok_or_else(|| "failed to derive STARK query index".to_owned())?;
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

/// Build a deterministic V1 STARK/FRI envelope with an explicit verifier-owned AIR section.
pub fn prove_stark_fri_air_envelope_bytes(
    params: StarkFriParamsV1,
    transcript_label: String,
    circuit_id: String,
    public_digest: [u8; 32],
) -> Result<Vec<u8>, String> {
    if circuit_id.is_empty() || circuit_id.len() > MAX_TRANSCRIPT_LABEL_LEN {
        return Err("invalid STARK AIR circuit_id".to_owned());
    }
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
    let composition_levels = merkle_levels_from_values(&params, &composition_values)
        .ok_or_else(|| "failed to build STARK AIR composition commitment".to_owned())?;
    let composition_root = merkle_root_from_levels(&composition_levels)
        .ok_or_else(|| "failed to derive STARK AIR composition root".to_owned())?;
    let extra_query_roots = [trace_root, composition_root, public_digest];

    let mut envelope = synthesize_stark_fri_envelope_from_values(
        params,
        transcript_label,
        composition_values,
        &extra_query_roots,
    )?;
    if envelope.proof.commits.roots.first().copied() != Some(composition_root) {
        return Err("STARK AIR composition root does not match FRI base root".to_owned());
    }

    let query_roots = stark_air_query_roots(&envelope.proof.commits.roots, None)
        .into_iter()
        .chain(extra_query_roots)
        .collect::<Vec<_>>();
    let mut openings = Vec::with_capacity(envelope.proof.queries.len());
    for qi in 0..envelope.proof.queries.len() {
        let index = derive_query_index(
            &envelope.transcript_label,
            &envelope.params,
            &query_roots,
            qi,
        )
        .ok_or_else(|| "failed to derive STARK AIR query index".to_owned())?;
        let next_index = (index + 1) % domain;
        let row_path = merkle_path_from_levels(index, &trace_levels)
            .ok_or_else(|| "failed to open STARK AIR row".to_owned())?;
        let next_row_path = merkle_path_from_levels(next_index, &trace_levels)
            .ok_or_else(|| "failed to open next STARK AIR row".to_owned())?;
        let composition_path = merkle_path_from_levels(index, &composition_levels)
            .ok_or_else(|| "failed to open STARK AIR composition".to_owned())?;
        let composition_value = stark_air_composition_value(
            index,
            domain,
            &public_digest,
            &rows[index],
            &rows[next_index],
        )
        .ok_or_else(|| "failed to evaluate opened STARK AIR composition".to_owned())?;
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
    envelope.proof.air = Some(StarkAirProofV1 {
        version: 1,
        circuit_id,
        public_digest,
        trace_root,
        composition_root,
        trace_width: u16::try_from(stark_air_trace_width())
            .map_err(|_| "STARK AIR trace width does not fit u16".to_owned())?,
        openings,
    });
    norito::to_bytes(&envelope).map_err(|err| format!("failed to encode STARK envelope: {err}"))
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
    if circuit_id.is_empty() || circuit_id.len() > MAX_TRANSCRIPT_LABEL_LEN {
        return Err("invalid STARK AIR circuit_id".to_owned());
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

    let rows = (0..domain)
        .map(|index| {
            zk_ace_air_row(index, &witness_limbs, &statement_digest)
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

    let mut envelope = synthesize_stark_fri_envelope_from_values(
        params,
        transcript_label,
        composition_values,
        &extra_query_roots,
    )?;
    if envelope.proof.commits.roots.first().copied() != Some(composition_root) {
        return Err("ZK-ACE AIR composition root does not match FRI base root".to_owned());
    }

    let query_roots = stark_air_query_roots(&envelope.proof.commits.roots, None)
        .into_iter()
        .chain(extra_query_roots)
        .collect::<Vec<_>>();
    let mut openings = Vec::with_capacity(envelope.proof.queries.len());
    for qi in 0..envelope.proof.queries.len() {
        let index = derive_query_index(
            &envelope.transcript_label,
            &envelope.params,
            &query_roots,
            qi,
        )
        .ok_or_else(|| "failed to derive ZK-ACE AIR query index".to_owned())?;
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
        circuit_id,
        public_digest,
        trace_root,
        composition_root,
        trace_width: u16::try_from(zk_ace_air_trace_width())
            .map_err(|_| "ZK-ACE AIR trace width does not fit u16".to_owned())?,
        openings,
    });
    norito::to_bytes(&envelope).map_err(|err| format!("failed to encode STARK envelope: {err}"))
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
        || opening.row.len() > limits.max_air_width
        || opening.next_row.len() > limits.max_air_width
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
    opened_fri_value == opening.composition_value
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

fn verify_stark_fri_envelope_with_context(
    bytes: &[u8],
    limits: &StarkVerifierLimits,
    context: StarkAirVerificationContext<'_>,
) -> bool {
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
    let Some(air) = env.proof.air.as_ref() else {
        return false;
    };
    if air.version != 1
        || air.circuit_id.is_empty()
        || air.trace_width as usize != context.trace_width()
        || air.trace_width as usize > limits.max_air_width
        || air.openings.len() != query_count
        || roots.first().copied() != Some(air.composition_root)
    {
        return false;
    }
    let query_roots = stark_air_query_roots(roots, Some(air));
    let total_domain = 1usize << env.params.n_log2;
    if total_domain == 0 {
        return false;
    }
    let fold_arity = env.params.fold_arity as usize;

    for (qi, chain) in env.proof.queries.iter().enumerate() {
        if chain.len() != expected_chain_len {
            return false;
        }
        let base_index =
            match derive_query_index(&env.transcript_label, &env.params, &query_roots, qi) {
                Some(idx) => idx % total_domain,
                None => return false,
            };
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
