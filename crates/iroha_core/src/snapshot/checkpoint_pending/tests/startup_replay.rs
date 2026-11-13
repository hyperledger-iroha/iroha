//! Startup persists original certified replay cuts before publishing their State.

use super::*;
use crate::snapshot::{CapturedStateSnapshot, CheckpointDeferral, SNAPSHOT_CURRENT_FILE_NAME};
use crate::state::StateReadOnly as _;
use crate::sumeragi::driver::traits::{PublicationDeferral, PublicationError};
use crate::sumeragi::node::{self, NodeError, PrepareInputs, ReplayError};

fn configured_replay_writer(state: Arc<crate::state::State>, directory: &Path) -> SnapshotMaker {
    let mut writer = maker(Arc::clone(&state), directory);
    writer.checkpoints.receiver = Some(
        state
            .runtime_checkpoint_capture
            .arm(writer.max_payload_bytes),
    );
    writer
}

fn replay_inputs(chain: &crate::sumeragi::test_chain::CertifiedTestChain) -> PrepareInputs {
    let (events, _) = tokio::sync::broadcast::channel(4096);
    PrepareInputs {
        state: Arc::clone(chain.state()),
        events,
        genesis: None,
        genesis_account: chain.genesis_account().clone(),
        consensus_mode: chain.validated_genesis().consensus_metadata().mode.into(),
    }
}

#[test]
fn startup_replay_recovers_checkpoint_after_crash_before_capture_and_acknowledgement() {
    let mut original = chain();
    let state = Arc::clone(original.state());
    let inputs = replay_inputs(&original);
    let proposal = original.proposal(Some(HOUR_MS), Vec::new());
    let mut pending = original
        .begin_proposal(proposal, Default::default())
        .unwrap();
    let result = pending.result();
    pending.prepare(Signers::Quorum).unwrap();
    pending.append_prepared().unwrap();
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 2);
    drop(pending);
    drop(original); // Original Worker joins; its never-published execution owner is gone.
    let directory = tempfile::tempdir().unwrap();
    let mut writer = configured_replay_writer(Arc::clone(&state), directory.path());
    assert!(entries(directory.path()).unwrap().is_empty());
    let prepared = node::prepare_with_checkpoint_writer(inputs, &mut writer).unwrap();
    assert_eq!(state.committed_height(), 2);
    assert_eq!(
        state.view().native_execution_tip().unwrap().result(),
        result
    );
    let checkpoints = checkpoint_store::entries(directory.path()).unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!((checkpoints[0].0, checkpoints[0].1), (2, HOUR_MS));
    assert_eq!(
        std::fs::read_to_string(checkpoints[0].2.join(crate::snapshot::SNAPSHOT_FILE_NAME))
            .unwrap(),
        CapturedStateSnapshot::capture(&state).unwrap().json
    );
    assert!(entries(directory.path()).unwrap().is_empty());
    assert!(
        !directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists(),
        "startup checkpoint preservation must not publish a rolling pointer"
    );
    assert!(writer.latest_block_hash.is_none());
    drop(prepared);
}

