/// Routes every input to the default lane from the admission view only, counting
/// each view-derived routing call.
struct ViewCountingRouter {
    view_calls: Arc<AtomicUsize>,
}
impl LaneRouter for ViewCountingRouter {
    fn try_route(
        &self,
        _tx: &dyn TransactionRoutingView,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        Ok(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
    }
    fn try_route_without_state(
        &self,
        _tx: &dyn TransactionRoutingView,
    ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
        Ok(None)
    }
    fn try_route_with_view(
        &self,
        tx: &dyn TransactionRoutingView,
        _state_view: &StateView<'_>,
    ) -> Result<RoutingDecision, RoutingResolveError> {
        self.view_calls.fetch_add(1, Ordering::Relaxed);
        self.try_route(tx)
    }
}
struct ViewCountingGossipFixture {
    state: State,
    queue: Queue,
    view_calls: Arc<AtomicUsize>,
    _time_handle: iroha_primitives::time::MockTimeHandle,
    time_source: TimeSource,
}
fn view_counting_gossip_fixture() -> ViewCountingGossipFixture {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let view_calls = Arc::new(AtomicUsize::new(0));
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(ViewCountingRouter {
            view_calls: Arc::clone(&view_calls),
        }),
    );
    ViewCountingGossipFixture {
        state,
        queue,
        view_calls,
        _time_handle: time_handle,
        time_source,
    }
}
fn single_default_plan() -> RoutingPlan {
    RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
}

#[test]
fn gossip_admission_routes_once_in_one_view_and_retains_the_original_frame() {
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    let hash = tx.hash_as_entrypoint();
    let frame = tx.entrypoint_bytes();
    let admitted =
        fixture
            .queue
            .admit_gossip_with_state(tx, &fixture.state, &single_default_plan());
    assert!(
        matches!(&admitted, GossipAdmission::Pushed(Ok(route))
            if *route == RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
        "{admitted:?}"
    );
    assert_eq!(
        fixture.view_calls.load(Ordering::Relaxed),
        1,
        "the push reuses the plan that this view's installed router derived"
    );
    assert!(Arc::ptr_eq(
        &fixture.queue.txs.get(&hash).unwrap().entrypoint_bytes(),
        &frame
    ));
    assert_eq!(
        fixture.queue.routing_plan_hint(&hash),
        Some(single_default_plan())
    );
    assert_eq!(fixture.queue.tx_gossip.pop(), Some(hash));
}

#[test]
fn separate_route_and_push_derive_the_view_plan_twice() {
    // The sequence that gossip admission replaces: each call takes its own fence and
    // view, so the push derives the plan again.
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    let plan = fixture
        .queue
        .route_plan_with_state(&tx, &fixture.state)
        .expect("view route resolves");
    fixture
        .queue
        .push_with_lane_with_state_and_routing_plan(tx, &fixture.state, plan)
        .expect("routed push admits");
    assert_eq!(fixture.view_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn gossip_admission_refuses_an_advertised_plan_that_differs_from_the_local_plan() {
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    let hash = tx.hash_as_entrypoint();
    let advertised = RoutingPlan::single(RoutingDecision::new(LaneId::new(9), DataSpaceId::new(9)));
    let admitted = fixture
        .queue
        .admit_gossip_with_state(tx, &fixture.state, &advertised);
    assert!(
        matches!(&admitted, GossipAdmission::PlanMismatch(local) if *local == single_default_plan()),
        "{admitted:?}"
    );
    assert_eq!(fixture.view_calls.load(Ordering::Relaxed), 1);
    assert!(!fixture.queue.contains_entrypoint_hash(hash));
    assert_eq!(fixture.queue.tx_gossip.pop(), None);
}

#[test]
fn gossip_admission_reports_an_unresolved_local_route_without_admission() {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let unknown = DataSpaceId::new(4_243);
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: unknown,
        }),
    );
    queue.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash_as_entrypoint();
    match queue.admit_gossip_with_state(tx, &state, &single_default_plan()) {
        GossipAdmission::Unrouted(error) => {
            crate::queue::router::assert_completed_routing_error_eq(
                &error,
                &RoutingResolveError::UnknownDataspace {
                    dataspace_id: unknown,
                },
                format_args!("gossip admission routing rejection"),
            )
        }
        other => panic!("expected an unresolved local route, got {other:?}"),
    }
    assert!(!queue.contains_entrypoint_hash(hash));
}

