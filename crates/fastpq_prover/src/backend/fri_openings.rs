//! Read-only Merkle levels shared by all openings of each FRI layer.

use super::{ExecutionMode, MerkleTreeRoleV1, build_merkle_levels_with_mode};
use crate::{Error, Result};
use fastpq_isi::GoldilocksDigest384V1;

struct LayerTree {
    leaves: Vec<GoldilocksDigest384V1>,
    levels: Option<Vec<Vec<GoldilocksDigest384V1>>>,
}

/// Retain each layer tree, preserving query validation and path order.
pub(super) struct FriOpeningTrees {
    layers: Vec<LayerTree>,
    mode: ExecutionMode,
    #[cfg(test)]
    tree_builds: usize,
}

impl FriOpeningTrees {
    /// Retain layer leaves until their first requested authentication path.
    pub(super) fn new(leaves: Vec<Vec<GoldilocksDigest384V1>>, mode: ExecutionMode) -> Self {
        Self {
            layers: leaves
                .into_iter()
                .map(|leaves| LayerTree {
                    leaves,
                    levels: None,
                })
                .collect(),
            mode,
            #[cfg(test)]
            tree_builds: 0,
        }
    }

    /// Retain already committed levels without rehashing or cloning their leaves.
    ///
    /// The caller owns the trusted trees produced by the native builder and
    /// must preserve their committed round order. This constructor checks only
    /// stored geometry; it does not authenticate external digests or tree roles.
    pub(super) fn from_levels(
        levels: Vec<Vec<Vec<GoldilocksDigest384V1>>>,
        mode: ExecutionMode,
    ) -> Result<Self> {
        for tree in &levels {
            if tree.len() < 2 || tree.last().map(Vec::len) != Some(1) {
                return Err(crate::Error::InvalidTraceShape {
                    details: "retained FRI tree requires one final root".to_owned(),
                });
            }
            for pair in tree.windows(2) {
                let width = pair[0].len();
                let parents = width / 2;
                let expected = if parents > 1 && !parents.is_multiple_of(2) {
                    parents + 1
                } else {
                    parents
                };
                if width < 2 || !width.is_multiple_of(2) || pair[1].len() != expected {
                    return Err(crate::Error::InvalidTraceShape {
                        details: "retained FRI tree has an invalid stored level width".to_owned(),
                    });
                }
            }
        }
        Ok(Self {
            layers: levels
                .into_iter()
                .map(|levels| LayerTree {
                    leaves: Vec::new(),
                    levels: Some(levels),
                })
                .collect(),
            mode,
            #[cfg(test)]
            tree_builds: 0,
        })
    }

    /// Count only lazy builds performed after cache creation.
    #[cfg(test)]
    pub(super) fn tree_build_count(&self) -> usize {
        self.tree_builds
    }

