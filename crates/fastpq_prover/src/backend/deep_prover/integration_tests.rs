//! Typed streamed commitments retain exact frontiers and complete terminal binding.

use super::*;
use crate::backend::{
    GOLDILOCKS_MODULUS,
    merkle_multiproof::{MultiproofLimits, MultiproofPlan},
};

/// Narrow one small fixture position or level to its `u32` wire field.
fn narrow_u32(value: usize) -> u32 {
    u32::try_from(value).expect("fixture position fits u32")
}

fn coefficient_limits() -> CoefficientLimits {
    CoefficientLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 1,
    }
}

fn stream_limits() -> StreamLimits {
    StreamLimits {
        digest_execution: DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        max_hashes: usize::MAX,
    }
}

fn dense(seed: u64) -> F {
    F::new([seed + 1, 2 * seed + 3, 3 * seed + 5, 5 * seed + 7]).unwrap()
}

fn changed_digest(digest: Digest) -> Digest {
    let mut words = digest.words();
    words[0] = (words[0] + 1) % GOLDILOCKS_MODULUS;
    Digest::new(words).unwrap()
}

fn multiproof(leaves: usize, indices: &[usize]) -> MultiproofPlan {
    MultiproofPlan::new(
        leaves,
        indices,
        MultiproofLimits {
            max_depth: 23,
            max_queried_leaves: QUERY_COUNT,
            max_siblings: QUERY_COUNT * 23,
            max_parent_hashes: QUERY_COUNT * 23,
        },
    )
    .unwrap()
}

#[test]
fn streamed_fri_frontier_rejects_changed_leaves_siblings_context_and_oracle() {
    let binding = Context::new(b"streamed final FRI frontier integrity").unwrap();
    let oracle = Oracle::Fri(4);
    let replay = CoefficientReplayPlan::fri(4, coefficient_limits()).unwrap();
    let coefficients = (0..replay.degree())
        .map(|i| dense(i as u64))
        .collect::<Vec<_>>();
    let indices = [0, 1, 31, 64, 127];
    let actual = commit(
        replay,
        &binding,
        oracle,
        &indices,
        &[&coefficients],
        stream_limits(),
    )
    .unwrap();
    let plan = multiproof(128, &indices);
    let mut levels = vec![
        (0..128)
            .map(|index| {
                let payload = (0..4)
                    .flat_map(|position| {
                        let x = replay.domain().point(index + position * 128);
                        coefficients
                            .iter()
                            .rev()
                            .fold(F::ZERO, |sum, &value| sum.mul_base(x).add(value))
                            .to_le_bytes()
                    })
                    .collect::<Vec<_>>();
                binding
                    .hash_leaf(oracle, narrow_u32(index), &payload)
                    .unwrap()
            })
            .collect::<Vec<_>>(),
    ];
    while levels.last().unwrap().len() > 1 {
        let level = levels.len();
        let next = levels
            .last()
            .unwrap()
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| {
                binding
                    .hash_parent(
                        oracle,
                        narrow_u32(level),
                        narrow_u32(index),
                        pair[0],
                        pair[1],
                    )
                    .unwrap()
            })
            .collect();
        levels.push(next);
    }
    assert_eq!(
        levels.iter().map(Vec::len).collect::<Vec<_>>(),
        [128, 64, 32, 16, 8, 4, 2, 1]
    );
    assert_eq!(actual.root, levels.last().unwrap()[0]);
    assert_eq!(actual.siblings.len(), plan.work().siblings);
    for (&value, position) in actual.siblings.iter().zip(plan.sibling_positions()) {
        assert_eq!(value, levels[position.level][position.index]);
    }
    let leaves = indices.map(|index| levels[0][index]);
    let verify = |context: &Context, oracle, leaves: &[Digest], siblings: &[Digest]| {
        plan.verify_parallel_with(
            actual.root,
            leaves,
            siblings,
            |level, index, left, right| {
                context
                    .hash_parent(oracle, narrow_u32(level), narrow_u32(index), left, right)
                    .map_err(binding_error)
            },
        )
    };
    assert_eq!(
        verify(&binding, oracle, &leaves, &actual.siblings).unwrap(),
        plan.work()
    );
    let mut altered_leaves = leaves;
    altered_leaves[2] = changed_digest(altered_leaves[2]);
    assert!(verify(&binding, oracle, &altered_leaves, &actual.siblings).is_err());
    let mut altered_siblings = actual.siblings.clone();
    altered_siblings[0] = changed_digest(altered_siblings[0]);
    assert!(verify(&binding, oracle, &leaves, &altered_siblings).is_err());
    assert!(
        verify(
            &binding,
            oracle,
            &leaves,
            &actual.siblings[..actual.siblings.len() - 1]
        )
        .is_err()
    );
    let other = Context::new(b"another streamed final FRI context").unwrap();
    assert!(verify(&other, oracle, &leaves, &actual.siblings).is_err());
    assert!(verify(&binding, Oracle::Fri(3), &leaves, &actual.siblings).is_err());
}

#[test]
fn streamed_terminal_binds_every_linear_value_and_requires_its_duplicate_parent() {
    let binding = Context::new(b"complete streamed terminal integrity").unwrap();
    let replay = CoefficientReplayPlan::terminal(coefficient_limits()).unwrap();
    let coefficients = [dense(19), dense(23)];
    let actual = commit(
        replay,
        &binding,
        Oracle::Terminal,
        &[],
        &[&coefficients],
        stream_limits(),
    )
    .unwrap();
    let terminal = actual.terminal().unwrap();
    assert_eq!(terminal.len(), 128);
    for (index, &value) in terminal.iter().enumerate() {
        assert_eq!(
            value,
            coefficients[0].add(coefficients[1].mul_base(replay.domain().point(index)))
        );
    }
    assert_ne!(terminal[0], terminal[127]);
    let payload = terminal
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let leaf = binding.hash_leaf(Oracle::Terminal, 0, &payload).unwrap();
    assert_ne!(actual.root, leaf);
    assert!(actual.siblings.is_empty());
    let plan = multiproof(1, &[0]);
    let verify = |root, leaf| {
        plan.verify_with(root, &[leaf], &[], |level, index, left, right| {
            binding
                .hash_parent(
                    Oracle::Terminal,
                    narrow_u32(level),
                    narrow_u32(index),
                    left,
                    right,
                )
                .map_err(binding_error)
        })
    };
    let work = verify(actual.root, leaf).unwrap();
    assert_eq!(work.parent_hashes, 1);
    assert_eq!(work.queried_leaves, 1);
    assert!(verify(leaf, leaf).is_err());
    for index in [0, terminal.len() / 2, terminal.len() - 1] {
        let mut changed = payload.clone();
        changed[index * F::BYTES..(index + 1) * F::BYTES]
            .copy_from_slice(&terminal[index].add(F::ONE).to_le_bytes());
        let changed_leaf = binding.hash_leaf(Oracle::Terminal, 0, &changed).unwrap();
        assert!(verify(actual.root, changed_leaf).is_err());
    }
    assert!(
        binding
            .hash_leaf(Oracle::Terminal, 0, &payload[..payload.len() - F::BYTES])
            .is_err()
    );
    assert!(
        binding
            .hash_parent(Oracle::Terminal, 1, 0, leaf, changed_digest(leaf))
            .is_err()
    );
}
