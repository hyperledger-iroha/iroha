//! Actual carrier capture must retain World notifications through State writers.

use super::*;
use mv::{PublicationCleanup, PublicationPreparationError};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct TopologyProbe {
    journal: Option<mv::cell::Detached<Vec<PeerId>, ()>>,
    cleanup: Option<PublicationCleanup<()>>,
}

struct ProbeTopologyOnWorldRelease {
    state: Arc<State>,
    original: Mutex<TopologyProbe>,
    calls: AtomicUsize,
    admitted: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    other: AtomicUsize,
    unavailable: AtomicUsize,
}

impl Wake for ProbeTopologyOnWorldRelease {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Ok(mut original) = self.original.try_lock() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        if original.cleanup.is_some() {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let Some(journal) = original.journal.take() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        // Reaching admission proves the predecessor identity was checked; the
        // next Busy is the actual native writer, not a fabricated observation.
        match journal.try_prepare_publication(&self.state.commit_topology, |_, _| {
            self.admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        }) {
            Ok(prepared) => {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                let (journal, cleanup) = prepared.abort();
                original.journal = Some(journal);
                original.cleanup = Some(cleanup);
            }
            Err((journal, error, cleanup)) => {
                if matches!(error, PublicationPreparationError::Busy(_)) {
                    self.busy.fetch_add(1, Ordering::SeqCst);
                } else {
                    self.other.fetch_add(1, Ordering::SeqCst);
                }
                original.journal = Some(journal);
                original.cleanup = Some(cleanup);
            }
        }
    }
}