#[test]
fn startup_replay_preserves_more_cadence_cuts_than_spool_capacity_without_retention() {
    let mut source = chain();
    for hours in 1..=5 {
        source.commit_at(hours * HOUR_MS, Vec::new());
    }
    let destination = chain();
    assert_eq!(
        source.genesis().encode_wire().unwrap(),
        destination.genesis().encode_wire().unwrap()
    );
    let state = Arc::clone(destination.state());
    let inputs = replay_inputs(&destination);
    for height in 2..=source.height() {
        destination
            .kura()
            .store_block(source.committed(height).block().clone())
            .unwrap();
    }
    drop(destination);
    let directory = tempfile::tempdir().unwrap();
    let mut writer = configured_replay_writer(Arc::clone(&state), directory.path());
    let prepared = node::prepare_with_checkpoint_writer(inputs, &mut writer).unwrap();
    assert_eq!(state.committed_height() as u64, source.height());
    assert_eq!(
        CapturedStateSnapshot::capture(&state).unwrap().json,
        CapturedStateSnapshot::capture(source.state()).unwrap().json
    );
    let checkpoints = checkpoint_store::entries(directory.path()).unwrap();
    assert_eq!(
        checkpoints
            .iter()
            .map(|(height, _, _)| *height)
            .collect::<Vec<_>>(),
        vec![2, 3, 4, 5, 6]
    );
    assert!(entries(directory.path()).unwrap().is_empty());
    assert!(!directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists());
    // At the final committed time normal retention would retire at least one older cut.
    // Startup has retained every certified original and leaves that policy action to readiness.
    let schedule = writer.current_checkpoint_schedule().unwrap();
    let held: Vec<_> = checkpoints
        .iter()
        .map(|(height, time, _)| (*height, *time))
        .collect();
    assert!(schedule.retained(&held).len() < held.len());
    drop(prepared);
}

#[test]
fn startup_checkpoint_persistence_refusal_keeps_certified_height_unpublished() {
    let mut original = chain();
    let inputs = replay_inputs(&original);
    let state = Arc::clone(original.state());
    let proposal = original.proposal(Some(HOUR_MS), Vec::new());
    let mut pending = original
        .begin_proposal(proposal, Default::default())
        .unwrap();
    pending.prepare(Signers::Quorum).unwrap();
    pending.append_prepared().unwrap();
    drop(pending);
    drop(original);
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("blocked-store");
    std::fs::write(&store, b"occupied by a file").unwrap();
    let mut writer = configured_replay_writer(Arc::clone(&state), &store);
    let error = node::prepare_with_checkpoint_writer(inputs, &mut writer)
        .err()
        .expect("persistence must refuse before State visibility");
    assert!(matches!(
        error,
        NodeError::Replay {
            height: 2,
            reason: ReplayError::Execution(PublicationError::Deferred(
                PublicationDeferral::Checkpoint(CheckpointDeferral::Persistence { height: 2, .. })
            ))
        }
    ));
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 2);
    assert_eq!(std::fs::read(store).unwrap(), b"occupied by a file");
}

#[test]
fn startup_checkpoint_writer_refuses_a_different_original_state() {
    let first = chain();
    let second = chain();
    let directory = tempfile::tempdir().unwrap();
    let mut writer = configured_replay_writer(Arc::clone(first.state()), directory.path());
    let error = node::prepare_with_checkpoint_writer(replay_inputs(&second), &mut writer)
        .err()
        .expect("foreign State owner must refuse");
    assert!(matches!(
        error,
        NodeError::Replay {
            reason: ReplayError::Execution(PublicationError::Deferred(
                PublicationDeferral::Checkpoint(CheckpointDeferral::Persistence {
                    kind: io::ErrorKind::InvalidInput,
                    ..
                })
            )),
            ..
        }
    ));
    assert_eq!(first.state().committed_height(), 1);
    assert_eq!(second.state().committed_height(), 1);
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn startup_checkpoint_corruption_is_terminal_before_certified_visibility() {
    let mut original = chain();
    let inputs = replay_inputs(&original);
    let state = Arc::clone(original.state());
    let receiver = state
        .runtime_checkpoint_capture
        .start(NonZeroUsize::new(64 * 1024 * 1024).unwrap());
    let directory = tempfile::tempdir().unwrap();
    let old_writer = maker(Arc::clone(&state), directory.path());
    let key = old_writer.signing_key.clone();
    let proposal = original.proposal(Some(HOUR_MS), Vec::new());
    let mut pending = original
        .begin_proposal(proposal, Default::default())
        .unwrap();
    pending.publish(Signers::Quorum).unwrap_err();
    let job = receiver.try_recv().unwrap();
    old_writer.persist_pending_checkpoint(&job.capture).unwrap();
    let held = entries(directory.path()).unwrap().remove(0);
    let signature = held
        .path
        .join(crate::snapshot::SNAPSHOT_SIGNATURE_FILE_NAME);
    std::fs::write(&signature, b"00").unwrap();
    drop(pending);
    drop(job);
    drop(receiver);
    drop(old_writer);
    drop(original);
    state.runtime_checkpoint_capture.stop();
    let mut writer = configured_replay_writer(Arc::clone(&state), directory.path());
    writer.signing_key = key;
    let error = node::prepare_with_checkpoint_writer(inputs, &mut writer)
        .err()
        .expect("permanent pending artifact mismatch must stop startup");
    assert!(matches!(
        error,
        NodeError::Replay {
            height: 2,
            reason: ReplayError::Execution(PublicationError::RecoveryRequired(_)),
        }
    ));
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 2);
    assert_eq!(std::fs::read(signature).unwrap(), b"00");
    assert_eq!(entries(directory.path()).unwrap().len(), 1);
    assert!(
        checkpoint_store::entries(directory.path())
            .unwrap()
            .is_empty()
    );
    assert!(!directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists());
}

