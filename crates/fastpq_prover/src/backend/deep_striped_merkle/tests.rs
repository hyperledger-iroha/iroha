//! Exhaustive small frontier parity against materialized natural-order trees.

use super::*;

fn limits() -> StreamLimits {
    StreamLimits {
        digest_execution: crate::DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        max_hashes: usize::MAX,
    }
}
fn hash(level: usize, index: usize, left: Digest, right: Digest) -> Result<Digest> {
    {
        let mut hash = fastpq_isi::keccak256::Sha3_256V1::new();
        hash.update(b"test:striped:parent:");
        hash.update(&(level as u64).to_le_bytes());
        hash.update(&(index as u64).to_le_bytes());
        hash.update(left.as_bytes());
        hash.update(right.as_bytes());
        Ok(hash.finalize())
    }
}
fn leaves(size: usize) -> Vec<Digest> {
    (0..size)
        .map(|i| Digest::from_bytes(core::array::from_fn(|lane| (i * 7 + lane + 11) as u8)))
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

fn batch_hash(
    level: usize,
    indices: &[usize],
    left: &[[u8; 32]],
    right: &mut [[u8; 32]],
) -> Result<()> {
    for ((&index, &left), right) in indices.iter().zip(left).zip(right.iter_mut()) {
        *right = hash(level, index, digest(left), digest(*right))?.into_bytes();
    }
    Ok(())
}

#[test]
fn batched_rows_preserve_every_small_frontier_and_canonical_parent_coordinate() {
    for count in [1, 2, 4, 8] {
        let leaves = leaves(count);
        let expected = reference(&leaves);
        for mask in 0_usize..1 << count {
            let queries = (0..count)
                .filter(|i| mask & (1 << i) != 0)
                .collect::<Vec<_>>();
            for stripes in [1, 2, 4, 8].into_iter().filter(|&n| n <= count) {
                for capacity in [1, 3, 8] {
                    let plan = StripedMerklePlan::new(count, stripes, &queries, limits()).unwrap();
                    let frontier = plan
                        .openings
                        .as_ref()
                        .map(|p| {
                            p.sibling_positions()
                                .iter()
                                .map(|p| expected[p.level][p.index])
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let mut stream = plan.start().unwrap();
                    for stripe in 0..stripes {
                        for start in (0..count / stripes).step_by(capacity) {
                            let length = (count / stripes - start).min(capacity);
                            let indices = (start..start + length)
                                .map(|row| stripe + row * stripes)
                                .collect::<Vec<_>>();
                            let mut values = SecretPolynomial::zeroed(length).unwrap();
                            for (&index, value) in indices.iter().zip(values.iter_mut()) {
                                *value = leaves[index].into_bytes();
                            }
                            stream
                                .push_batch(&indices, &mut values, batch_hash, hash)
                                .unwrap();
                        }
                    }
                    let actual = stream.finish(hash).unwrap();
                    assert_eq!(actual.root, expected.last().unwrap()[0]);
                    assert_eq!(actual.siblings, frontier);
                    assert_eq!(actual.parent_hashes, (count - 1).max(1));
                    assert_eq!(actual.leaf_hashes, count);
                }
            }
        }
    }
}

#[test]
fn malformed_or_partial_parent_batches_poison_the_complete_stream() {
    for indices in [vec![], vec![0, 1], vec![0, 2, 4], vec![2]] {
        let mut stream = StripedMerklePlan::new(4, 2, &[], limits())
            .unwrap()
            .start()
            .unwrap();
        let mut values = SecretPolynomial::zeroed(indices.len()).unwrap();
        assert!(
            stream
                .push_batch(&indices, &mut values, batch_hash, hash)
                .is_err()
        );
        assert!(
            stream
                .push_batch(&[0], &mut [[0; 32]], batch_hash, hash)
                .is_err()
        );
        assert!(stream.finish(hash).is_err());
    }
    // Every opaque digest bit pattern is valid; malformed field-word rejection
    // belonged to the retired digest. Ordering/shape and callback failures remain.
    for marker in [0_u8, 0xff] {
        let mut stream = StripedMerklePlan::new(4, 2, &[], limits())
            .unwrap()
            .start()
            .unwrap();
        stream
            .push_batch(&[0, 2], &mut [[1; 32]; 2], batch_hash, hash)
            .unwrap();
        let partial = |_: usize, _: &[usize], _: &[[u8; 32]], right: &mut [[u8; 32]]| {
            right[0] = [marker; 32];
            Err(invalid("partial parent failure"))
        };
        assert!(
            stream
                .push_batch(&[1, 3], &mut [[2; 32]; 2], partial, hash)
                .is_err()
        );
        assert!(stream.finish(hash).is_err());
    }
    let mut stream = StripedMerklePlan::new(2, 1, &[], limits())
        .unwrap()
        .start()
        .unwrap();
    assert!(
        stream
            .push_batch(&[0, 1], &mut [[1; 32]; 2], batch_hash, |_, _, _, _| Err(
                invalid("upper hash failure")
            ))
            .is_err()
    );
    assert!(stream.finish(hash).is_err());
}

#[test]
fn full_capacity_batches_preserve_sparse_frontiers_across_run_boundaries() {
    let count = super::super::deep_leaf_batch::CAPACITY * 4;
    let values = leaves(count);
    let expected = reference(&values);
    let queries = [0, 1, count / 2 - 1, count / 2, count - 2, count - 1];
    let plan = StripedMerklePlan::new(count, 2, &queries, limits()).unwrap();
    let positions = plan.openings.as_ref().unwrap().sibling_positions().to_vec();
    let mut stream = plan.start().unwrap();
    for stripe in 0..2 {
        for start in (0..count / 2).step_by(super::super::deep_leaf_batch::CAPACITY) {
            let indices = (start..start + super::super::deep_leaf_batch::CAPACITY)
                .map(|row| stripe + 2 * row)
                .collect::<Vec<_>>();
            let mut batch = SecretPolynomial::zeroed(indices.len()).unwrap();
            for (&index, value) in indices.iter().zip(batch.iter_mut()) {
                *value = values[index].into_bytes();
            }
            stream
                .push_batch(&indices, &mut batch, batch_hash, hash)
                .unwrap();
        }
    }
    let result = stream.finish(hash).unwrap();
    assert_eq!(result.root, expected.last().unwrap()[0]);
    assert_eq!(
        result.siblings,
        positions
            .iter()
            .map(|p| expected[p.level][p.index])
            .collect::<Vec<_>>()
    );
    assert_eq!(result.parent_hashes, count - 1);
    assert_eq!(result.leaf_hashes, count);
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
    // Every rejected insertion poisons the owner, including after coverage.
    assert!(stream.finish(hash).is_err());
}

#[test]
fn actual_masked_rows_use_canonical_binding_and_match_materialized_frontiers() {
    use crate::backend::{
        compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH,
        deep_binding::{Context, Oracle},
        deep_masked_replay::MaskedTraceReplay,
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
        None,
    )
    .unwrap();
    let all = materialized_row_leaves(&mut replay, &binding, leaves_count);
    let parent = |level: usize, index: usize, left, right| {
        binding
            .hash_parent(
                Oracle::Row,
                u32::try_from(level).unwrap(),
                u32::try_from(index).unwrap(),
                left,
                right,
            )
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
    assert_full_row_plan_budget(&binding, &mut replay);
}

/// Hash every replayed row under its canonical row-leaf binding, in natural order.
fn materialized_row_leaves(
    replay: &mut crate::backend::deep_masked_replay::MaskedTraceReplay,
    binding: &crate::backend::deep_binding::Context,
    leaves_count: usize,
) -> Vec<Digest> {
    use crate::backend::{
        compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH, deep_binding::Oracle,
    };
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
                    .hash_leaf(
                        Oracle::Row,
                        u32::try_from(stripe.global_index(row)).unwrap(),
                        &bytes,
                    )
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
    all
}

/// A complete-domain row plan charges its frames and rejects one byte less.
fn assert_full_row_plan_budget(
    binding: &crate::backend::deep_binding::Context,
    replay: &mut crate::backend::deep_masked_replay::MaskedTraceReplay,
) {
    use crate::backend::deep_masked_replay::{MaskedReplayPlan, ReplayLimits};
    let full = MaskedReplayPlan::new(ReplayLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 3,
    })
    .unwrap();
    let plan = RowCommitmentPlan::new(full, binding, &[], limits()).unwrap();
    assert!(plan.payload_bytes > full.payload_bytes + 7 * 65_536 * 32);
    assert!(
        RowCommitmentPlan::new(
            full,
            binding,
            &[],
            StreamLimits {
                max_payload_bytes: plan.payload_bytes - 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(plan.build(replay, binding).is_err());
}

#[test]
fn extra_push_after_complete_coverage_poisoned_even_if_error_is_ignored() {
    let binding =
        super::super::deep_binding::Context::new(b"complete stream rejected extra write").unwrap();
    for scalar_extra in [false, true] {
        let mut stream = StripedMerklePlan::new(1, 1, &[], limits())
            .unwrap()
            .start()
            .unwrap();
        stream.push(0, leaves(1)[0], hash).unwrap();
        if scalar_extra {
            assert!(stream.push(0, leaves(1)[0], hash).is_err());
        } else {
            assert!(
                stream
                    .push_batch(&[0], &mut [leaves(1)[0].into_bytes()], batch_hash, hash)
                    .is_err()
            );
        }
        assert!(stream.finish(hash).is_err());
    }
    // Cached complete coverage has the same poisoning rule; the first root
    // callback must never run after the rejected extra insertion.
    let cache = super::super::deep_node_cache::NodeCachePlan::new(
        super::super::deep_binding::Oracle::Fri(4),
    )
    .unwrap()
    .start(&binding)
    .unwrap();
    let mut stream = StripedMerklePlan::new(128, 1, &[], limits())
        .unwrap()
        .start_cached(cache)
        .unwrap();
    for (i, leaf) in leaves(128).into_iter().enumerate() {
        stream.push(i, leaf, hash).unwrap();
    }
    assert!(stream.push(128, leaves(1)[0], hash).is_err());
    assert!(
        stream
            .finish(|_, _, _, _| panic!("poisoned cached stream cannot hash"))
            .is_err()
    );
}
