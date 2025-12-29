//! Shielded commitment Merkle tests (domain‑tagged Blake2b‑32 leaves).
//!
//! Properties validated:
//! - Domain‑tagged leaf hashing is stable and distinct from raw/prehashed bytes.
//! - Explicit empty‑tree root obeys `R_{d+1} = H(R_d || R_d)`.
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
    // Depth 0: root equals domain‑tagged zero leaf
    let r0 = MerkleTree::<[u8; 32]>::shielded_empty_root(0);
    let l0 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
    assert_eq!(r0, <Hash as Into<[u8; 32]>>::into(Hash::from(l0)));

    // Check a few levels satisfy R_{d+1} = H(R_d || R_d)
    let mut prev = r0;
    for d in 0..4u8 {
        let next = MerkleTree::<[u8; 32]>::shielded_empty_root(d + 1);
        // Manual parent
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&prev);
        buf[32..].copy_from_slice(&prev);
        let manual = Hash::new(buf);
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
    const GOLDENS: &[(u8, &str)] = &[
        (
            0,
            "D6BF3EAAC5E6107CA805D08C4C788968A88AE2050268A8585C47BEB03F296C0F",
        ),
        (
            1,
            "6A5E6172A92EA453201C2BFF0517CC5B35CECB95F13B5724F7D9731B802570DB",
        ),
        (
            2,
            "872470D163B62FDEAFFA5EA605C4C942690249BB8E2636F324F9F4F4B42F0197",
        ),
        (
            3,
            "F16CEA0AC60DBAC3267337EBCE2366FA0DBBC044E8B9EEBB1CD49E7238890E77",
        ),
        (
            4,
            "77C86326A6B44FF12ECB4A26210CFF1007A8955985489488B1403F69B85BD3E1",
        ),
        (
            5,
            "EF5CAA7789EF5A5CE09E616C1CDE579A976A51A2D3A47CBF89280E2618C324C5",
        ),
        (
            8,
            "2577885FFA39174A44504289BB613B914F96157D9C502E5697057F05BB410817",
        ),
        (
            12,
            "803DAFC24BF2B020CB57F1951ADBF3DB6150A820CFA939CBC84DD86A4C4DF9F1",
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