    /// Read a canonical leaf path, constructing its FRI layer on first use.
    ///
    /// Retained levels use the existing builder's duplicate-last padding. No
    /// tree or internal node is rebuilt for a later path in the same layer.
    ///
    /// # Errors
    /// Returns the existing coordinate or node-hash error for an invalid path.
    pub(super) fn path(
        &mut self,
        round: usize,
        leaf_index: usize,
    ) -> Result<Vec<GoldilocksDigest384V1>> {
        let role = MerkleTreeRoleV1::Fri(
            u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
        );
        let layer_count = self.layers.len();
        let layer = self
            .layers
            .get_mut(round)
            .ok_or(Error::QueryIndexOutOfRange {
                index: round,
                len: layer_count,
            })?;
        if layer.levels.is_none() {
            if layer.leaves.is_empty() {
                return Err(Error::QueryIndexOutOfRange {
                    index: leaf_index,
                    len: 0,
                });
            }
            #[cfg(test)]
            {
                self.tree_builds += 1;
            }
            layer.levels = Some(build_merkle_levels_with_mode(
                &layer.leaves,
                role,
                self.mode,
            )?);
            // The first retained level already contains the padded leaves.
            // Release the original allocation instead of retaining it twice.
            layer.leaves = Vec::new();
        }
        let levels = layer.levels.as_ref().expect("initialized Merkle levels");
        let leaf_count = levels.first().expect("nonempty leaf layer").len();
        if leaf_index >= leaf_count {
            return Err(Error::QueryIndexOutOfRange {
                index: leaf_index,
                len: leaf_count,
            });
        }
        let mut index = leaf_index;
        let mut path = Vec::with_capacity(levels.len().saturating_sub(1));
        for level in levels.iter().take(levels.len().saturating_sub(1)) {
            let sibling_index = if index.is_multiple_of(2) {
                index + 1
            } else {
                index.saturating_sub(1)
            };
            path.push(level.get(sibling_index).copied().unwrap_or(level[index]));
            index /= 2;
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        FriDomain, GoldilocksFp4V1, fold_round, hash_fri_leaves_with_mode,
        hash_fri_terminal_leaves, merkle_paths_for_leaf_indices, merkle_root_for_role,
        open_fri_query_chains, verify_merkle_path_for_role,
    };
    use fastpq_isi::FASTPQ_FINAL_V1;
    use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;

    fn layer_values() -> Vec<Vec<GoldilocksFp4V1>> {
        let params = FASTPQ_FINAL_V1;
        let mut domain =
            FriDomain::from_lde_parameters(params.lde_root, params.lde_log_size, 32, 7).unwrap();
        let mut layers = vec![
            (0..32)
                .map(|index| {
                    let x = GoldilocksFp4V1::from_base(domain.point(index)).unwrap();
                    GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap().add(x.mul(x))
                })
                .collect::<Vec<_>>(),
        ];
        while layers.last().unwrap().len() > 4 {
            let beta = GoldilocksFp4V1::new([5, 6, 7, 8]).unwrap();
            let next = fold_round(layers.last().unwrap(), 2, beta, domain).unwrap();
            layers.push(next);
            domain = domain.folded(2);
        }
        layers
    }

    fn layer_leaves(values: &[Vec<GoldilocksFp4V1>]) -> Vec<Vec<GoldilocksDigest384V1>> {
        values
            .iter()
            .enumerate()
            .map(|(round, layer)| {
                if round + 1 == values.len() {
                    hash_fri_terminal_leaves(round, layer).unwrap()
                } else {
                    hash_fri_leaves_with_mode(round, layer, 2, ExecutionMode::Cpu).unwrap()
                }
            })
            .collect()
    }

    #[test]
    fn multiple_query_chains_preserve_canonical_strided_paths_and_duplicates() {
        let values = layer_values();
        let leaves = layer_leaves(&values);
        // Upper-half queries and nonzero strided-group positions exercise both
        // x and -x on the nontrivial input coset; order and duplicates are kept.
        let indices = [0, 19, 7, 31, 19, 16, 3];
        let openings = open_fri_query_chains(&values, &indices, 2, ExecutionMode::Cpu).unwrap();
        assert_eq!(openings.len(), indices.len());
        assert_eq!(openings[1], openings[4]);
        for (&initial_index, opening) in indices.iter().zip(openings) {
            assert_eq!(opening.initial_index as usize, initial_index);
            let mut index = initial_index;
            for (round, opened) in opening.rounds.iter().enumerate() {
                let output_len = values[round].len() / 2;
                let leaf_index = index % output_len;
                let expected_values = vec![
                    values[round][leaf_index],
                    values[round][leaf_index + output_len],
                ];
                assert_eq!(opened.round as usize, round);
                assert_eq!(opened.index as usize, index);
                assert_eq!(opened.values, expected_values);
                assert_eq!(opened.folded_value, values[round + 1][leaf_index]);
                let role = MerkleTreeRoleV1::Fri(round as u32);
                let canonical = merkle_paths_for_leaf_indices(
                    &leaves[round],
                    &[leaf_index],
                    role,
                    ExecutionMode::Cpu,
                )
                .unwrap()
                .remove(0);
                assert_eq!(
                    opened.merkle_path,
                    canonical
                        .iter()
                        .copied()
                        .map(WireDigest::from)
                        .collect::<Vec<_>>()
                );
                assert!(
                    verify_merkle_path_for_role(
                        role,
                        merkle_root_for_role(&leaves[round], role).unwrap(),
                        leaves[round][leaf_index],
                        leaf_index,
                        &canonical,
                    )
                    .unwrap()
                );
                index = leaf_index;
            }
            let final_round = leaves.len() - 1;
            let canonical = merkle_paths_for_leaf_indices(
                &leaves[final_round],
                &[0],
                MerkleTreeRoleV1::Fri(final_round as u32),
                ExecutionMode::Cpu,
            )
            .unwrap()
            .remove(0);
            assert_eq!(opening.final_index as usize, index);
            assert_eq!(opening.final_values, *values.last().unwrap());
            assert_eq!(
                opening.final_merkle_path,
                canonical
                    .into_iter()
                    .map(WireDigest::from)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn terminal_only_chains_authenticate_the_same_complete_leaf() {
        let values = vec![layer_values().pop().unwrap()];
        let indices = [3, 0, 2, 3];
        let openings = open_fri_query_chains(&values, &indices, 2, ExecutionMode::Cpu).unwrap();
        let leaves = layer_leaves(&values);
        let canonical = merkle_paths_for_leaf_indices(
            &leaves[0],
            &[0],
            MerkleTreeRoleV1::Fri(0),
            ExecutionMode::Cpu,
        )
        .unwrap()
        .remove(0);
        for (&index, opening) in indices.iter().zip(openings) {
            assert_eq!(opening.initial_index as usize, index);
            assert_eq!(opening.final_index as usize, index);
            assert!(opening.rounds.is_empty());
            assert_eq!(opening.final_values, values[0]);
            assert_eq!(
                opening.final_merkle_path,
                canonical
                    .iter()
                    .copied()
                    .map(WireDigest::from)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn actual_tree_builds_are_independent_of_query_count() {
        let values = layer_values();
        let leaves = layer_leaves(&values);
        let mut trees = FriOpeningTrees::new(leaves.clone(), ExecutionMode::Cpu);
        assert_eq!(trees.tree_builds, 0, "construction itself hashes no tree");
        for repetition in 0..3 {
            for (round, layer) in leaves.iter().enumerate() {
                for leaf_index in (0..layer.len()).chain([0, 0]) {
                    let actual = trees.path(round, leaf_index).unwrap();
                    if repetition == 0 {
                        let canonical = merkle_paths_for_leaf_indices(
                            layer,
                            &[leaf_index],
                            MerkleTreeRoleV1::Fri(round as u32),
                            ExecutionMode::Cpu,
                        )
                        .unwrap()
                        .remove(0);
                        assert_eq!(actual, canonical);
                    }
                }
            }
            assert_eq!(trees.tree_builds, leaves.len());
            assert!(trees.layers.iter().all(|layer| layer.leaves.is_empty()));
            // The canonical builder hashes one parent for each pair in every
            // non-root level. These 16/8/4/1-leaf layers require 26 parents;
            // further queries read their levels without adding any tree builds.
            let parent_count: usize = trees
                .layers
                .iter()
                .map(|layer| {
                    let levels = layer.levels.as_ref().unwrap();
                    levels[..levels.len() - 1]
                        .iter()
                        .map(|level| level.len() / 2)
                        .sum::<usize>()
                })
                .sum();
            assert_eq!(parent_count, 26);
        }
    }

    #[test]
    fn retained_levels_preserve_all_paths_without_any_lazy_builds() {
        let mut leaves = layer_leaves(&layer_values());
        // Exercise both odd-leaf and odd-parent duplicate-last padding in the
        // generic builder, in addition to every actual FRI/terminal geometry.
        leaves.push(
            (0..5)
                .map(|index| GoldilocksDigest384V1::new([index; 6]).unwrap())
                .collect(),
        );
        let levels = leaves
            .iter()
            .enumerate()
            .map(|(round, layer)| {
                build_merkle_levels_with_mode(
                    layer,
                    MerkleTreeRoleV1::Fri(round as u32),
                    ExecutionMode::Cpu,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let padded_lengths = levels.iter().map(|tree| tree[0].len()).collect::<Vec<_>>();
        let mut trees = FriOpeningTrees::from_levels(levels, ExecutionMode::Cpu).unwrap();
        assert!(trees.layers.iter().all(|layer| layer.leaves.is_empty()));
        for repetitions in [0, 1, 7, 136] {
            for (round, layer) in leaves.iter().enumerate() {
                let indices = (0..padded_lengths[round]).collect::<Vec<_>>();
                let canonical = merkle_paths_for_leaf_indices(
                    layer,
                    &indices,
                    MerkleTreeRoleV1::Fri(round as u32),
                    ExecutionMode::Cpu,
                )
                .unwrap();
                for _ in 0..repetitions {
                    for (index, expected) in canonical.iter().enumerate() {
                        assert_eq!(&trees.path(round, index).unwrap(), expected);
                    }
                }
            }
            assert_eq!(trees.tree_build_count(), 0);
        }
        for (round, &length) in padded_lengths.iter().enumerate() {
            assert!(
                matches!(trees.path(round, length), Err(Error::QueryIndexOutOfRange { index, len }) if index == length && len == length)
            );
        }
        assert!(
            matches!(trees.path(leaves.len(), 0), Err(Error::QueryIndexOutOfRange { index, len }) if index == leaves.len() && len == leaves.len())
        );
        assert_eq!(trees.tree_build_count(), 0);
    }

    #[test]
    fn retained_levels_reject_malformed_geometry_before_path_access() {
        let digest = GoldilocksDigest384V1::new([1; 6]).unwrap();
        for widths in [
            vec![],
            vec![1],
            vec![2],
            vec![0, 1],
            vec![1, 1],
            vec![3, 2, 1],
            vec![4, 1],
            vec![6, 3, 2, 1],
            vec![2, 2, 1],
            vec![2, 1, 1],
            vec![2, 0],
        ] {
            let tree = widths
                .into_iter()
                .map(|width| vec![digest; width])
                .collect();
            assert!(matches!(
                FriOpeningTrees::from_levels(vec![tree], ExecutionMode::Cpu),
                Err(Error::InvalidTraceShape { .. }),
            ));
        }
        let mut empty = FriOpeningTrees::from_levels(Vec::new(), ExecutionMode::Cpu).unwrap();
        assert!(matches!(
            empty.path(0, 0),
            Err(Error::QueryIndexOutOfRange { index: 0, len: 0 })
        ));
        assert_eq!(empty.tree_build_count(), 0);
    }

    #[test]
    fn layer_paths_preserve_odd_padding_and_coordinate_errors() {
        let leaves = (0..5)
            .map(|index| GoldilocksDigest384V1::new([index; 6]).unwrap())
            .collect::<Vec<_>>();
        let mut trees = FriOpeningTrees::new(vec![leaves.clone(), Vec::new()], ExecutionMode::Cpu);
        // The canonical generic tree builder pads five leaves to six.
        for leaf_index in [0, 4, 5] {
            let canonical = merkle_paths_for_leaf_indices(
                &leaves,
                &[leaf_index],
                MerkleTreeRoleV1::Fri(0),
                ExecutionMode::Cpu,
            )
            .unwrap()
            .remove(0);
            assert_eq!(trees.path(0, leaf_index).unwrap(), canonical);
        }
        assert!(matches!(
            trees.path(0, 6),
            Err(Error::QueryIndexOutOfRange { index: 6, len: 6 })
        ));
        assert!(matches!(
            trees.path(1, 0),
            Err(Error::QueryIndexOutOfRange { index: 0, len: 0 })
        ));
        assert!(matches!(
            trees.path(2, 0),
            Err(Error::QueryIndexOutOfRange { index: 2, len: 2 })
        ));
        assert_eq!(trees.tree_builds, 1);
    }

    #[test]
    fn query_chain_empty_inputs_and_invalid_coordinates_keep_their_errors() {
        let values = layer_values();
        assert!(
            open_fri_query_chains(&[], &[0], 2, ExecutionMode::Cpu)
                .unwrap()
                .is_empty()
        );
        assert!(
            open_fri_query_chains(&values, &[], 2, ExecutionMode::Cpu)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            open_fri_query_chains(&values, &[32], 2, ExecutionMode::Cpu),
            Err(Error::QueryIndexOutOfRange { index: 32, len: 32 })
        ));
        assert!(matches!(
            open_fri_query_chains(&values, &[0], 4, ExecutionMode::Cpu),
            Err(Error::FriArity(4))
        ));
        let terminal = vec![values.last().unwrap().clone()];
        assert!(matches!(
            open_fri_query_chains(&terminal, &[4], 2, ExecutionMode::Cpu),
            Err(Error::QueryIndexOutOfRange { index: 4, len: 4 })
        ));
        let mut short_next = values.clone();
        short_next[1] = vec![GoldilocksFp4V1::ZERO; 2];
        assert!(matches!(
            open_fri_query_chains(&short_next, &[7], 2, ExecutionMode::Cpu),
            Err(Error::QueryIndexOutOfRange { index: 7, len: 2 })
        ));
        if let Ok(overflow) = usize::try_from(u64::from(u32::MAX) + 1) {
            assert!(matches!(
                open_fri_query_chains(&values, &[overflow], 2, ExecutionMode::Cpu),
                Err(Error::QueryIndexOverflow { index }) if index == overflow
            ));
        }
    }
}