#[test]
fn carrier_capture_unlocks_state_topology_before_world_parameters_notification() {
    let (state, proposal, topology, context) = crate::state::carrier_preparation::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let parameters_probe = state
        .world
        .parameters
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let topology_probe = state
        .commit_topology
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    // Execute the real authenticated candidate once. No synthetic StateBlock,
    // scalar handoff, or reconstruction stands in for the retained execution.
    let prepared = crate::state::carrier_preparation::tests::prepare(
        &state,
        proposal.clone(),
        &topology,
        &context,
    )
    .unwrap_or_else(|(_, error)| panic!("prepare original carrier: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(prepared.state());
    let physical_admissions = AtomicUsize::new(0);
    let (parameters_probe, error, refused) = parameters_probe
        .try_prepare_publication(&state.world.parameters, |_, _| {
            physical_admissions.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        })
        .err()
        .expect("the original carrier holds the real World parameters writers");
    assert_eq!(physical_admissions.load(Ordering::SeqCst), 1);
    let PublicationPreparationError::Busy(observation) = error else {
        panic!("expected original physical parameters contention");
    };
    assert!(!observation.is_poisoned());
    let callback = Arc::new(ProbeTopologyOnWorldRelease {
        state: Arc::clone(&state),
        original: Mutex::new(TopologyProbe {
            journal: Some(topology_probe),
            cleanup: None,
        }),
        calls: AtomicUsize::new(0),
        admitted: AtomicUsize::new(0),
        acquired: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
        other: AtomicUsize::new(0),
        unavailable: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut poll = Context::from_waker(&waker);
    let mut released = observation.clone().wait_for_release();
    assert!(Pin::new(&mut released).poll(&mut poll).is_pending());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 0);

    let admission_calls = AtomicUsize::new(0);
    let captured = prepared
        .prepare_journals(None, None, |inputs| {
            admission_calls.fetch_add(1, Ordering::SeqCst);
            admit_journals_for_test(inputs)
        })
        .unwrap_or_else(|error| panic!("capture original carrier: {error}"));
    // Keep all returned journals alive until after every physical probe and
    // identity/image assertion. Probe cleanup is never destroyed inside Wake.
    drop(refused);
    let (topology_probe, cleanup) = {
        let mut original = callback.original.lock().unwrap();
        (original.journal.take().unwrap(), original.cleanup.take())
    };
    drop(cleanup);
    assert!(Pin::new(&mut released).poll(&mut poll).is_ready());
    assert!(!observation.is_poisoned());
    assert_eq!(admission_calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.admitted.load(Ordering::SeqCst), 1);
    assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
    assert_eq!(callback.other.load(Ordering::SeqCst), 0);
    assert_eq!(callback.unavailable.load(Ordering::SeqCst), 0);
    assert_eq!(callback.acquired.load(Ordering::SeqCst), 1);
    assert!(parameters_probe.matches_current(&state.world.parameters));
    assert!(topology_probe.matches_current(&state.commit_topology));
    assert!(captured.components.world.matches_current(&state.world));
    assert!(captured.components.runtime.matches_current(&state));
    assert_eq!(captured.components.world.mode(), mv::BlockMode::Ordinary);
    assert_eq!(captured.execution_prefix_commitment(), prefix);
    assert_eq!(captured.checkpoint, checkpoint);
    assert_eq!(captured.valid.as_ref().hash(), proposal.hash());
    assert_eq!(*captured.context, context);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop((captured, parameters_probe, topology_probe));
}

// Membership uses a parking_lot writer: an actual Cell unwind can poison its
// physical mutex while this original membership writer remains healthy.
struct MembershipProbe<Cleanup> {
    journal: Option<storage_transactions::DetachedTransactionsBlock>,
    cleanup: Option<Cleanup>,
}

enum MembershipProbeResult {
    Acquired,
    Busy,
    Other,
}

struct ProbeMembershipOnWorldRelease<Cleanup, Inspect> {
    state: Arc<State>,
    original: Mutex<MembershipProbe<Cleanup>>,
    inspect: Inspect,
    calls: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    other: AtomicUsize,
    unavailable: AtomicUsize,
}

trait MembershipProbeWitness: Wake {
    fn observations(&self) -> [usize; 5];
    // Called only outside Wake. The opaque native abort cleanup is deliberately
    // retained in the callback owner, then destroyed after its mutex is free.
    fn take_original(&self) -> storage_transactions::DetachedTransactionsBlock;
}

impl<Cleanup, Inspect> Wake for ProbeMembershipOnWorldRelease<Cleanup, Inspect>
where
    Cleanup: Send + 'static,
    Inspect: Fn(
            storage_transactions::DetachedTransactionsBlock,
            &State,
        ) -> (
            storage_transactions::DetachedTransactionsBlock,
            Option<Cleanup>,
            MembershipProbeResult,
        ) + Send
        + Sync
        + 'static,
{
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Ok(mut original) = self.original.try_lock() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        if original.cleanup.is_some() {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let Some(journal) = original.journal.take() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        let (journal, cleanup, result) = (self.inspect)(journal, &self.state);
        original.journal = Some(journal);
        original.cleanup = cleanup;
        match result {
            MembershipProbeResult::Acquired => &self.acquired,
            MembershipProbeResult::Busy => &self.busy,
            MembershipProbeResult::Other => &self.other,
        }
        .fetch_add(1, Ordering::SeqCst);
    }
}

impl<Cleanup, Inspect> MembershipProbeWitness for ProbeMembershipOnWorldRelease<Cleanup, Inspect>
where
    Cleanup: Send + 'static,
    Inspect: Fn(
            storage_transactions::DetachedTransactionsBlock,
            &State,
        ) -> (
            storage_transactions::DetachedTransactionsBlock,
            Option<Cleanup>,
            MembershipProbeResult,
        ) + Send
        + Sync
        + 'static,
{
    fn observations(&self) -> [usize; 5] {
        [
            self.calls.load(Ordering::SeqCst),
            self.acquired.load(Ordering::SeqCst),
            self.busy.load(Ordering::SeqCst),
            self.other.load(Ordering::SeqCst),
            self.unavailable.load(Ordering::SeqCst),
        ]
    }

    fn take_original(&self) -> storage_transactions::DetachedTransactionsBlock {
        let (journal, cleanup) = {
            let mut original = self.original.lock().unwrap();
            (original.journal.take().unwrap(), original.cleanup.take())
        };
        drop(cleanup);
        journal
    }
}

fn membership_probe_before_prepare(
    state: &State,
) -> storage_transactions::DetachedTransactionsBlock {
    let mut block = state.transactions.block();
    block.insert_block(
        std::collections::HashSet::new(),
        std::num::NonZeroUsize::new(state.transactions.latest_height() + 1).unwrap(),
    );
    block.prepare_commit().unwrap().detach()
}

fn membership_probe_callback(
    state: &Arc<State>,
    journal: storage_transactions::DetachedTransactionsBlock,
) -> Arc<impl MembershipProbeWitness + use<>> {
    // Infer the private native abort-cleanup type from its actual constructor;
    // no erased Box or test-only visibility expansion is needed.
    let inspect =
        |journal: storage_transactions::DetachedTransactionsBlock, state: &State| match journal
            .try_prepare_publication(&state.transactions, |_, _| Ok::<_, ()>(()))
        {
            Ok(prepared) => {
                let (journal, cleanup) = prepared.abort();
                (journal, Some(cleanup), MembershipProbeResult::Acquired)
            }
            Err((journal, error, cleanup)) => {
                let result = if matches!(error, PublicationPreparationError::Busy(_)) {
                    MembershipProbeResult::Busy
                } else {
                    MembershipProbeResult::Other
                };
                (journal, Some(cleanup), result)
            }
        };
    Arc::new(ProbeMembershipOnWorldRelease {
        state: Arc::clone(state),
        original: Mutex::new(MembershipProbe {
            journal: Some(journal),
            cleanup: None,
        }),
        inspect,
        calls: AtomicUsize::new(0),
        acquired: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
        other: AtomicUsize::new(0),
        unavailable: AtomicUsize::new(0),
    })
}

fn held_cell_observation<V: mv::Value>(
    target: &mv::cell::Cell<V>,
    journal: mv::cell::Detached<V, ()>,
) -> (
    mv::cell::Detached<V, ()>,
    concread::release::ReleaseWait,
    PublicationCleanup<()>,
) {
    let physical_admissions = AtomicUsize::new(0);
    let (journal, error, cleanup) = journal
        .try_prepare_publication(target, |_, _| {
            physical_admissions.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        })
        .err()
        .expect("actual original Cell writer is held");
    assert_eq!(physical_admissions.load(Ordering::SeqCst), 1);
    let PublicationPreparationError::Busy(wait) = error else {
        panic!("expected original physical Cell contention");
    };
    (journal, wait, cleanup)
}

fn held_membership_observation(
    state: &State,
    journal: storage_transactions::DetachedTransactionsBlock,
) -> (
    storage_transactions::DetachedTransactionsBlock,
    concread::release::ReleaseWait,
) {
    let (journal, error, _cleanup) = journal
        .try_prepare_publication(&state.transactions, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("actual original membership writer is held");
    let PublicationPreparationError::Busy(wait) = error else {
        panic!("expected original physical membership contention");
    };
    (journal, wait)
}

#[test]
fn carrier_capture_refused_original_drop_releases_membership_before_world_notification() {
    let (state, proposal, topology, context) = crate::state::carrier_preparation::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let parameters = state
        .world
        .parameters
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let membership = membership_probe_before_prepare(&state);
    let prepared =
        crate::state::carrier_preparation::tests::prepare(&state, proposal, &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("original carrier: {error}"));
    let original_state = std::ptr::from_ref(prepared.state());
    let prefix = prepared.execution_prefix_commitment();
    let (parameters, wait, refused) = held_cell_observation(&state.world.parameters, parameters);
    let (membership, membership_wait) = held_membership_observation(&state, membership);
    let callback = membership_probe_callback(&state, membership);
    let waker = Waker::from(Arc::clone(&callback));
    let mut future = wait.clone().wait_for_release();
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );

    let error = prepared
        .prepare_journals(None, None, |_| Err::<(), _>("original retention refusal"))
        .err()
        .expect("real journal admission refuses");
    let CarrierJournalPreparationError::JournalAdmission {
        carrier,
        provider,
        reputation,
        error: reason,
    } = &error
    else {
        panic!("retain the exact original carrier on normal refusal");
    };
    assert_eq!(*reason, "original retention refusal");
    assert!(provider.is_none() && reputation.is_none());
    assert_eq!(std::ptr::from_ref(carrier.state()), original_state);
    assert_eq!(carrier.execution_prefix_commitment(), prefix);
    assert_eq!(callback.observations(), [0; 5]);
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(error);

    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
    assert_eq!(callback.observations(), [1, 1, 0, 0, 0]);
    assert!(!wait.is_poisoned());
    assert!(!membership_wait.is_poisoned());
    let membership = callback.take_original();
    drop(refused);
    assert_eq!(
        membership.observe_predecessor(&state.transactions),
        storage_transactions::MembershipPredecessorStatus::Current
    );
    assert!(parameters.matches_current(&state.world.parameters));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop((membership, parameters));
}

#[test]
fn carrier_capture_admission_panic_releases_healthy_membership_before_world_notification() {
    let (state, proposal, topology, context) = crate::state::carrier_preparation::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let parameters = state
        .world
        .parameters
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let membership = membership_probe_before_prepare(&state);
    let prepared =
        crate::state::carrier_preparation::tests::prepare(&state, proposal, &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("original carrier: {error}"));
    let (parameters, wait, refused) = held_cell_observation(&state.world.parameters, parameters);
    let (membership, membership_wait) = held_membership_observation(&state, membership);
    let callback = membership_probe_callback(&state, membership);
    let waker = Waker::from(Arc::clone(&callback));
    let mut future = wait.clone().wait_for_release();
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.prepare_journals(None, None, |_| -> Result<(), &'static str> {
            panic!("injected actual carrier journal admission panic");
        })
    }));
    assert!(result.is_err());
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
    assert_eq!(callback.observations(), [1, 1, 0, 0, 0]);
    // The actual std Cell writer unwinds; the original parking_lot membership
    // writer is healthy. Neither the test nor the implementation masks that.
    assert!(wait.is_poisoned());
    assert!(!membership_wait.is_poisoned());
    let membership = callback.take_original();
    drop(refused);
    assert_eq!(
        membership.observe_predecessor(&state.transactions),
        storage_transactions::MembershipPredecessorStatus::Current
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop((membership, parameters));
}

