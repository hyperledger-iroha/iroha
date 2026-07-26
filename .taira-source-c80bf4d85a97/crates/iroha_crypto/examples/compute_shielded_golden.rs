//! Compute and print shielded Merkle golden vectors (hex, uppercase).

use iroha_crypto::{Hash, MerkleTree};

fn hex(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}

fn main() {
    // Empty roots
    for d in [0u8, 1, 4] {
        let r = MerkleTree::<[u8; 32]>::shielded_empty_root(d);
        println!("EMPTY_DEPTH_{} {}", d, hex(&Hash::prehashed(r)));
    }

    // 1-leaf (cm=0)
    let l0 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([0u8; 32]);
    let t1: MerkleTree<[u8; 32]> = [l0].into_iter().collect();
    let r1 = t1.root().expect("root");
    println!("ROOT_1 {}", hex(&Hash::from(r1)));

    // 2-leaf (cm=0, cm=1)
    let l1 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([1u8; 32]);
    let t2: MerkleTree<[u8; 32]> = [l0, l1].into_iter().collect();
    let r2 = t2.root().expect("root");
    println!("ROOT_2 {}", hex(&Hash::from(r2)));
    // Proofs for idx 0 and 1: print audit path siblings (Some hex or NONE)
    let p2_0 = t2.get_proof(0).unwrap();
    print!("PROOF_2_IDX0");
    for s in p2_0.audit_path() {
        match s {
            Some(h) => print!(" {}", hex(&Hash::from(*h))),
            None => print!(" NONE"),
        }
    }
    println!();
    let p2_1 = t2.get_proof(1).unwrap();
    print!("PROOF_2_IDX1");
    for s in p2_1.audit_path() {
        match s {
            Some(h) => print!(" {}", hex(&Hash::from(*h))),
            None => print!(" NONE"),
        }
    }
    println!();

    // 5-leaf (cm=0..=4), print root and proof for idx 3
    let l2 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([2u8; 32]);
    let l3 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([3u8; 32]);
    let l4 = MerkleTree::<[u8; 32]>::shielded_leaf_from_commitment([4u8; 32]);
    let t5: MerkleTree<[u8; 32]> = [l0, l1, l2, l3, l4].into_iter().collect();
    let r5 = t5.root().expect("root");
    println!("ROOT_5 {}", hex(&Hash::from(r5)));
    let p5_3 = t5.get_proof(3).unwrap();
    print!("PROOF_5_IDX3");
    for s in p5_3.audit_path() {
        match s {
            Some(h) => print!(" {}", hex(&Hash::from(*h))),
            None => print!(" NONE"),
        }
    }
    println!();
}
