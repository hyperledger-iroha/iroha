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
use iroha_data_model::block::consensus_v2 as wire;

/// A (key, value) pair for SMT inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvPair {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

/// Dedicated execution-witness key tag for a finalized Kagemusha V2 top-up.
pub(crate) const KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG: u8 = 0xD1;
/// Exact tagged-key length: one domain byte and a 32-byte operation id.
pub(crate) const KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_BYTES: usize = 33;
/// Maximum top-up anchors committed by one block.
///
/// Sixteen leaves require at most four 32-byte Merkle siblings. Together with
/// a compact Commit-QC this keeps two independent origins inside the peer
/// envelope's 9,211-byte raw budget even at branch depth 64.
pub(crate) const KAGEMUSHA_V2_MAX_TOPUP_ANCHORS_PER_BLOCK: usize =
    wire::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK as usize;
const KAGEMUSHA_V2_TOPUP_NODE_DOMAIN: &[u8] = b"iroha:kagemusha:v2:topup-node";
const KAGEMUSHA_V2_TOPUP_EMPTY_DOMAIN: &[u8] = b"iroha:kagemusha:v2:topup-empty";

/// Canonical balanced-Merkle path for one block-local Kagemusha top-up leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaTopUpMerkleProof {
    /// Zero-based position in operation-id order.
    pub leaf_index: u32,
    /// Number of real (non-padding) leaves committed by the block.
    pub leaf_count: u32,
    /// Siblings from leaf level to root.
    pub siblings: Vec<Hash>,
}

/// Deterministic block-local commitment material retained for finality proofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaTopUpBlockCommitment {
    /// Root of all non-Kagemusha writes in the execution witness.
    pub ordinary_writes_root: Hash,
    /// Root of the canonical balanced top-up tree.
    pub topup_anchor_root: Hash,
    /// Exact final post-state root authenticated by the Commit QC.
    pub post_state_root: Hash,
    /// Canonically sorted top-up key/value leaves.
    pub leaves: Vec<KvPair>,
    /// One path aligned with each entry in [`Self::leaves`].
    pub proofs: Vec<KagemushaTopUpMerkleProof>,
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
        leaves.insert(k.to_vec(), leaf_hash(pair));
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

