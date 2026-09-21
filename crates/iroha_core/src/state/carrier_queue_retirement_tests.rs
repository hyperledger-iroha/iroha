//! Exact original Queue identity and independently releasable carrier retirement work.

use super::*;
use crate::{
    queue::Queue,
    state::carrier_preparation::queue_retirement::{
        CarrierQueueRetirement, CarrierQueueRetirementError,
    },
    sumeragi::v2_apply::carrier_queue_retirement::OriginalCarrierQueue,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Wake, Waker},
};

fn fixture(replace: bool) -> (Box<State>, PreparedCarrierGeometry) {
    let lane = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(1),
        alias: "retiring-queue".to_owned(),
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        NonZeroU32::new(2).unwrap(),
        vec![iroha_data_model::nexus::LaneConfig::default(), lane.clone()],
    )
    .unwrap();
    let state = Box::new(State::new_with_pre_genesis_nexus_for_testing(
        World::default(),
        nexus,
        LiveQueryStore::start_test(),
    ));
    let geometry = capture(&state, replace, lane);
    (state, geometry)
}

fn capture(
    state: &State,
    replace: bool,
    lane: iroha_data_model::nexus::LaneConfig,
) -> PreparedCarrierGeometry {
    let mut block = state.merge_preexecution_block(header());
    stage_structural_manual_plan(
        &mut block,
        iroha_data_model::nexus::LaneLifecyclePlan {
            additions: if replace {
                vec![lane.clone()]
            } else {
                Vec::new()
            },
            retire: vec![lane.id],
        },
    );
    block.prepare_carrier_geometry().unwrap()
}

fn queue() -> Queue {
    Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        tokio::sync::broadcast::channel(8).0,
    )
}

#[test]
fn original_queue_cut_binds_retirement_and_replacement_until_drop() {
    for replace in [false, true] {
        let (state, mut geometry) = fixture(replace);
        let queue = queue();
        let source = OriginalCarrierQueue::for_test(&state, &queue);
        let observer = source.try_observe().unwrap();
        let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
        let proof = CarrierQueueRetirement::try_new(
            &state,
            &geometry,
            header(),
            &source,
            observer.try_into_cut().unwrap(),
        )
        .unwrap();
        assert!(queue.try_lock_lane_retirement_observer().is_err());
        assert!(geometry.has_queue_custody(&state, header(), Some(&proof)));
        assert!(!geometry.has_queue_custody(&state, header(), None));
        let mut other_header = header();
        other_header.set_view_change_index(1);
        assert!(!proof.authenticates(&state, &geometry, other_header));
        let pending = geometry._pending.as_mut().unwrap();
        pending
            .catalog_update
            .previous_lane_incarnations
            .insert(LaneId::new(1), Hash::new(b"foreign incarnation"));
        assert!(!proof.authenticates(&state, &geometry, header()));
        drop(lifecycle);
        drop(proof);
        drop(queue.try_lock_lane_retirement_observer().unwrap());
        assert_eq!(state.committed_height(), 0);
    }
}

#[test]
fn empty_decoy_queue_and_foreign_state_never_supply_original_cut() {
    let (state, geometry) = fixture(false);
    let queue = queue();
    let decoy = self::queue();
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    let (error, cleanup) = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        decoy
            .try_lock_lane_retirement_observer()
            .unwrap()
            .try_into_cut()
            .unwrap(),
    )
    .err()
    .unwrap();
    drop(cleanup);
    assert!(matches!(error, CarrierQueueRetirementError::ForeignQueue));
    drop(decoy.try_lock_lane_retirement_observer().unwrap());
    let foreign = State::new_for_testing(
        World::default(),
        Arc::clone(&state.kura),
        LiveQueryStore::start_test(),
    );
    let (error, cleanup) = CarrierQueueRetirement::try_new(
        &foreign,
        &geometry,
        header(),
        &source,
        source.try_observe().unwrap().try_into_cut().unwrap(),
    )
    .err()
    .unwrap();
    drop(cleanup);
    assert!(matches!(error, CarrierQueueRetirementError::ForeignState));
    drop(queue.try_lock_lane_retirement_observer().unwrap());
    assert_eq!(state.committed_height(), 0);
    assert_eq!(foreign.committed_height(), 0);
}

