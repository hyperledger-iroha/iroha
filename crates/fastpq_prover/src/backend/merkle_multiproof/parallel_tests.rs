//! Deterministic bounded parallel verification against the serial multiproof owner.

use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use super::*;
use crate::backend::compact_v1::{Context, Oracle};

fn limits() -> MultiproofLimits {
    MultiproofLimits {
        max_depth: 19,
        max_queried_leaves: 512,
        max_siblings: 9_728,
        max_parent_hashes: 9_728,
    }
}

#[test]
fn parallel_parents_preserve_complete_six_lane_trees_roots_and_work() {
    let parallel = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let context =
        Context::new(b"independent complete six-lane parallel verification fixture").unwrap();
    for (round, count, query_count) in [(17, 1, 1), (15, 8, 3), (10, 256, 173), (8, 1_024, 375)] {
        let oracle = Oracle::Fri(round);
        let indices: Vec<_> = (0..query_count).map(|i| i * count / query_count).collect();
        let mut leaves: Vec<_> = (0..count)
            .map(|i| {
                let mut bytes = vec![0; if round == 17 { 128 } else { 64 }];
                bytes[..8].copy_from_slice(&u64::try_from(i + 1).unwrap().to_le_bytes());
                context
                    .hash_leaf(oracle, u32::try_from(i).unwrap(), &bytes)
                    .unwrap()
            })
            .collect();
        if count == 1 {
            leaves.push(leaves[0]);
        }
        let parent = |level: usize, index: usize, left, right| {
            context
                .hash_parent(
                    oracle,
                    u32::try_from(level).unwrap(),
                    u32::try_from(index).unwrap(),
                    left,
                    right,
                )
                .map_err(|_| shape("six-lane fixture parent rejected"))
        };
        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            levels.push(
                levels
                    .last()
                    .unwrap()
                    .chunks_exact(2)
                    .enumerate()
                    .map(|(i, pair)| parent(levels.len(), i, pair[0], pair[1]).unwrap())
                    .collect(),
            );
        }
        let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
        let siblings = plan.open_with(&levels, parent).unwrap();
        let selected: Vec<_> = indices.iter().map(|&i| levels[0][i]).collect();
        let root = levels.last().unwrap()[0];
        let reference = plan
            .verify_with(root, &selected, &siblings, parent)
            .unwrap();
        for pool in [&serial, &parallel] {
            let calls = AtomicUsize::new(0);
            let actual = pool
                .install(|| {
                    plan.verify_parallel_with(
                        root,
                        &selected,
                        &siblings,
                        |level, index, left, right| {
                            calls.fetch_add(1, Ordering::Relaxed);
                            parent(level, index, left, right)
                        },
                    )
                })
                .unwrap();
            assert_eq!(actual, reference);
            assert_eq!(calls.load(Ordering::Relaxed), reference.parent_hashes);
            assert_eq!(actual.max_frontier_width, query_count);
        }
        let mut changed_root = root.words();
        changed_root[5] ^= 1;
        let changed_root = Digest::new(changed_root).unwrap();
        assert_eq!(
            format!(
                "{:?}",
                plan.verify_with(changed_root, &selected, &siblings, parent)
            ),
            format!(
                "{:?}",
                parallel.install(|| plan.verify_parallel_with(
                    changed_root,
                    &selected,
                    &siblings,
                    parent
                ))
            ),
        );
        if !siblings.is_empty() {
            let mut changed = siblings.clone();
            let mut words = changed[0].words();
            words[4] ^= 1;
            changed[0] = Digest::new(words).unwrap();
            assert_eq!(
                format!("{:?}", plan.verify_with(root, &selected, &changed, parent)),
                format!(
                    "{:?}",
                    parallel
                        .install(|| plan.verify_parallel_with(root, &selected, &changed, parent))
                ),
            );
        }
    }
}

