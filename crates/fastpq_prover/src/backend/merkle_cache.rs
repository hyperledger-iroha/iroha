//! Bounded reuse of exact native-STARK Merkle node computations within one verification.

use std::collections::BTreeMap;

use super::{MerkleTreeRoleV1, merkle_node_hash};
use crate::Result;
use fastpq_isi::GoldilocksDigest384V1;

/// Maximum number of native node digests retained by one verification call.
const MAX_CACHED_NODES: usize = 4_096;

/// Every input to the typed internal-node hash, including both complete children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NodeKey {
    role: MerkleTreeRoleV1,
    level: usize,
    parent_index: usize,
    left: GoldilocksDigest384V1,
    right: GoldilocksDigest384V1,
}

/// Cache exact node hashes across authenticated openings in one verification.
///
/// Construct this locally for each proof. Entries are merely hash computations;
/// they never stand in for checking the supplied path against its claimed root.
/// The fixed entry ceiling bounds retained memory, and saturation only disables
/// further insertion. Existing entries remain usable without an eviction policy.
pub(crate) struct MerkleNodeCache {
    nodes: BTreeMap<NodeKey, GoldilocksDigest384V1>,
    max_entries: usize,
    #[cfg(test)]
    hash_computations: usize,
}

impl Default for MerkleNodeCache {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            max_entries: MAX_CACHED_NODES,
            #[cfg(test)]
            hash_computations: 0,
        }
    }
}

impl MerkleNodeCache {
    /// Verify a complete authentication path while reusing identical node hashes.
    ///
    /// The canonical sole-leaf tree still requires a nonempty path. Residual high
    /// index bits and a root mismatch are rejected even when every node is cached.
    ///
    /// # Errors
    /// Returns an error when a typed internal-node digest cannot be framed.
    pub(crate) fn verify_path(
        &mut self,
        role: MerkleTreeRoleV1,
        root: GoldilocksDigest384V1,
        leaf: GoldilocksDigest384V1,
        leaf_index: usize,
        path: &[GoldilocksDigest384V1],
    ) -> Result<bool> {
        if path.is_empty() {
            return Ok(false);
        }
        let mut current = leaf;
        let mut index = leaf_index;
        for (level, &sibling) in path.iter().enumerate() {
            let parent_index = index / 2;
            let (left, right) = if index.is_multiple_of(2) {
                (current, sibling)
            } else {
                (sibling, current)
            };
            current = self.node_hash(NodeKey {
                role,
                level: level + 1,
                parent_index,
                left,
                right,
            })?;
            index = parent_index;
        }
        Ok(index == 0 && current == root)
    }

    fn node_hash(&mut self, key: NodeKey) -> Result<GoldilocksDigest384V1> {
        if let Some(&digest) = self.nodes.get(&key) {
            return Ok(digest);
        }
        let digest = merkle_node_hash(key.role, key.level, key.parent_index, key.left, key.right)?;
        #[cfg(test)]
        {
            self.hash_computations += 1;
        }
        if self.nodes.len() < self.max_entries {
            self.nodes.insert(key, digest);
        }
        Ok(digest)
    }

    #[cfg(test)]
    fn with_test_limit(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.min(MAX_CACHED_NODES),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        ExecutionMode, merkle_paths_for_leaf_indices, merkle_root_for_role,
        verify_merkle_path_for_role,
    };

    fn leaves(count: usize) -> Vec<GoldilocksDigest384V1> {
        (0..count)
            .map(|index| {
                let first = u64::try_from(index).expect("small test index") * 8;
                GoldilocksDigest384V1::new(std::array::from_fn(|lane| {
                    first + u64::try_from(lane).expect("six lanes") + 1
                }))
                .expect("small canonical test words")
            })
            .collect()
    }

    fn changed_lane(digest: GoldilocksDigest384V1, lane: usize) -> GoldilocksDigest384V1 {
        let mut words = digest.words();
        words[lane] = if words[lane] == super::super::GOLDILOCKS_MODULUS - 1 {
            0
        } else {
            words[lane] + 1
        };
        GoldilocksDigest384V1::new(words).expect("canonical replacement word")
    }