#[test]
fn malformed_captured_retirement_route_releases_original_queue_cut() {
    let (state, mut geometry) = fixture(false);
    let queue = queue();
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    geometry
        ._pending
        .as_mut()
        .unwrap()
        .catalog_update
        .previous_lane_incarnations
        .remove(&LaneId::new(1));
    let result = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        source.try_observe().unwrap().try_into_cut().unwrap(),
    );
    let (error, cleanup) = result
        .err()
        .expect("missing predecessor incarnation must refuse");
    drop(cleanup);
    let CarrierQueueRetirementError::Geometry(LaneLifecycleError::RuntimeCatalog(reason)) = error
    else {
        panic!("expected captured-route geometry error: {error:?}");
    };
    assert_eq!(
        reason,
        "retiring Queue route has no captured nonzero incarnation"
    );
    drop(queue.try_lock_lane_retirement_observer().unwrap());
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}
fn poll(wait: &mut concread::release::ReleaseFuture) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::new(NoopWake))))
}

#[test]
fn pending_queue_work_releases_without_applying_the_blocked_carrier() {
    let (mut state, _) = fixture(false);
    let (queue, clock) = crate::queue::tests::carrier_retirement_queue_fixture(&mut state);
    let lane = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::new(1))
        .unwrap()
        .clone();
    let geometry = capture(&state, false, lane);
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    let observer = source.try_observe().unwrap();
    let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
    let (refused, cleanup) = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        observer.try_into_cut().unwrap(),
    )
    .err()
    .unwrap();
    drop(lifecycle);
    drop(cleanup);
    let CarrierQueueRetirementError::Pending {
        lane,
        dataspace,
        incarnation,
        wait,
    } = refused
    else {
        panic!("expected typed original-route pending work: {refused:?}")
    };
    assert_eq!(lane, LaneId::new(1));
    assert_eq!(dataspace, DataSpaceId::UNIVERSAL);
    assert_eq!(Some(incarnation), state.lane_incarnation(lane));
    let mut wait = wait.wait_for_release();
    assert!(poll(&mut wait).is_pending());
    // An ordinary expiry sweep progresses independently of this blocked height.
    clock.advance(std::time::Duration::from_secs(86400));
    assert_eq!(queue.cull_expired_entries_if_due(), 1);
    assert!(poll(&mut wait).is_ready());
    assert_eq!(state.committed_height(), 0);
    let observer = source.try_observe().unwrap();
    let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
    let proof = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        observer.try_into_cut().unwrap(),
    )
    .unwrap();
    assert!(geometry.has_queue_custody(&state, header(), Some(&proof)));
    drop(lifecycle);
    drop(proof);
}

#[test]
fn queue_cut_does_not_replace_kura_or_original_geometry_authority() {
    let (state, mut geometry) = fixture(false);
    let queue = queue();
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    let observer = source.try_observe().unwrap();
    let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
    let proof = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        observer.try_into_cut().unwrap(),
    )
    .unwrap();
    let foreign = Kura::blank_kura_for_testing();
    let lease = foreign.try_publication_lease().unwrap();
    let result = geometry.complete_under(
        &state,
        header(),
        &mut state.tiered_backend.lock(),
        &lease,
        Some(&proof),
    );
    assert!(
        matches!(result, Err(LaneLifecycleError::Storage(reason)) if reason.contains("another original Kura"))
    );
    assert!(geometry.raw.is_none());
    assert!(geometry.tiered.is_none());
    assert_eq!(state.committed_height(), 0);
    drop(lifecycle);
    drop(proof);
    drop(queue.try_lock_lane_retirement_observer().unwrap());
}

#[test]
fn sticky_queue_fault_revokes_retained_retirement_before_storage_or_visibility() {
    let (state, mut geometry) = fixture(false);
    let queue = queue();
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    let observer = source.try_observe().unwrap();
    let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
    let proof = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        observer.try_into_cut().unwrap(),
    )
    .unwrap();
    crate::queue::tests::fault_carrier_retirement_queue_fixture(&queue);
    assert!(matches!(
        proof.ensure_available(),
        Err(CarrierQueueRetirementError::Unavailable(
            crate::queue::QueueLaneRetirementUnavailable::DurabilityFault
        ))
    ));
    assert!(!geometry.has_queue_custody(&state, header(), Some(&proof)));
    let lease = state.kura.try_publication_lease().unwrap();
    assert!(
        geometry
            .complete_under(
                &state,
                header(),
                &mut state.tiered_backend.lock(),
                &lease,
                Some(&proof)
            )
            .is_err()
    );
    assert!(geometry.raw.is_none());
    assert!(geometry.tiered.is_none());
    assert_eq!(state.committed_height(), 0);
    drop(lifecycle);
    drop(proof);
}

