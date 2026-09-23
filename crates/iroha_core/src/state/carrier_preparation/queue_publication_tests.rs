//! Signed retirement decisions retain their original Queue through State visibility.

use super::*;
use crate::sumeragi::v2_apply::V2ApplyService;

struct PublishedAtRelease {
    state: Arc<State>,
    queue: Arc<Queue>,
    observed: Mutex<Vec<(usize, u64)>>,
}

impl Wake for PublishedAtRelease {
    fn wake(self: Arc<Self>) {
        // These acquire actual original owners, so an early release callback
        // cannot pass merely because the committed counters were updated.
        let commit = self
            .state
            .state_commit_lock
            .try_lock_or_wait()
            .expect("retry callback follows commit unlock");
        let write = self
            .state
            .state_write_lock
            .try_lock_or_wait()
            .expect("retry callback follows State writer unlock");
        let kura = self
            .state
            .kura
            .try_publication_lease()
            .expect("retry callback follows every Kura fence unlock");
        let queue = self
            .queue
            .try_lock_lane_retirement_observer()
            .expect("retry callback follows Queue observer unlock")
            .try_into_cut()
            .expect("retry callback follows every Queue fence unlock");
        drop(queue);
        drop(kura);
        drop(write);
        drop(commit);
        self.observed.lock().unwrap().push((
            self.state.committed_height(),
            self.state.state_view_generation(),
        ));
    }
}

// Keep the foreign constructor's large State/World scratch off the caller's
// frame while it constructs the independent, authenticated lifecycle fixture.
#[inline(never)]
fn foreign_service_state(state: &State) -> Arc<State> {
    Arc::new(State::new_with_chain_and_network_id_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        state.chain_id_ref().clone(),
        *state.network_id_ref(),
    ))
}

#[test]
fn signed_retirement_and_replacement_publish_once_under_original_service_queue_cut() {
    for replacement in [false, true] {
        let fixture = fixture_lifecycle_decision_with_retirement(Some(replacement));
        assert_signed_retirement_publication(replacement, fixture);
    }
}

