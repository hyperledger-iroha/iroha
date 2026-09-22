//! Bounded path replacement with caller-owned workspace and immutable storage.

use super::{
    Hash, MerkleMapNode, MerkleMapNodeRef, MerkleMapReadError, MerkleMapRoot, MerkleMapValueRef,
    common_prefix, prefix, raw_bit,
};
use crate::MerkleMapError;

/// One compare-and-replace operation against an authenticated map version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleMapEdit<V> {
    /// Canonical, domain-separated key hash.
    pub key: Hash,
    /// Expected value in the original version; `None` asserts absence.
    pub expected: Option<Hash>,
    /// Replacement value; `None` removes the key.
    pub after: Option<MerkleMapValueRef<V>>,
}

/// Caller-owned immutable storage addressed by explicit physical references.
///
/// Implementations must preserve existing bindings, including on failure or
/// unwind. A successful write must make the node available to subsequent reads.
/// Allocation admission, physical encoding, durability, provisional-node cleanup
/// and root publication belong to the caller. A returned root becomes durable
/// authority only after that owner completes its persistence protocol.
pub trait MerkleMapNodeStore {
    /// Fixed-size node location; its backing generation is retained by the owner.
    type NodeLocation: Copy;
    /// Fixed-size value location; value preimages remain the caller's concern.
    type ValueLocation: Copy;
    /// Original local storage or resource-admission failure.
    type Error;

    /// Read one exact content identity; missing storage is not key absence.
    ///
    /// # Errors
    /// Returns the original local failure without replacing it with absence.
    fn read(
        &mut self,
        reference: &MerkleMapNodeRef<Self::NodeLocation>,
    ) -> Result<Option<MerkleMapNode<Self::NodeLocation, Self::ValueLocation>>, Self::Error>;

    /// Append a node and return its explicit location, preserving old bindings.
    ///
    /// Children are already present when a branch is written. Repeated writes
    /// of the same node may be deduplicated. A failure may leave new unreachable
    /// nodes, but must never alter an existing version or publish a root.
    ///
    /// # Errors
    /// Returns the original local failure; the prepared root is not returned.
    fn write(
        &mut self,
        node: MerkleMapNode<Self::NodeLocation, Self::ValueLocation>,
    ) -> Result<Self::NodeLocation, Self::Error>;
}

/// Fixed path storage, explicitly owned and admitted by the update caller.
///
/// The complete allocation is `size_of::<Self>()`; construction and reuse need
/// no heap allocation. Admit this storage and the store's I/O/write resources
/// before allocating it. It never retains a resident history or storage handle.
/// Stale public descriptors may remain after a call, but each new operation
/// uses only its own authenticated path, including after error or unwind.
pub struct MerkleMapUpdateWorkspace<N: Copy, V: Copy> {
    path: [Option<(MerkleMapNodeRef<N>, MerkleMapNode<N, V>)>; super::MAX_PATH_NODES],
}

impl<N: Copy, V: Copy> MerkleMapUpdateWorkspace<N, V> {
    /// Maximum number of loaded nodes on one 256-bit key path.
    pub const NODE_CAPACITY: usize = super::MAX_PATH_NODES;

    /// Construct empty fixed-size storage without allocating from the heap.
    pub const fn new() -> Self {
        Self {
            path: [None; super::MAX_PATH_NODES],
        }
    }
}