    fn paths(
        leaves: &[GoldilocksDigest384V1],
        role: MerkleTreeRoleV1,
    ) -> Vec<Vec<GoldilocksDigest384V1>> {
        let indices = (0..leaves.len()).collect::<Vec<_>>();
        merkle_paths_for_leaf_indices(leaves, &indices, role, ExecutionMode::Cpu)
            .expect("valid test leaf indices")
    }

    #[test]
    fn shared_paths_reuse_computed_nodes_and_match_uncached_verification() {
        let role = MerkleTreeRoleV1::AirTrace;
        let leaves = leaves(4);
        let root = merkle_root_for_role(&leaves, role).unwrap();
        let paths = paths(&leaves, role);
        let mut cache = MerkleNodeCache::default();
        assert_eq!(cache.max_entries, 4_096);
        for _ in 0..2 {
            for (index, (&leaf, path)) in leaves.iter().zip(&paths).enumerate() {
                let uncached = verify_merkle_path_for_role(role, root, leaf, index, path).unwrap();
                assert!(uncached);
                assert_eq!(
                    cache.verify_path(role, root, leaf, index, path).unwrap(),
                    uncached
                );
            }
            assert_eq!(
                cache.nodes.len(),
                3,
                "four leaves have three distinct parents"
            );
            assert_eq!(
                cache.hash_computations, 3,
                "shared paths reuse every parent"
            );
        }
    }

    #[test]
    fn changed_child_lanes_at_identical_coordinates_cannot_reuse_old_hashes() {
        let role = MerkleTreeRoleV1::Lde;
        let leaves = leaves(2);
        let root = merkle_root_for_role(&leaves, role).unwrap();
        let paths = paths(&leaves, role);
        let mut cache = MerkleNodeCache::default();
        assert!(
            cache
                .verify_path(role, root, leaves[0], 0, &paths[0])
                .unwrap()
        );
        for lane in 0..6 {
            for (leaf, path) in [
                (changed_lane(leaves[0], lane), paths[0].clone()),
                (leaves[0], vec![changed_lane(paths[0][0], lane)]),
            ] {
                let uncached = verify_merkle_path_for_role(role, root, leaf, 0, &path).unwrap();
                assert!(
                    !uncached,
                    "changing either complete child invalidates the root"
                );
                assert_eq!(
                    cache.verify_path(role, root, leaf, 0, &path).unwrap(),
                    uncached
                );
            }
        }
        assert_eq!(cache.nodes.len(), 13);
        assert_eq!(cache.hash_computations, 13);
        assert!(
            cache
                .verify_path(role, root, leaves[1], 1, &paths[1])
                .unwrap()
        );
        assert_eq!(
            cache.hash_computations, 13,
            "opposite branch orders the same children"
        );
    }

    #[test]
    fn tree_roles_and_full_fri_rounds_have_separate_entries() {
        let leaf = leaves(1)[0];
        let mut cache = MerkleNodeCache::default();
        let roles = [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(1),
            MerkleTreeRoleV1::Fri(u32::MAX),
        ];
        for (offset, role) in roles.into_iter().enumerate() {
            let root = merkle_root_for_role(&[leaf], role).unwrap();
            let path = [leaf];
            assert!(verify_merkle_path_for_role(role, root, leaf, 0, &path).unwrap());
            assert!(cache.verify_path(role, root, leaf, 0, &path).unwrap());
            assert_eq!(cache.nodes.len(), offset + 1);
            assert_eq!(cache.hash_computations, offset + 1);
        }
    }

