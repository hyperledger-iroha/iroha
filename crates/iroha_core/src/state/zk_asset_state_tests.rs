//! Confidential asset-tree state regression tests.

use super::*;

fn push_dummy_root(state: &mut ZkAssetState, seed: u8) {
    state
        .push_commitment(
            [seed; 32],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect("canonical test commitment");
}

#[test]
fn record_frontier_checkpoint_reports_evictions() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    let first = state
        .record_frontier_checkpoint(1, 1, 5)
        .expect("canonical empty root");
    assert!(first.recorded);
    assert_eq!(first.evicted, 0);
    push_dummy_root(&mut state, 2);
    let second = state
        .record_frontier_checkpoint(2, 1, 5)
        .expect("canonical empty root");
    assert!(second.recorded);
    assert_eq!(second.evicted, 0);

    // Exceed depth bound so the oldest checkpoint is dropped.
    push_dummy_root(&mut state, 10);
    let third = state
        .record_frontier_checkpoint(10, 1, 1)
        .expect("canonical empty root");
    assert!(third.recorded);
    assert!(
        third.evicted >= 1,
        "expected an eviction once the depth bound is exceeded"
    );

    // When depth bound is zero, keep only the latest checkpoint.
    push_dummy_root(&mut state, 20);
    let before_cp = state.frontier_checkpoints.len();
    let fourth = state
        .record_frontier_checkpoint(20, 1, 0)
        .expect("canonical empty root");
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

#[test]
fn empty_checkpoint_uses_profile_root() {
    let mut state = ZkAssetState::default();
    let update = state
        .record_frontier_checkpoint(1, 1, 4)
        .expect("canonical empty tree");
    assert!(update.recorded);
    assert_eq!(state.frontier_checkpoints.len(), 1);
    assert_eq!(
        state.frontier_checkpoints[0].root,
        ConfidentialTreeProfile::PoseidonPastaV1.empty_root()
    );
    state
        .validate_tree_integrity()
        .expect("empty checkpoint follows the profile");
}

#[test]
fn tree_integrity_rejects_tampered_retained_root() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    push_dummy_root(&mut state, 2);
    state.root_history[0][0] ^= 0x80;
    let before = state.commitments.clone();

    let error = state
        .push_commitment(
            [3; 32],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("tampered retained roots must fail closed");
    assert!(error.contains("root history"));
    assert_eq!(state.commitments, before);
}

#[test]
fn tree_integrity_rejects_tampered_checkpoint() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    state
        .record_frontier_checkpoint(1, 1, 4)
        .expect("canonical checkpoint");
    state.frontier_checkpoints[0].root[0] ^= 0x80;

    let error = state
        .validate_tree_integrity()
        .expect_err("tampered checkpoint must fail closed");
    assert!(error.contains("checkpoint root"));
}

#[test]
fn invalid_commitment_is_rolled_back() {
    let mut state = ZkAssetState::default();
    let error = state
        .push_commitment(
            [0; 32],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("zero is not a canonical confidential commitment");
    assert!(error.contains("non-zero and canonical"));
    assert!(state.commitments.is_empty());
    assert!(state.root_history.is_empty());
}

#[test]
fn commitment_batch_is_atomic() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    let before_commitments = state.commitments.clone();
    let before_roots = state.root_history.clone();

    let error = state
        .push_commitments(
            &[[2; 32], [0; 32], [3; 32]],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("one malformed commitment rejects the complete batch");
    assert!(error.contains("non-zero and canonical"));
    assert_eq!(state.commitments, before_commitments);
    assert_eq!(state.root_history, before_roots);
}

#[test]
fn capacity_overflow_rejects_the_complete_batch_without_residue() {
    let mut state = ZkAssetState::default();
    state.commitments = vec![[1; 32]; state.tree_profile.capacity() - 1];
    let before_commitments = state.commitments.clone();
    let before_roots = state.root_history.clone();

    let error = state
        .push_commitments(
            &[[2; 32], [3; 32]],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("two outputs cannot partially consume the final tree slot");
    assert!(error.contains("tree capacity"));
    assert_eq!(state.commitments, before_commitments);
    assert_eq!(state.root_history, before_roots);
}

#[test]
fn persisted_tree_profile_roundtrips_and_is_required() {
    for circuit_id in [
        crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
        crate::zk::confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        crate::zk::confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID,
    ] {
        assert_eq!(
            ConfidentialTreeProfile::for_circuit_id(circuit_id),
            Some(ConfidentialTreeProfile::PoseidonPastaV1)
        );
    }
    assert_eq!(
        ConfidentialTreeProfile::for_circuit_id("unsupported/confidential-tree"),
        None
    );

    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    let encoded = norito::to_bytes(&state).expect("encode ZK asset state");
    let decoded =
        norito::decode_from_bytes::<ZkAssetState>(&encoded).expect("decode ZK asset state");
    assert_eq!(
        decoded.tree_profile,
        ConfidentialTreeProfile::PoseidonPastaV1
    );
    decoded
        .validate_tree_integrity()
        .expect("decoded profile state remains canonical");

    let mut missing_profile = norito::json::to_value(&state).expect("encode ZK asset JSON state");
    missing_profile
        .as_object_mut()
        .expect("ZK asset state object")
        .remove("tree_profile");
    assert!(
        norito::json::from_value::<ZkAssetState>(missing_profile).is_err(),
        "first-release snapshots must explicitly persist the tree profile"
    );

    let mut unknown_profile_field =
        norito::json::to_value(&state).expect("encode ZK asset JSON state");
    unknown_profile_field
        .as_object_mut()
        .expect("ZK asset state object")
        .insert("legacy_tree".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<ZkAssetState>(unknown_profile_field).is_err(),
        "unknown tree encodings must not be ignored"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn telemetry_stats_reflect_tree_state() {
    let mut state = ZkAssetState::default();
    state
        .push_commitment(
            [1; 32],
            NonZeroUsize::new(4).expect("non-zero root history cap"),
        )
        .expect("canonical commitment");
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
