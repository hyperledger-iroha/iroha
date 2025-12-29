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

use iroha_crypto::{Hash, MerkleTree};

fn to_hex(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}

fn hex_upper(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}

// Placeholders — to be filled with stable values and enabled later.
const HEX_EMPTY_D0: &str = "D6BF3EAAC5E6107CA805D08C4C788968A88AE2050268A8585C47BEB03F296C0F";
const HEX_EMPTY_D1: &str = "6A5E6172A92EA453201C2BFF0517CC5B35CECB95F13B5724F7D9731B802570DB";
const HEX_EMPTY_D4: &str = "77C86326A6B44FF12ECB4A26210CFF1007A8955985489488B1403F69B85BD3E1";

const HEX_ROOT_1: &str = "D6BF3EAAC5E6107CA805D08C4C788968A88AE2050268A8585C47BEB03F296C0F";
const HEX_ROOT_2: &str = "6FFC8514FD8EAFA13A5C2D143E7523CA62B57F46718EFF16E0F2DA99415D3E47";
const HEX_ROOT_5: &str = "5C1D7D72EE828CEB5C2D8DCBF0F9D1987548C8BE7EE806547BC64DBD7A7D7C09";

// 2-leaf proofs (siblings bottom→top)
const HEX_PROOF_2_IDX0: &[&str] =
    &["C5AA2F1DE8D40A66EE683CCF7AB5BA40E1665FE60762F478D491CEB177B7C4D1"];
const HEX_PROOF_2_IDX1: &[&str] =
    &["D6BF3EAAC5E6107CA805D08C4C788968A88AE2050268A8585C47BEB03F296C0F"];

// 5-leaf proof for idx=3 (siblings bottom→top)
const HEX_PROOF_5_IDX3: &[&str] = &[
    "4EE7268911FDE5CB145C651235754CE791E897ECFB678694DA7C68705685AF7D",
    "6FFC8514FD8EAFA13A5C2D143E7523CA62B57F46718EFF16E0F2DA99415D3E47",
    "3D5C3F15075A1610525DA1E949BB885FC9EAD1BF195C2D7F8B659A4D79A2299F",
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
    assert!(p1.clone().verify(&l0, &r1, 9));

    // 2‑leaf tree (leaf0, leaf1 = domain‑tagged ones)
    let l1 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([1u8; 32]);
    let t2: MerkleTree<[u8; 32]> = [l0, l1].into_iter().collect();
    let r2 = t2.root().expect("root");
    assert_eq!(HEX_ROOT_2, to_hex(&Hash::from(r2)));
    let p2_l = t2.get_proof(0).expect("proof");
    let p2_r = t2.get_proof(1).expect("proof");
    assert!(p2_l.clone().verify(&l0, &r2, 9));
    assert!(p2_r.clone().verify(&l1, &r2, 9));
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
    assert!(p3.clone().verify(&l3, &r5, 9));
    // Compare audit path hex
    assert_eq!(HEX_PROOF_5_IDX3.len(), p3.audit_path().len());
    for (got, exp) in p3.audit_path().iter().zip(HEX_PROOF_5_IDX3.iter().copied()) {
        match got {
            Some(h) => assert_eq!(exp, to_hex(&Hash::from(*h))),
            None => panic!("unexpected NONE in 5-leaf proof idx3"),
        }
    }
}
