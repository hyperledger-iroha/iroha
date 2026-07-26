use iroha_crypto::MerkleTree;
use ivm::ByteMerkleTree;

#[test]
fn byte_merkle_tree_matches_crypto_merkle_root() {
    // Generate random data and compare roots for chunk=32
    let mut data = vec![0u8; 1024 + 13];
    let mut state = 0xdecafbad_d00df00d_u64;
    for byte in &mut data {
        state ^= state << 7;
        state ^= state >> 9;
        state = state.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        *byte = state as u8;
    }
    let chunk = 32usize;

    // Build ByteMerkleTree root
    let bmt = ByteMerkleTree::from_bytes(&data, chunk);
    let root_a = bmt.root();

    // Build leaves as SHA256 of 32-byte chunks (last chunk zero-padded)
    let mut leaves = Vec::new();
    let mut i = 0usize;
    use sha2::{Digest, Sha256};
    while i < data.len() {
        let end = (i + chunk).min(data.len());
        let mut buf = [0u8; 32];
        buf[..end - i].copy_from_slice(&data[i..end]);
        let digest = Sha256::digest(buf);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        leaves.push(out);
        i += chunk;
    }
    if leaves.is_empty() {
        leaves.push([0u8; 32]);
    }
    let root_b = MerkleTree::from_hashed_leaves_sha256(leaves)
        .root()
        .expect("at least one leaf");
    assert_eq!(root_a, *root_b.as_ref());
}
