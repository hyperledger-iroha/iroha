//! Original relay snapshots must not retain store guards while opening World readers.

use super::*;
use mv::BlockPublication;
use std::{cell::RefCell, sync::mpsc};

thread_local! {
    /// Bounded test observation immediately before the actual validation call.
    static VALIDATION_OBSERVER: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

/// Observe the next real candidate validation on this test thread only.
pub(in crate::state) fn before_validation() {
    let observer = VALIDATION_OBSERVER.with(|slot| slot.borrow_mut().take());
    if let Some(observer) = observer {
        observer();
    }
}

#[test]
fn contiguous_relay_snapshot_retains_original_candidates_after_pruning() {
    let (state, validators) = setup_lane_relay_burn_state();
    ensure_merge_carrier_parent_for_test(&state);
    let first = sample_lane_relay_envelope_for_state(&state, 1, LaneId::SINGLE, &validators);
    let second = sample_lane_relay_envelope_for_state(&state, 2, LaneId::SINGLE, &validators);
    let mut store = LaneRelayStore::default();
    store.insert(second.clone()).unwrap();
    assert!(
        store
            .next_relays_with_merge_material(&BTreeMap::new())
            .is_empty()
    );
    store.insert(first.clone()).unwrap();
    let original = store.next_relays_with_merge_material(&BTreeMap::new());
    assert_eq!(original, vec![first.clone()]);
    let previous = BTreeMap::from([(
        (first.lane_id, first.dataspace_id, first.lane_incarnation),
        MergeLaneSnapshot {
            lane_id: first.lane_id,
            lane_incarnation: first.lane_incarnation,
            incarnation_activation_height: 1,
            proposal_height: first.block_header.height().get(),
            dataspace_id: first.dataspace_id,
            lane_block_height: first.block_height,
            tip_hash: first.block_header.hash(),
            merge_hint_root: first.merge_hint_root().unwrap(),
            settlement_commitment: first.settlement_commitment.clone(),
            settlement_hash: first.settlement_hash,
            relay_envelope: Some(first.clone()),
        },
    )]);
    let successor = store.next_relays_with_merge_material(&previous);
    assert_eq!(successor, vec![second]);
    store.prune_lanes(&BTreeSet::from([LaneId::SINGLE]));
    assert!(store.snapshot().is_empty());
    assert_eq!(original, vec![first]);
    assert_eq!(successor[0].block_height, 2);
}

#[test]
fn relay_candidate_validation_releases_store_before_waiting_for_world_reader() {
    let (state, validators) = setup_lane_relay_burn_state();
    ensure_merge_carrier_parent_for_test(&state);
    let envelope = sample_lane_relay_envelope_for_state(&state, 1, LaneId::SINGLE, &validators);
    seed_verified_lane_relay_record(&state, &envelope);
    let state = Arc::new(state);
    let (at_validation_tx, at_validation_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let worker_state = Arc::clone(&state);
    let worker = std::thread::spawn(move || {
        VALIDATION_OBSERVER.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                let _ = at_validation_tx.send(());
                // A timeout cannot strand the worker or manufacture readiness.
                let _ = resume_rx.recv_timeout(Duration::from_secs(10));
            }));
        });
        let candidates = worker_state.merge_entry_candidates_from_lane_relays();
        let _ = done_tx.send(candidates);
        VALIDATION_OBSERVER.with(|slot| drop(slot.borrow_mut().take()));
    });
    let at_validation = at_validation_rx.recv_timeout(Duration::from_secs(10));
    // The production builder has retained its exact relay candidates. Hold the
    // actual smart-contract map's active-reader mutex, which the real verifier
    // next needs through World::view; this is not a synthetic readiness gate.
    let mut blocker_dirty = false;
    let mut world_reader = at_validation.as_ref().ok().map(|()| {
        let mut block = state.world.smart_contract_state.block();
        // A clean block prepares only undo; make the private current map dirty
        // so this actual publication slot also retains its active-reader lock.
        block.insert(
            "relay_reader_blocker_private"
                .parse()
                .expect("private state key"),
            Vec::new(),
        );
        blocker_dirty = block.is_dirty();
        let mut original = block.publication_slot();
        original.prepare_publication();
        original
    });
    let _ = resume_tx.send(());
    let while_held = done_rx.recv_timeout(Duration::from_millis(50));
    let mut relay_release = None;
    let relay_writer_free = if let Some(mut writer) = state.lane_relays.try_write() {
        writer.prune_lanes(&BTreeSet::from([LaneId::SINGLE]));
        relay_release = Some(writer.release_deferred());
        true
    } else {
        false
    };
    // Release the original blocker and join on every observation outcome before
    // asserting, including a regression where the old relay reader is retained.
    drop(world_reader.take());
    let (blocked_on_world, result) = match while_held {
        Ok(result) => (false, Ok(result)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            (true, done_rx.recv_timeout(Duration::from_secs(10)))
        }
        Err(error) => (false, Err(error)),
    };
    let joined = worker.join();
    drop(relay_release);
    assert!(
        at_validation.is_ok(),
        "actual candidate validation was reached"
    );
    assert!(joined.is_ok(), "candidate worker completed without panic");
    assert!(
        blocker_dirty,
        "the blocker prepares the original current map"
    );
    assert!(
        blocked_on_world,
        "actual World reader acquisition must wait"
    );
    assert!(
        relay_writer_free,
        "validation must release its original relay reader"
    );
    let candidates = result.expect("candidate validation resumes after actual reader release");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].lane_snapshots.len(), 1);
    assert_eq!(
        candidates[0].lane_snapshots[0].relay_envelope.as_ref(),
        Some(&envelope)
    );
    assert!(state.lane_relays.read().snapshot().is_empty());
}
