//! Borrowed canonical authentication siblings without temporary path storage.

use std::iter::FusedIterator;

use super::{CompleteBinaryTree, MerkleTree};
use crate::HashOf;

struct ProofSiblings<'tree, T> {
    tree: &'tree MerkleTree<T>,
    index: usize,
    remaining: usize,
}

impl<T> MerkleTree<T> {
    /// Borrow the canonical leaf-to-root sibling sequence without allocating.
    ///
    /// `None` siblings retain the canonical right-edge promotion semantics.
    /// Returns `None` for an index outside the retained tree geometry, exactly
    /// as [`Self::get_proof`]. The immutable tree outlives every yielded view;
    /// the yielded fixed hashes are copied from that original owner.
    pub fn proof_siblings(
        &self,
        leaf_index: u32,
    ) -> Option<impl ExactSizeIterator<Item = Option<HashOf<T>>> + FusedIterator + '_> {
        Some(ProofSiblings {
            tree: self,
            index: self.index_in_tree(leaf_index as usize)?,
            remaining: self.height() as usize,
        })
    }
}

impl<T> Iterator for ProofSiblings<'_, T> {
    type Item = Option<HashOf<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let parent = self
            .tree
            .parent_index(self.index)
            .expect("immutable canonical tree height matches its parent chain");
        let sibling = self
            .tree
            .sibling_index(self.index)
            .and_then(|index| self.tree.get(index))
            .copied();
        self.index = parent;
        self.remaining -= 1;
        Some(sibling)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for ProofSiblings<'_, T> {}
impl<T> FusedIterator for ProofSiblings<'_, T> {}

#[cfg(test)]
mod tests;
