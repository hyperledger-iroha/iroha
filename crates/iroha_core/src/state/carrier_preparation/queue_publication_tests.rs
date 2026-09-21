//! Signed retirement decisions retain their original Queue through State visibility.

use super::*;
use crate::sumeragi::v2_apply::V2ApplyService;

struct PublishedAtRelease {
    state: Arc<State>,
    observed: Mutex<Vec<(usize, u64)>>,
}

impl Wake for PublishedAtRelease {
    fn wake(self: Arc<Self>) {
        self.observed.lock().unwrap().push((
            self.state.committed_height(),
            self.state.state_view_generation(),
        ));
    }
}

#[test]
fn signed_retirement_and_replacement_publish_once_under_original_service_queue_cut() {
    for replacement in [false, true] {
        let (boxed, decision, queue) =
            fixture_lifecycle_decision_with_retirement(Some(replacement));
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
            .try_prepare_physical(&state, None, |_, _| Ok::<_, Infallible>(()))
            .err()
            .expect("actual retirement needs the original service Queue");
        assert!(matches!(
            error,
            CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::Missing)
        ));
        assert_eq!(state.state_view_generation(), generation);
        assert_eq!(decision.block().encode_wire().unwrap(), wire);
        let foreign = Arc::new(State::new_with_chain_and_network_id_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            state.chain_id_ref().clone(),
            *state.network_id_ref(),
        ));
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
            .try_prepare_physical(
                &state,
                Some(&foreign_source),
                |_, _| Ok::<_, Infallible>(()),
            )
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
            .try_prepare_physical(&state, Some(&source), |_, _| Ok::<_, Infallible>(()))
            .err()
            .expect("the actual original Queue owner must defer publication");
        let CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::Busy {
            field,
            wait,
        }) = error
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
            .try_prepare_physical(&state, Some(&source), |_, _| Ok::<_, Infallible>(()))
            .unwrap_or_else(|(_, error)| {
                panic!("original service physical acquisition: {error:?}")
            });
        let mut release = queue
            .try_lock_lane_retirement_observer()
            .err()
            .expect("original cut held")
            .wait_for_release();
        let observed = Arc::new(PublishedAtRelease {
            state: Arc::clone(&state),
            observed: Mutex::new(Vec::new()),
        });
        let waker = Waker::from(Arc::clone(&observed));
        assert!(
            Pin::new(&mut release)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let published = physical.publish().unwrap_or_else(|(_, error)| {
            panic!("signed terminal retirement publication: {error:?}")
        });
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
}