/// Split the canonical last-write-wins witness and build its bounded Kagemusha
/// commitment, if the block contains at least one top-up anchor.
///
/// # Errors
///
/// A tagged key with the wrong shape, a zero operation id/digest, or more than
/// [`KAGEMUSHA_V2_MAX_TOPUP_ANCHORS_PER_BLOCK`] anchors fails closed.
pub fn build_kagemusha_topup_block_commitment(
    writes: &[KvPair],
) -> Result<Option<KagemushaTopUpBlockCommitment>, &'static str> {
    let mut canonical = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for pair in writes {
        canonical.insert(pair.key.clone(), pair.value.clone());
    }

    let mut ordinary_writes = Vec::new();
    let mut leaves = Vec::new();
    for (key, value) in canonical {
        let pair = KvPair { key, value };
        if pair.key.first() == Some(&KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG) {
            validate_kagemusha_topup_leaf(&pair)?;
            leaves.push(pair);
        } else {
            ordinary_writes.push(pair);
        }
    }
    if leaves.is_empty() {
        return Ok(None);
    }
    if leaves.len() > KAGEMUSHA_V2_MAX_TOPUP_ANCHORS_PER_BLOCK {
        return Err("Kagemusha V2 top-up anchor count exceeds the consensus limit");
    }

    let leaf_count = u32::try_from(leaves.len())
        .map_err(|_| "Kagemusha V2 top-up anchor count does not fit u32")?;
    let width = leaves.len().next_power_of_two();
    let depth = usize::try_from(width.trailing_zeros())
        .map_err(|_| "Kagemusha V2 top-up tree depth does not fit usize")?;
    let mut levels = Vec::with_capacity(depth.saturating_add(1));
    let mut current = leaves.iter().map(leaf_hash).collect::<Vec<_>>();
    current.resize(width, kagemusha_topup_empty_hash());
    levels.push(current.clone());
    for level in 0..depth {
        let level =
            u16::try_from(level).map_err(|_| "Kagemusha V2 top-up tree level does not fit u16")?;
        current = current
            .chunks_exact(2)
            .map(|pair| kagemusha_topup_node_hash(level, pair[0], pair[1]))
            .collect();
        levels.push(current.clone());
    }
    let topup_anchor_root = current
        .first()
        .copied()
        .ok_or("Kagemusha V2 top-up tree unexpectedly has no root")?;

    let proofs = (0..leaves.len())
        .map(|leaf_index| {
            let mut index = leaf_index;
            let mut siblings = Vec::with_capacity(depth);
            for nodes in levels.iter().take(depth) {
                siblings.push(nodes[index ^ 1]);
                index /= 2;
            }
            Ok(KagemushaTopUpMerkleProof {
                leaf_index: u32::try_from(leaf_index)
                    .map_err(|_| "Kagemusha V2 top-up leaf index does not fit u32")?,
                leaf_count,
                siblings,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;

    let ordinary_writes_root = compute_post_state_root(&[], &ordinary_writes);
    let post_state_root = wire::ExecutionCommitment::topup_post_state_root(
        leaf_count,
        ordinary_writes_root,
        topup_anchor_root,
    );
    Ok(Some(KagemushaTopUpBlockCommitment {
        ordinary_writes_root,
        topup_anchor_root,
        post_state_root,
        leaves,
        proofs,
    }))
}

/// Return the consensus post-state root, preserving the legacy root byte for
/// blocks without Kagemusha top-ups.
pub fn compute_consensus_post_state_root(
    reads: &[KvPair],
    writes: &[KvPair],
) -> Result<Hash, &'static str> {
    match build_kagemusha_topup_block_commitment(writes)? {
        Some(commitment) => Ok(commitment.post_state_root),
        None => Ok(compute_post_state_root(reads, writes)),
    }
}

/// Verify one exact top-up leaf against a Commit-QC-authenticated post root.
#[must_use]
pub fn verify_kagemusha_topup_write_inclusion(
    target: &KvPair,
    proof: &KagemushaTopUpMerkleProof,
    ordinary_writes_root: Hash,
    expected_post_state_root: Hash,
) -> bool {
    if validate_kagemusha_topup_leaf(target).is_err()
        || proof.leaf_count == 0
        || usize::try_from(proof.leaf_count)
            .ok()
            .is_none_or(|count| count > KAGEMUSHA_V2_MAX_TOPUP_ANCHORS_PER_BLOCK)
        || proof.leaf_index >= proof.leaf_count
    {
        return false;
    }
    let Ok(leaf_count) = usize::try_from(proof.leaf_count) else {
        return false;
    };
    let expected_depth = match usize::try_from(leaf_count.next_power_of_two().trailing_zeros()) {
        Ok(depth) => depth,
        Err(_) => return false,
    };
    if proof.siblings.len() != expected_depth {
        return false;
    }

    let mut current = leaf_hash(target);
    let mut index = match usize::try_from(proof.leaf_index) {
        Ok(index) => index,
        Err(_) => return false,
    };
    for (level, sibling) in proof.siblings.iter().copied().enumerate() {
        let Ok(level) = u16::try_from(level) else {
            return false;
        };
        current = if index & 1 == 0 {
            kagemusha_topup_node_hash(level, current, sibling)
        } else {
            kagemusha_topup_node_hash(level, sibling, current)
        };
        index /= 2;
    }
    wire::ExecutionCommitment::topup_post_state_root(
        proof.leaf_count,
        ordinary_writes_root,
        current,
    ) == expected_post_state_root
}

fn validate_kagemusha_topup_leaf(pair: &KvPair) -> Result<(), &'static str> {
    if pair.key.len() != KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_BYTES
        || pair.key[0] != KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG
    {
        return Err("Kagemusha V2 top-up witness key has the wrong shape");
    }
    if pair.key[1..].iter().all(|byte| *byte == 0) {
        return Err("Kagemusha V2 top-up operation id must be nonzero");
    }
    if pair.value.len() != Hash::LENGTH || pair.value.iter().all(|byte| *byte == 0) {
        return Err("Kagemusha V2 top-up anchor digest must be a nonzero 32-byte value");
    }
    Ok(())
}

fn kagemusha_topup_empty_hash() -> Hash {
    Hash::new(KAGEMUSHA_V2_TOPUP_EMPTY_DOMAIN)
}

fn kagemusha_topup_node_hash(level: u16, left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(
        KAGEMUSHA_V2_TOPUP_NODE_DOMAIN.len() + 1 + core::mem::size_of::<u16>() + 2 * Hash::LENGTH,
    );
    preimage.extend_from_slice(KAGEMUSHA_V2_TOPUP_NODE_DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(&level.to_le_bytes());
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}

fn hash_bytes(b: &[u8]) -> [u8; 32] {
    let h = Hash::new(b);
    <[u8; 32]>::from(h)
}

fn leaf_hash(pair: &KvPair) -> Hash {
    let key_hash = hash_bytes(&pair.key);
    let value_hash = hash_bytes(&pair.value);
    let mut preimage = Vec::with_capacity(1 + 32 + 32);
    preimage.push(0x00);
    preimage.extend_from_slice(&key_hash);
    preimage.extend_from_slice(&value_hash);
    Hash::new(preimage)
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
    let rem_bits = (len_bits % 8) as u8;
    debug_assert!(rem_bits < 8);
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
    let mask = (1u8 << rem_bits) - 1;
    bytes[full_bytes] &= mask;
    bytes.truncate(full_bytes + 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(k: &str, v: &str) -> KvPair {
        KvPair::new(k.as_bytes(), v.as_bytes())
    }

    fn manual_hash_bytes(bytes: &[u8]) -> [u8; 32] {
        <[u8; 32]>::from(Hash::new(bytes))
    }

    fn manual_node_hash(left: Hash, right: Hash) -> Hash {
        let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        preimage.push(0x01);
        preimage.extend_from_slice(left.as_ref());
        preimage.extend_from_slice(right.as_ref());
        Hash::new(preimage)
    }

    fn manual_leaf_hash(pair: &KvPair) -> Hash {
        let key_hash = manual_hash_bytes(&pair.key);
        let value_hash = manual_hash_bytes(&pair.value);
        let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        preimage.push(0x00);
        preimage.extend_from_slice(&key_hash);
        preimage.extend_from_slice(&value_hash);
        Hash::new(preimage)
    }

    fn manual_single_leaf_root(pair: &KvPair) -> Hash {
        let empty = Hash::new([]);
        let mut current = manual_leaf_hash(pair);
        let mut prefix = manual_hash_bytes(&pair.key).to_vec();
        let mut len_bits = 256u16;
        while len_bits > 0 {
            let parent = parent_prefix(&prefix, len_bits);
            let right_id = child_prefix(&parent, len_bits, true);
            current = if right_id == prefix {
                manual_node_hash(empty, current)
            } else {
                manual_node_hash(current, empty)
            };
            prefix = parent;
            len_bits -= 1;
        }
        current
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

    #[test]
    fn smt_hash_preimages_and_missing_children_match_formal_gate() {
        let pair = kv("leaf-key", "leaf-value");
        let root = compute_post_state_root(std::slice::from_ref(&pair), &[]);
        assert_eq!(root, manual_single_leaf_root(&pair));

        let key_changed = compute_post_state_root(&[kv("other-key", "leaf-value")], &[]);
        let value_changed = compute_post_state_root(&[kv("leaf-key", "other-value")], &[]);
        assert_ne!(root, key_changed);
        assert_ne!(root, value_changed);

        let left = Hash::prehashed([0x11; Hash::LENGTH]);
        let right = Hash::prehashed([0x22; Hash::LENGTH]);
        let mut untagged_node = Vec::with_capacity(2 * Hash::LENGTH);
        untagged_node.extend_from_slice(left.as_ref());
        untagged_node.extend_from_slice(right.as_ref());
        assert_eq!(node_hash(left, right), manual_node_hash(left, right));
        assert_ne!(node_hash(left, right), Hash::new(untagged_node));
        assert_ne!(node_hash(left, right), manual_node_hash(right, left));
    }

    #[test]
    fn duplicate_keys_and_canonical_order_match_formal_gate() {
        let duplicate_reads = [kv("same", "old"), kv("same", "new")];
        let last_read = [kv("same", "new")];
        let first_read = [kv("same", "old")];
        assert_eq!(
            compute_post_state_root(&duplicate_reads, &[]),
            compute_post_state_root(&last_read, &[])
        );
        assert_ne!(
            compute_post_state_root(&duplicate_reads, &[]),
            compute_post_state_root(&first_read, &[])
        );

        let duplicate_writes = [kv("same", "old"), kv("same", "new")];
        assert_eq!(
            compute_post_state_root(&[], &duplicate_writes),
            compute_post_state_root(&[], &last_read)
        );

        let ordered = [kv("a", "1"), kv("b", "2"), kv("c", "3")];
        let reordered = [kv("c", "3"), kv("a", "1"), kv("b", "2")];
        assert_eq!(
            compute_post_state_root(&ordered, &[]),
            compute_post_state_root(&reordered, &[])
        );
        assert_eq!(
            compute_post_state_root(&[], &ordered),
            compute_post_state_root(&[], &reordered)
        );
    }

    #[test]
    fn prefix_truncation_and_child_bit_order_match_formal_gate() {
        let bytes = [0b1010_1100, 0b1111_0000];
        assert_eq!(truncate_prefix(&bytes, 0), Vec::<u8>::new());
        assert_eq!(truncate_prefix(&bytes, 8), vec![0b1010_1100]);
        assert_eq!(truncate_prefix(&bytes, 5), vec![0b0000_1100]);

        assert_eq!(parent_prefix(&[0b1010_1101], 5), vec![0b0000_1101]);
        assert_eq!(child_prefix(&[], 1, false), vec![0b0000_0000]);
        assert_eq!(child_prefix(&[], 1, true), vec![0b0000_0001]);

        let parent = [0b0000_1101];
        assert_eq!(child_prefix(&parent, 5, false), vec![0b0000_1101]);
        assert_eq!(child_prefix(&parent, 5, true), vec![0b0001_1101]);
        assert_eq!(child_prefix(&[0xFF, 0xFF], 9, false), vec![0xFF, 0x00]);
        assert_eq!(child_prefix(&[0xFF, 0xFF], 9, true), vec![0xFF, 0x01]);
    }

    fn topup_leaf(operation: u8, digest: u8) -> KvPair {
        let mut key = vec![KAGEMUSHA_V2_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        key.extend_from_slice(&[operation; 32]);
        KvPair::new(key, vec![digest; 32])
    }

    #[test]
    fn kagemusha_post_root_is_unchanged_without_topup_anchors() {
        let reads = vec![kv("read", "old")];
        let writes = vec![kv("balance", "10.75"), kv("supply", "100")];
        assert_eq!(
            compute_consensus_post_state_root(&reads, &writes).expect("ordinary root"),
            compute_post_state_root(&reads, &writes)
        );
        assert!(
            build_kagemusha_topup_block_commitment(&writes)
                .expect("ordinary writes")
                .is_none()
        );
    }

    #[test]
    fn kagemusha_balanced_paths_roundtrip_at_every_supported_shape() {
        for count in [1_usize, 2, 3, 4, 8, 16] {
            let mut writes = vec![kv("balance", "10.75"), kv("supply", "100")];
            writes.extend((1..=count).rev().map(|index| {
                topup_leaf(
                    u8::try_from(index).expect("fixture operation"),
                    u8::try_from(index + 32).expect("fixture digest"),
                )
            }));
            let commitment = build_kagemusha_topup_block_commitment(&writes)
                .expect("valid top-up writes")
                .expect("top-up commitment");
            assert_eq!(commitment.leaves.len(), count);
            assert_eq!(commitment.proofs.len(), count);
            assert_eq!(
                commitment.proofs[0].siblings.len(),
                count.next_power_of_two().trailing_zeros() as usize
            );
            assert!(
                commitment
                    .leaves
                    .windows(2)
                    .all(|pair| pair[0].key < pair[1].key)
            );
            for (leaf, proof) in commitment.leaves.iter().zip(&commitment.proofs) {
                assert!(verify_kagemusha_topup_write_inclusion(
                    leaf,
                    proof,
                    commitment.ordinary_writes_root,
                    commitment.post_state_root,
                ));
            }
            assert_eq!(
                compute_consensus_post_state_root(&[], &writes).expect("consensus root"),
                commitment.post_state_root
            );
        }
    }

    #[test]
    fn kagemusha_paths_are_independent_of_unrelated_write_count() {
        let anchors = vec![topup_leaf(1, 0xA1), topup_leaf(2, 0xA2)];
        let compact = build_kagemusha_topup_block_commitment(&anchors)
            .expect("compact block")
            .expect("top-up commitment");
        let mut noisy = anchors;
        noisy.extend((0_u32..1_000).map(|index| {
            KvPair::new(
                format!("ordinary-{index:04}").into_bytes(),
                index.to_le_bytes().to_vec(),
            )
        }));
        let noisy = build_kagemusha_topup_block_commitment(&noisy)
            .expect("noisy block")
            .expect("top-up commitment");
        assert_eq!(compact.topup_anchor_root, noisy.topup_anchor_root);
        assert_eq!(compact.proofs, noisy.proofs);
        assert_ne!(compact.ordinary_writes_root, noisy.ordinary_writes_root);
        assert_ne!(compact.post_state_root, noisy.post_state_root);
    }

    #[test]
    fn kagemusha_inclusion_rejects_every_binding_mutation() {
        let writes = vec![
            kv("balance", "4.50"),
            topup_leaf(1, 0xA1),
            topup_leaf(2, 0xA2),
            topup_leaf(3, 0xA3),
        ];
        let commitment = build_kagemusha_topup_block_commitment(&writes)
            .expect("valid block")
            .expect("top-up commitment");
        let target = &commitment.leaves[1];
        let proof = &commitment.proofs[1];
        let verifies = |leaf: &KvPair,
                        candidate: &KagemushaTopUpMerkleProof,
                        ordinary_root: Hash,
                        post_root: Hash| {
            verify_kagemusha_topup_write_inclusion(leaf, candidate, ordinary_root, post_root)
        };
        assert!(verifies(
            target,
            proof,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));

        let mut wrong_key = target.clone();
        wrong_key.key[1] ^= 0x80;
        assert!(!verifies(
            &wrong_key,
            proof,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        let mut wrong_value = target.clone();
        wrong_value.value[0] ^= 0x80;
        assert!(!verifies(
            &wrong_value,
            proof,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        assert!(!verifies(
            target,
            proof,
            Hash::new(b"wrong ordinary root"),
            commitment.post_state_root
        ));
        assert!(!verifies(
            target,
            proof,
            commitment.ordinary_writes_root,
            Hash::new(b"wrong post root")
        ));

        let mut wrong_index = proof.clone();
        wrong_index.leaf_index = (wrong_index.leaf_index + 1) % wrong_index.leaf_count;
        assert!(!verifies(
            target,
            &wrong_index,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        let mut wrong_count = proof.clone();
        wrong_count.leaf_count += 1;
        assert!(!verifies(
            target,
            &wrong_count,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        let mut missing = proof.clone();
        missing.siblings.pop();
        assert!(!verifies(
            target,
            &missing,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        let mut extra = proof.clone();
        extra.siblings.push(Hash::new(b"extra"));
        assert!(!verifies(
            target,
            &extra,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
        let mut forged = proof.clone();
        forged.siblings[0] = Hash::new(b"forged sibling");
        assert!(!verifies(
            target,
            &forged,
            commitment.ordinary_writes_root,
            commitment.post_state_root
        ));
    }

    #[test]
    fn kagemusha_commitment_rejects_malformed_zero_and_oversized_sets() {
        let mut malformed_key = topup_leaf(1, 1);
        malformed_key.key.pop();
        assert!(build_kagemusha_topup_block_commitment(&[malformed_key]).is_err());

        let mut zero_operation = topup_leaf(1, 1);
        zero_operation.key[1..].fill(0);
        assert!(build_kagemusha_topup_block_commitment(&[zero_operation]).is_err());

        let mut zero_digest = topup_leaf(1, 1);
        zero_digest.value.fill(0);
        assert!(build_kagemusha_topup_block_commitment(&[zero_digest]).is_err());

        let oversized = (1..=KAGEMUSHA_V2_MAX_TOPUP_ANCHORS_PER_BLOCK + 1)
            .map(|index| {
                topup_leaf(
                    u8::try_from(index).expect("bounded fixture operation"),
                    u8::try_from(index + 32).expect("bounded fixture digest"),
                )
            })
            .collect::<Vec<_>>();
        assert!(build_kagemusha_topup_block_commitment(&oversized).is_err());
        assert!(compute_consensus_post_state_root(&[], &oversized).is_err());
    }

    #[test]
    fn kagemusha_duplicate_operation_uses_only_the_final_digest() {
        let stale = topup_leaf(7, 0xA1);
        let final_leaf = topup_leaf(7, 0xA2);
        let commitment = build_kagemusha_topup_block_commitment(&[stale, final_leaf.clone()])
            .expect("last-write-wins block")
            .expect("top-up commitment");
        assert_eq!(commitment.leaves, vec![final_leaf.clone()]);
        assert!(verify_kagemusha_topup_write_inclusion(
            &final_leaf,
            &commitment.proofs[0],
            commitment.ordinary_writes_root,
            commitment.post_state_root,
        ));
    }

    #[test]
    fn mask_tail_bits_handles_byte_boundaries_without_fallible_invariants() {
        let mut empty = vec![0xAA];
        mask_tail_bits(&mut empty, 0);
        assert_eq!(empty, Vec::<u8>::new());

        let mut exact = vec![0xAA, 0x55, 0xFF];
        mask_tail_bits(&mut exact, 16);
        assert_eq!(exact, vec![0xAA, 0x55]);

        let mut partial = vec![0xAA, 0xFF];
        mask_tail_bits(&mut partial, 9);
        assert_eq!(partial, vec![0xAA, 0x01]);

        let mut short = Vec::new();
        mask_tail_bits(&mut short, 15);
        assert_eq!(short, vec![0x00, 0x00]);
    }
}
