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
        let foreign = Arc::new(State::new_for_testing(
            crate::state::World::default(),
            Arc::clone(&state.kura),
            crate::query::store::LiveQueryStore::start_test(),
        ));
        let foreign_service = V2ApplyService::new(
            Arc::clone(&foreign),
            phase_queue(),
            Arc::clone(&state.kura),
            None,
            None,
            foreign.sumeragi_block_cadence(),
            iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            events,
            Vec::new(),
        );
        let foreign_source = foreign_service.carrier_queue_source();
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
        let source = service.carrier_queue_source();
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
