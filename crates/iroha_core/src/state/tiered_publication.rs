//! Immutable tiered persistence payload captured from the completed World overlay.
//! A cold backend captures every tiered row; an initialized backend captures the
//! exact changed values and deletions. Publication never substitutes live World
//! values. Persistence failures remain local and cannot undo decided State.

use super::*;

pub(super) struct PreparedTieredSnapshot {
    payload: Option<TieredSnapshotPayload>,
    // On abandonment, observe release after the stored payload is destroyed.
    #[cfg(test)]
    _capture: Option<capture_observer::Capture>,
}

impl PreparedTieredSnapshot {
    pub(super) fn prepare(world: &WorldBlock<'_>, worker: &TieredSnapshotWorker) -> Self {
        let complete = {
            let backend = worker.inner.backend.lock();
            if !backend.enabled() {
                return Self {
                    payload: None,
                    #[cfg(test)]
                    _capture: None,
                };
            }
            !backend.snapshot_baseline_ready()
        };
        // Carrier journal admission precedes this allocation. TODO: connect its
        // complete cold-baseline charge to the production candidate budget.
        let payload = if complete {
            world.tiered_snapshot_payload_with_scope(true)
        } else {
            world.tiered_snapshot_payload()
        };
        Self {
            payload: Some(payload),
            #[cfg(test)]
            _capture: capture_observer::captured(),
        }
    }

    #[cfg(test)]
    pub(in crate::state) fn payload_for_test(&self) -> Option<&TieredSnapshotPayload> {
        self.payload.as_ref()
    }

    pub(super) fn publish(self, state_ref: &State, replay_prevalidation: bool) {
        if replay_prevalidation {
            return;
        }
        let Some(payload) = self.payload else {
            return;
        };
        // Scheduling may wait for one bounded slot. The worker consumes only
        // owned payloads and the backend lock, never any State or World lock.
        if let Err(payload) = state_ref.tiered_snapshot_worker.schedule(payload) {
            let mut backend = state_ref.tiered_backend.lock();
            if let Err(err) = backend.record_world_snapshot_with_payload(&payload) {
                warn!(?err, "tiered-state: failed to record retained snapshot");
            }
            #[cfg(feature = "telemetry")]
            record_tiered_snapshot_metrics(&backend, &state_ref.telemetry);
        }
    }
}

/// Thread-scoped observation of capture custody, including abandonment order.
/// This does not track a payload transferred into the persistence worker.
#[cfg(test)]
pub(in crate::state) mod capture_observer {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    std::thread_local! {
        static OBSERVER: std::cell::RefCell<Option<Arc<Counts>>> = const {
            std::cell::RefCell::new(None)
        };
    }

    #[derive(Default)]
    pub(in crate::state) struct Counts {
        captured: AtomicUsize,
        released: AtomicUsize,
    }

    impl Counts {
        pub(in crate::state) fn captured(&self) -> usize {
            self.captured.load(Ordering::SeqCst)
        }

        pub(in crate::state) fn released(&self) -> usize {
            self.released.load(Ordering::SeqCst)
        }
    }

    pub(super) struct Capture(Arc<Counts>);

    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.released.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub(super) fn captured() -> Option<Capture> {
        OBSERVER.with(|observer| {
            observer.borrow().as_ref().map(|counts| {
                counts.captured.fetch_add(1, Ordering::SeqCst);
                Capture(Arc::clone(counts))
            })
        })
    }