impl<N: Copy, V: Copy> Default for MerkleMapUpdateWorkspace<N, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// A failed preparation leaves the original root and its nodes unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MerkleMapUpdateError<E> {
    /// The old version could not be authenticated and read.
    #[error(transparent)]
    Read(#[from] MerkleMapReadError<E>),
    /// The original value or checked entry count rejects this edit.
    #[error(transparent)]
    Edit(#[from] MerkleMapError),
    /// The store did not acknowledge a new immutable node.
    #[error("Merkle map node write failed: {0}")]
    Write(E),
}

impl<N: Copy> MerkleMapRoot<N> {
    /// Prepare a replacement version using one bounded authenticated path.
    ///
    /// Verifies the independently owned root, old value and entry-count
    /// arithmetic before any write. Exact no-ops return this root without
    /// writing. A changed path is emitted child before parent, retaining all
    /// untouched subtree references without loading them. At most 257 nodes
    /// are read and at most 257 written, independent of history size.
    ///
    /// This method never publishes a root or mutates this version. The caller
    /// must keep root authority and admission through its own durable commit;
    /// immutable provisional writes can remain unreachable on error or unwind.
    ///
    /// # Errors
    /// Returns authenticated-read, preimage/count or original write failures.
    pub fn replace<S: MerkleMapNodeStore<NodeLocation = N>>(
        &self,
        expected_root: &Hash,
        edit: MerkleMapEdit<S::ValueLocation>,
        workspace: &mut MerkleMapUpdateWorkspace<N, S::ValueLocation>,
        store: &mut S,
    ) -> Result<Self, MerkleMapUpdateError<S::Error>> {
        let mut path_len = 0;
        let actual = self
            .lookup(expected_root, &edit.key, |reference| {
                let node = store.read(reference)?;
                if let Some(node) = node {
                    // `lookup` makes at most NODE_CAPACITY calls, even for malformed
                    // storage. No recorded descriptor is used until it succeeds.
                    workspace.path[path_len] = Some((*reference, node));
                    path_len += 1;
                }
                Ok(node)
            })?
            .map(|value| value.hash);
        if actual != edit.expected {
            return Err(MerkleMapError::PreimageMismatch {
                expected: edit.expected,
                actual,
            }
            .into());
        }
        if actual == edit.after.map(|value| value.hash) {
            return Ok(*self);
        }
        let len = match (actual, edit.after) {
            (None, Some(_)) => self.len.checked_add(1),
            (Some(_), None) => self.len.checked_sub(1),
            _ => Some(self.len),
        }
        .ok_or(MerkleMapError::Capacity)?;
        let terminal = path_len.checked_sub(1).and_then(|i| workspace.path[i]);
        let mut replacement = if let Some(value) = edit.after {
            let mut reference = write_node(
                store,
                MerkleMapNode::Leaf {
                    key: edit.key,
                    value,
                },
            )?;
            if actual.is_none()
                && let Some((terminal_ref, terminal)) = terminal
            {
                let first = match &terminal {
                    MerkleMapNode::Leaf { key, .. } => key.as_ref(),
                    MerkleMapNode::Branch { prefix, .. } => prefix,
                };
                // Successful absence stopped at a divergent leaf or prefix, so
                // this new split strictly precedes the terminal's own depth.
                let bit = common_prefix(edit.key.as_ref(), first);
                let (left, right) = if raw_bit(edit.key.as_ref(), bit) {
                    (terminal_ref, reference)
                } else {
                    (reference, terminal_ref)
                };
                reference = write_node(
                    store,
                    MerkleMapNode::Branch {
                        bit,
                        prefix: prefix(&edit.key, bit),
                        left,
                        right,
                    },
                )?;
            }
            Some(reference)
        } else {
            None
        };
        for (_, node) in workspace.path[..path_len.saturating_sub(1)]
            .iter()
            .rev()
            .flatten()
        {
            if let MerkleMapNode::Branch {
                bit,
                prefix,
                left,
                right,
            } = *node
            {
                let take_right = raw_bit(edit.key.as_ref(), bit);
                replacement = Some(if let Some(child) = replacement {
                    write_node(
                        store,
                        MerkleMapNode::Branch {
                            bit,
                            prefix,
                            left: if take_right { left } else { child },
                            right: if take_right { child } else { right },
                        },
                    )?
                } else {
                    // Removing a leaf collapses its parent to the untouched
                    // sibling. That reference needs neither a read nor a write.
                    if take_right { left } else { right }
                });
            }
        }
        Ok(Self {
            len,
            node: replacement,
        })
    }
}

fn write_node<S: MerkleMapNodeStore>(
    store: &mut S,
    node: MerkleMapNode<S::NodeLocation, S::ValueLocation>,
) -> Result<MerkleMapNodeRef<S::NodeLocation>, MerkleMapUpdateError<S::Error>> {
    let hash = node.hash();
    let location = store.write(node).map_err(MerkleMapUpdateError::Write)?;
    Ok(MerkleMapNodeRef { hash, location })
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
