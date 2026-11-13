//! Durable checkpoint ownership across lost publishers and startup reconciliation.

use super::*;
use crate::snapshot::checkpoint_capture::tests::{HOUR_MS, chain, commit_checkpoint, maker};
use crate::sumeragi::test_chain::Signers;
use std::{num::NonZeroUsize, sync::Arc};

#[test]
fn durable_before_visibility_survives_original_owner_loss_and_actual_successor() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let key = writer.signing_key.clone();
    let before = state.committed_height();
    let proposal = chain.proposal(Some(HOUR_MS), Vec::new());
    let mut original = chain.begin_proposal(proposal, Default::default()).unwrap();
    assert!(
        original
            .publish(Signers::Quorum)
            .unwrap_err()
            .contains("durable acknowledgement")
    );
    let job = receiver.try_recv().unwrap();
    job.acknowledge(writer.persist_pending_checkpoint(&job.capture));
    assert_eq!(state.committed_height(), before);
    assert_eq!(entries(directory.path()).unwrap().len(), 1);
    assert!(
        checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    writer.reconcile_pending_checkpoints(None).unwrap();
    assert_eq!(
        entries(directory.path()).unwrap().len(),
        1,
        "previsibility acknowledgement grants no retention authority"
    );
    let original_json = job.capture.snapshot.json.clone();
    drop(original);
    drop(job);
    drop(receiver);
    drop(writer);
    state.runtime_checkpoint_capture.stop();
    // A restart reconstructs from the exact certified durable suffix. It does not
    // sign another result or recapture a replacement checkpoint for the lost owner.
    chain.replay_durable_unobserved_suffix().unwrap();
    assert_eq!(state.committed_height(), before + 1);
    assert_eq!(
        crate::snapshot::CapturedStateSnapshot::capture(&state)
            .unwrap()
            .json,
        original_json
    );
    let mut restarted = maker(Arc::clone(&state), directory.path());
    restarted.signing_key = key;
    restarted.reconcile_pending_checkpoints(None).unwrap();
    assert!(
        checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty(),
        "tip has no certified H+1 yet"
    );
    // The fixture authors its normal clock transaction; no idle or empty block bypass.
    chain.commit_at(HOUR_MS + 1, Vec::new());
    restarted.reconcile_pending_checkpoints(None).unwrap();
    assert!(entries(directory.path()).unwrap().is_empty());
    assert_eq!(
        checkpoint_store::entries(directory.path()).unwrap()[0].0,
        (before + 1) as u64
    );
}

#[test]
fn visible_before_promotion_restores_from_pending_after_rolling_generation_loss() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let key = writer.signing_key.clone();
    let job = commit_checkpoint(&mut chain, HOUR_MS, &receiver, &writer);
    let height = job.capture.height;
    assert!(job.visible());
    drop(job);
    drop(writer);
    drop(receiver);
    state.runtime_checkpoint_capture.stop();
    std::fs::remove_dir_all(
        directory
            .path()
            .join(super::super::SNAPSHOT_GENERATIONS_DIR_NAME),
    )
    .unwrap();
    let mut restarted = maker(Arc::clone(&state), directory.path());
    restarted.signing_key = key;
    restarted.reconcile_pending_checkpoints(None).unwrap();
    assert_eq!(entries(directory.path()).unwrap().len(), 1);
    assert!(
        checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    chain.commit_at(HOUR_MS + 1, Vec::new());
    let candidates = super::super::restorable_snapshot_generations(
        directory.path(),
        state.committed_height(),
        0,
        restarted.signing_key.public_key(),
        state.chain_id_ref(),
        *state.network_id_ref(),
    )
    .unwrap()
    .unwrap();
    assert!(
        candidates
            .names
            .iter()
            .any(|location| matches!(location, SnapshotLocation::PendingCheckpoint { .. }))
    );
    let restored = super::super::try_read_snapshot_with_initializer(
        &state.ivm_execution_budget(),
        directory.path(),
        chain.kura(),
        &state.lane_manifests.read().clone(),
        &state.nexus_snapshot(),
        || state.query_handle.clone(),
        crate::kura::BlockCount(state.committed_height()),
        restarted.merkle_chunk_size,
        restarted.max_payload_bytes,
        restarted.resource_policy,
        restarted.signing_key.public_key(),
        state.chain_id_ref(),
        state.network_id_ref(),
        &|restored| {
            restored
                .install_snapshot_reconciliation_configuration(&state)
                .map_err(super::super::TryReadError::ReconciliationConfiguration)
        },
        #[cfg(feature = "telemetry")]
        crate::telemetry::StateTelemetry::default(),
        &restarted.read_buffer_budget,
    )
    .unwrap();
    assert!(restored.authenticate_restored_keyed_state().unwrap());
    assert_eq!(restored.committed_height() as u64, height);
    restarted.reconcile_pending_checkpoints(None).unwrap();
    assert!(entries(directory.path()).unwrap().is_empty());
    assert_eq!(
        checkpoint_store::entries(directory.path()).unwrap()[0].0,
        height
    );
}

#[test]
fn corrupt_pending_artifacts_are_refused_before_waiting_for_successor() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let job = commit_checkpoint(&mut chain, HOUR_MS, &receiver, &writer);
    let pending = entries(directory.path()).unwrap().remove(0);
    for artifact in [
        super::super::SNAPSHOT_SIGNATURE_FILE_NAME,
        super::super::SNAPSHOT_MERKLE_FILE_NAME,
        super::super::SNAPSHOT_FILE_NAME,
    ] {
        let path = pending.path.join(artifact);
        let original = std::fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[0] ^= 1;
        std::fs::write(&path, changed).unwrap();
        assert!(
            writer.reconcile_pending_checkpoints(None).is_err(),
            "{artifact} corruption must not look like missing H+1"
        );
        assert!(
            checkpoint_store::entries(directory.path())
                .unwrap()
                .is_empty()
        );
        std::fs::write(path, original).unwrap();
    }
    writer.reconcile_pending_checkpoints(None).unwrap();
    assert_eq!(entries(directory.path()).unwrap().len(), 1);
    assert!(!job.abandoned());
    state.runtime_checkpoint_capture.stop();
}

#[test]
fn full_pending_quota_defers_the_same_original_until_a_visible_slot_is_promoted() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let writer = maker(Arc::clone(&state), directory.path());
    let first = commit_checkpoint(&mut chain, HOUR_MS, &receiver, &writer);
    let _second = commit_checkpoint(&mut chain, 2 * HOUR_MS, &receiver, &writer);
    let proposal = chain.proposal(Some(3 * HOUR_MS), Vec::new());
    let before = state.committed_height();
    let mut original = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = original.result();
    original.publish(Signers::Quorum).unwrap_err();
    let job = receiver.try_recv().unwrap();
    let error = writer.persist_pending_checkpoint(&job.capture).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    job.acknowledge(Err(error));
    assert!(
        original
            .publish(Signers::Quorum)
            .unwrap_err()
            .contains("persistence refused")
    );
    assert_eq!(original.result(), result);
    assert_eq!(state.committed_height(), before);
    assert_eq!(entries(directory.path()).unwrap().len(), 2);
    writer.promote_visible_checkpoint(&first.capture).unwrap();
    job.acknowledge(writer.persist_pending_checkpoint(&job.capture));
    original.publish(Signers::Quorum).unwrap();
    assert_eq!(original.result(), result);
    drop(original);
    assert_eq!(state.committed_height(), before + 1);
    state.runtime_checkpoint_capture.stop();
}

mod resource_refusal;
mod startup_replay;
