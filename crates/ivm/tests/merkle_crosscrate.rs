use ivm::ByteMerkleTree;
use sha2::{Digest, Sha256};

// Compare ByteMerkleTree root to canonical iroha_crypto::MerkleTree root for the same hashed leaves.
#[test]
fn merkle_root_matches_canonical_for_randomized_leaves() {
    // Deterministic pseudo-random generator to avoid external deps
    fn prng(mut x: u64) -> impl Iterator<Item = u64> {
        std::iter::from_fn(move || {
            // xorshift64*
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            Some(x.wrapping_mul(0x2545F4914F6CDD1D))
        })
    }

    // Build a tree with a moderate number of leaves to keep the test fast
    const CHUNK: usize = 32;
    const N: usize = 4096; // 4096 leaves → 131072 bytes

    // Prepare canonical leaves vector initialized to zero-hash as ByteMerkleTree does
    let zero_hash = {
        let buf = [0u8; CHUNK];
        let d = Sha256::digest(buf);
        let mut out = [0u8; 32];
        out.copy_from_slice(&d);
        out
    };
    let mut leaves = vec![zero_hash; N];

    // Build a ByteMerkleTree and update a few pseudo-random leaves with pseudo-random bytes
    let tree = ByteMerkleTree::new(N, CHUNK);
    let mut it = prng(0xDEADBEEFCAFEBABEu64);
    for _ in 0..32 {
        let idx = (it.next().unwrap() as usize) % N;
        let mut buf = [0u8; CHUNK];
        // fill with PRNG bytes
        for b in &mut buf {
            *b = (it.next().unwrap() >> 56) as u8;
        }
        tree.update_leaf(idx, &buf);
        // Mirror the leaf hash into the canonical vector
        let d = Sha256::digest(buf);
        let mut out = [0u8; 32];
        out.copy_from_slice(&d);
        leaves[idx] = out;
    }

    // Compute roots: ByteMerkleTree vs canonical MerkleTree
    let got = tree.root();
    let canonical = iroha_crypto::MerkleTree::from_hashed_leaves_sha256(leaves)
        .root()
        .expect("non-empty");
    assert_eq!(&got[..], canonical.as_ref());
}
