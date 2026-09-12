//! Shielded commitment Merkle tests (domain‑tagged Blake2b‑32 leaves).
//!
//! Properties validated:
//! - Domain‑tagged leaf hashing is stable and distinct from raw/prehashed bytes.
//! - Empty roots include the generic leaf domain and obey
//!   `R_{d+1} = H(internal_domain || R_d || R_d)`.
//! - Empty‑tree root for small depths matches the root of a perfect tree
//!   constructed from 2^d identical zero‑leaves.
use iroha_crypto::{Hash, HashOf, MerkleTree};
fn hex_upper(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}
#[test]
fn shielded_leaf_is_domain_tagged() {
    let cm = [0u8; 32];
    let leaf = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment(cm);
    // Compare against Hash::prehashed(cm) to ensure the domain tag changes the digest.
    let pre = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(cm));
    assert_ne!(hex_upper(&Hash::from(leaf)), hex_upper(&Hash::from(pre)));
}
#[test]
fn empty_root_recursive_identity() {
    // Depth 0: the shielded commitment passes through the generic leaf domain.
    let r0 = MerkleTree::<[u8; 32]>::shielded_empty_root(0);
    let l0 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
    let expected_leaf = Hash::new_from_chunks(&[b"iroha:merkle:leaf:v1\0", l0.as_ref()]);
    assert_eq!(r0, <Hash as Into<[u8; 32]>>::into(expected_leaf));
    // Check the independently spelled internal domain at each level.
    let mut prev = r0;
    for d in 0..4u8 {
        let next = MerkleTree::<[u8; 32]>::shielded_empty_root(d + 1);
        // Manual parent
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&prev);
        buf[32..].copy_from_slice(&prev);
        let manual = Hash::new_from_chunks(&[b"iroha:merkle:internal:v1\0", &buf]);
        assert_eq!(next, <Hash as Into<[u8; 32]>>::into(manual));
        prev = next;
    }
}
#[test]
fn empty_root_matches_perfect_tree_small_depth() {
    for depth in 0..=4u8 {
        let perfect_leaf_count = 1usize << depth;
        let leaf = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
        let tree: MerkleTree<[u8; 32]> = std::iter::repeat_n(leaf, perfect_leaf_count).collect();
        let root_tree = tree.root().map_or([0u8; 32], |h| *h.as_ref());
        let root_empty = MerkleTree::<[u8; 32]>::shielded_empty_root(depth);
        assert_eq!(root_tree, root_empty, "depth {depth}");
    }
}
#[test]
fn shielded_leaf_golden_vectors() {
    let zero = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
    assert_eq!(
        hex_upper(&Hash::from(zero)),
        "D6BF3EAAC5E6107CA805D08C4C788968A88AE2050268A8585C47BEB03F296C0F",
        "zero commitment leaf"
    );
    let ff = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0xFFu8; 32]);
    assert_eq!(
        hex_upper(&Hash::from(ff)),
        "F0F7F7E77BCCE31B59E37D1DD5B8183491F141D6EAD4ADC04081AECA6070517B",
        "0xFF commitment leaf"
    );
}
#[test]
fn shielded_empty_root_golden_vectors() {
    // Independent Python hashlib.blake2b(digest_size=32) vectors. Each hash sets
    // the final-byte low bit. Hash the shielded commitment, then the generic
    // leaf domain, then the internal domain at every parent; all tags end in NUL.
    const GOLDENS: &[(u8, &str)] = &[
        (
            0,
            "00E7E4B201291FCABF1EE078A09F8EC3A5D73608971F64F352E10045B3041695",
        ),
        (
            1,
            "C135A96E299AD0BC4CD5A1818696E603DBA410993C194154274BA827EDDD8193",
        ),
        (
            2,
            "062AD9A32A3E9DD3C1925B80198B7957433CD90E93241B3BFDD2EB22EE25E583",
        ),
        (
            3,
            "122B1724362D3260E406604CE87FAAD8BE1B298AA836455CD852135254656C45",
        ),
        (
            4,
            "428A69DC240C901D8F4F736ABA5B298F8597F60C4C5BF01BD495615D0E2249C7",
        ),
        (
            5,
            "E71ED395B7F1A67A2544466B8C0F2D49FBCB492A4A37AE1CA3D6BB340D33E2AB",
        ),
        (
            8,
            "335FA2337834BECF009F9311436FF24F89B3378200FE5E50DEE5264172EF3F71",
        ),
        (
            12,
            "D4FAAE9F41DD99329346D8FE5F666BFAD81E0F7CA5AC6A975BF56E6A0BC7E88B",
        ),
    ];
    for &(depth, expected) in GOLDENS {
        let root = MerkleTree::<[u8; 32]>::shielded_empty_root(depth);
        assert_eq!(
            hex::encode_upper(root),
            expected,
            "shielded empty root mismatch for depth {depth}"
        );
    }
}
