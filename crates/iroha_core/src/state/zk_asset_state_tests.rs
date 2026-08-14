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
fn hot_append_rejects_tampered_current_root_history_tail() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    push_dummy_root(&mut state, 2);
    state
        .root_history
        .last_mut()
        .expect("retained current root")[0] ^= 0x80;
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
fn full_integrity_rejects_tampered_older_retained_root() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    push_dummy_root(&mut state, 2);
    state.root_history[0][0] ^= 0x01;
    let error = state
        .validate_tree_integrity()
        .expect_err("recovery audit must reject an older retained-root mismatch");
    assert!(error.contains("root history"));
}
#[test]
fn hot_append_rejects_tampered_incremental_frontier() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    state.tree_frontier[0]
        .as_mut()
        .expect("one-leaf frontier slot")[0] ^= 0x01;
    let before = state.commitments.clone();
    let error = state
        .push_commitment(
            [2; 32],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("tampered incremental frontier must fail closed");
    assert!(error.contains("frontier") || error.contains("current root"));
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
    let before_frontier = state.tree_frontier;
    let before_current_root = state.persisted_root;
    let error = state
        .push_commitments(
            &[[2; 32], [0; 32], [3; 32]],
            NonZeroUsize::new(64).expect("non-zero root history cap"),
        )
        .expect_err("one malformed commitment rejects the complete batch");
    assert!(error.contains("non-zero and canonical"));
    assert_eq!(state.commitments, before_commitments);
    assert_eq!(state.root_history, before_roots);
    assert_eq!(state.tree_frontier, before_frontier);
    assert_eq!(state.persisted_root, before_current_root);
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
fn tree_frontier_json_roundtrips_and_rejects_wrong_cardinality() {
    let mut state = ZkAssetState::default();
    push_dummy_root(&mut state, 1);
    let encoded = norito::json::to_json(&state).expect("encode ZK asset JSON state");
    let decoded: ZkAssetState =
        norito::json::from_str(&encoded).expect("decode ZK asset JSON state");
    assert_eq!(decoded.tree_frontier, state.tree_frontier);
    let value = norito::json::to_value(&state).expect("encode ZK asset JSON value");
    let frontier = value
        .as_object()
        .and_then(|object| object.get("tree_frontier"))
        .and_then(norito::json::Value::as_array)
        .expect("tree frontier JSON array");
    assert_eq!(
        frontier.len(),
        crate::zk::confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2
    );
    assert!(
        frontier[0].as_str().is_some(),
        "present frontier nodes use canonical hex strings"
    );
    assert!(
        frontier.iter().any(norito::json::Value::is_null),
        "unused frontier slots remain explicit null values"
    );
    let mut short = value.clone();
    short
        .as_object_mut()
        .and_then(|object| object.get_mut("tree_frontier"))
        .and_then(norito::json::Value::as_array_mut)
        .expect("mutable tree frontier JSON array")
        .pop();
    let error = norito::json::from_value::<ZkAssetState>(short)
        .expect_err("a 15-entry tree frontier must be rejected");
    assert!(error.to_string().contains("expected exactly 16"));
    let mut malformed = value.clone();
    malformed
        .as_object_mut()
        .and_then(|object| object.get_mut("tree_frontier"))
        .and_then(norito::json::Value::as_array_mut)
        .expect("mutable tree frontier JSON array")[0] =
        norito::json::Value::String("GG".repeat(32));
    let error = norito::json::from_value::<ZkAssetState>(malformed)
        .expect_err("a malformed frontier digest must be rejected");
    assert!(error.to_string().contains("invalid hex digit"));
    let mut long = value;
    long.as_object_mut()
        .and_then(|object| object.get_mut("tree_frontier"))
        .and_then(norito::json::Value::as_array_mut)
        .expect("mutable tree frontier JSON array")
        .push(norito::json::Value::Null);
    let error = norito::json::from_value::<ZkAssetState>(long)
        .expect_err("a 17-entry tree frontier must be rejected");
    assert!(error.to_string().contains("expected exactly 16"));
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
    assert_eq!(decoded.tree_frontier, state.tree_frontier);
    assert_eq!(decoded.persisted_root, state.persisted_root);
    decoded
        .validate_tree_integrity()
        .expect("decoded profile state remains canonical");
    let current_json = norito::json::to_value(&state).expect("encode ZK asset JSON state");
    for (field, value) in [
        ("mode", norito::json::Value::String("Hybrid".to_owned())),
        ("allow_shield", norito::json::Value::Bool(true)),
        ("allow_unshield", norito::json::Value::Bool(true)),
    ] {
        assert!(
            !current_json
                .as_object()
                .expect("ZK asset state object")
                .contains_key(field),
            "retired field {field} must not be serialized"
        );
        let mut retired_shape = current_json.clone();
        retired_shape
            .as_object_mut()
            .expect("ZK asset state object")
            .insert(field.to_owned(), value);
        assert!(
            norito::json::from_value::<ZkAssetState>(retired_shape).is_err(),
            "first-release snapshots must reject retired field {field}"
        );
    }
    let mut missing_profile = norito::json::to_value(&state).expect("encode ZK asset JSON state");
    missing_profile
        .as_object_mut()
        .expect("ZK asset state object")
        .remove("tree_profile");
    assert!(
        norito::json::from_value::<ZkAssetState>(missing_profile).is_err(),
        "first-release snapshots must explicitly persist the tree profile"
    );
    let mut missing_frontier = norito::json::to_value(&state).expect("encode ZK asset JSON state");
    missing_frontier
        .as_object_mut()
        .expect("ZK asset state object")
        .remove("tree_frontier");
    assert!(
        norito::json::from_value::<ZkAssetState>(missing_frontier).is_err(),
        "first-release snapshots must explicitly persist the incremental frontier"
    );
    let mut missing_current_root =
        norito::json::to_value(&state).expect("encode ZK asset JSON state");
    missing_current_root
        .as_object_mut()
        .expect("ZK asset state object")
        .remove("persisted_root");
    assert!(
        norito::json::from_value::<ZkAssetState>(missing_current_root).is_err(),
        "first-release snapshots must explicitly persist the current root"
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
    state
        .record_frontier_checkpoint(10, 1, 4)
        .expect("canonical checkpoint");
    let telemetry_snapshot = state.telemetry_stats(3, 1);
    assert_eq!(telemetry_snapshot.commitments, 2);
    assert!(
        telemetry_snapshot.tree_depth >= 1,
        "tree depth should reflect inserted commitment"
    );
    assert_eq!(telemetry_snapshot.root_history, 2);
    assert_eq!(telemetry_snapshot.frontier_checkpoints, 1);
    assert_eq!(telemetry_snapshot.last_checkpoint_height, 10);
    assert_eq!(telemetry_snapshot.last_checkpoint_commitments, 2);
    assert_eq!(telemetry_snapshot.root_evictions, 3);
    assert_eq!(telemetry_snapshot.frontier_evictions, 1);
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
