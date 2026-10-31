//! Coalesce the local tiered projection while canonical startup replay owns State.

use super::*;

/// Startup-only persistence scope. It grants no execution or restoration authority.
pub(crate) struct StartupTieredReplay {
    state: Arc<State>,
}

impl State {
    /// Defer derived row snapshots until startup has authenticated its final World.
    /// The caller must own startup before any live execution producer is admitted.
    pub(crate) fn defer_tiered_snapshots_for_startup_replay(
        self: &Arc<Self>,
    ) -> Result<StartupTieredReplay, String> {
        let worker = &self.tiered_snapshot_worker.inner;
        worker
            .startup_replay_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "tiered startup replay already has an owner".to_owned())?;
        let owner = StartupTieredReplay {
            state: Arc::clone(self),
        };
        // The worker never acquires State/World locks. Finish its previously
        // accepted payloads before invalidating their baseline or replaying.
        let mut pending = worker.pending.lock();
        while pending.payload.is_some() || pending.processing {
            worker.cvar.wait(&mut pending);
        }
        drop(pending);
        worker.backend.lock().invalidate_snapshot_baseline();
        Ok(owner)
    }
}

impl StartupTieredReplay {
    /// Persist the complete authenticated tip before live execution can start.
    pub(crate) fn finish(self) -> Result<(), String> {
        if !self.state.tiered_backend.lock().enabled() {
            return Ok(());
        }
        // Startup has joined each serialized execution request and no live producer
        // exists. Retain the exact completed World, releasing its locks before I/O.
        let payload = {
            let world = self
                .state
                .world
                .try_block(&self.state.ivm_execution_budget())
                .map_err(|error| format!("final startup tiered capture failed: {error}"))?;
            world.tiered_snapshot_payload_with_scope(true)
        };
        let mut backend = self.state.tiered_backend.lock();
        backend
            .record_world_snapshot_with_payload(&payload)
            .map_err(|error| format!("final startup tiered snapshot failed: {error:#}"))?;
        #[cfg(feature = "telemetry")]
        record_tiered_snapshot_metrics(&backend, &self.state.telemetry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::store::LiveQueryStore;

    fn fixture() -> (Arc<State>, tempfile::TempDir) {
        let mut world = World::new();
        world
            .smart_contract_state
            .insert("kept".parse().unwrap(), vec![1]);
        world
            .smart_contract_state
            .insert("deleted".parse().unwrap(), vec![2]);
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let directory = tempfile::tempdir().unwrap();
        *state.tiered_backend.lock() = TieredStateBackend::new(
            true,
            0,
            0,
            0,
            Some(directory.path().to_path_buf()),
            None,
            0,
            0,
        );
        state.tiered_snapshot_worker = TieredSnapshotWorker::inert(
            Arc::clone(&state.tiered_backend),
            #[cfg(feature = "telemetry")]
            None,
        );
        (Arc::new(state), directory)
    }

    fn manifest_count(directory: &std::path::Path) -> usize {
        std::fs::read_dir(directory)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("manifest.json").is_file())
            .count()
    }

    fn mutate(state: &State, value: u8) {
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert("kept".parse().unwrap(), vec![value]);
        world
            .smart_contract_state
            .remove("deleted".parse().unwrap());
        let prepared = PreparedTieredSnapshot::prepare(&world, &state.tiered_snapshot_worker);
        world.commit();
        prepared.publish(state);
    }

    fn assert_final_values(state: &State, value: u8) {
        use sha2::{Digest, Sha256};
        let backend = state.tiered_backend.lock();
        assert!(backend.snapshot_baseline_ready());
        let manifest = backend.last_manifest().unwrap();
        let manifest = norito::json::to_value(manifest).unwrap();
        let rows: Vec<_> = manifest["hot_entries"]
            .as_array()
            .unwrap()
            .iter()
            .chain(manifest["cold_entries"].as_array().unwrap())
            .collect();
        let key: StatePath = "kept".parse().unwrap();
        let deleted: StatePath = "deleted".parse().unwrap();
        let key = norito::json::to_value(&norito::codec::Encode::encode(&key)).unwrap();
        let deleted = norito::json::to_value(&norito::codec::Encode::encode(&deleted)).unwrap();
        assert!(!rows.iter().any(|row| row["key_payload"] == deleted));
        assert_eq!(
            rows.iter().find(|row| row["key_payload"] == key).unwrap()["value_hash_hex"]
                .as_str()
                .unwrap(),
            hex::encode(Sha256::digest(norito::json::to_vec(&vec![value]).unwrap())),
        );
    }

    #[test]
    fn startup_replay_persists_one_complete_projection_and_resumes_incremental_work() {
        let (state, directory) = fixture();
        state
            .tiered_backend
            .lock()
            .record_world_snapshot(&state.world)
            .unwrap();
        assert_eq!(manifest_count(directory.path()), 1);
        let replay = state.defer_tiered_snapshots_for_startup_replay().unwrap();
        assert!(state.defer_tiered_snapshots_for_startup_replay().is_err());
        for value in 3..20 {
            mutate(&state, value);
        }
        assert_eq!(manifest_count(directory.path()), 1);
        replay.finish().unwrap();
        assert_eq!(manifest_count(directory.path()), 2);
        assert_final_values(&state, 19);
        mutate(&state, 20);
        assert_eq!(manifest_count(directory.path()), 3);
        assert_final_values(&state, 20);
    }

    #[test]
    fn abandoned_startup_replay_requires_a_complete_next_capture() {
        let (state, directory) = fixture();
        state
            .tiered_backend
            .lock()
            .record_world_snapshot(&state.world)
            .unwrap();
        let replay = state.defer_tiered_snapshots_for_startup_replay().unwrap();
        mutate(&state, 3);
        drop(replay);
        assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
        assert_eq!(manifest_count(directory.path()), 1);
        // This later overlay never touches the deleted row. Only a complete
        // capture can remove that stale row from the previous persisted image.
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert("later".parse().unwrap(), vec![4]);
        let prepared = PreparedTieredSnapshot::prepare(&world, &state.tiered_snapshot_worker);
        world.commit();
        prepared.publish(&state);
        assert_eq!(manifest_count(directory.path()), 2);
        assert_final_values(&state, 3);
    }

    #[test]
    fn failed_startup_projection_releases_scope_and_keeps_baseline_invalid() {
        let (state, directory) = fixture();
        let replay = state.defer_tiered_snapshots_for_startup_replay().unwrap();
        mutate(&state, 9);
        // A colliding staging entry refuses local projection publication; it
        // cannot change the already executed World or leave replay suppression on.
        let collision = directory.path().join("00000000000000000001.staging");
        std::fs::write(&collision, b"occupied").unwrap();
        assert!(replay.finish().is_err());
        assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
        assert!(
            !state
                .tiered_snapshot_worker
                .inner
                .startup_replay_active
                .load(Ordering::Acquire)
        );
        assert_eq!(
            state.world.smart_contract_state.view().get("kept"),
            Some(&vec![9])
        );
        std::fs::remove_file(collision).unwrap();
        mutate(&state, 10);
        assert_final_values(&state, 10);
    }
}

impl Drop for StartupTieredReplay {
    fn drop(&mut self) {
        // Entry invalidates the baseline; a failed/abandoned startup therefore
        // forces a complete capture on the next normal publication. Only a
        // successful complete finish can make incremental snapshots ready again.
        self.state
            .tiered_snapshot_worker
            .inner
            .startup_replay_active
            .store(false, Ordering::Release);
    }
}
