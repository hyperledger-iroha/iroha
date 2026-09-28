//! Real typed FRI/terminal root, grouped leaf and minimal frontier regressions.

use super::*;
use crate::backend::{
    deep_coefficient_replay::CoefficientLimits,
    merkle_multiproof::{MultiproofLimits, MultiproofPlan},
};
fn replay_limits() -> CoefficientLimits {
    CoefficientLimits {
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_full_passes: 2,
    }
}
fn limits() -> StreamLimits {
    StreamLimits {
        digest_execution: crate::DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        max_hashes: usize::MAX,
    }
}
fn horner(coefficients: &[F], point: u64) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul_base(point).add(value))
}
fn dense(i: usize) -> F {
    F::new([i as u64 + 1, i as u64 + 3, i as u64 + 7, i as u64 + 13]).unwrap()
}

#[test]
fn last_fri_root_and_selected_fibers_match_materialized_canonical_tree() {
    let binding = Context::new(b"coefficient commitment canonical FRI parity").unwrap();
    let plan = CoefficientReplayPlan::fri(4, replay_limits()).unwrap();
    let coefficients = (0..plan.degree()).map(dense).collect::<Vec<_>>();
    let sources = [&coefficients[..]];
    let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
    let query = [0, 1, 63, 64, 127];
    let commit_plan =
        CoefficientCommitmentPlan::new(plan, &binding, Oracle::Fri(4), &query, limits()).unwrap();
    assert_eq!(commit_plan.leaf_hashes, 128);
    assert_eq!(commit_plan.parent_hashes, 127);
    assert!(
        CoefficientCommitmentPlan::new(
            plan,
            &binding,
            Oracle::Fri(4),
            &query,
            StreamLimits {
                max_payload_bytes: commit_plan.payload_bytes - 1,
                ..limits()
            }
        )
        .is_err()
    );
    let actual = commit_plan.build(&mut replay, &binding).unwrap();
    let all = (0..plan.rows())
        .map(|i| horner(&coefficients, plan.domain().point(i)))
        .collect::<Vec<_>>();
    let groups = (0..128)
        .map(|index| {
            (0..4)
                .map(|position| all[index + position * 128])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (&index, opened) in query.iter().zip(actual.openings()) {
        assert_eq!(opened, groups[index]);
    }
    assert!(actual.terminal().is_err());
    let leaves = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let bytes = group
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect::<Vec<_>>();
            binding
                .hash_leaf(Oracle::Fri(4), index as u32, &bytes)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let verifier = MultiproofPlan::new(
        128,
        &query,
        MultiproofLimits {
            max_depth: 7,
            max_queried_leaves: 64,
            max_siblings: 448,
            max_parent_hashes: 448,
        },
    )
    .unwrap();
    let parent = |level: usize, index: usize, left, right| {
        binding
            .hash_parent(Oracle::Fri(4), level as u32, index as u32, left, right)
            .map_err(binding_error)
    };
    verifier
        .verify_with(
            actual.root,
            &query.iter().map(|&i| leaves[i]).collect::<Vec<_>>(),
            &actual.siblings,
            parent,
        )
        .unwrap();
    let mut level = leaves;
    let mut depth = 1;
    while level.len() > 1 {
        level = level
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| parent(depth, index, pair[0], pair[1]).unwrap())
            .collect();
        depth += 1;
    }
    assert_eq!(actual.root, level[0]);
    let root_only = CoefficientCommitmentPlan::new(plan, &binding, Oracle::Fri(4), &[], limits())
        .unwrap()
        .build(&mut replay, &binding)
        .unwrap();
    assert_eq!(root_only.root, actual.root);
    assert_eq!(root_only.openings().count(), 0);
    assert!(root_only.siblings.is_empty());
}

#[test]
fn complete_terminal_and_phase_preflight_are_exact() {
    let binding = Context::new(b"complete coefficient terminal").unwrap();
    let plan = CoefficientReplayPlan::terminal(replay_limits()).unwrap();
    let coefficients = [dense(11), dense(23)];
    let sources = [&coefficients[..]];
    let mut replay = CoefficientReplay::new(plan, &sources).unwrap();
    assert!(CoefficientCommitmentPlan::new(plan, &binding, Oracle::Row, &[], limits()).is_err());
    assert!(CoefficientCommitmentPlan::new(plan, &binding, Oracle::Fri(4), &[], limits()).is_err());
    assert!(
        CoefficientCommitmentPlan::new(plan, &binding, Oracle::Terminal, &[0], limits()).is_err()
    );
    let commit =
        CoefficientCommitmentPlan::new(plan, &binding, Oracle::Terminal, &[], limits()).unwrap();
    let actual = commit.build(&mut replay, &binding).unwrap();
    let expected = (0..128)
        .map(|i| horner(&coefficients, plan.domain().point(i)))
        .collect::<Vec<_>>();
    assert_eq!(actual.terminal().unwrap(), expected);
    assert!(
        plan.domain()
            .evaluations_have_degree_below(actual.terminal().unwrap(), 2)
            .unwrap()
    );
    let bytes = expected
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let leaf = binding.hash_leaf(Oracle::Terminal, 0, &bytes).unwrap();
    assert_eq!(
        actual.root,
        binding
            .hash_parent(Oracle::Terminal, 1, 0, leaf, leaf)
            .unwrap()
    );
    assert!(actual.siblings.is_empty());
    assert!(pack(&coefficients, &mut [0; 63]).is_err());
    let quotient = CoefficientReplayPlan::quotient_and_mask(replay_limits()).unwrap();
    assert!(
        CoefficientCommitmentPlan::new(quotient, &binding, Oracle::QuotientAndMask, &[0], limits())
            .is_err()
    );
    let quotient_plan =
        CoefficientCommitmentPlan::new(quotient, &binding, Oracle::QuotientAndMask, &[], limits())
            .unwrap();
    assert_eq!(quotient_plan.leaf_hashes, 8_388_608);
    assert_eq!(quotient_plan.parent_hashes, 8_388_607);
    assert!(quotient_plan.build(&mut replay, &binding).is_err());
}

#[test]
fn cached_fri_openings_are_byte_identical_to_full_reference_traversal() {
    let binding = Context::new(b"actual cached FRI fiber parity").unwrap();
    let plan = CoefficientReplayPlan::fri(4, replay_limits()).unwrap();
    let coefficients = (0..plan.degree()).map(dense).collect::<Vec<_>>();
    let sources = [&coefficients[..]];
    for query in [
        vec![0],
        vec![127],
        vec![0, 1, 63, 64, 126, 127],
        (0..128).step_by(2).collect(),
    ] {
        let mut reference_replay = CoefficientReplay::new(plan, &sources).unwrap();
        let reference =
            CoefficientCommitmentPlan::new(plan, &binding, Oracle::Fri(4), &query, limits())
                .unwrap()
                .build(&mut reference_replay, &binding)
                .unwrap();
        let mut cached_replay = CoefficientReplay::new(plan, &sources).unwrap();
        let mut committed =
            CoefficientCommitmentPlan::new(plan, &binding, Oracle::Fri(4), &[], limits())
                .unwrap()
                .commit(&mut cached_replay, &binding)
                .unwrap();
        let cache = committed
            .cache
            .take()
            .unwrap()
            .bind(&binding, Oracle::Fri(4), committed.root)
            .unwrap();
        let opened = open_cached(
            cache,
            &mut cached_replay,
            &query,
            crate::DigestExecutionV1::Cpu,
        )
        .unwrap();
        assert_eq!(opened.root, reference.root);
        assert_eq!(opened.siblings, reference.siblings);
        assert_eq!(
            opened.openings().collect::<Vec<_>>(),
            reference.openings().collect::<Vec<_>>()
        );
        let mut expected = vec![0; reference.selected.len() * F::BYTES];
        let mut actual = vec![0; opened.selected.len() * F::BYTES];
        pack(&reference.selected, &mut expected).unwrap();
        pack(&opened.selected, &mut actual).unwrap();
        assert_eq!(actual, expected);
        assert!(cached_replay.ensure_pass_available().is_err());
    }
}
