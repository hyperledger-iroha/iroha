//! Minimal deterministic Sparse Merkle Tree (SMT) verifier for SBV‑AM prototype.
//!
//! This module computes a post-state root from a set of `(reads, writes)` without
//! requiring the full state. It is intended for prototype verification and is
//! designed to be deterministic across peers and platforms.
//!
//! Design notes
//! - Key paths are derived by hashing raw keys with `Blake2b-32` (`iroha_crypto::Hash`).
//! - Leaf hash = `H(0x00 || key_hash || value_hash)`.
//! - Internal node hash = `H(0x01 || left_hash || right_hash)`.
//! - Missing children resolve to a fixed empty hash `H("")`.
//! - The tree is a binary sparse tree of depth 256 over the key-hash bits.
//! - Reads bind the computation for pure read transactions (no writes). When a
//!   block performs any writes, the post-state root only commits to the writes
//!   so incidental reads cannot perturb the digest. This keeps post roots in
//!   sync with the order-independent write accumulator from the previous
//!   prototype while `parent_state_root` continues to bind the read set.
//!
//! This is a minimal, internal component. It does not attempt optimizations and
//! runs in O((R+W) * 256). Avoid using in hot paths beyond prototyping.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;

/// A (key, value) pair for SMT inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvPair {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

impl KvPair {
    pub fn new(key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Compute the deterministic post-state root from read and write sets.
///
/// - `reads`: witnessed key/value pairs read during execution.
/// - `writes`: key/value pairs written during execution (override reads on conflict).
pub fn compute_post_state_root(reads: &[KvPair], writes: &[KvPair]) -> Hash {
    // Default empty hash used for absent children
    let empty = Hash::new([]);

    // Build the leaf map at depth 256: key prefix (exact hash bytes) -> leaf hash
    // When writes are present we bind only to them so order-independent write
    // sets produce identical roots. Pure read transactions still bind to reads.
    let mut leaves: BTreeMap<Vec<u8>, Hash> = BTreeMap::new();

    let mut insert_leaf = |pair: &KvPair| {
        let k = hash_bytes(&pair.key);
        let v = hash_bytes(&pair.value);
        let mut leaf_preimage = Vec::with_capacity(1 + 32 + 32);
        leaf_preimage.push(0x00);
        leaf_preimage.extend_from_slice(&k);
        leaf_preimage.extend_from_slice(&v);
        let leaf = Hash::new(leaf_preimage);
        leaves.insert(k.to_vec(), leaf);
    };

    let inputs = if writes.is_empty() { reads } else { writes };
    for pair in inputs {
        insert_leaf(pair);
    }

    // Early return: no leaves
    if leaves.is_empty() {
        return empty;
    }

    // Iteratively fold leaves up to the root across 256 levels.
    // Each level maps a bit-prefix (length in bits) to a node hash.
    // Represent prefixes as byte vectors with unused tail bits in the last byte masked to 0.
    let mut cur_len_bits: u16 = 256;
    let mut cur: BTreeMap<Vec<u8>, Hash> = leaves;

    while cur_len_bits > 0 {
        // Collect parent prefixes to compute at the next level
        let mut parents: BTreeSet<Vec<u8>> = BTreeSet::new();
        for prefix in cur.keys() {
            parents.insert(parent_prefix(prefix, cur_len_bits));
        }

        let mut next: BTreeMap<Vec<u8>, Hash> = BTreeMap::new();
        for p in parents {
            let left_id = child_prefix(&p, cur_len_bits, false);
            let right_id = child_prefix(&p, cur_len_bits, true);
            let left = cur.get(&left_id).copied().unwrap_or(empty);
            let right = cur.get(&right_id).copied().unwrap_or(empty);
            let parent_hash = node_hash(left, right);
            next.insert(p, parent_hash);
        }

        cur = next;
        cur_len_bits -= 1;
        if cur_len_bits == 0 {
            // At the root there must be exactly one entry; if not, fold deterministically.
            break;
        }
    }

    // Root extraction: cur may contain 1+ nodes if inputs were empty at some high level.
    // Fold deterministically by ordering keys and hashing left-to-right.
    if cur.len() == 1 {
        cur.into_values().next().unwrap_or(empty)
    } else {
        // Deterministic fold
        let mut acc = empty;
        for h in cur.into_values() {
            acc = node_hash(acc, h);
        }
        acc
    }
}

fn hash_bytes(b: &[u8]) -> [u8; 32] {
    let h = Hash::new(b);
    <[u8; 32]>::from(h)
}

fn node_hash(left: Hash, right: Hash) -> Hash {
    let mut buf = Vec::with_capacity(1 + 32 + 32);
    buf.push(0x01);
    buf.extend_from_slice(left.as_ref());
    buf.extend_from_slice(right.as_ref());
    Hash::new(buf)
}

fn parent_prefix(prefix: &[u8], len_bits: u16) -> Vec<u8> {
    debug_assert!(len_bits >= 1);
    let new_len = len_bits - 1;
    truncate_prefix(prefix, new_len)
}

fn child_prefix(parent: &[u8], child_len_bits: u16, right: bool) -> Vec<u8> {
    // child_len_bits is current level length; parent has length-1
    debug_assert!(child_len_bits >= 1);
    let mut out = parent.to_vec();
    let bit_idx = child_len_bits - 1;
    let byte_idx = (bit_idx / 8) as usize;
    let bit_off = (bit_idx % 8) as u8;
    if out.len() <= byte_idx {
        out.resize(byte_idx + 1, 0);
    }
    // Set or clear the last bit according to left/right
    let mask = 1u8 << bit_off;
    if right {
        out[byte_idx] |= mask;
    } else {
        out[byte_idx] &= !mask;
    }
    // Mask off unused tail bits beyond child_len_bits
    mask_tail_bits(&mut out, child_len_bits);
    out
}

fn truncate_prefix(prefix: &[u8], len_bits: u16) -> Vec<u8> {
    if len_bits == 0 {
        return Vec::new();
    }
    let byte_len = usize::from(len_bits.div_ceil(8));
    let mut out = prefix[..core::cmp::min(prefix.len(), byte_len)].to_vec();
    mask_tail_bits(&mut out, len_bits);
    out
}

fn mask_tail_bits(bytes: &mut Vec<u8>, len_bits: u16) {
    let full_bytes = usize::from(len_bits / 8);
    let rem_bits = u8::try_from(len_bits % 8).expect("remainder must be < 8");
    if rem_bits == 0 {
        // Drop any trailing bytes beyond full_bytes
        if bytes.len() > full_bytes {
            bytes.truncate(full_bytes);
        }
        return;
    }
    if bytes.len() < full_bytes + 1 {
        bytes.resize(full_bytes + 1, 0);
    }
    // Keep only the low `rem_bits` in the last byte
    let mask = if rem_bits == 0 {
        0
    } else {
        u8::try_from((1u16 << rem_bits) - 1).unwrap_or(u8::MAX)
    };
    bytes[full_bytes] &= mask;
    bytes.truncate(full_bytes + 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(k: &str, v: &str) -> KvPair {
        KvPair::new(k.as_bytes(), v.as_bytes())
    }

    #[test]
    fn empty_inputs_yield_empty_hash() {
        let h = compute_post_state_root(&[], &[]);
        assert_eq!(h, Hash::new([]));
    }

    #[test]
    fn single_write_matches_order_independence() {
        let w = [kv("a", "1")];
        let h1 = compute_post_state_root(&[], &w);
        let h2 = compute_post_state_root(&[kv("z", "0")], &w); // extra read unrelated
        assert_eq!(h1, h2);
    }

    #[test]
    fn pure_reads_bind_root_when_no_writes() {
        let r1 = [kv("alpha", "1")];
        let r2 = [kv("alpha", "1"), kv("beta", "2")];
        let h1 = compute_post_state_root(&r1, &[]);
        let h2 = compute_post_state_root(&r2, &[]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn writes_override_reads_for_same_key() {
        let r = [kv("k", "old")];
        let w = [kv("k", "new")];
        let h = compute_post_state_root(&r, &w);
        let h_only_w = compute_post_state_root(&[], &w);
        assert_eq!(h, h_only_w);
    }

    #[test]
    fn multiple_keys_deterministic_fold() {
        let r = [kv("a", "1"), kv("b", "2")];
        let w = [kv("c", "3")];
        let h1 = compute_post_state_root(&r, &w);
        let h2 = compute_post_state_root(&[r[1].clone(), r[0].clone()], &w);
        assert_eq!(h1, h2);
    }
}
