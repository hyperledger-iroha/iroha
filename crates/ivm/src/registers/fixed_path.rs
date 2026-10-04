//! Fixed register authentication paths copied from the canonical borrowed tree.

use super::{HashOf, MerkleTree, Registers, VMError, register_leaf_index};

/// Exact authentication depth of the canonical 256-register tree.
pub const REGISTER_MERKLE_PATH_DEPTH: usize = 8;

impl Registers {
    /// Return the eight canonical siblings for register `idx` without allocating.
    ///
    /// # Errors
    /// Returns [`VMError::RegisterOutOfBounds`] for an invalid register index.
    pub fn merkle_path(
        &self,
        idx: usize,
    ) -> Result<[[u8; 32]; REGISTER_MERKLE_PATH_DEPTH], VMError> {
        self.merkle_root_and_path(idx).map(|(_, path)| path)
    }

    /// Return the original root and eight leaf-to-root siblings without allocating.
    ///
    /// Dirty register nodes are rewritten in their existing fixed backing before
    /// the root and path are copied under the same original tree guard.
    ///
    /// # Errors
    /// Returns [`VMError::RegisterOutOfBounds`] for an invalid register index.
    pub fn merkle_root_and_path(
        &self,
        idx: usize,
    ) -> Result<
        (
            HashOf<MerkleTree<[u8; 32]>>,
            [[u8; 32]; REGISTER_MERKLE_PATH_DEPTH],
        ),
        VMError,
    > {
        let leaf_index = register_leaf_index(idx)?;
        let tree = self.ensure_built_and_lock();
        let root = tree.root().expect("the register tree is non-empty");
        let mut siblings = tree
            .proof_siblings(leaf_index)
            .expect("validated index belongs to the fixed register tree");
        assert_eq!(siblings.len(), REGISTER_MERKLE_PATH_DEPTH);
        let path = std::array::from_fn(|_| {
            *siblings
                .next()
                .expect("fixed path height was checked")
                .expect("a complete register tree has no absent siblings")
                .as_ref()
        });
        Ok((root, path))
    }
}

#[cfg(test)]
mod tests;
