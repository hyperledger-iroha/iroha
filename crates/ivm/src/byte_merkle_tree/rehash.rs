//! Fixed retained-leaf updates with original caller policy and locked acceptance.

use super::{ByteMerkleTree, cpu_leaves};
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
use super::{CanonicalNodes, MerkleLeaves};
use crate::vector::{Sha256Context, Sha256Observed};
use rayon::prelude::*;

/// Locked original destination, still unchanged until its owner accepts it.
/// Neither a readback digest nor a caller can replace the retained tree backing.
#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
pub(crate) struct LockedLeafUpdate<'tree, 'input> {
    tree: &'tree ByteMerkleTree,
    digests: &'input [[u8; 32]],
    // Release leaves before nodes when acceptance declines or unwinds.
    leaves: parking_lot::MutexGuard<'tree, MerkleLeaves>,
    nodes: parking_lot::MutexGuard<'tree, CanonicalNodes>,
}

#[cfg(any(test, all(target_os = "macos", feature = "metal")))]
impl LockedLeafUpdate<'_, '_> {
    /// Copy only after original device/configuration acceptance, then complete
    /// canonical-node refresh while the caller still holds that original owner.
    pub(crate) fn install(self) {
        let Self {
            tree,
            digests,
            mut leaves,
            mut nodes,
        } = self;
        leaves.copy_from_slice(digests);
        nodes.mark_stale();
        drop(leaves);
        tree.refresh_locked(&mut nodes);
    }
}

impl ByteMerkleTree {
    /// Shared actual in-place CPU traversal, retaining the existing lazy node
    /// refresh. Ordinary work reduces the same observation; synthetic work
    /// changes only the completion bank. The complete cost includes root().
    pub(crate) fn rehash_parallel_in_context(
        &self,
        data: &[u8],
        context: Sha256Context,
    ) -> Sha256Observed {
        let mut nodes = self.nodes.lock();
        let mut leaves = self.leaves.lock();
        let observed = leaves
            .par_iter_mut()
            .enumerate()
            .map(|(index, leaf)| {
                let (digest, observed) =
                    cpu_leaves::digest_leaf(data, index, self.chunk, self.zero_hash, context);
                *leaf = digest;
                observed
            })
            .reduce(Sha256Observed::default, Sha256Observed::merge);
        let observed = std::hint::black_box(observed);
        nodes.mark_stale();
        observed
    }

    /// Retain both original destination locks before checking device acceptance.
    /// A malformed result or dropped update never changes either representation.
    #[cfg(any(test, all(target_os = "macos", feature = "metal")))]
    pub(crate) fn lock_leaf_update<'tree, 'input>(
        &'tree self,
        digests: &'input [[u8; 32]],
    ) -> Option<LockedLeafUpdate<'tree, 'input>> {
        let nodes = self.nodes.lock();
        let leaves = self.leaves.lock();
        if leaves.len() != digests.len() {
            return None;
        }
        Some(LockedLeafUpdate {
            tree: self,
            digests,
            leaves,
            nodes,
        })
    }
}

#[cfg(test)]
mod tests;
