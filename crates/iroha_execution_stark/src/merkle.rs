//! BLAKE2b-384 binary Merkle trees with compact batch openings.
//!
//! Leaves and inner nodes use distinct framed domains, so a leaf can never be confused with an
//! inner node. A batch opening sends, level by level in ascending index order, only the siblings
//! that the verifier cannot recompute from the opened leaves.

use rayon::prelude::*;

pub use crate::hash::Digest;
use crate::hash::hash;

const LEAF_CONTEXT: &[u8] = b"merkle leaf";
const NODE_CONTEXT: &[u8] = b"merkle node";

/// Hashes one leaf's canonical bytes.
pub fn hash_leaf(bytes: &[u8]) -> Digest {
    hash(LEAF_CONTEXT, &[bytes])
}

/// Hashes two children.
pub fn hash_node(left: &Digest, right: &Digest) -> Digest {
    hash(NODE_CONTEXT, &[left, right])
}

/// A complete binary tree over a power-of-two number of leaf digests.
#[derive(Clone, Debug)]
pub struct MerkleTree {
    /// `levels[0]` are the leaves; the last level holds the root.
    levels: Vec<Vec<Digest>>,
}

impl MerkleTree {
    /// Builds the tree.
    ///
    /// # Panics
    /// Panics unless the number of leaves is a nonzero power of two.
    pub fn new(leaves: Vec<Digest>) -> Self {
        assert!(
            leaves.len().is_power_of_two(),
            "leaf count must be a power of two"
        );
        let mut levels = vec![leaves];
        while levels.last().expect("nonempty").len() > 1 {
            let below = levels.last().expect("nonempty");
            let above: Vec<Digest> = if below.len() >= 1024 {
                below
                    .par_chunks_exact(2)
                    .map(|pair| hash_node(&pair[0], &pair[1]))
                    .collect()
            } else {
                below
                    .chunks_exact(2)
                    .map(|pair| hash_node(&pair[0], &pair[1]))
                    .collect()
            };
            levels.push(above);
        }
        Self { levels }
    }

    /// The root digest.
    pub fn root(&self) -> Digest {
        self.levels.last().expect("nonempty")[0]
    }

    /// `log2` of the leaf count.
    pub fn depth(&self) -> u32 {
        (self.levels.len() - 1) as u32
    }

    /// Siblings needed to authenticate the sorted, deduplicated leaf `indices`.
    pub fn open_batch(&self, indices: &[usize]) -> Vec<Digest> {
        let mut known: Vec<usize> = indices.to_vec();
        let mut siblings = Vec::new();
        for level in &self.levels[..self.levels.len() - 1] {
            let mut next = Vec::with_capacity(known.len());
            let mut i = 0;
            while i < known.len() {
                let index = known[i];
                let sibling = index ^ 1;
                if i + 1 < known.len() && known[i + 1] == sibling {
                    i += 2;
                } else {
                    siblings.push(level[sibling]);
                    i += 1;
                }
                next.push(index >> 1);
            }
            next.dedup();
            known = next;
        }
        siblings
    }
}

/// Verifies a batch opening produced by [`MerkleTree::open_batch`].
///
/// `leaves` must be sorted by index without duplicates.
pub fn verify_batch(
    root: &Digest,
    depth: u32,
    leaves: &[(usize, Digest)],
    siblings: &[Digest],
) -> bool {
    if leaves.is_empty() || leaves.windows(2).any(|w| w[0].0 >= w[1].0) {
        return false;
    }
    if leaves
        .iter()
        .any(|(index, _)| (*index as u128) >= (1u128 << depth))
    {
        return false;
    }
    let mut current: Vec<(usize, Digest)> = leaves.to_vec();
    let mut cursor = 0usize;
    for _ in 0..depth {
        let mut next = Vec::with_capacity(current.len());
        let mut i = 0;
        while i < current.len() {
            let (index, digest) = current[i];
            let sibling_index = index ^ 1;
            let sibling = if i + 1 < current.len() && current[i + 1].0 == sibling_index {
                i += 2;
                current[i - 1].1
            } else {
                let Some(sibling) = siblings.get(cursor) else {
                    return false;
                };
                cursor += 1;
                i += 1;
                *sibling
            };
            let parent = if index & 1 == 0 {
                hash_node(&digest, &sibling)
            } else {
                hash_node(&sibling, &digest)
            };
            next.push((index >> 1, parent));
        }
        current = next;
    }
    cursor == siblings.len() && current.len() == 1 && current[0].0 == 0 && current[0].1 == *root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::DIGEST_BYTES;

    fn leaves(n: usize) -> Vec<Digest> {
        (0..n)
            .map(|i| hash_leaf(&(i as u64).to_le_bytes()))
            .collect()
    }

    #[test]
    fn batch_openings_verify_and_reject_tampering() {
        let tree = MerkleTree::new(leaves(64));
        let root = tree.root();
        for indices in [
            vec![0usize],
            vec![1, 2, 3],
            vec![5, 9, 33, 63],
            (0..64).collect(),
        ] {
            let siblings = tree.open_batch(&indices);
            let opened: Vec<(usize, Digest)> =
                indices.iter().map(|&i| (i, tree.levels[0][i])).collect();
            assert!(verify_batch(&root, tree.depth(), &opened, &siblings));
            let mut bad = opened.clone();
            bad[0].1[0] ^= 1;
            assert!(!verify_batch(&root, tree.depth(), &bad, &siblings));
            if !siblings.is_empty() {
                let mut bad_siblings = siblings.clone();
                bad_siblings[0][DIGEST_BYTES - 1] ^= 1;
                assert!(!verify_batch(&root, tree.depth(), &opened, &bad_siblings));
                assert!(!verify_batch(&root, tree.depth(), &opened, &siblings[1..]));
            }
            let mut extra = siblings.clone();
            extra.push([0; DIGEST_BYTES]);
            assert!(!verify_batch(&root, tree.depth(), &opened, &extra));
        }
        assert!(!verify_batch(
            &root,
            tree.depth(),
            &[(64, [0; DIGEST_BYTES])],
            &[]
        ));
    }

    #[test]
    fn leaf_and_node_domains_differ() {
        let a = [1u8; DIGEST_BYTES];
        let mut both = [0u8; 2 * DIGEST_BYTES];
        both[..DIGEST_BYTES].copy_from_slice(&a);
        both[DIGEST_BYTES..].copy_from_slice(&a);
        assert_ne!(hash_leaf(&both), hash_node(&a, &a));
    }

    #[test]
    fn merkle_hashes_match_independent_vectors() {
        let hex = |bytes: &[u8]| -> String {
            use std::fmt::Write as _;
            bytes.iter().fold(String::new(), |mut output, byte| {
                write!(output, "{byte:02x}").expect("string write");
                output
            })
        };
        assert_eq!(
            hex(&hash_leaf(b"abc")),
            "ddc3caf6375624653864bd1a11e5fe5e2d098a7e29683d6f3fbeb6bf10eb684cca463ebd81494bc22db11921af27cdd7"
        );
        assert_eq!(
            hex(&hash_node(&[1; DIGEST_BYTES], &[2; DIGEST_BYTES])),
            "42e6dbd4105ba166cefbeb3090fd7f96c043002e8c6844fd11de072832729d6a8b29fee451fe31e05626fff64ba1648e"
        );
    }
}
