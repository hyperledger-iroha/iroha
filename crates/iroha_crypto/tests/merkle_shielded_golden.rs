//! Golden vectors for shielded commitment Merkle roots and proofs.
//!
//! This file is intended to host fixed, hard‑coded hex vectors for:
//! - Empty‑tree root at selected depths
//! - Trees with 1 and 2 commitments
//! - A deeper tree (e.g., 5 leaves)
//! - Membership proofs for a chosen leaf at each height
//!
//! The constants below were captured from the reference implementation.
//! If the hashing rules change, re-run
//! `cargo test -p iroha_crypto --test merkle_shielded_golden -- --nocapture`
//! to dump updated values and replace the hex literals.
use iroha_crypto::{Hash, MerkleTree, MerkleTreeCommitment};
use std::num::NonZeroU64;
fn to_hex(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}
fn hex_upper(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}
// Placeholders — to be filled with stable values and enabled later.
const HEX_EMPTY_D0: &str = "00E7E4B201291FCABF1EE078A09F8EC3A5D73608971F64F352E10045B3041695";
const HEX_EMPTY_D1: &str = "C135A96E299AD0BC4CD5A1818696E603DBA410993C194154274BA827EDDD8193";
const HEX_EMPTY_D4: &str = "428A69DC240C901D8F4F736ABA5B298F8597F60C4C5BF01BD495615D0E2249C7";
const HEX_ROOT_1: &str = "00E7E4B201291FCABF1EE078A09F8EC3A5D73608971F64F352E10045B3041695";
const HEX_ROOT_2: &str = "C56A47B4A1D93964261C53FFDC753CA804E26404BBBA835614AEB513221CE543";
const HEX_ROOT_5: &str = "C4F8B14528E8277E17E59720C1F3DDBF2F5CBA686040FB5A876ED2F2B6A0A79D";
// 2-leaf proofs (siblings bottom→top)
const HEX_PROOF_2_IDX0: &[&str] =
    &["37EB3F01F72BA4DE7381793FE711EE68FDC334A9BDCC963D048F9B963A929701"];
const HEX_PROOF_2_IDX1: &[&str] =
    &["00E7E4B201291FCABF1EE078A09F8EC3A5D73608971F64F352E10045B3041695"];
// 5-leaf proof for idx=3 (siblings bottom→top)
const HEX_PROOF_5_IDX3: &[&str] = &[
    "F04069B786E254DB31BC57B6E17B0FC151BE4B6109B8990DB2C70B586AC549A5",
    "C56A47B4A1D93964261C53FFDC753CA804E26404BBBA835614AEB513221CE543",
    "077ED50C7F0C75B50F8199802C66D3F6947E73582BD6E618E546F809F4269B1F",
];
#[test]
fn golden_empty_roots_match_constants() {
    let r0 = MerkleTree::<[u8; 32]>::shielded_empty_root(0);
    let r1 = MerkleTree::<[u8; 32]>::shielded_empty_root(1);
    let r4 = MerkleTree::<[u8; 32]>::shielded_empty_root(4);
    assert_eq!(HEX_EMPTY_D0, hex_upper(&Hash::prehashed(r0)));
    assert_eq!(HEX_EMPTY_D1, hex_upper(&Hash::prehashed(r1)));
    assert_eq!(HEX_EMPTY_D4, hex_upper(&Hash::prehashed(r4)));
}
#[test]
fn golden_small_trees_and_membership_proofs() {
    // 1‑leaf tree (leaf = domain‑tagged zero)
    let l0 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
    let t1: MerkleTree<[u8; 32]> = [l0].into_iter().collect();
    let r1 = t1.root().expect("root");
    assert_eq!(HEX_ROOT_1, to_hex(&Hash::from(r1)));
    let p1 = t1.get_proof(0).expect("proof");
    assert!(p1.verify(&l0, &t1.commitment().expect("commitment")));
    // 2‑leaf tree (leaf0, leaf1 = domain‑tagged ones)
    let l1 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([1u8; 32]);
    let t2: MerkleTree<[u8; 32]> = [l0, l1].into_iter().collect();
    let r2 = t2.root().expect("root");
    assert_eq!(HEX_ROOT_2, to_hex(&Hash::from(r2)));
    let p2_l = t2.get_proof(0).expect("proof");
    let p2_r = t2.get_proof(1).expect("proof");
    let c2 = t2.commitment().expect("commitment");
    assert!(p2_l.verify(&l0, &c2));
    assert!(p2_r.verify(&l1, &c2));
    let wrong_count_commitment = MerkleTreeCommitment::new(
        *c2.root(),
        NonZeroU64::new(4).expect("wrong test count remains non-zero"),
    );
    assert!(!p2_l.verify(&l0, &wrong_count_commitment));
    assert!(!p2_r.verify(&l1, &wrong_count_commitment));
    // Compare audit paths (length and hex)
    assert_eq!(HEX_PROOF_2_IDX0.len(), p2_l.audit_path().len());
    for (got, exp) in p2_l
        .audit_path()
        .iter()
        .zip(HEX_PROOF_2_IDX0.iter().copied())
    {
        match got {
            Some(h) => assert_eq!(exp, to_hex(&Hash::from(*h))),
            None => panic!("unexpected NONE in 2-leaf proof idx0"),
        }
    }
    assert_eq!(HEX_PROOF_2_IDX1.len(), p2_r.audit_path().len());
    for (got, exp) in p2_r
        .audit_path()
        .iter()
        .zip(HEX_PROOF_2_IDX1.iter().copied())
    {
        match got {
            Some(h) => assert_eq!(exp, to_hex(&Hash::from(*h))),
            None => panic!("unexpected NONE in 2-leaf proof idx1"),
        }
    }
    // 5‑leaf tree (deeper) — use different commitments
    let l2 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([2u8; 32]);
    let l3 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([3u8; 32]);
    let l4 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([4u8; 32]);
    let t5: MerkleTree<[u8; 32]> = [l0, l1, l2, l3, l4].into_iter().collect();
    let r5 = t5.root().expect("root");
    assert_eq!(HEX_ROOT_5, to_hex(&Hash::from(r5)));
    // Prove index 3
    let p3 = t5.get_proof(3).expect("proof");
    assert!(p3.verify(&l3, &t5.commitment().expect("commitment")));
    // Compare audit path hex
    assert_eq!(HEX_PROOF_5_IDX3.len(), p3.audit_path().len());
    for (got, exp) in p3.audit_path().iter().zip(HEX_PROOF_5_IDX3.iter().copied()) {
        match got {
            Some(h) => assert_eq!(exp, to_hex(&Hash::from(*h))),
            None => panic!("unexpected NONE in 5-leaf proof idx3"),
        }
    }
}