    pub(in crate::state) fn observe<T>(action: impl FnOnce(Arc<Counts>) -> T) -> T {
        struct Restore(Option<Arc<Counts>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                OBSERVER.with(|observer| observer.replace(self.0.take()));
            }
        }
        let counts = Arc::new(Counts::default());
        let _restore =
            Restore(OBSERVER.with(|observer| observer.replace(Some(Arc::clone(&counts)))));
        action(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::store::LiveQueryStore;
    use sha2::{Digest as _, Sha256};

    fn manifest_value_hash(value: &[u8]) -> String {
        hex::encode(Sha256::digest(
            norito::json::to_vec(&value.to_vec()).unwrap(),
        ))
    }

    #[test]
    fn synchronous_cold_and_incremental_publication_use_only_retained_values() {
        let mut world = World::new();
        let a: StatePath = "retained-a".parse().unwrap();
        let b: StatePath = "retained-b".parse().unwrap();
        let later: StatePath = "later-only".parse().unwrap();
        world.smart_contract_state.insert(a.clone(), vec![1_u8]);
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let temp = tempfile::tempdir().unwrap();
        *state.tiered_backend.lock() =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        state.tiered_snapshot_worker = TieredSnapshotWorker::inert(
            Arc::clone(&state.tiered_backend),
            #[cfg(feature = "telemetry")]
            None,
        );
        let mut block = state.world.block();
        block.smart_contract_state.insert(b.clone(), vec![2_u8]);
        let cold = PreparedTieredSnapshot::prepare(&block, &state.tiered_snapshot_worker);
        block.commit();
        // Count the complete captured World through the independent ordinary snapshot path;
        // State construction also seeds canonical default rows outside this fixture's two keys.
        let reference_dir = tempfile::tempdir().unwrap();
        let mut reference = TieredStateBackend::new(
            true,
            0,
            0,
            0,
            Some(reference_dir.path().to_path_buf()),
            None,
            0,
            0,
        );
        reference.record_world_snapshot(&state.world).unwrap();
        let captured_entry_count = reference.last_manifest().unwrap().total_entries;
        assert!(captured_entry_count >= 2);
        // This is a persistence projection test, not carrier finality. Advance
        // the live World after capture to detect any fallback value substitution.
        let mut block = state.world.block();
        block.smart_contract_state.insert(a.clone(), vec![91_u8]);
        block.smart_contract_state.insert(b.clone(), vec![92_u8]);
        block.smart_contract_state.insert(later, vec![93_u8]);
        block.commit();
        cold.publish(&state, false);
        {
            let backend = state.tiered_backend.lock();
            assert!(backend.snapshot_baseline_ready());
            let manifest = backend.last_manifest().unwrap();
            assert_eq!(
                manifest.total_entries, captured_entry_count,
                "cold capture includes the unchanged baseline, not later rows"
            );
            let encoded = norito::json::to_json(manifest).unwrap();
            assert!(encoded.contains(&manifest_value_hash(&[1])));
            assert!(encoded.contains(&manifest_value_hash(&[2])));
            assert!(!encoded.contains(&manifest_value_hash(&[91])));
            assert!(!encoded.contains(&manifest_value_hash(&[92])));
            assert!(!encoded.contains(&manifest_value_hash(&[93])));
        }
        let mut block = state.world.block();
        block.smart_contract_state.insert(b.clone(), vec![3_u8]);
        let incremental = PreparedTieredSnapshot::prepare(&block, &state.tiered_snapshot_worker);
        block.commit();
        let mut block = state.world.block();
        block.smart_contract_state.insert(b, vec![94_u8]);
        block.commit();
        incremental.publish(&state, false);
        let backend = state.tiered_backend.lock();
        let manifest = backend.last_manifest().unwrap();
        assert_eq!(manifest.total_entries, captured_entry_count);
        let encoded = norito::json::to_json(manifest).unwrap();
        assert!(encoded.contains(&manifest_value_hash(&[1])));
        assert!(encoded.contains(&manifest_value_hash(&[3])));
        assert!(!encoded.contains(&manifest_value_hash(&[94])));
        assert!(!encoded.contains(&manifest_value_hash(&[93])));
    }

    #[test]
    fn preparation_retains_exact_changed_keys_without_publishing_world() {
        let mut state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let temp = tempfile::tempdir().unwrap();
        *state.tiered_backend.lock() =
            TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 0, 0);
        state.tiered_snapshot_worker = TieredSnapshotWorker::inert(
            Arc::clone(&state.tiered_backend),
            #[cfg(feature = "telemetry")]
            None,
        );
        let mut world = state.world.block();
        let key: StatePath = "prepared-tiered".parse().unwrap();
        world
            .smart_contract_state
            .insert(key.clone(), vec![4, 5, 6]);
        let prepared = PreparedTieredSnapshot::prepare(&world, &state.tiered_snapshot_worker);
        assert!(
            prepared
                .payload
                .as_ref()
                .is_some_and(|payload| !payload.is_empty())
        );
        assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
        drop(world);
        assert!(state.world.smart_contract_state.view().get(&key).is_none());
        // Disposable replay neither schedules nor writes a persistence artifact.
        prepared.publish(&state, true);
        assert!(!state.tiered_backend.lock().snapshot_baseline_ready());
        assert!(state.world.smart_contract_state.view().get(&key).is_none());
    }
}
