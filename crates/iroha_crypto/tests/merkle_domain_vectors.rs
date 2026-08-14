//! Golden vectors for consensus Merkle leaves and roots with domain tags.
//!
//! These vectors validate the finalized Merkle specification:
//! - Consensus hash: Blake2b-32 via `Hash::new` (with `Hash`'s LSB-invariant).
//! - Domain tags for leaves:
//!   - `TransactionEntrypoint`: "`iroha:merkle:tx_entry:v1\0`"
//!   - `TransactionResult`:    "`iroha:merkle:tx_result:v1\0`"
//!   - generic Merkle boundary: "`iroha:merkle:leaf:v1\0`"
//! - Inner nodes: Hash("`iroha:merkle:internal:v1\0`" || left || right)
//! - Odd leaf: promote left (no duplication)
use iroha_crypto::{Hash, HashOf, MerkleTree};
const TAG_TX_ENTRY: &[u8] = b"iroha:merkle:tx_entry:v1\x00";
fn hex_upper(h: &Hash) -> String {
    hex::encode_upper(h.as_ref())
}
fn leaf_hash(payload: &[u8]) -> Hash {
    Hash::new([TAG_TX_ENTRY, payload].concat())
}
#[test]
fn merkle_tx_entry_golden_vectors() {
    // Leaves for example payloads TX1..TX5
    let l1 = HashOf::<[u8; 32]>::from_untyped_unchecked(leaf_hash(b"TX1"));
    let l2 = HashOf::<[u8; 32]>::from_untyped_unchecked(leaf_hash(b"TX2"));
    let l3 = HashOf::<[u8; 32]>::from_untyped_unchecked(leaf_hash(b"TX3"));
    let l4 = HashOf::<[u8; 32]>::from_untyped_unchecked(leaf_hash(b"TX4"));
    let l5 = HashOf::<[u8; 32]>::from_untyped_unchecked(leaf_hash(b"TX5"));
    // Check leaf digests against golden vectors (from the spec)
    assert_eq!(
        hex_upper(&Hash::from(l1)),
        "39E2385BD17D8EEB4E2D188E04462A667B63D05FD78AD83E8382448DA92586AF"
    );
    assert_eq!(
        hex_upper(&Hash::from(l2)),
        "D4C9A98BD4B54A21D7B2FC3BAD664EE24744ECC1895FAA753A8C2781EB33464F"
    );
    assert_eq!(
        hex_upper(&Hash::from(l3)),
        "445094E8A2710968976FE4DE3FFE369643813514D147C0B66A03C05AA1D04BC1"
    );
    assert_eq!(
        hex_upper(&Hash::from(l4)),
        "E4EFAE9519B6D93AD04091CFA4426C776501FEC1E6E5EB34A93E624E928370AB"
    );
    assert_eq!(
        hex_upper(&Hash::from(l5)),
        "C11F8521B37737189040C425F8FB03D423F7B0B8DFBFD3DE5C9E746224326BA1"
    );
    // Empty block → no root
    let empty: MerkleTree<[u8; 32]> = [].into_iter().collect();
    assert!(empty.root().is_none());
    // One leaf → root = Hash(generic-leaf-domain || leaf1)
    let one: MerkleTree<[u8; 32]> = [l1].into_iter().collect();
    let root1 = one.root().expect("root");
    assert_eq!(
        hex_upper(&Hash::from(root1)),
        "456F653A4B11D8365FD11F548D358190F962DB4F08029CD11B18761E735FB9E9"
    );
    // Two leaves → Hash(internal-domain || domain(l1) || domain(l2))
    let two: MerkleTree<[u8; 32]> = [l1, l2].into_iter().collect();
    let root2 = two.root().expect("root");
    assert_eq!(
        hex_upper(&Hash::from(root2)),
        "8FAC977219F39BC9814C27646BCDC4E596495298F1B91591DCD0F5FF9A8B83FB"
    );
    // Three leaves (odd promotion on rightmost)
    let three: MerkleTree<[u8; 32]> = [l1, l2, l3].into_iter().collect();
    let root3 = three.root().expect("root");
    assert_eq!(
        hex_upper(&Hash::from(root3)),
        "CA262FB43518D436EB2911D23EE515E691F8D3BEECE65C17681CB4CCA44EC7C7"
    );
    // Four leaves (perfect tree)
    let four: MerkleTree<[u8; 32]> = [l1, l2, l3, l4].into_iter().collect();
    let root4 = four.root().expect("root");
    assert_eq!(
        hex_upper(&Hash::from(root4)),
        "A43A7568FBE96C94407E0C3DDF7AC2AB34C3861EA0FF0A102B75D90B212FA92B"
    );
    // Five leaves (deeper tree)
    let five: MerkleTree<[u8; 32]> = [l1, l2, l3, l4, l5].into_iter().collect();
    let root5 = five.root().expect("root");
    assert_eq!(
        hex_upper(&Hash::from(root5)),
        "B7152435918EA5C83D1CBE093F9FD720BF1A70CA1AD895ACA7DD08F798D91B39"
    );
}
