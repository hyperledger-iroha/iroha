//! Canonical bounded Merkle multipaths with caller-fixed parent hashing.
//!
//! Trusted geometry and sorted unique queried indices determine every sibling
//! position. A proof contributes only that exact ordered list of canonical
//! digests, never node IDs, tree geometry or a subset of the requested leaves.
//! Siblings are ordered by increasing level from the leaves, then increasing
//! index within that level. Verification retains at most two queried frontiers;
//! planning and verification use O(q log L) work/space, not an L-sized tree.
//!
//! A sole leaf has an empty sibling frontier but still uses the existing root
//! `node(role, level=1, index=0, leaf, leaf)`. It is never its own root. Native
//! digest types admit only six canonical Goldilocks coordinates; future wire
//! decoding must preserve that admission boundary.
//!
//! The test-only compact protocol now wraps these plans in a distinct canonical
//! shared-opening Norito DTO. TODO: Qualify the complete protocol/profile and
//! bounded raw-byte admission before production integration. This helper changes
//! neither production proof encoding nor verification.

#[cfg(test)]
use fastpq_isi::GoldilocksDigest384DomainPrefixV1;
use fastpq_isi::GoldilocksDigest384V1 as Digest;

#[cfg(test)]
use super::{MERKLE_NODE_PHASE_V1, MerkleTreeRoleV1, digest_domain_prefix_v1, hash_at_prefix_v1};
use crate::{Error, Result};

// One immutable prefix is borrowed for every parent at the current level.
// Reconstruction walks levels in order, so retaining earlier prefixes would
// consume memory without avoiding any further work. Role and FRI round belong
// to this private per-tree owner; every call still hashes its exact index and
// both complete child digests as two separately framed fields.
#[cfg(test)]
struct LevelPrefixHasher {
    role: MerkleTreeRoleV1,
    prefix: Option<(usize, GoldilocksDigest384DomainPrefixV1<'static>)>,
    #[cfg(test)]
    prefix_builds: usize,
}

#[cfg(test)]
impl LevelPrefixHasher {
    fn new(role: MerkleTreeRoleV1) -> Self {
        Self {
            role,
            prefix: None,
            #[cfg(test)]
            prefix_builds: 0,
        }
    }

    fn hash(&mut self, level: usize, index: usize, left: Digest, right: Digest) -> Result<Digest> {
        if self.prefix.as_ref().map(|(cached_level, _)| *cached_level) != Some(level) {
            let prefix = digest_domain_prefix_v1(
                self.role.role(),
                MERKLE_NODE_PHASE_V1,
                level,
                self.role.counter(),
            )?;
            self.prefix = Some((level, prefix));
            #[cfg(test)]
            {
                self.prefix_builds += 1;
            }
        }
        let prefix = &self.prefix.as_ref().expect("initialized level prefix").1;
        hash_at_prefix_v1(prefix, index, &[&left.to_le_bytes(), &right.to_le_bytes()])
    }
}

/// Caller policy checked before allocating a plan or performing node hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MultiproofLimits {
    /// Maximum supported binary tree depth; a sole leaf uses one hash level.
    pub(super) max_depth: usize,
    /// Maximum distinct queried leaf digests.
    pub(super) max_queried_leaves: usize,
    /// Maximum supplied sibling digests across all levels.
    pub(super) max_siblings: usize,
    /// Maximum internal node hashes needed to reconstruct the root.
    pub(super) max_parent_hashes: usize,
}

/// Verifier-derived location, never a proof-supplied node identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SiblingPosition {
    /// Zero denotes leaves; one denotes their immediate parents.
    pub(super) level: usize,
    /// Natural left-to-right index at that level.
    pub(super) index: usize,
}

/// Exact geometry-derived successful verification work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MultiproofWork {
    /// Number of distinct caller-requested leaf digests.
    pub(super) queried_leaves: usize,
    /// Number of sibling digests consumed exactly once.
    pub(super) siblings: usize,
    /// Internal hashes, with each shared ancestor computed once.
    pub(super) parent_hashes: usize,
    /// Maximum width of either current/next digest frontier.
    pub(super) max_frontier_width: usize,
}

/// Immutable shape derived exclusively from trusted geometry and query indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MultiproofPlan {
    leaf_count: usize,
    depth: usize,
    indices: Vec<usize>,
    siblings: Vec<SiblingPosition>,
    work: MultiproofWork,
}