    #[test]
    fn levels_and_parent_indices_are_exact_hash_inputs() {
        let children = leaves(2);
        let mut cache = MerkleNodeCache::default();
        for level in [1, 2] {
            for parent_index in [0, 1] {
                let key = NodeKey {
                    role: MerkleTreeRoleV1::Fri(7),
                    level,
                    parent_index,
                    left: children[0],
                    right: children[1],
                };
                let uncached =
                    merkle_node_hash(key.role, level, parent_index, key.left, key.right).unwrap();
                assert_eq!(cache.node_hash(key).unwrap(), uncached);
                assert_eq!(cache.node_hash(key).unwrap(), uncached);
            }
        }
        assert_eq!(cache.nodes.len(), 4);
        assert_eq!(cache.hash_computations, 4);
    }

    #[test]
    fn cached_paths_still_reject_empty_paths_high_indices_and_wrong_roots() {
        let role = MerkleTreeRoleV1::AirComposition;
        let leaves = leaves(4);
        let root = merkle_root_for_role(&leaves, role).unwrap();
        let paths = paths(&leaves, role);
        let mut cache = MerkleNodeCache::default();
        assert!(
            !cache
                .verify_path(role, leaves[0], leaves[0], 0, &[])
                .unwrap()
        );
        assert_eq!(cache.hash_computations, 0);
        assert!(
            cache
                .verify_path(role, root, leaves[0], 0, &paths[0])
                .unwrap()
        );
        let warmed_computations = cache.hash_computations;
        let wrong_root = changed_lane(root, 5);
        assert!(
            !cache
                .verify_path(role, wrong_root, leaves[0], 0, &paths[0])
                .unwrap()
        );
        assert_eq!(cache.hash_computations, warmed_computations);
        for invalid_index in [4, 8, usize::MAX] {
            let uncached =
                verify_merkle_path_for_role(role, root, leaves[0], invalid_index, &paths[0])
                    .unwrap();
            assert!(!uncached);
            assert_eq!(
                cache
                    .verify_path(role, root, leaves[0], invalid_index, &paths[0])
                    .unwrap(),
                uncached
            );
        }
    }

    #[test]
    fn odd_leaf_counts_and_the_sole_leaf_match_uncached_verification() {
        let role = MerkleTreeRoleV1::Fri(3);
        let mut cache = MerkleNodeCache::default();
        for count in [1, 3, 5, 9] {
            let leaves = leaves(count);
            let root = merkle_root_for_role(&leaves, role).unwrap();
            let paths = paths(&leaves, role);
            for (index, (&leaf, path)) in leaves.iter().zip(&paths).enumerate() {
                let uncached = verify_merkle_path_for_role(role, root, leaf, index, path).unwrap();
                assert!(uncached);
                assert_eq!(
                    cache.verify_path(role, root, leaf, index, path).unwrap(),
                    uncached
                );
            }
        }
    }

    #[test]
    fn capacity_exhaustion_changes_only_hash_reuse() {
        let role = MerkleTreeRoleV1::AirTrace;
        let leaves = leaves(8);
        let root = merkle_root_for_role(&leaves, role).unwrap();
        let paths = paths(&leaves, role);
        for limit in [0, 2] {
            let mut cache = MerkleNodeCache::with_test_limit(limit);
            assert!(
                cache
                    .verify_path(role, root, leaves[0], 0, &paths[0])
                    .unwrap()
            );
            let retained = cache.nodes.clone();
            assert_eq!(retained.len(), limit);
            let previous_computations = cache.hash_computations;
            for (index, (&leaf, path)) in leaves.iter().zip(&paths).enumerate() {
                for candidate in [leaf, changed_lane(leaf, 5)] {
                    let uncached =
                        verify_merkle_path_for_role(role, root, candidate, index, path).unwrap();
                    assert_eq!(
                        cache
                            .verify_path(role, root, candidate, index, path)
                            .unwrap(),
                        uncached
                    );
                }
            }
            assert_eq!(
                cache.nodes, retained,
                "no insertion or eviction after saturation"
            );
            assert!(cache.hash_computations > previous_computations);
        }
        assert_eq!(
            MerkleNodeCache::with_test_limit(MAX_CACHED_NODES + 1).max_entries,
            MAX_CACHED_NODES
        );
    }
}
