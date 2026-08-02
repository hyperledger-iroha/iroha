//! Confidential asset-tree state regression tests.

use super::*;

fn push_dummy_root(state: &mut ZkAssetState, seed: u8) {
    state.root_history.push([seed; 32]);
}

#[test]
fn record_frontier_checkpoint_reports_evictions() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    let first = state.record_frontier_checkpoint(1, 1, 5);
    assert!(first.recorded);
    assert_eq!(first.evicted, 0);
    push_dummy_root(&mut state, 2);
    let second = state.record_frontier_checkpoint(2, 1, 5);
    assert!(second.recorded);
    assert_eq!(second.evicted, 0);

    // Exceed depth bound so the oldest checkpoint is dropped.
    push_dummy_root(&mut state, 10);
    let third = state.record_frontier_checkpoint(10, 1, 1);
    assert!(third.recorded);
    assert!(
        third.evicted >= 1,
        "expected an eviction once the depth bound is exceeded"
    );

    // When depth bound is zero, keep only the latest checkpoint.
    push_dummy_root(&mut state, 20);
    let before_cp = state.frontier_checkpoints.len();
    let fourth = state.record_frontier_checkpoint(20, 1, 0);
    assert!(fourth.recorded);
    assert!(
        fourth.evicted >= before_cp.saturating_sub(1) as u64,
        "expected at least one eviction when depth bound is zero"
    );
    assert!(
        !state.frontier_checkpoints.is_empty(),
        "frontier checkpoints should retain the newest entry"
    );
    assert_eq!(
        state
            .frontier_checkpoints
            .last()
            .expect("checkpoint present")
            .height,
        20
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn telemetry_stats_reflect_tree_state() {
    let mut state = ZkAssetState::default();
    state.push_commitment([1; 32], 4);
    push_dummy_root(&mut state, 2);
    state.frontier_checkpoints.push(FrontierCheckpoint {
        height: 10,
        commitment_count: 1,
        root: [2; 32],
    });
    let telemetry_snapshot = state.telemetry_stats(3, 1);
    assert_eq!(telemetry_snapshot.commitments, 1);
    assert!(
        telemetry_snapshot.tree_depth >= 1,
        "tree depth should reflect inserted commitment"
    );
    assert_eq!(telemetry_snapshot.root_history, 2);
    assert_eq!(telemetry_snapshot.frontier_checkpoints, 1);
    assert_eq!(telemetry_snapshot.last_checkpoint_height, 10);
    assert_eq!(telemetry_snapshot.last_checkpoint_commitments, 1);
    assert_eq!(telemetry_snapshot.root_evictions, 3);
    assert_eq!(telemetry_snapshot.frontier_evictions, 1);
}
