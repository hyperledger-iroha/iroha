//! Exhaustive small frontier parity against materialized natural-order trees.

use super::*;
use crate::backend::{MerkleTreeRoleV1, merkle_node_hash};

fn limits() -> StreamLimits {
    StreamLimits {
        digest_execution: crate::DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        max_hashes: usize::MAX,
    }
}
fn hash(level: usize, index: usize, left: Digest, right: Digest) -> Result<Digest> {
    merkle_node_hash(MerkleTreeRoleV1::Fri(9), level, index, left, right)
}
fn leaves(size: usize) -> Vec<Digest> {
    (0..size)
        .map(|i| Digest::new(core::array::from_fn(|lane| (i * 7 + lane + 11) as u64)).unwrap())
        .collect()
}
fn reference(leaves: &[Digest]) -> Vec<Vec<Digest>> {
    let mut tree = vec![leaves.to_vec()];
    loop {
        let previous = tree.last().unwrap();
        let next = previous
            .chunks(2)
            .enumerate()
            .map(|(index, children)| {
                hash(
                    tree.len(),
                    index,
                    children[0],
                    *children.get(1).unwrap_or(&children[0]),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let done = next.len() == 1;
        tree.push(next);
        if done {
            return tree;
        }
    }
}

#[test]
fn every_small_frontier_and_stripe_shape_matches_the_existing_tree() {
    for count in [1, 2, 4, 8] {
        let leaves = leaves(count);
        let tree = reference(&leaves);
        for mask in 0_usize..1 << count {
            let queries = (0..count)
                .filter(|i| mask & (1 << i) != 0)
                .collect::<Vec<_>>();
            for stripes in [1, 2, 4, 8].into_iter().filter(|&n| n <= count) {
                let plan = StripedMerklePlan::new(count, stripes, &queries, limits()).unwrap();
                let expected_frontier = plan
                    .openings
                    .as_ref()
                    .map(|p| {
                        p.sibling_positions()
                            .iter()
                            .map(|p| tree[p.level][p.index])
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let verifier = plan.openings.clone();
                let mut stream = plan.start().unwrap();
                for stripe in 0..stripes {
                    for row in 0..count / stripes {
                        let index = stripe + row * stripes;
                        stream.push(index, leaves[index], hash).unwrap();
                    }
                }
                let actual = stream.finish(hash).unwrap();
                assert_eq!(actual.root, tree.last().unwrap()[0]);
                assert_eq!(actual.siblings, expected_frontier);
                assert_eq!(actual.leaf_hashes, count);
                assert_eq!(actual.parent_hashes, (count - 1).max(1));
                if let Some(verifier) = verifier {
                    verifier
                        .verify_with(
                            actual.root,
                            &queries.iter().map(|&i| leaves[i]).collect::<Vec<_>>(),
                            &actual.siblings,
                            hash,
                        )
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn actual_128_stripe_geometry_has_bounded_stacks_and_sparse_frontier() {
    let queries = (0..64).map(|i| 1 + i * 131_071).collect::<Vec<_>>();
    let plan = StripedMerklePlan::new(8_388_608, 128, &queries, limits()).unwrap();
    assert_eq!(plan.lower_levels, 7);
    assert_eq!(plan.rows, 65_536);
    assert_eq!(plan.upper_levels, 17);
    assert_eq!(plan.leaf_hashes, 8_388_608);
    assert_eq!(plan.parent_hashes, 8_388_607);
    assert!(plan.payload_bytes < 23 * 1024 * 1024);
    assert!(
        StripedMerklePlan::new(
            8_388_608,
            128,
            &queries,
            StreamLimits {
                max_payload_bytes: plan.payload_bytes - 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        StripedMerklePlan::new(
            8_388_608,
            128,
            &queries,
            StreamLimits {
                max_hashes: plan.leaf_hashes + plan.parent_hashes - 1,
                ..limits()
            }
        )
        .is_err()
    );
    let leaves = leaves(1024);
    let tree = reference(&leaves);
    for queries in [
        vec![0, 1, 127, 128, 511, 512, 1023],
        vec![2, 130, 258, 386],
        vec![],
    ] {
        let plan = StripedMerklePlan::new(1024, 128, &queries, limits()).unwrap();
        let expected = plan
            .openings
            .as_ref()
            .map(|p| {
                p.sibling_positions()
                    .iter()
                    .map(|p| tree[p.level][p.index])
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut stream = plan.start().unwrap();
        for stripe in 0..128 {
            for row in 0..8 {
                let index = stripe + 128 * row;
                stream.push(index, leaves[index], hash).unwrap();
            }
        }
        let actual = stream.finish(hash).unwrap();
        assert_eq!(actual.root, tree.last().unwrap()[0]);
        assert_eq!(actual.siblings, expected);
    }
}

#[test]
fn malformed_and_failed_streams_cannot_return_a_root() {
    for (leaves, stripes, queries) in [
        (0, 1, vec![]),
        (8, 3, vec![]),
        (8, 16, vec![]),
        (8, 2, vec![8]),
        (8, 2, vec![1, 1]),
        (8, 2, vec![2, 1]),
    ] {
        assert!(StripedMerklePlan::new(leaves, stripes, &queries, limits()).is_err());
    }
    let values = leaves(4);
    let mut stream = StripedMerklePlan::new(4, 2, &[0, 3], limits())
        .unwrap()
        .start()
        .unwrap();
    stream.push(0, values[0], hash).unwrap();
    assert!(
        stream.push(1, values[1], hash).is_err(),
        "next index must be two"
    );
    assert!(stream.push(2, values[2], hash).is_err());
    assert!(stream.finish(hash).is_err());
    let mut stream = StripedMerklePlan::new(4, 1, &[], limits())
        .unwrap()
        .start()
        .unwrap();
    stream.push(0, values[0], hash).unwrap();
    assert!(
        stream
            .push(1, values[1], |_, _, _, _| Err(invalid("hash failure")))
            .is_err()
    );
    assert!(stream.finish(hash).is_err());
    let stream = StripedMerklePlan::new(1, 1, &[0], limits())
        .unwrap()
        .start()
        .unwrap();
    assert!(stream.finish(hash).is_err());
    let mut stream = StripedMerklePlan::new(1, 1, &[0], limits())
        .unwrap()
        .start()
        .unwrap();
    stream.push(0, values[0], hash).unwrap();
    assert!(stream.push(0, values[0], hash).is_err());
    // Rejecting an extra request leaves the already-complete valid stream intact.
    assert_eq!(
        stream.finish(hash).unwrap().root,
        reference(&values[..1])[1][0]
    );
}

#[test]
fn actual_masked_rows_use_canonical_binding_and_match_materialized_frontiers() {
    use crate::backend::{
        compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH,
        deep_binding::{Context, Oracle},
        deep_masked_replay::{MaskedReplayPlan, MaskedTraceReplay, ReplayLimits},
    };
    let binding = Context::new(b"row stream actual canonical binding regression").unwrap();
    let source = [3, 5, 7, 11];
    let sources = [&source[..]; WIDTH];
    let mut replay = MaskedTraceReplay::arithmetic_fixture(&sources, 4, 2).unwrap();
    let queries = [0, 1, 127, 128, 255, 511];
    let leaves_count = replay.plan().lde_rows();
    let tree = StripedMerklePlan::new(leaves_count, 128, &queries, limits()).unwrap();
    let verifier = tree.openings.clone().unwrap();
    assert!(RowCommitmentPlan::new(replay.plan(), &binding, &queries, limits()).is_err());
    let actual = stream_rows(
        &mut replay,
        &binding,
        &queries,
        tree,
        crate::DigestExecutionV1::Cpu,
    )
    .unwrap();
    let mut all = vec![Digest::default(); leaves_count];
    replay
        .visit_all(|stripe| {
            for row in 0..stripe.rows() {
                let mut cells = [0; WIDTH];
                stripe.fill_row(row, &mut cells)?;
                let bytes = cells
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect::<Vec<_>>();
                all[stripe.global_index(row)] = binding
                    .hash_leaf(Oracle::Row, stripe.global_index(row) as u32, &bytes)
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
    let parent = |level: usize, index: usize, left, right| {
        binding
            .hash_parent(Oracle::Row, level as u32, index as u32, left, right)
            .map_err(binding_error)
    };
    let selected = actual
        .rows
        .iter()
        .map(|row| {
            let bytes = row
                .values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect::<Vec<_>>();
            let leaf = binding.hash_leaf(Oracle::Row, row.index, &bytes).unwrap();
            assert_eq!(leaf, all[row.index as usize]);
            leaf
        })
        .collect::<Vec<_>>();
    verifier
        .verify_with(actual.root, &selected, &actual.siblings, parent)
        .unwrap();
    let mut levels = vec![all];
    while levels.last().unwrap().len() > 1 {
        let level = levels.len();
        levels.push(
            levels
                .last()
                .unwrap()
                .chunks_exact(2)
                .enumerate()
                .map(|(index, pair)| parent(level, index, pair[0], pair[1]).unwrap())
                .collect(),
        );
    }
    assert_eq!(actual.root, levels.last().unwrap()[0]);
    assert_eq!(
        actual.siblings,
        verifier
            .sibling_positions()
            .iter()
            .map(|p| levels[p.level][p.index])
            .collect::<Vec<_>>()
    );
    assert!(binding.tree_frame_bytes(Oracle::Row).unwrap() > WIDTH * 8);
    let full = MaskedReplayPlan::new(ReplayLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 3,
    })
    .unwrap();
    let plan = RowCommitmentPlan::new(full, &binding, &[], limits()).unwrap();
    assert!(plan.payload_bytes > full.payload_bytes + 7 * 65_536 * 48);
    assert!(
        RowCommitmentPlan::new(
            full,
            &binding,
            &[],
            StreamLimits {
                max_payload_bytes: plan.payload_bytes - 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(plan.build(&mut replay, &binding).is_err());
}