// Construction and publication use independent scratch frames. Their original
// State, decision and Queue move intact between phases on the default test stack.
#[inline(never)]
fn assert_signed_retirement_publication(
    replacement: bool,
    (boxed, decision, queue): (Box<State>, CheckpointDecision<()>, Arc<Queue>),
) {
    let state: Arc<State> = boxed.into();
    let (events, _) = tokio::sync::broadcast::channel(8);
    let service = V2ApplyService::new(
        Arc::clone(&state),
        Arc::clone(&queue),
        Arc::clone(&state.kura),
        None,
        None,
        state.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events.clone(),
        Vec::new(),
    );
    let header = decision.block().header();
    let checkpoint = decision.journals.checkpoint;
    let wire = decision.block().encode_wire().unwrap();
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    assert_eq!(state.committed_height(), 1);
    let (decision, error) = decision
        .try_prepare_physical(&state, None)
        .err()
        .expect("actual retirement needs the original service Queue");
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::Missing)
    ));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(decision.block().encode_wire().unwrap(), wire);
    let foreign = foreign_service_state(&state);
    assert_eq!(foreign.chain_id, state.chain_id);
    assert_eq!(foreign.network_id, state.network_id);
    assert!(!Arc::ptr_eq(&foreign.kura, &state.kura));
    let foreign_service = V2ApplyService::new(
        Arc::clone(&foreign),
        phase_queue(),
        Arc::clone(&foreign.kura),
        None,
        None,
        foreign.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        Vec::new(),
    );
    let foreign_source = foreign_service.carrier_queue_source();
    assert!(foreign_source.belongs_to(&foreign));
    assert!(!foreign_source.belongs_to(&state));
    let (decision, error) = decision
        .try_prepare_physical(&state, Some(&foreign_source))
        .err()
        .expect("another service State cannot substitute custody");
    assert!(matches!(
        error,
        CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::ForeignState)
    ));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.state_view_generation(), generation);
    let source = service.carrier_queue_source();
    let held = queue.try_lock_lane_retirement_observer().unwrap();
    let (decision, error) = decision
        .try_prepare_physical(&state, Some(&source))
        .err()
        .expect("the actual original Queue owner must defer publication");
    let CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::Busy { field, wait }) =
        error
    else {
        panic!("expected original Queue physical contention: {error:?}");
    };
    assert_eq!(field, "lane_reservation_transition_lock");
    assert_eq!(decision.block().encode_wire().unwrap(), wire);
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert!(state.state_commit_lock.try_lock_or_wait().is_ok());
    assert!(state.kura.try_publication_lease().is_ok());
    let mut retry = wait.wait_for_release();
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut retry, &wakes).is_pending());
    drop(held);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut retry, &wakes).is_ready());
    let physical = decision
        .try_prepare_physical(&state, Some(&source))
        .unwrap_or_else(|(_, error)| panic!("original service physical acquisition: {error:?}"));
    let mut release = queue
        .try_lock_lane_retirement_observer()
        .err()
        .expect("original cut held")
        .wait_for_release();
    let observed = Arc::new(PublishedAtRelease {
        state: Arc::clone(&state),
        queue: Arc::clone(&queue),
        observed: Mutex::new(Vec::new()),
    });
    let waker = Waker::from(Arc::clone(&observed));
    assert!(
        Pin::new(&mut release)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let published = physical
        .publish()
        .unwrap_or_else(|(_, error)| panic!("signed terminal retirement publication: {error:?}"));
    assert_eq!(published.block().header(), header);
    assert_eq!(state.committed_height(), 2);
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(
        *observed.observed.lock().unwrap(),
        vec![(2, generation + 2)]
    );
    assert!(
        Pin::new(&mut release)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
    assert_ne!(checkpoint, before);
    let lanes = state.nexus_snapshot().lane_catalog;
    let lane = lanes
        .lanes()
        .iter()
        .find(|lane| lane.id == iroha_model_base::topology::LaneId::new(1));
    if replacement {
        assert_eq!(lane.unwrap().alias, "published-lifecycle");
    } else {
        assert!(lane.is_none());
    }
    drop(published);
    assert_eq!(state.state_view_generation(), generation + 2);
    drop(queue.try_lock_lane_retirement_observer().unwrap());
    assert!(state.state_commit_lock.try_lock_or_wait().is_ok());
}

#[test]
fn state_fence_refusal_defers_callbacks_through_original_queue_and_kura() {
    struct Reenter {
        state: Arc<State>,
        queue: Arc<Queue>,
        blocked: &'static str,
        wakes: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert_fences_free_except(&self.state, self.blocked);
            assert!(self.state.kura.try_publication_lease().is_ok());
            assert!(
                self.queue
                    .try_lock_lane_retirement_observer()
                    .unwrap()
                    .try_into_cut()
                    .is_ok()
            );
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (boxed, mut decision, queue) = fixture_lifecycle_decision_with_retirement(Some(false));
    let state: Arc<State> = boxed.into();
    let (events, _) = tokio::sync::broadcast::channel(8);
    let service = V2ApplyService::new(
        Arc::clone(&state),
        Arc::clone(&queue),
        Arc::clone(&state.kura),
        None,
        None,
        state.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        Vec::new(),
    );
    let source = service.carrier_queue_source();
    let wire = decision.block().encode_wire().unwrap();
    let generation = state.state_view_generation();
    for blocked in ["lane_lifecycle_lock", "state_write_lock"] {
        // Retain the first actual notification, allowing the next acquisition's
        // release to wake an already registered observer of this same mutex.
        let first = state.state_commit_lock.lock();
        let mut wait = state
            .state_commit_lock
            .try_lock_or_wait()
            .err()
            .unwrap()
            .wait_for_release();
        let first_retirement = first.release_deferred();
        let callback = Arc::new(Reenter {
            state: Arc::clone(&state),
            queue: Arc::clone(&queue),
            blocked,
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&callback));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let held = hold(&state, blocked);
        let (retry, error) = decision
            .try_prepare_physical(&state, Some(&source))
            .err()
            .expect("exact State owner is busy");
        let CarrierPhysicalPreparationError::Fence {
            field,
            wait: actual_wait,
        } = error
        else {
            panic!("expected State refusal")
        };
        assert_eq!(field, blocked);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(retry.block().encode_wire().unwrap(), wire);
        assert_eq!(state.state_view_generation(), generation);
        drop(held);
        assert!(
            Pin::new(&mut actual_wait.wait_for_release())
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        drop(wait);
        drop(first_retirement);
        decision = retry;
    }
    drop(
        decision
            .try_prepare_physical(&state, Some(&source))
            .unwrap_or_else(|(_, error)| panic!("same original retry: {error:?}")),
    );
}