#[test]
fn parallel_cardinality_rejections_do_no_hash_work() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    for count in [1, 8, 128] {
        let indices: Vec<_> = (0..count).step_by(2).collect();
        let plan = MultiproofPlan::new(count, &indices, limits()).unwrap();
        let leaves = vec![Digest::default(); indices.len()];
        let siblings = vec![Digest::default(); plan.work().siblings];
        for (bad_leaves, bad_siblings) in [
            (leaves[..leaves.len() - 1].to_vec(), siblings.clone()),
            (
                {
                    let mut value = leaves.clone();
                    value.push(Digest::default());
                    value
                },
                siblings.clone(),
            ),
            (leaves.clone(), {
                let mut value = siblings.clone();
                value.push(Digest::default());
                value
            }),
        ] {
            let expected = plan.verify_with(
                Digest::default(),
                &bad_leaves,
                &bad_siblings,
                |_, _, _, _| panic!("bad counts reached serial hash"),
            );
            let actual = pool.install(|| {
                plan.verify_parallel_with(
                    Digest::default(),
                    &bad_leaves,
                    &bad_siblings,
                    |_, _, _, _| panic!("bad counts reached parallel hash"),
                )
            });
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }
    }
}

#[test]
fn parallel_errors_use_first_canonical_parent_without_retries_or_later_levels() {
    let indices: Vec<_> = (0..375).map(|i| i * 1_024 / 375).collect();
    let plan = MultiproofPlan::new(1_024, &indices, limits()).unwrap();
    let leaves = vec![Digest::default(); indices.len()];
    let siblings = vec![Digest::default(); plan.work().siblings];
    let mut positions = Vec::new();
    plan.reconstruct(&leaves, &siblings, |level, index, _, _| {
        positions.push((level, index));
        Ok(Digest::default())
    })
    .unwrap();
    for failed_level in [1, 2, 4] {
        let this_level: Vec<_> = positions
            .iter()
            .copied()
            .filter(|p| p.0 == failed_level)
            .collect();
        let failures = [
            this_level[1],
            this_level[this_level.len() / 2],
            *this_level.last().unwrap(),
        ];
        let hash = |level, index, _, _| {
            if failures.contains(&(level, index)) {
                Err(Error::QueryIndexOutOfRange { index, len: level })
            } else {
                Ok(Digest::default())
            }
        };
        let expected = plan
            .verify_with(Digest::default(), &leaves, &siblings, hash)
            .unwrap_err();
        for workers in [1, 4, 64] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let visited = Mutex::new(std::collections::BTreeSet::new());
            let actual = pool
                .install(|| {
                    plan.verify_parallel_with(
                        Digest::default(),
                        &leaves,
                        &siblings,
                        |level, index, left, right| {
                            assert!(
                                level <= failed_level,
                                "a later level ran after parent rejection"
                            );
                            assert!(
                                visited.lock().unwrap().insert((level, index)),
                                "parent retried"
                            );
                            if (level, index) == failures[0] {
                                for _ in 0..100 {
                                    std::thread::yield_now();
                                }
                            }
                            hash(level, index, left, right)
                        },
                    )
                })
                .unwrap_err();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            if workers == 1 {
                let last = *visited.lock().unwrap().last().unwrap();
                assert_eq!(
                    last, failures[0],
                    "single-worker fallback must stop immediately"
                );
            }
        }
    }
}

#[test]
fn maximum_query_frontier_caps_live_parent_jobs_even_in_a_larger_pool() {
    assert_eq!(MAX_PARALLEL_PARENT_JOBS, 32);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(64)
        .build()
        .unwrap();
    let indices: Vec<_> = (0..512).map(|i| 2 * i).collect();
    let plan = MultiproofPlan::new(1_024, &indices, limits()).unwrap();
    let leaves = vec![Digest::default(); indices.len()];
    let siblings = vec![Digest::default(); plan.work().siblings];
    let live = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let calls = AtomicUsize::new(0);
    let actual = pool
        .install(|| {
            plan.verify_parallel_with(Digest::default(), &leaves, &siblings, |_, _, _, _| {
                let active = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(active, Ordering::SeqCst);
                calls.fetch_add(1, Ordering::Relaxed);
                std::thread::yield_now();
                live.fetch_sub(1, Ordering::SeqCst);
                Ok(Digest::default())
            })
        })
        .unwrap();
    assert_eq!(actual, plan.work());
    assert_eq!(calls.load(Ordering::Relaxed), actual.parent_hashes);
    assert_eq!(live.load(Ordering::SeqCst), 0);
    assert!(peak.load(Ordering::SeqCst) <= MAX_PARALLEL_PARENT_JOBS);
    assert_eq!(actual.max_frontier_width, 512);
}
