//! Executing-State transitive validation retains actual history reader notices.

use super::*;
use std::{
    future::Future,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

struct ReaderProbe {
    state: Arc<State>,
    calls: AtomicUsize,
    blocked: AtomicBool,
}
impl Wake for ReaderProbe {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !self.state.transactions.reader_test_writer_available()
            || self.state.state_commit_lock.try_lock().is_none()
            || self.state.lane_lifecycle_lock.try_lock().is_none()
        {
            self.blocked.store(true, Ordering::SeqCst);
        }
    }
}

state_test! { consensus_stack transitive_certified_merge_reads_retire_after_executing_state
    for unwind in [false, true] {
        let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
        let state = Arc::new(state);
        let probe = Arc::new(ReaderProbe {
            state: Arc::clone(&state), calls: AtomicUsize::new(0), blocked: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut block = state.merge_preexecution_block(carrier.header().clone());
            let waits = [
                state.block_hashes.map().unwrap().observe_reader_release(),
                state.transactions.reader_release_wait_for_tests(),
            ];
            pending = Some(waits.map(|wait| Box::pin(wait.wait_for_release())));
            for future in pending.as_mut().unwrap() {
                assert!(future.as_mut().poll(&mut context).is_pending());
            }
            block.stage_certified_merge_entry(&entry, ConsensusMode::Permissioned)
                .expect("accepted complete autonomous merge stages through actual validation");
            assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
            assert!(!state.transactions.reader_test_writer_available());
            if unwind { panic!("unwind after actual transitive validation"); }
            drop(block);
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
        assert!(!probe.blocked.load(Ordering::SeqCst));
        for future in pending.as_mut().unwrap() {
            assert!(future.as_mut().poll(&mut context).is_ready());
        }
    }
}

state_test! { consensus_stack transitive_catalog_refusal_retains_original_readers_until_state_drop
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let state = Arc::new(state);
    let probe = Arc::new(ReaderProbe {
        state: Arc::clone(&state), calls: AtomicUsize::new(0), blocked: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&probe));
    let mut context = Context::from_waker(&waker);
    let mut block = state.merge_preexecution_block(carrier.header().clone());
    let waits = [
        state.block_hashes.map().unwrap().observe_reader_release(),
        state.transactions.reader_release_wait_for_tests(),
    ];
    let mut pending = waits.map(|wait| Box::pin(wait.wait_for_release()));
    for future in &mut pending { assert!(future.as_mut().poll(&mut context).is_pending()); }
    let wrong_catalog = Hash::new(b"different original lane catalog");
    assert_ne!(wrong_catalog, entry.lane_catalog_hash);
    let error = state.validate_merge_lane_authority_catalog_live_with_releases(
        wrong_catalog,
        &entry.active_lanes,
        &entry.lane_authority_catalog,
        entry.merge_qc.carrier_height,
        &mut block.read_releases.lifecycle,
    ).expect_err("exact authority comparison must reject after the actual full view");
    assert!(matches!(error, MergeLedgerCommitError::CatalogMismatch));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert!(!state.transactions.reader_test_writer_available());
    drop(block);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    assert!(!probe.blocked.load(Ordering::SeqCst));
    for future in &mut pending { assert!(future.as_mut().poll(&mut context).is_ready()); }
}
