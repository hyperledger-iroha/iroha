//! Genuine retained checkpoint and stopped-store replay owners; no report fixtures grant authority.

use super::*;
use crate::{
    query::store::LiveQueryStore,
    snapshot::{self, checkpoint_capture::tests::chain},
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, fixture_keys},
};
use iroha_config::{
    base::WithOrigin,
    kura::{FsyncMode, InitMode},
    parameters::{actual, defaults},
};
use std::path::Path;

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("floor-checkpoint-rehearsal".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}

fn config(path: &Path) -> actual::Kura {
    actual::Kura {
        init_mode: InitMode::Fast,
        store_dir: WithOrigin::inline(path.to_path_buf()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        native_context_archive_max_bytes: defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
        blocks_in_memory: defaults::kura::BLOCKS_IN_MEMORY,
        history_checkpoint_cache_capacity: defaults::kura::HISTORY_CHECKPOINT_CACHE_CAPACITY,
        block_hash_history_bytes: defaults::kura::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes: defaults::kura::TRANSACTION_HISTORY_BYTES,
        membership_storage: defaults::kura::MEMBERSHIP_STORAGE_POLICY,
        fastpq_artifacts: defaults::kura::FASTPQ_ARTIFACT_POLICY,
        debug_output_new_blocks: false,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
    }
}

// The producer is quiescent in this fixture. The actual owner below opens the resulting
// original through production Fast admission and never gains a test-only lock bypass.
fn copy_fixture(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == crate::kura::STORE_ROOT_LOCK_FILE_NAME {
            continue;
        }
        let destination = to.join(entry.file_name());
        let metadata = entry.metadata().unwrap();
        if metadata.is_dir() {
            std::fs::create_dir(&destination).unwrap();
            copy_fixture(&entry.path(), &destination);
            std::fs::set_permissions(&destination, metadata.permissions()).unwrap();
        } else {
            assert!(metadata.is_file());
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

struct Fixture {
    chain: CertifiedTestChain,
    _source_dir: tempfile::TempDir,
    snapshots: tempfile::TempDir,
    source: Arc<Kura>,
    copy: Arc<Kura>,
    template: Box<State>,
    checkpoints: SnapshotCheckpoints,
    height: u64,
}

fn keep(state: &State, root: &Path, key: &KeyPair, chunk: NonZeroUsize) {
    snapshot::try_write_snapshot(state, root, key, chunk).unwrap();
    let name = std::fs::read_to_string(root.join(snapshot::SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
    checkpoint_store::keep(
        root,
        &root
            .join(snapshot::SNAPSHOT_GENERATIONS_DIR_NAME)
            .join(name.trim_end()),
        state.committed_height() as u64,
        state
            .view()
            .native_execution_tip()
            .unwrap()
            .creation_time_ms(),
        defaults::snapshot::MAX_PAYLOAD_BYTES,
        chunk,
    )
    .unwrap();
}

fn fixture(suffix: usize) -> Fixture {
    let mut chain = chain();
    chain.commit(Vec::new());
    let height = chain.height();
    let snapshots = tempfile::tempdir().unwrap();
    let key = KeyPair::random();
    let chunk = NonZeroUsize::new(4096).unwrap();
    keep(chain.state(), snapshots.path(), &key, chunk);
    for _ in 0..suffix {
        chain.commit(Vec::new());
    }
    if suffix > 0 {
        // A newer retained snapshot must never replace the explicitly requested H.
        keep(chain.state(), snapshots.path(), &key, chunk);
    }
    let source_dir = tempfile::tempdir().unwrap();
    copy_fixture(&chain.kura().store_root(), source_dir.path());
    let configuration = config(source_dir.path());
    let nexus = chain.state().nexus_snapshot();
    let (source, _) = Kura::new_with_configured_lane_catalog(
        &configuration,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
    )
    .unwrap();
    let copy = replay_check::copy_store(
        &source,
        &configuration,
        &nexus.lane_config,
        &nexus.configured_lane_catalog,
        &mut |_| {},
    )
    .unwrap();
    let budget = chain.state().ivm_execution_budget();
    let world = World::try_with_execution_budget([], [], [], &budget).unwrap();
    let mut template = Box::new(
        State::try_new_with_chain_and_network_id(
            budget,
            world,
            Arc::clone(&copy),
            LiveQueryStore::start_test(),
            chain.state().chain_id_ref().clone(),
            chain.network_id(),
            #[cfg(feature = "telemetry")]
            crate::telemetry::StateTelemetry::default(),
        )
        .unwrap(),
    );
    template
        .install_snapshot_reconciliation_configuration(chain.state())
        .unwrap();
    *template.lane_manifests.write() = chain.state().lane_manifests.read().clone();
    let checkpoints = SnapshotCheckpoints {
        store_dir: snapshots.path().to_path_buf(),
        verification_key: key.public_key().clone(),
        max_payload_bytes: defaults::snapshot::MAX_PAYLOAD_BYTES,
        merkle_chunk_size: chunk,
        read_buffer_budget: AllocationBudget::new(defaults::snapshot::MAX_READ_BUFFER_BYTES.get()),
    };
    Fixture {
        chain,
        _source_dir: source_dir,
        snapshots,
        source,
        copy,
        template,
        checkpoints,
        height,
    }
}

fn restore(f: &Fixture) -> Result<RestoredFloorCheckpoint, FloorRehearsalError> {
    f.checkpoints.restore_exact_for_floor(
        f.height,
        Arc::clone(&f.source),
        &f.template,
        &f.chain.state().nexus_snapshot(),
        &f.chain.state().lane_manifests.read().clone(),
        None,
        SnapshotResourcePolicy::default(),
    )
}

fn replay(f: &Fixture) -> ReplayedFloorCheckpoint {
    restore(f).unwrap().replay(&mut |_, _| {}).unwrap()
}

#[test]
fn exact_retained_checkpoint_replays_only_its_suffix_and_signs_the_model_statement() {
    run(|| {
        let f = fixture(2);
        let restored = restore(&f).unwrap();
        assert_eq!(restored.state.committed_height() as u64, f.height);
        assert!(
            restored
                .state
                .ivm_execution_budget()
                .same_pool(&f.template.ivm_execution_budget())
        );
        let mut heights = Vec::new();
        let completed = restored
            .replay(&mut |height, _| heights.push(height))
            .unwrap();
        assert_eq!(heights, [f.height + 1, f.height + 2]);
        assert_eq!(
            completed
                .checked_replayed_state()
                .unwrap()
                .committed_height(),
            (f.height + 2) as usize,
        );
        let key = fixture_keys().remove(0);
        let (floor, signed) =
            crate::runtime::checkpoint::attest_replayed_floor(completed, &key).unwrap();
        assert_eq!(floor.height, f.height);
        signed.verify_for(&floor).unwrap();
        let ordinary = iroha_data_model::runtime::checkpoint::RuntimeStartupFloorV1::statement(
            &floor,
            key.public_key().clone(),
        )
        .unwrap()
        .sign(key.private_key())
        .unwrap();
        assert_eq!(
            signed, ordinary,
            "canonical domains and statement bytes stay unchanged"
        );
    });
}

#[test]
fn missing_exact_height_and_missing_successor_never_fall_back() {
    run(|| {
        let f = fixture(3);
        assert!(matches!(
            f.checkpoints.restore_exact_for_floor(
                1,
                Arc::clone(&f.source),
                &f.template,
                &f.chain.state().nexus_snapshot(),
                &f.chain.state().lane_manifests.read().clone(),
                None,
                SnapshotResourcePolicy::default(),
            ),
            Err(FloorRehearsalError::Snapshot(
                TryReadError::NativeExecutionReplayRequired
            ))
        ));
        assert!(matches!(
            f.checkpoints.restore_exact_for_floor(
                f.height + 1,
                Arc::clone(&f.source),
                &f.template,
                &f.chain.state().nexus_snapshot(),
                &f.chain.state().lane_manifests.read().clone(),
                None,
                SnapshotResourcePolicy::default(),
            ),
            Err(FloorRehearsalError::Invalid(_))
        ));
        let f = fixture(0);
        assert!(matches!(
            restore(&f),
            Err(FloorRehearsalError::Invalid(
                "requested checkpoint requires its actual certified successor"
            ))
        ));
    });
}

#[test]
fn live_source_and_same_store_cannot_mint_a_rehearsal_owner() {
    run(|| {
        let f = fixture(1);
        assert!(matches!(
            f.checkpoints.restore_exact_for_floor(
                f.height,
                Arc::clone(f.chain.kura()),
                &f.template,
                &f.chain.state().nexus_snapshot(),
                &f.chain.state().lane_manifests.read().clone(),
                None,
                SnapshotResourcePolicy::default(),
            ),
            Err(FloorRehearsalError::Invalid(
                "original source must own the stopped store's read-only root lock"
            ))
        ));
        assert!(matches!(
            StoppedSource::capture(
                Arc::clone(&f.source),
                &f.source,
                f.template.ivm_execution_budget(),
            ),
            Err(FloorRehearsalError::Invalid(
                "replay requires a distinct temporary Strict Kura"
            ))
        ));
    });
}

#[test]
fn replayed_checkpoint_refuses_changed_original_artifacts_and_outside_committee_key() {
    run(|| {
        let f = fixture(1);
        assert!(matches!(
            crate::runtime::checkpoint::attest_replayed_floor(replay(&f), &KeyPair::random()),
            Err(FloorRehearsalError::Invalid(
                "signer is not in the replayed current committee"
            ))
        ));
        let completed = replay(&f);
        let artifact = completed
            .generation
            .snapshot_directory
            .join(snapshot::SNAPSHOT_SIGNATURE_FILE_NAME);
        let bytes = std::fs::read(&artifact).unwrap();
        std::fs::remove_file(&artifact).unwrap();
        std::fs::write(&artifact, bytes).unwrap();
        assert!(matches!(
            completed.checked_replayed_state(),
            Err(FloorRehearsalError::Snapshot(_))
        ));
        assert!(matches!(
            crate::runtime::checkpoint::attest_replayed_floor(completed, &fixture_keys()[0]),
            Err(FloorRehearsalError::Snapshot(_))
        ));
    });
}

#[test]
fn exact_restore_retains_original_read_buffer_refusal_and_retries_without_fallback() {
    run(|| {
        let f = fixture(1);
        let budget = &f.checkpoints.read_buffer_budget;
        let held = budget.try_reserve_bytes(budget.limit_bytes()).unwrap();
        assert!(matches!(
            restore(&f),
            Err(FloorRehearsalError::Snapshot(
                TryReadError::PayloadAllocation(_)
            ))
        ));
        drop(held);
        assert_eq!(restore(&f).unwrap().floor.height, f.height);
    });
}

#[test]
fn another_terminal_copy_is_refused_before_snapshot_restore() {
    run(|| {
        let f = fixture(1);
        let other = fixture(2);
        assert!(matches!(
            StoppedSource::capture(
                Arc::clone(&f.source),
                &other.copy,
                f.template.ivm_execution_budget(),
            ),
            Err(FloorRehearsalError::Invalid(
                "original stopped source and replay copy no longer name the exact same tip"
            ))
        ));
        assert!(
            f.snapshots
                .path()
                .join(snapshot::SNAPSHOT_CURRENT_FILE_NAME)
                .is_file()
        );
    });
}

#[test]
fn cached_terminal_frame_does_not_hide_replaced_original_journal() {
    run(|| {
        let f = fixture(1);
        let completed = replay(&f);
        // Capture/replay has already populated Kura's native frame cache. The physical
        // journal handle must still detect same-byte replacement before signing.
        let path = Kura::canonical_storage_path(&f.source.store_root()).join("blocks.data");
        let moved = path.with_extension("original");
        std::fs::rename(&path, &moved).unwrap();
        std::fs::copy(&moved, &path).unwrap();
        assert!(matches!(
            crate::runtime::checkpoint::attest_replayed_floor(completed, &fixture_keys()[0]),
            Err(FloorRehearsalError::Storage(_))
        ));
    });
}

#[test]
fn ordinary_h_plus_one_authentication_rejects_changed_complete_state() {
    run(|| {
        let f = fixture(1);
        let restored = restore(&f).unwrap();
        // Test-only access to the private owner proves that retained bytes alone do not
        // bypass prepare_restored's complete-State/H+1 check.
        let mut world = restored
            .state
            .world
            .try_block(&restored.state.ivm_execution_budget())
            .unwrap();
        world
            .smart_contract_state
            .insert("checkpoint-populated-state".parse().unwrap(), vec![9]);
        world.commit();
        assert!(matches!(
            restored.replay(&mut |_, _| {}),
            Err(FloorRehearsalError::Replay(ReplayCheckError::Start(_)))
        ));
    });
}

#[test]
fn final_cold_root_failure_never_creates_the_successful_continuation() {
    run(|| {
        let f = fixture(1);
        let restored = restore(&f).unwrap();
        let private_test_observer = Arc::clone(&restored.state);
        let outcome = restored.replay(&mut |_, _| {
            let mut world = private_test_observer
                .world
                .try_block(&private_test_observer.ivm_execution_budget())
                .unwrap();
            world
                .smart_contract_state
                .insert("checkpoint-populated-state".parse().unwrap(), vec![42]);
            world.commit();
        });
        assert!(matches!(outcome,
            Err(FloorRehearsalError::Replay(ReplayCheckError::NotMatched(report)))
                if matches!(report.outcome, replay_check::ReplayCheckOutcome::Diverged(_))));
    });
}

#[test]
fn floor_signing_returns_original_view_contention_without_retry_or_signature() {
    run(|| {
        let f = fixture(1);
        let restored = restore(&f).unwrap();
        let observer = Arc::clone(&restored.state);
        let completed = restored.replay(&mut |_, _| {}).unwrap();
        let held = observer.lane_manifests.write();
        let key = fixture_keys().remove(0);
        let result = crate::runtime::checkpoint::attest_replayed_floor(completed, &key);
        assert!(matches!(
            result,
            Err(FloorRehearsalError::View(
                crate::state::StateViewError::Busy(_)
            ))
        ));
        drop(held);
        assert!(observer.try_view_once().is_ok());
    });
}

#[test]
fn restored_policy_helper_refuses_empty_state_and_missing_frozen_compliance() {
    run(|| {
        let mut f = fixture(1);
        let nexus = f.chain.state().nexus_snapshot();
        let manifests = f.chain.state().lane_manifests.read().clone();
        assert!(matches!(
            f.template
                .prepare_restored_replay_configuration(nexus.clone(), &manifests, None,),
            Err(crate::state::RestoredReplayConfigurationError::NotRestored)
        ));
        let restored = restore(&f).unwrap();
        let mut nexus = nexus;
        nexus.compliance.enabled = true;
        let mut state = restored.state;
        assert!(matches!(
            Arc::get_mut(&mut state)
                .unwrap()
                .prepare_restored_replay_configuration(nexus, &manifests, None,),
            Err(crate::state::RestoredReplayConfigurationError::ComplianceOwner)
        ));
    });
}

#[test]
fn retained_checkpoint_time_comes_from_certified_history_and_survives_rolling_gc() {
    run(|| {
        let f = fixture(1);
        let exact = checkpoint_store::entries(f.snapshots.path())
            .unwrap()
            .into_iter()
            .find(|(height, _, _)| *height == f.height)
            .unwrap();
        let wrong = exact
            .2
            .with_file_name(format!("{}-{}", exact.0, exact.1 + 1));
        std::fs::rename(&exact.2, &wrong).unwrap();
        assert!(matches!(restore(&f), Err(FloorRehearsalError::Snapshot(
            TryReadError::SnapshotGenerationInvalid { reason, .. }
        )) if reason.contains("restored native tip")));
        std::fs::rename(&wrong, &exact.2).unwrap();
        std::fs::remove_file(
            f.snapshots
                .path()
                .join(snapshot::SNAPSHOT_CURRENT_FILE_NAME),
        )
        .unwrap();
        std::fs::remove_dir_all(
            f.snapshots
                .path()
                .join(snapshot::SNAPSHOT_GENERATIONS_DIR_NAME),
        )
        .unwrap();
        let (floor, signed) =
            crate::runtime::checkpoint::attest_replayed_floor(replay(&f), &fixture_keys()[0])
                .unwrap();
        assert_eq!(floor.committed_ms, exact.1);
        signed.verify_for(&floor).unwrap();
    });
}
