//! Fixed-capacity inclusion and absence paths for one authenticated map root.
//!
//! A path contains only the nodes visited by one key. Its caller must obtain
//! the expected root from an independent owner; neither the proof nor its
//! claimed root establishes State finality or complete table enumeration.

use std::convert::Infallible;

use super::{
    Hash, MerkleMap, MerkleMapNode, MerkleMapNodeRef, MerkleMapReadError, MerkleMapRoot,
    MerkleMapValueRef, NodeKind, common_bits, key_bit, prefix,
};

const MAX_LOOKUP_PATH_NODES: usize = Hash::LENGTH * 8 + 1;
const UNVISITED: u16 = u16::MAX;
// The bounded proof remains caller-owned and allocation-free. Keep its large
// fixed array explicit instead of introducing a fallible heap-backed proof.
static EMPTY_LOOKUP_NODES: [Option<MerkleMapNode<u16, ()>>; MAX_LOOKUP_PATH_NODES] =
    [None; MAX_LOOKUP_PATH_NODES];

/// One bounded authenticated lookup path for a 256-bit-key Merkle map.
///
/// Physical node locations are path indices, excluded from logical hashes.
/// The fixed array admits at most one node per key bit plus a terminal leaf;
/// unused slots cannot be interpreted as authenticated absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleMapLookupProof {
    entries: u64,
    top: Option<Hash>,
    used: u16,
    nodes: [Option<MerkleMapNode<u16, ()>>; MAX_LOOKUP_PATH_NODES],
}

impl MerkleMapLookupProof {
    /// Return the map entry count claimed by this untrusted path.
    pub fn entry_count(&self) -> u64 {
        self.entries
    }

    /// Compute the map root claimed by this path, without authenticating it.
    pub fn claimed_root(&self) -> Hash {
        MerkleMapRoot::from_parts(
            self.entries,
            self.top.map(|hash| MerkleMapNodeRef { hash, location: 0 }),
        )
        .hash()
    }

    /// Authenticate inclusion or absence against an independently owned root.
    ///
    /// A missing path node, malformed descriptor, unused trailing node, or
    /// inconsistent root is an error, never evidence of key absence. The
    /// verifier uses the same canonical compressed-path rules as external-node
    /// lookup and performs at most 257 node reads.
    ///
    /// # Errors
    /// Returns a root, node-content or path error on a forged or incomplete
    /// proof. `Ok(None)` is reserved for authenticated key absence.
    pub fn verify(
        &self,
        expected_root: &Hash,
        key: &Hash,
    ) -> Result<Option<Hash>, MerkleMapReadError<Infallible>> {
        let used = usize::from(self.used);
        if used > MAX_LOOKUP_PATH_NODES
            || self.nodes[used..].iter().any(Option::is_some)
            || (self.top.is_some() != (used != 0))
        {
            return Err(MerkleMapReadError::InvalidPath);
        }
        let root = MerkleMapRoot::from_parts(
            self.entries,
            self.top.map(|hash| MerkleMapNodeRef { hash, location: 0 }),
        );
        let mut read = 0_usize;
        let value = root.lookup::<(), Infallible>(expected_root, key, |reference| {
            read += 1;
            Ok(self
                .nodes
                .get(usize::from(reference.location))
                .filter(|_| usize::from(reference.location) < used)
                .copied()
                .flatten())
        })?;
        if read != used {
            return Err(MerkleMapReadError::InvalidPath);
        }
        Ok(value.map(|value| value.hash))
    }
}

impl MerkleMap {
    /// Export only the compressed search path for one public key.
    ///
    /// Cloned immutable map versions can export their own paths independently.
    /// This method performs no allocation and does not confer root authority.
    pub fn prove_lookup(&self, key: &Hash) -> MerkleMapLookupProof {
        let mut proof = MerkleMapLookupProof {
            entries: self.len,
            top: self.node.as_ref().map(|node| node.hash),
            used: 0,
            nodes: EMPTY_LOOKUP_NODES,
        };
        let mut node = self.node.as_deref();
        while let Some(current) = node {
            let index = usize::from(proof.used);
            assert!(
                index < MAX_LOOKUP_PATH_NODES,
                "a 256-bit key has a bounded path"
            );
            let (descriptor, next) = match &current.kind {
                NodeKind::Leaf(value) => (
                    MerkleMapNode::Leaf {
                        key: current.first,
                        value: MerkleMapValueRef {
                            hash: *value,
                            location: (),
                        },
                    },
                    None,
                ),
                NodeKind::Branch { bit, left, right } => {
                    let next =
                        (common_bits(key, &current.first) >= *bit).then(|| key_bit(key, *bit));
                    let location =
                        u16::try_from(index + 1).expect("a 256-bit key path fits a u16 location");
                    (
                        MerkleMapNode::Branch {
                            bit: *bit,
                            prefix: prefix(&current.first, *bit),
                            left: MerkleMapNodeRef {
                                hash: left.hash,
                                location: if next == Some(false) {
                                    location
                                } else {
                                    UNVISITED
                                },
                            },
                            right: MerkleMapNodeRef {
                                hash: right.hash,
                                location: if next == Some(true) {
                                    location
                                } else {
                                    UNVISITED
                                },
                            },
                        },
                        next.map(|rightward| if rightward { &**right } else { &**left }),
                    )
                }
            };
            proof.nodes[index] = Some(descriptor);
            proof.used += 1;
            node = next;
        }
        proof
    }
}

#[cfg(test)]
#[path = "proof/tests.rs"]
mod tests;