impl MultiproofPlan {
    /// Derive a minimal frontier for nonempty sorted unique in-range indices.
    ///
    /// The supplied limits belong to the verifier's fixed profile. Empty and
    /// non-power-of-two trees are unsupported rather than silently padded.
    pub(super) fn new(
        leaf_count: usize,
        indices: &[usize],
        limits: MultiproofLimits,
    ) -> Result<Self> {
        if !leaf_count.is_power_of_two() || indices.is_empty() {
            return Err(shape(
                "multiproof requires a power-of-two tree and nonempty queries",
            ));
        }
        let depth = (leaf_count.trailing_zeros() as usize).max(1);
        check_limit("max_multiproof_depth", depth, limits.max_depth)?;
        check_limit(
            "max_multiproof_queries",
            indices.len(),
            limits.max_queried_leaves,
        )?;
        for (position, &index) in indices.iter().enumerate() {
            if index >= leaf_count {
                return Err(Error::QueryIndexOutOfRange {
                    index,
                    len: leaf_count,
                });
            }
            if position != 0 && indices[position - 1] >= index {
                return Err(shape("multiproof queries must be sorted and unique"));
            }
        }
        let mut current = reserved(indices.len())?;
        current.extend_from_slice(indices);
        let mut siblings = Vec::new();
        let mut parent_hashes = 0_usize;
        if leaf_count == 1 {
            parent_hashes = 1;
            check_limit(
                "max_multiproof_parent_hashes",
                parent_hashes,
                limits.max_parent_hashes,
            )?;
        } else {
            for level in 0..depth {
                let mut next = reserved(current.len())?;
                let mut position = 0;
                while position < current.len() {
                    let index = current[position];
                    let paired = index.is_multiple_of(2)
                        && current.get(position + 1).copied() == Some(index + 1);
                    if !paired {
                        let count = siblings
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| shape("multiproof sibling count overflow"))?;
                        check_limit("max_multiproof_siblings", count, limits.max_siblings)?;
                        siblings
                            .try_reserve(1)
                            .map_err(|_| shape("multiproof sibling allocation failed"))?;
                        siblings.push(SiblingPosition {
                            level,
                            index: index ^ 1,
                        });
                    }
                    parent_hashes = parent_hashes
                        .checked_add(1)
                        .ok_or_else(|| shape("multiproof parent count overflow"))?;
                    check_limit(
                        "max_multiproof_parent_hashes",
                        parent_hashes,
                        limits.max_parent_hashes,
                    )?;
                    next.push(index / 2);
                    position += if paired { 2 } else { 1 };
                }
                current = next;
            }
        }
        let mut owned_indices = reserved(indices.len())?;
        owned_indices.extend_from_slice(indices);
        let work = MultiproofWork {
            queried_leaves: indices.len(),
            siblings: siblings.len(),
            parent_hashes,
            max_frontier_width: indices.len(),
        };
        Ok(Self {
            leaf_count,
            depth,
            indices: owned_indices,
            siblings,
            work,
        })
    }

    /// Return the exact ordered sibling locations for prover extraction.
    pub(super) fn sibling_positions(&self) -> &[SiblingPosition] {
        &self.siblings
    }

    /// Return exact work/counts usable for admission before sibling decoding.
    pub(super) const fn work(&self) -> MultiproofWork {
        self.work
    }

    /// Extract canonical siblings from existing tree levels with exact geometry.
    ///
    /// Every level length is checked before extraction. Selected leaves and
    /// siblings must reconstruct the supplied tree root under the requested
    /// role. Unqueried cached subtrees need not be rehashed or reallocated.
    #[cfg(test)]
    pub(super) fn open(
        &self,
        role: MerkleTreeRoleV1,
        levels: &[Vec<Digest>],
    ) -> Result<Vec<Digest>> {
        let mut hasher = LevelPrefixHasher::new(role);
        self.open_with(levels, |level, index, left, right| {
            hasher.hash(level, index, left, right)
        })
    }

    /// Extract exactly this plan's frontier under a caller-fixed parent hash.
    ///
    /// The callback receives only coordinates derived from trusted geometry.
    /// All cached level widths are checked before the first hash invocation.
    #[cfg(test)]
    pub(super) fn open_with(
        &self,
        levels: &[Vec<Digest>],
        hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<Vec<Digest>> {
        if levels.len() != self.depth + 1 {
            return Err(shape("multiproof tree level count mismatch"));
        }
        for (level, nodes) in levels.iter().enumerate() {
            let expected = if self.leaf_count == 1 {
                if level == 0 { 2 } else { 1 }
            } else {
                self.leaf_count >> level
            };
            if nodes.len() != expected {
                return Err(shape("multiproof tree level width mismatch"));
            }
        }
        if self.leaf_count == 1 && levels[0][0] != levels[0][1] {
            return Err(shape(
                "multiproof sole leaf must be duplicated in cached tree",
            ));
        }
        let mut leaves = reserved(self.indices.len())?;
        leaves.extend(self.indices.iter().map(|&index| levels[0][index]));
        let mut siblings = reserved(self.siblings.len())?;
        siblings.extend(
            self.siblings
                .iter()
                .map(|position| levels[position.level][position.index]),
        );
        self.verify_with(levels[self.depth][0], &leaves, &siblings, hash)?;
        Ok(siblings)
    }

    /// Verify exactly the caller's leaves and canonical minimal sibling list.
    ///
    /// Leaf and sibling counts are checked before any hashing. Native `Digest`
    /// constructors/decoders reject every coordinate at or above the modulus;
    /// no raw bytes or unchecked scalar conversions enter this helper.
    #[cfg(test)]
    pub(super) fn verify(
        &self,
        role: MerkleTreeRoleV1,
        root: Digest,
        leaves: &[Digest],
        siblings: &[Digest],
    ) -> Result<MultiproofWork> {
        // Construct lazily from inside reconstruct: malformed leaf/sibling
        // cardinalities must fail before even the cached prefix is permuted.
        let mut hasher = LevelPrefixHasher::new(role);
        self.verify_with(root, leaves, siblings, |level, index, left, right| {
            hasher.hash(level, index, left, right)
        })
    }

    /// Authenticate this exact frontier under a caller-fixed parent hash.
    ///
    /// Malformed cardinalities invoke no callback. Each shared parent is hashed
    /// once at its plan-derived level/index; the sole leaf is duplicated into
    /// its required parent. Hash failures propagate immediately without retry.
    pub(super) fn verify_with(
        &self,
        root: Digest,
        leaves: &[Digest],
        siblings: &[Digest],
        hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<MultiproofWork> {
        let computed = self.reconstruct(leaves, siblings, hash)?;
        if computed != root {
            return Err(Error::QueryMerklePathMismatch { index: 0 });
        }
        Ok(self.work)
    }

    fn reconstruct(
        &self,
        leaves: &[Digest],
        siblings: &[Digest],
        mut hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<Digest> {
        if leaves.len() != self.indices.len() || siblings.len() != self.siblings.len() {
            return Err(shape("multiproof leaf or sibling count mismatch"));
        }
        if self.leaf_count == 1 {
            return hash(1, 0, leaves[0], leaves[0]);
        }
        let mut current = reserved(self.indices.len())?;
        current.extend(self.indices.iter().copied().zip(leaves.iter().copied()));
        let mut consumed = 0;
        for level in 0..self.depth {
            let mut next = reserved(current.len())?;
            let mut position = 0;
            while position < current.len() {
                let (index, value) = current[position];
                let paired = index.is_multiple_of(2)
                    && current.get(position + 1).map(|node| node.0) == Some(index + 1);
                let (left, right) = if paired {
                    (value, current[position + 1].1)
                } else {
                    // The private plan's frontier is derived by this exact
                    // schedule. No proof coordinate can alter this location.
                    let sibling = siblings[consumed];
                    consumed += 1;
                    if index.is_multiple_of(2) {
                        (value, sibling)
                    } else {
                        (sibling, value)
                    }
                };
                next.push((index / 2, hash(level + 1, index / 2, left, right)?));
                position += if paired { 2 } else { 1 };
            }
            current = next;
        }
        if consumed != siblings.len() || current.len() != 1 || current[0].0 != 0 {
            return Err(shape(
                "multiproof frontier did not terminate at the unique root",
            ));
        }
        Ok(current[0].1)
    }
}

fn reserved<T>(capacity: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| shape("multiproof frontier allocation failed"))?;
    Ok(values)
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

fn shape(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::backend::{
        ExecutionMode, GOLDILOCKS_MODULUS, build_merkle_levels_with_mode, merkle_node_hash,
        merkle_paths_for_leaf_indices, verify_merkle_path_for_role,
    };

    fn limits() -> MultiproofLimits {
        MultiproofLimits {
            max_depth: 32,
            max_queried_leaves: 1_024,
            max_siblings: 32_768,
            max_parent_hashes: 32_768,
        }
    }

    fn leaf(index: usize) -> Digest {
        Digest::new(core::array::from_fn(|lane| {
            1 + 7 * index as u64 + lane as u64
        }))
        .unwrap()
    }

    fn changed(digest: Digest, lane: usize) -> Digest {
        let mut words = digest.words();
        words[lane] = if words[lane] == GOLDILOCKS_MODULUS - 1 {
            0
        } else {
            words[lane] + 1
        };
        Digest::new(words).unwrap()
    }

    // Independent set-based ancestor enumeration checks canonical frontier
    // ordering and minimality without reproducing the adjacent-pair walk.
    fn reference_positions(count: usize, indices: &[usize]) -> Vec<SiblingPosition> {
        if count == 1 {
            return Vec::new();
        }
        let mut selected = indices.iter().copied().collect::<BTreeSet<_>>();
        let mut frontier = Vec::new();
        for level in 0..count.trailing_zeros() as usize {
            let mut missing = BTreeSet::new();
            for &index in &selected {
                if !selected.contains(&(index ^ 1)) {
                    missing.insert(index ^ 1);
                }
            }
            frontier.extend(
                missing
                    .into_iter()
                    .map(|index| SiblingPosition { level, index }),
            );
            selected = selected.into_iter().map(|index| index / 2).collect();
        }
        frontier
    }

    fn verify_one_shot(
        plan: &MultiproofPlan,
        role: MerkleTreeRoleV1,
        root: Digest,
        leaves: &[Digest],
        siblings: &[Digest],
    ) -> Result<MultiproofWork> {
        // Independent original one-shot framing/permutation, retaining the
        // exact original reconstruction callback and root/error behavior.
        let computed = plan.reconstruct(leaves, siblings, |level, index, left, right| {
            merkle_node_hash(role, level, index, left, right)
        })?;
        if computed != root {
            return Err(Error::QueryMerklePathMismatch { index: 0 });
        }
        Ok(plan.work())
    }

    #[test]
    fn shake_parent_adapter_authenticates_sparse_frontiers_and_terminal_with_exact_work() {
        use crate::backend::compact_v1::{Context, Oracle};
        let context = Context::new(b"complete public context for shared six-lane adapter").unwrap();
        let wrong_context =
            Context::new(b"changed complete public context for shared six-lane adapter").unwrap();
        for (round, count, bytes, indices) in [
            (15, 8_usize, 64, vec![0, 3, 7]),
            (16, 4, 64, vec![1, 2]),
            (17, 1, 128, vec![0]),
        ] {
            let oracle = Oracle::Fri(round);
            let mut leaves: Vec<_> = (0..count)
                .map(|i| {
                    let mut payload = vec![0; bytes];
                    payload[..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
                    context.hash_leaf(oracle, i as u32, &payload).unwrap()
                })
                .collect();
            if count == 1 {
                leaves.push(leaves[0]);
            }
            let mut levels = vec![leaves];
            while levels.last().unwrap().len() > 1 {
                let level = levels.len();
                let parents = levels
                    .last()
                    .unwrap()
                    .chunks_exact(2)
                    .enumerate()
                    .map(|(i, pair)| {
                        context
                            .hash_parent(oracle, level as u32, i as u32, pair[0], pair[1])
                            .unwrap()
                    })
                    .collect();
                levels.push(parents);
            }
            let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
            let mut opening_calls = 0;
            let siblings = plan
                .open_with(&levels, |level, index, left, right| {
                    opening_calls += 1;
                    context
                        .hash_parent(oracle, level as u32, index as u32, left, right)
                        .map_err(|_| shape("candidate parent hash failed"))
                })
                .unwrap();
            assert_eq!(opening_calls, plan.work().parent_hashes);
            assert_eq!(
                siblings,
                plan.sibling_positions()
                    .iter()
                    .map(|p| levels[p.level][p.index])
                    .collect::<Vec<_>>()
            );
            let selected: Vec<_> = indices.iter().map(|&i| levels[0][i]).collect();
            let root = levels.last().unwrap()[0];
            let mut positions = BTreeSet::new();
            let work = plan
                .verify_with(root, &selected, &siblings, |level, index, left, right| {
                    assert!(
                        positions.insert((level, index)),
                        "shared parent was rehashed"
                    );
                    context
                        .hash_parent(oracle, level as u32, index as u32, left, right)
                        .map_err(|_| shape("candidate parent hash failed"))
                })
                .unwrap();
            assert_eq!(work, plan.work());
            assert_eq!(positions.len(), work.parent_hashes);
            for altered in [&wrong_context, &context] {
                let role = if std::ptr::eq(altered, &context) {
                    Oracle::Fri(round - 1)
                } else {
                    oracle
                };
                assert!(
                    plan.verify_with(root, &selected, &siblings, |level, index, left, right| {
                        altered
                            .hash_parent(role, level as u32, index as u32, left, right)
                            .map_err(|_| shape("candidate parent hash failed"))
                    })
                    .is_err()
                );
            }
            for bad in 0..selected.len() + siblings.len() + 1 {
                let mut selected = selected.clone();
                let mut siblings = siblings.clone();
                let mut root = root;
                let value = if bad < selected.len() {
                    &mut selected[bad]
                } else if bad < selected.len() + siblings.len() {
                    &mut siblings[bad - selected.len()]
                } else {
                    &mut root
                };
                *value = changed(*value, 5);
                assert!(
                    plan.verify_with(root, &selected, &siblings, |level, index, left, right| {
                        context
                            .hash_parent(oracle, level as u32, index as u32, left, right)
                            .map_err(|_| shape("candidate parent hash failed"))
                    })
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn caller_hash_adapter_checks_all_cardinalities_before_hashing_and_never_retries() {
        let plan = MultiproofPlan::new(8, &[0, 3, 7], limits()).unwrap();
        let valid_leaves = vec![Digest::default(); 3];
        let valid_siblings = vec![Digest::default(); plan.work().siblings];
        for (leaves, siblings) in [
            (&valid_leaves[..2], valid_siblings.as_slice()),
            (
                valid_leaves.as_slice(),
                &valid_siblings[..valid_siblings.len() - 1],
            ),
            (valid_leaves.as_slice(), &[Digest::default(); 8][..]),
            (&[Digest::default(); 4][..], valid_siblings.as_slice()),
        ] {
            assert!(
                plan.verify_with(Digest::default(), leaves, siblings, |_, _, _, _| panic!(
                    "malformed cardinality reached hashing"
                ))
                .is_err()
            );
        }
        let mut calls = 0;
        let error = plan
            .verify_with(
                Digest::default(),
                &valid_leaves,
                &valid_siblings,
                |_, _, _, _| {
                    calls += 1;
                    Err(shape("injected unrecoverable parent failure"))
                },
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("injected unrecoverable parent failure")
        );
        assert_eq!(calls, 1);
        for levels in [
            vec![],
            vec![vec![Digest::default(); 8]; 4],
            vec![vec![Digest::default(); 1]; 4],
        ] {
            assert!(
                plan.open_with(&levels, |_, _, _, _| panic!(
                    "malformed cache reached hashing"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn shared_parent_prefix_preserves_roles_rounds_roots_paths_and_work() {
        for role in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(17),
            MerkleTreeRoleV1::Fri(u32::MAX),
        ] {
            for count in [1, 2, 16] {
                let indices = (0..count)
                    .filter(|index| index % 3 == 0 || *index + 1 == count)
                    .collect::<Vec<_>>();
                let leaves = (0..count).map(leaf).collect::<Vec<_>>();
                let levels =
                    build_merkle_levels_with_mode(&leaves, role, ExecutionMode::Cpu).unwrap();
                let root = levels.last().unwrap()[0];
                let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
                let selected = indices
                    .iter()
                    .map(|&index| leaves[index])
                    .collect::<Vec<_>>();
                let siblings = plan.open(role, &levels).unwrap();
                assert_eq!(
                    plan.verify(role, root, &selected, &siblings).unwrap(),
                    plan.work()
                );
                assert_eq!(
                    verify_one_shot(&plan, role, root, &selected, &siblings).unwrap(),
                    plan.work()
                );
                let mut hasher = LevelPrefixHasher::new(role);
                let mut calls = 0;
                let reconstructed = plan
                    .reconstruct(&selected, &siblings, |level, index, left, right| {
                        calls += 1;
                        hasher.hash(level, index, left, right)
                    })
                    .unwrap();
                assert_eq!(reconstructed, root);
                assert_eq!(calls, plan.work().parent_hashes);
                assert_eq!(hasher.prefix_builds, plan.depth);
                let paths =
                    merkle_paths_for_leaf_indices(&leaves, &indices, role, ExecutionMode::Cpu)
                        .unwrap();
                for ((&index, &value), path) in indices.iter().zip(&selected).zip(paths) {
                    assert!(verify_merkle_path_for_role(role, root, value, index, &path).unwrap());
                }
                for lane in 0..6 {
                    for (bad_root, bad_leaves, bad_siblings) in [
                        (changed(root, lane), selected.clone(), siblings.clone()),
                        (
                            root,
                            {
                                let mut bad = selected.clone();
                                bad[0] = changed(bad[0], lane);
                                bad
                            },
                            siblings.clone(),
                        ),
                        (root, selected.clone(), {
                            let mut bad = siblings.clone();
                            if let Some(first) = bad.first_mut() {
                                *first = changed(*first, lane);
                            }
                            bad
                        }),
                    ] {
                        assert_eq!(
                            format!(
                                "{:?}",
                                plan.verify(role, bad_root, &bad_leaves, &bad_siblings)
                            ),
                            format!(
                                "{:?}",
                                verify_one_shot(&plan, role, bad_root, &bad_leaves, &bad_siblings)
                            ),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn shared_parent_prefix_binds_full_indices_levels_and_fresh_children() {
        let left = Digest::new([0, 1, 2, 3, 4, GOLDILOCKS_MODULUS - 1]).unwrap();
        let right = Digest::new([GOLDILOCKS_MODULUS - 1, 4, 3, 2, 1, 0]).unwrap();
        let mut indices = vec![0, 1, 7, usize::MAX];
        if let Ok(index) = usize::try_from(u64::from(u32::MAX) + 1) {
            indices.push(index);
        }
        for role in [
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(u32::MAX),
        ] {
            let mut hasher = LevelPrefixHasher::new(role);
            for level in [1, 19, usize::MAX] {
                for &index in &indices {
                    assert_eq!(
                        hasher.hash(level, index, left, right).unwrap(),
                        merkle_node_hash(role, level, index, left, right).unwrap()
                    );
                    for lane in 0..6 {
                        let changed_left = changed(left, lane);
                        let changed_right = changed(right, lane);
                        assert_eq!(
                            hasher.hash(level, index, changed_left, right).unwrap(),
                            merkle_node_hash(role, level, index, changed_left, right).unwrap()
                        );
                        assert_eq!(
                            hasher.hash(level, index, left, changed_right).unwrap(),
                            merkle_node_hash(role, level, index, left, changed_right).unwrap()
                        );
                    }
                    assert_eq!(
                        hasher.hash(level, index, right, left).unwrap(),
                        merkle_node_hash(role, level, index, right, left).unwrap()
                    );
                }
            }
            assert_eq!(hasher.prefix_builds, 3);
            // The one-entry cache also remains correct if a private caller
            // revisits a level; that visit must rebuild, never reuse another level.
            assert_eq!(
                hasher.hash(1, 0, left, right).unwrap(),
                merkle_node_hash(role, 1, 0, left, right).unwrap()
            );
            assert_eq!(hasher.prefix_builds, 4);
        }
        assert!(core::mem::size_of::<GoldilocksDigest384DomainPrefixV1<'static>>() <= 512);
        assert!(core::mem::size_of::<LevelPrefixHasher>() <= 1024);
    }

    #[test]
    fn shared_parent_prefix_count_is_depth_bounded_for_sparse_maximum_geometry() {
        let count = 1_usize << 19;
        for queries in [1, 7, 136, 272] {
            let indices = (0..queries)
                .map(|index| index * (count - 1) / queries)
                .collect::<Vec<_>>();
            let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
            let leaves = (0..queries).map(leaf).collect::<Vec<_>>();
            let siblings = (0..plan.work().siblings)
                .map(|index| leaf(index + 1000))
                .collect::<Vec<_>>();
            let mut hasher = LevelPrefixHasher::new(MerkleTreeRoleV1::Fri(17));
            let mut calls = 0;
            let root = plan
                .reconstruct(&leaves, &siblings, |level, index, left, right| {
                    calls += 1;
                    hasher.hash(level, index, left, right)
                })
                .unwrap();
            assert_eq!(hasher.prefix_builds, 19);
            assert_eq!(calls, plan.work().parent_hashes);
            assert_eq!(
                verify_one_shot(&plan, MerkleTreeRoleV1::Fri(17), root, &leaves, &siblings)
                    .unwrap(),
                plan.work()
            );
        }
    }

    #[test]
    fn shared_parent_prefix_cardinality_errors_do_zero_prefix_work() {
        for (count, indices) in [(1, vec![0]), (16, vec![0, 3, 15])] {
            let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
            let leaves = indices.iter().copied().map(leaf).collect::<Vec<_>>();
            let siblings = (0..plan.work().siblings).map(leaf).collect::<Vec<_>>();
            let mut invalid = vec![
                (leaves[..leaves.len() - 1].to_vec(), siblings.clone()),
                (
                    {
                        let mut extra = leaves.clone();
                        extra.push(leaf(99));
                        extra
                    },
                    siblings.clone(),
                ),
                (leaves.clone(), {
                    let mut extra = siblings.clone();
                    extra.push(leaf(99));
                    extra
                }),
            ];
            if !siblings.is_empty() {
                invalid.push((leaves.clone(), siblings[..siblings.len() - 1].to_vec()));
            }
            for (leaves, siblings) in invalid {
                let mut hasher = LevelPrefixHasher::new(MerkleTreeRoleV1::Lde);
                let result = plan.reconstruct(&leaves, &siblings, |level, index, left, right| {
                    hasher.hash(level, index, left, right)
                });
                assert!(matches!(result, Err(Error::InvalidTraceShape { .. })));
                assert_eq!(hasher.prefix_builds, 0);
                assert!(hasher.prefix.is_none());
                assert_eq!(
                    format!(
                        "{:?}",
                        plan.verify(MerkleTreeRoleV1::Lde, leaf(99), &leaves, &siblings)
                    ),
                    format!(
                        "{:?}",
                        verify_one_shot(&plan, MerkleTreeRoleV1::Lde, leaf(99), &leaves, &siblings)
                    ),
                );
            }
        }
    }

    #[test]
    fn every_small_nonempty_query_set_has_the_canonical_frontier_and_hash_count() {
        for count in [1_usize, 2, 4, 8] {
            for bitmap in 1_usize..(1 << count) {
                let indices = (0..count)
                    .filter(|&index| bitmap & (1_usize << index) != 0)
                    .collect::<Vec<_>>();
                let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
                assert_eq!(
                    plan.sibling_positions(),
                    reference_positions(count, &indices)
                );
                // The queried leaves and minimal siblings form the frontier
                // of a full binary reconstruction tree. Its parent count is
                // frontier cardinality minus one, except the duplicated sole
                // leaf, which always contributes one typed internal hash.
                assert_eq!(
                    plan.work().parent_hashes,
                    (indices.len() + plan.work().siblings - 1).max(1)
                );
                assert!(
                    plan.sibling_positions()
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                );
            }
        }
    }

    #[test]
    fn frontier_is_minimal_and_matches_existing_paths_across_shapes_and_roles() {
        for role in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(17),
        ] {
            for count in [1, 2, 4, 8, 16, 32] {
                let leaves = (0..count).map(leaf).collect::<Vec<_>>();
                let levels =
                    build_merkle_levels_with_mode(&leaves, role, ExecutionMode::Cpu).unwrap();
                let mut sets = vec![
                    vec![0],
                    vec![count - 1],
                    (0..count).collect(),
                    (0..count).step_by(2).collect(),
                ];
                if count > 1 {
                    sets.push(vec![0, count - 1]);
                }
                if count > 4 {
                    sets.push(vec![1, 2, 3, count - 1]);
                }
                sets.sort();
                sets.dedup();
                for indices in sets {
                    let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
                    assert_eq!(
                        plan.sibling_positions(),
                        reference_positions(count, &indices)
                    );
                    let siblings = plan.open(role, &levels).unwrap();
                    let queried = indices
                        .iter()
                        .map(|&index| leaves[index])
                        .collect::<Vec<_>>();
                    let work = plan
                        .verify(role, levels.last().unwrap()[0], &queried, &siblings)
                        .unwrap();
                    assert_eq!(work, plan.work());
                    assert_eq!(work.siblings, siblings.len());
                    assert_eq!(work.max_frontier_width, indices.len());
                    assert!(work.parent_hashes <= indices.len() * plan.depth);
                    let paths =
                        merkle_paths_for_leaf_indices(&leaves, &indices, role, ExecutionMode::Cpu)
                            .unwrap();
                    for ((&index, &value), path) in indices.iter().zip(&queried).zip(&paths) {
                        assert!(
                            verify_merkle_path_for_role(
                                role,
                                levels.last().unwrap()[0],
                                value,
                                index,
                                path
                            )
                            .unwrap()
                        );
                    }
                    if indices.len() == count {
                        assert!(siblings.is_empty());
                        assert_eq!(work.parent_hashes, (count - 1).max(1));
                    }
                }
            }
        }
    }

    #[test]
    fn single_leaf_uses_duplicated_root_without_a_redundant_wire_sibling() {
        let role = MerkleTreeRoleV1::Fri(17);
        let value = leaf(1);
        let levels = build_merkle_levels_with_mode(&[value], role, ExecutionMode::Cpu).unwrap();
        assert_eq!(levels[0], vec![value, value]);
        let plan = MultiproofPlan::new(1, &[0], limits()).unwrap();
        assert_eq!(plan.work().parent_hashes, 1);
        assert!(plan.open(role, &levels).unwrap().is_empty());
        let root = levels[1][0];
        assert!(verify_merkle_path_for_role(role, root, value, 0, &[value]).unwrap());
        assert!(plan.verify(role, root, &[value], &[]).is_ok());
        assert!(plan.verify(role, value, &[value], &[]).is_err());
        assert!(plan.verify(role, root, &[value], &[value]).is_err());
        assert!(
            plan.verify(MerkleTreeRoleV1::Fri(18), root, &[value], &[])
                .is_err()
        );
        let mut bad = levels;
        bad[0][1] = leaf(2);
        assert!(plan.open(role, &bad).is_err());
    }

    #[test]
    fn every_digest_coordinate_and_trusted_role_root_and_index_are_bound() {
        let role = MerkleTreeRoleV1::AirTrace;
        let leaves = (0..16).map(leaf).collect::<Vec<_>>();
        let levels = build_merkle_levels_with_mode(&leaves, role, ExecutionMode::Cpu).unwrap();
        let root = levels.last().unwrap()[0];
        let indices = [1, 2, 7, 12];
        let plan = MultiproofPlan::new(16, &indices, limits()).unwrap();
        let siblings = plan.open(role, &levels).unwrap();
        let queried = indices.map(|index| leaves[index]);
        for lane in 0..6 {
            assert!(
                plan.verify(role, changed(root, lane), &queried, &siblings)
                    .is_err()
            );
            for position in 0..queried.len() {
                let mut bad = queried;
                bad[position] = changed(bad[position], lane);
                assert!(plan.verify(role, root, &bad, &siblings).is_err());
            }
            for position in 0..siblings.len() {
                let mut bad = siblings.clone();
                bad[position] = changed(bad[position], lane);
                assert!(plan.verify(role, root, &queried, &bad).is_err());
            }
        }
        for wrong in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
        ] {
            assert!(plan.verify(wrong, root, &queried, &siblings).is_err());
        }
        for wrong in [[0, 2, 7, 12], [1, 3, 7, 12], [1, 2, 6, 12], [1, 2, 7, 13]] {
            let wrong_plan = MultiproofPlan::new(16, &wrong, limits()).unwrap();
            assert!(wrong_plan.verify(role, root, &queried, &siblings).is_err());
        }
        let mut swapped = siblings.clone();
        swapped.swap(0, 1);
        assert!(plan.verify(role, root, &queried, &swapped).is_err());
        let mut duplicated = siblings.clone();
        duplicated[1] = duplicated[0];
        assert!(plan.verify(role, root, &queried, &duplicated).is_err());
        let wrong_size = MultiproofPlan::new(32, &indices, limits()).unwrap();
        assert!(wrong_size.verify(role, root, &queried, &siblings).is_err());
    }

    #[test]
    fn geometry_query_and_resource_preflight_reject_malformed_shapes() {
        for (count, indices) in [
            (0, vec![0]),
            (3, vec![0]),
            (8, vec![]),
            (8, vec![8]),
            (8, vec![0, 8]),
            (8, vec![1, 1]),
            (8, vec![2, 1]),
        ] {
            assert!(MultiproofPlan::new(count, &indices, limits()).is_err());
        }
        let plan = MultiproofPlan::new(16, &[0, 3, 15], limits()).unwrap();
        let exact = MultiproofLimits {
            max_depth: 4,
            max_queried_leaves: 3,
            max_siblings: plan.work().siblings,
            max_parent_hashes: plan.work().parent_hashes,
        };
        assert_eq!(MultiproofPlan::new(16, &[0, 3, 15], exact).unwrap(), plan);
        for field in 0..4 {
            let mut restricted = exact;
            match field {
                0 => restricted.max_depth -= 1,
                1 => restricted.max_queried_leaves -= 1,
                2 => restricted.max_siblings -= 1,
                _ => restricted.max_parent_hashes -= 1,
            }
            assert!(matches!(
                MultiproofPlan::new(16, &[0, 3, 15], restricted),
                Err(Error::VerifierLimitExceeded { .. })
            ));
        }
        assert!(
            MultiproofPlan::new(
                1,
                &[0],
                MultiproofLimits {
                    max_parent_hashes: 0,
                    ..limits()
                }
            )
            .is_err()
        );
        let siblings = vec![leaf(1); plan.work().siblings];
        for (leaf_count, sibling_count) in [
            (2, siblings.len()),
            (4, siblings.len()),
            (3, siblings.len() - 1),
            (3, siblings.len() + 1),
        ] {
            let mut hash_calls = 0;
            let result = plan.reconstruct(
                &vec![leaf(0); leaf_count],
                &vec![leaf(1); sibling_count],
                |_, _, _, _| {
                    hash_calls += 1;
                    Ok(leaf(2))
                },
            );
            assert!(result.is_err());
            assert_eq!(hash_calls, 0, "malformed counts must fail before hashing");
        }
    }

    #[test]
    fn prover_checks_all_level_widths_and_selected_tree_consistency() {
        let role = MerkleTreeRoleV1::Lde;
        let leaves = (0..16).map(leaf).collect::<Vec<_>>();
        let levels = build_merkle_levels_with_mode(&leaves, role, ExecutionMode::Cpu).unwrap();
        let plan = MultiproofPlan::new(16, &[0, 5, 15], limits()).unwrap();
        for level in 0..levels.len() {
            let mut bad = levels.clone();
            bad[level].pop();
            assert!(plan.open(role, &bad).is_err());
            let mut bad = levels.clone();
            bad[level].push(leaf(99));
            assert!(plan.open(role, &bad).is_err());
        }
        let mut bad = levels.clone();
        bad.pop();
        assert!(plan.open(role, &bad).is_err());
        let mut bad = levels.clone();
        bad.push(vec![leaf(99)]);
        assert!(plan.open(role, &bad).is_err());
        for position in plan.sibling_positions() {
            let mut bad = levels.clone();
            bad[position.level][position.index] = changed(bad[position.level][position.index], 0);
            assert!(plan.open(role, &bad).is_err());
        }
        let mut bad = levels;
        bad[4][0] = leaf(99);
        assert!(plan.open(role, &bad).is_err());
    }

    #[test]
    fn native_digest_admission_rejects_noncanonical_coordinates() {
        for lane in 0..6 {
            let mut words = [GOLDILOCKS_MODULUS - 1; 6];
            assert!(Digest::new(words).is_some());
            for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
                words[lane] = invalid;
                assert!(Digest::new(words).is_none());
                let mut bytes = [0; 48];
                for (word, chunk) in words.iter().zip(bytes.chunks_exact_mut(8)) {
                    chunk.copy_from_slice(&word.to_le_bytes());
                }
                assert!(Digest::from_le_bytes(bytes).is_none());
            }
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn huge_geometry_requires_only_the_bounded_sparse_frontier() {
        let count = 1_usize << 32;
        let indices = [0, 7, 1 << 31, count - 1];
        let bounded = MultiproofLimits {
            max_depth: 32,
            max_queried_leaves: 4,
            max_siblings: 128,
            max_parent_hashes: 128,
        };
        let plan = MultiproofPlan::new(count, &indices, bounded).unwrap();
        assert_eq!(
            plan.sibling_positions(),
            reference_positions(count, &indices)
        );
        assert!(plan.siblings.len() <= 128);
        assert!(plan.work().parent_hashes <= 128);
        assert_eq!(plan.work().max_frontier_width, 4);
        let leaves = indices.map(leaf);
        let siblings = (0..plan.work().siblings)
            .map(|index| leaf(index + 100))
            .collect::<Vec<_>>();
        let role = MerkleTreeRoleV1::Fri(31);
        let mut calls = 0;
        let root = plan
            .reconstruct(&leaves, &siblings, |level, index, left, right| {
                calls += 1;
                merkle_node_hash(role, level, index, left, right)
            })
            .unwrap();
        assert_eq!(calls, plan.work().parent_hashes);
        assert_eq!(
            plan.verify(role, root, &leaves, &siblings).unwrap(),
            plan.work()
        );
        assert!(MultiproofPlan::new(1_usize << 33, &indices, bounded).is_err());
    }
}