#[test]
fn gossip_admission_rederives_a_state_free_plan_like_the_routed_push() {
    // A plan from the state-free probe is derived again from the view at push time,
    // exactly as the separate push did; only view-derived plans skip that step.
    struct StateFreeCountingRouter {
        calls: Arc<AtomicUsize>,
    }
    impl LaneRouter for StateFreeCountingRouter {
        fn try_route(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        }
    }
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StateFreeCountingRouter {
            calls: Arc::clone(&calls),
        }),
    );
    let tx = accepted_tx_by_someone(&time_source);
    let admitted = queue.admit_gossip_with_state(tx, &state, &single_default_plan());
    assert!(
        matches!(&admitted, GossipAdmission::Pushed(Ok(_))),
        "{admitted:?}"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn fenced_push_rederives_a_view_plan_from_a_replaced_router() {
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    let replaced_calls = Arc::new(AtomicUsize::new(0));
    let replaced: Arc<dyn LaneRouter> = Arc::new(ViewCountingRouter {
        view_calls: Arc::clone(&replaced_calls),
    });
    let mut retired = QueuePrivacyRetirements::default();
    let view = fixture.state.view();
    let admitted = fixture.queue.push_in_fenced_view(
        tx,
        &view,
        Some(LocalRoutingPlan {
            plan: single_default_plan(),
            view_router: Some(replaced),
        }),
        &mut retired,
        SubmissionOrigin::Remote,
        TeuBacklogPublication::AfterAdmission,
    );
    assert!(admitted.is_ok(), "{admitted:?}");
    assert_eq!(
        (
            fixture.view_calls.load(Ordering::Relaxed),
            replaced_calls.load(Ordering::Relaxed)
        ),
        (1, 0),
        "a plan from a router that is no longer installed is derived again"
    );
}

#[test]
fn local_routing_plan_records_only_a_view_derived_router() {
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    let view = fixture.state.view();
    let local = fixture
        .queue
        .local_routing_plan_in_view(&tx, &view)
        .expect("view route resolves");
    assert_eq!(local.plan, single_default_plan());
    let installed = Arc::clone(&*fixture.queue.router.read());
    assert!(
        local
            .view_router
            .as_ref()
            .is_some_and(|router| Arc::ptr_eq(router, &installed)),
        "a view-derived plan records the installed router"
    );
    let state_free = Queue::test_with_router(
        config_factory(),
        &fixture.time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
    );
    let local = state_free
        .local_routing_plan_in_view(&tx, &view)
        .expect("state-free route resolves");
    assert_eq!(local.plan, single_default_plan());
    assert!(
        local.view_router.is_none(),
        "a state-free probe plan is derived again at push"
    );
}

#[test]
fn local_routing_plan_rederives_a_resident_input_without_recording_a_router() {
    let fixture = view_counting_gossip_fixture();
    let tx = accepted_tx_by_someone(&fixture.time_source);
    fixture
        .queue
        .push_with_lane_with_state(tx.clone(), &fixture.state)
        .expect("admit the resident input");
    let calls_before = fixture.view_calls.load(Ordering::Relaxed);
    let view = fixture.state.view();
    let local = fixture
        .queue
        .local_routing_plan_in_view(&tx, &view)
        .expect("resident route resolves");
    assert_eq!(local.plan, single_default_plan());
    assert!(local.view_router.is_none());
    assert_eq!(
        fixture.view_calls.load(Ordering::Relaxed),
        calls_before + 1,
        "a resident input is routed from the view once"
    );
}

#[test]
fn synced_routing_view_realigns_a_stale_routing_projection() {
    let fixture = view_counting_gossip_fixture();
    let mut stale = fixture.state.nexus_snapshot();
    stale.fusion.exit_teu = stale.fusion.exit_teu.wrapping_add(1);
    fixture.queue.install_test_router_metadata_for_nexus(&stale);
    assert!(
        !fixture
            .queue
            .nexus_routing_matches(&fixture.state.nexus_snapshot())
    );
    let mut retired_state = QueuePrivacyRetirements::default();
    let mut retired_view = QueuePrivacyRetirements::default();
    let view = fixture
        .queue
        .synced_routing_view(&fixture.state, &mut retired_state, &mut retired_view)
        .expect("routing aligns with the committed view");
    assert!(fixture.queue.nexus_routing_matches(view.nexus()));
}

#[cfg(feature = "telemetry")]
#[test]
fn gossip_admission_leaves_teu_backlog_gauges_to_the_batch_publication() {
    let metrics = Arc::new(Metrics::default());
    let state = new_queue_test_state_with_telemetry(
        world_with_explicit_queue_fees(world_with_test_domains(), zero_rate_routing_fees()),
        Nexus::default(),
        LiveQueryStore::start_test(),
        StateTelemetry::new(Arc::clone(&metrics), true),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(ViewCountingRouter {
            view_calls: Arc::new(AtomicUsize::new(0)),
        }),
    );
    let lane = LaneId::SINGLE;
    let committed = || {
        metrics
            .nexus_scheduler_lane_teu_status
            .read()
            .expect("lane TEU cache")
            .get(&lane.as_u32())
            .map_or(0, |status| status.committed)
    };
    let tx = accepted_tx_by_someone(&time_source);
    let admitted = queue.admit_gossip_with_state(tx, &state, &single_default_plan());
    assert!(
        matches!(&admitted, GossipAdmission::Pushed(Ok(_))),
        "{admitted:?}"
    );
    let pending = queue
        .lane_teu_pending
        .get(&lane)
        .map(|entry| entry.value().teu)
        .expect("admission records its pending TEU");
    assert!(pending > 0);
    assert_eq!(committed(), 0, "admission leaves the gauges to its batch");
    queue.publish_teu_backlog_with_state(&state);
    let capacity = queue.nexus_limits.read().for_lane(lane).teu_capacity;
    assert_eq!(committed(), pending.min(capacity));
}