#[test]
fn state_capture_late_membership_refusal_retains_completed_world_and_runtime_until_joint_drop() {
    let (state, proposal, _topology, _context) =
        crate::state::carrier_preparation::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let parameters = state
        .world
        .parameters
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let membership = membership_probe_before_prepare(&state);
    // One real State acquisition, with its original header and all components
    // from the same owner. This malformed capture fixture deliberately never
    // calls insert_block; it is not an authenticated PreparedCarrier claim.
    let block = state.block(proposal.header());
    assert!(!block.transactions.has_staged_block());
    let (parameters, wait, refused) = held_cell_observation(&state.world.parameters, parameters);
    let (membership, membership_wait) = held_membership_observation(&state, membership);
    let callback = membership_probe_callback(&state, membership);
    let waker = Waker::from(Arc::clone(&callback));
    let mut future = wait.clone().wait_for_release();
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let StateBlockFields {
        world,
        canonical_runtime,
        commit_topology,
        prev_commit_topology,
        lane_consensus_contexts,
        transactions,
        block_hashes,
        ..
    } = block.into_fields();
    let mut pending = StateJournalCapture::new(
        world.capture_slot(),
        runtime_journals::RuntimeCapture::new(
            canonical_runtime.into_executing(),
            commit_topology.into_executing(),
            prev_commit_topology.into_executing(),
            lane_consensus_contexts.into_executing(),
        ),
        transactions.into_capture(),
        block_hashes.into_executing(),
    );
    assert!(matches!(
        pending.try_capture(),
        Err(StateCaptureError::Membership(
            storage_transactions::TransactionsBlockError::MissingInsertBlock
        ))
    ));
    // The actual later validation refused after World/runtime capture. Their
    // original releases must remain pending while the failed membership writer
    // is still in the enclosing caller's slot.
    assert_eq!(callback.observations(), [0; 5]);
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert!(!membership_wait.is_poisoned());
    drop(pending);

    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
    assert_eq!(callback.observations(), [1, 1, 0, 0, 0]);
    assert!(!wait.is_poisoned());
    assert!(!membership_wait.is_poisoned());
    let membership = callback.take_original();
    drop(refused);
    assert_eq!(
        membership.observe_predecessor(&state.transactions),
        storage_transactions::MembershipPredecessorStatus::Current
    );
    assert!(parameters.matches_current(&state.world.parameters));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    drop((membership, parameters));
}