#[test]
fn repeated_startup_replay_reuses_exact_retained_images_without_duplicate_spool() {
    let mut source = chain();
    source.commit_at(HOUR_MS, Vec::new());
    source.commit_at(2 * HOUR_MS, Vec::new());
    let directory = tempfile::tempdir().unwrap();
    let mut key: Option<iroha_crypto::KeyPair> = None;
    let mut first_bytes = Vec::new();
    for attempt in 0..2 {
        let destination = chain();
        let inputs = replay_inputs(&destination);
        let state = Arc::clone(destination.state());
        for height in 2..=source.height() {
            destination
                .kura()
                .store_block(source.committed(height).block().clone())
                .unwrap();
        }
        drop(destination);
        let mut writer = configured_replay_writer(state, directory.path());
        if let Some(original) = key.as_ref() {
            writer.signing_key = original.clone();
        }
        key = Some(writer.signing_key.clone());
        let prepared = node::prepare_with_checkpoint_writer(inputs, &mut writer).unwrap();
        let held = checkpoint_store::entries(directory.path()).unwrap();
        assert_eq!(held.len(), 2);
        assert!(entries(directory.path()).unwrap().is_empty());
        let bytes: Vec<_> = held
            .iter()
            .map(|(_, _, path)| {
                std::fs::read(path.join(crate::snapshot::SNAPSHOT_FILE_NAME)).unwrap()
            })
            .collect();
        if attempt == 0 {
            first_bytes = bytes;
        } else {
            assert_eq!(bytes, first_bytes);
        }
        assert!(!directory.path().join(SNAPSHOT_CURRENT_FILE_NAME).exists());
        drop(prepared);
    }
}

#[test]
fn startup_replay_rebuilds_only_the_interrupted_original_staging_identity() {
    let mut source = chain();
    source.commit_at(HOUR_MS, Vec::new());
    let destination = chain();
    let inputs = replay_inputs(&destination);
    let state = Arc::clone(destination.state());
    destination
        .kura()
        .store_block(source.committed(2).block().clone())
        .unwrap();
    drop(destination);
    let directory = tempfile::tempdir().unwrap();
    let stage = directory
        .path()
        .join(DIRECTORY)
        .join(format!(".staging-2-{HOUR_MS}"));
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(
        stage.join(crate::snapshot::SNAPSHOT_FILE_NAME),
        b"interrupted original copy",
    )
    .unwrap();
    let mut writer = configured_replay_writer(state, directory.path());
    let prepared = node::prepare_with_checkpoint_writer(inputs, &mut writer).unwrap();
    assert!(!stage.exists());
    assert!(entries(directory.path()).unwrap().is_empty());
    let held = checkpoint_store::entries(directory.path()).unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].0, 2);
    assert_eq!(
        std::fs::read_to_string(held[0].2.join(crate::snapshot::SNAPSHOT_FILE_NAME)).unwrap(),
        CapturedStateSnapshot::capture(source.state()).unwrap().json
    );
    drop(prepared);
}