#[test]
fn immutable_apply_service_exposes_only_its_actual_state_and_queue() {
    let (boxed, _) = fixture(false);
    let state: Arc<State> = boxed.into();
    let queue = Arc::new(queue());
    let (events, _) = tokio::sync::broadcast::channel(8);
    let service = crate::sumeragi::v2_apply::V2ApplyService::new(
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
    assert!(source.belongs_to(&state));
    let foreign = State::new_for_testing(
        World::default(),
        Arc::clone(&state.kura),
        LiveQueryStore::start_test(),
    );
    assert!(!source.belongs_to(&foreign));
    let decoy = self::queue();
    let cut = decoy
        .try_lock_lane_retirement_observer()
        .unwrap()
        .try_into_cut()
        .unwrap();
    assert!(!source.owns_cut(&cut));
    drop(cut);
    let cut = source.try_observe().unwrap().try_into_cut().unwrap();
    assert!(source.owns_cut(&cut));
    assert!(queue.try_lock_lane_retirement_observer().is_err());
    drop(cut);
    drop(queue.try_lock_lane_retirement_observer().unwrap());
}

#[test]
fn original_queue_cut_completes_retirement_storage_without_publishing_state() {
    for replace in [false, true] {
        let (state, mut geometry) = fixture(replace);
        let before = state.canonical_runtime.view().get().clone();
        let queue = queue();
        let source = OriginalCarrierQueue::for_test(&state, &queue);
        let lease = state.kura.try_publication_lease().unwrap();
        let observer = source.try_observe().unwrap();
        let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
        let proof = CarrierQueueRetirement::try_new(
            &state,
            &geometry,
            header(),
            &source,
            observer.try_into_cut().unwrap(),
        )
        .unwrap();
        let mut backend = state.tiered_backend.try_lock_or_wait().unwrap();
        geometry.prepare_under(&backend, &lease).unwrap();
        assert!(
            geometry
                .complete_under(&state, header(), &mut backend, &lease, Some(&proof))
                .unwrap()
                .updated_da_mapping()
                .is_some()
        );
        assert_eq!(
            geometry.raw.as_ref().unwrap().phase(),
            crate::kura::RawGeometryPhase::CatalogPublished
        );
        assert!(queue.try_lock_lane_retirement_observer().is_err());
        assert_eq!(state.canonical_runtime.view().get(), &before);
        assert_eq!(state.committed_height(), 0);
        drop(backend);
        drop(lifecycle);
        drop(proof);
        drop(queue.try_lock_lane_retirement_observer().unwrap());
    }
}

#[test]
fn route_refusal_retains_original_cut_cleanup_through_lifecycle() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reenter {
        state: Arc<State>,
        queue: Arc<Queue>,
        wakes: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(self.state.try_lock_lane_lifecycle_work_admission().is_ok());
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
    let (state, mut geometry) = fixture(false);
    let state: Arc<State> = state.into();
    let queue = Arc::new(queue());
    geometry
        ._pending
        .as_mut()
        .unwrap()
        .catalog_update
        .previous_lane_incarnations
        .remove(&LaneId::new(1));
    let source = OriginalCarrierQueue::for_test(&state, &queue);
    let observer = source.try_observe().unwrap();
    let lifecycle = state.try_lock_lane_lifecycle_work_admission().unwrap();
    let mut wait = source.try_observe().err().unwrap().wait_for_release();
    let callback = Arc::new(Reenter {
        state: Arc::clone(&state),
        queue: Arc::clone(&queue),
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let (error, cleanup) = CarrierQueueRetirement::try_new(
        &state,
        &geometry,
        header(),
        &source,
        observer.try_into_cut().unwrap(),
    )
    .err()
    .expect("malformed exact predecessor");
    assert!(matches!(error, CarrierQueueRetirementError::Geometry(_)));
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
    drop(lifecycle);
    drop(cleanup);
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
}
