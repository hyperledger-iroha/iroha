//! Test-only extensions and the stable v2 apply test module.
#[derive(Default)]
pub(super) struct FailureInjection {
    successful_frontier_pause: std::sync::Mutex<Option<Arc<SuccessfulApplyFrontierPause>>>,
    pub(super) kura_store: std::sync::atomic::AtomicBool,
    pub(super) wsv_checkpoint: std::sync::atomic::AtomicBool,
    pub(super) provider_ingest_archive_capture: std::sync::atomic::AtomicBool,
    pub(super) reputation_archive_capture: std::sync::atomic::AtomicBool,
}
/// Arrival, release and worker exit belong to the same synchronized observation.
#[derive(Default)]
struct ApplyFrontierBarrierState {
    arrived: bool,
    released: bool,
    worker_finished: bool,
}
/// A deterministic rendezvous whose arrival wait ends if its worker exits.
#[derive(Default)]
struct ApplyFrontierBarrier {
    state: std::sync::Mutex<ApplyFrontierBarrierState>,
    changed: std::sync::Condvar,
}
impl ApplyFrontierBarrier {
    fn arrive_and_wait(&self) {
        let mut state = self.state.lock().expect("frontier barrier lock");
        state.arrived = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).expect("frontier barrier wait");
        }
    }
    fn wait_until_arrived(&self) {
        let state = self.state.lock().expect("frontier observation lock");
        let state = self
            .changed
            .wait_while(state, |state| !state.arrived && !state.worker_finished)
            .expect("frontier observation wait");
        let arrived = state.arrived;
        drop(state);
        assert!(
            arrived,
            "successful Apply worker exited before reaching the test rendezvous"
        );
    }
    fn release(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .released = true;
        self.changed.notify_all();
    }
    fn mark_worker_finished(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .worker_finished = true;
        self.changed.notify_all();
    }
}
#[derive(Default)]
struct SuccessfulApplyFrontierPause {
    before_store: ApplyFrontierBarrier,
    after_store: ApplyFrontierBarrier,
}
struct ReleaseSuccessfulApply(Arc<SuccessfulApplyFrontierPause>);
impl Drop for ReleaseSuccessfulApply {
    fn drop(&mut self) {
        self.0.before_store.release();
        self.0.after_store.release();
    }
}
/// Wake observers on normal return, an Apply error or worker panic.
struct NotifySuccessfulApplyExit(Arc<SuccessfulApplyFrontierPause>);
impl Drop for NotifySuccessfulApplyExit {
    fn drop(&mut self) {
        self.0.before_store.mark_worker_finished();
        self.0.after_store.mark_worker_finished();
    }
}

#[test]
fn successful_apply_frontier_rendezvous_observes_both_gates_before_worker_completion() {
    let pause = Arc::new(SuccessfulApplyFrontierPause::default());
    std::thread::scope(|scope| {
        let _release = ReleaseSuccessfulApply(Arc::clone(&pause));
        let worker_pause = Arc::clone(&pause);
        let (completed, observed) = std::sync::mpsc::channel();
        let worker = crate::sumeragi::sumeragi_thread_builder("apply-frontier-both-gates")
            .spawn_scoped(scope, move || {
                let _finished = NotifySuccessfulApplyExit(Arc::clone(&worker_pause));
                worker_pause.before_store.arrive_and_wait();
                completed
                    .send("before-store released")
                    .expect("observer alive");
                worker_pause.after_store.arrive_and_wait();
                completed
                    .send("after-store released")
                    .expect("observer alive");
            })
            .expect("spawn frontier worker");
        pause.before_store.wait_until_arrived();
        assert_eq!(
            observed.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        pause.before_store.release();
        pause.after_store.wait_until_arrived();
        assert_eq!(observed.try_recv(), Ok("before-store released"));
        assert_eq!(
            observed.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        pause.after_store.release();
        worker.join().expect("worker completes after both releases");
        assert_eq!(observed.try_recv(), Ok("after-store released"));
        assert_eq!(
            observed.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        );
        // Completion must not erase either actual arrival.
        pause.before_store.wait_until_arrived();
        pause.after_store.wait_until_arrived();
    });
}

#[test]
fn successful_apply_frontier_worker_exit_before_arrival_notifies_both_gates() {
    // Exercise successful return, returned error and panic with no arrival.
    for disposition in 0..3 {
        let pause = Arc::new(SuccessfulApplyFrontierPause::default());
        std::thread::scope(|scope| {
            let _release = ReleaseSuccessfulApply(Arc::clone(&pause));
            let worker_pause = Arc::clone(&pause);
            let worker = crate::sumeragi::sumeragi_thread_builder("apply-frontier-early-exit")
                .spawn_scoped(scope, move || {
                    let _finished = NotifySuccessfulApplyExit(worker_pause);
                    match disposition {
                        0 => Ok(()),
                        1 => Err("Apply rejected before either gate"),
                        _ => panic!("Apply panicked before either gate"),
                    }
                })
                .expect("spawn exiting frontier worker");
            for gate in [&pause.before_store, &pause.after_store] {
                let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    gate.wait_until_arrived();
                }))
                .expect_err("worker exit cannot stand in for an actual storage observation");
                assert_eq!(
                    error
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| error.downcast_ref::<String>().map(String::as_str)),
                    Some("successful Apply worker exited before reaching the test rendezvous")
                );
            }
            let outcome = worker.join();
            match disposition {
                0 => assert_eq!(outcome.expect("normal worker return"), Ok(())),
                1 => assert_eq!(
                    outcome.expect("returned Apply error"),
                    Err("Apply rejected before either gate")
                ),
                _ => assert_eq!(
                    outcome
                        .expect_err("worker panic remains a panic")
                        .downcast_ref::<&str>(),
                    Some(&"Apply panicked before either gate")
                ),
            }
        });
    }
}

#[test]
fn successful_apply_frontier_worker_exit_between_gates_preserves_first_arrival() {
    let pause = Arc::new(SuccessfulApplyFrontierPause::default());
    std::thread::scope(|scope| {
        let _release = ReleaseSuccessfulApply(Arc::clone(&pause));
        let worker_pause = Arc::clone(&pause);
        let worker = crate::sumeragi::sumeragi_thread_builder("apply-frontier-between-gates")
            .spawn_scoped(scope, move || {
                let _finished = NotifySuccessfulApplyExit(Arc::clone(&worker_pause));
                worker_pause.before_store.arrive_and_wait();
                Err::<(), _>("Apply rejected after the first gate")
            })
            .expect("spawn frontier worker");
        pause.before_store.wait_until_arrived();
        pause.before_store.release();
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pause.after_store.wait_until_arrived();
        }))
        .expect_err("the first gate must not satisfy the second observation");
        assert_eq!(
            error
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| error.downcast_ref::<String>().map(String::as_str)),
            Some("successful Apply worker exited before reaching the test rendezvous")
        );
        assert_eq!(
            worker.join().expect("returned Apply error"),
            Err("Apply rejected after the first gate")
        );
        pause.before_store.wait_until_arrived();
    });
}

#[test]
fn successful_apply_frontier_observer_unwind_releases_and_joins_worker() {
    let pause = Arc::new(SuccessfulApplyFrontierPause::default());
    std::thread::scope(|scope| {
        let release = ReleaseSuccessfulApply(Arc::clone(&pause));
        let worker_pause = Arc::clone(&pause);
        let worker = crate::sumeragi::sumeragi_thread_builder("apply-frontier-observer-unwind")
            .spawn_scoped(scope, move || {
                let _finished = NotifySuccessfulApplyExit(Arc::clone(&worker_pause));
                worker_pause.before_store.arrive_and_wait();
                worker_pause.after_store.arrive_and_wait();
                "both gates released during observer unwind"
            })
            .expect("spawn frontier worker");
        let observer_pause = Arc::clone(&pause);
        let outcome = std::panic::catch_unwind(move || {
            let _release = release;
            observer_pause.before_store.wait_until_arrived();
            panic!("observer assertion failed at the first gate");
        });
        let error = outcome.expect_err("observer failure remains visible");
        assert_eq!(
            error
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| error.downcast_ref::<String>().map(String::as_str)),
            Some("observer assertion failed at the first gate")
        );
        assert_eq!(
            worker.join().expect("released worker completes normally"),
            "both gates released during observer unwind"
        );
        pause.before_store.wait_until_arrived();
        pause.after_store.wait_until_arrived();
    });
}

impl V2ApplyService {
    fn pause_successful_apply_frontier_for_test(&self) -> Arc<SuccessfulApplyFrontierPause> {
        let pause = Arc::new(SuccessfulApplyFrontierPause::default());
        let previous = self
            .test_failures
            .successful_frontier_pause
            .lock()
            .expect("install successful Apply barrier")
            .replace(Arc::clone(&pause));
        assert!(previous.is_none(), "one successful Apply pause per fixture");
        pause
    }
    pub(super) fn before_successful_apply_kura_store_for_test(&self) {
        let pause = self
            .test_failures
            .successful_frontier_pause
            .lock()
            .expect("read successful Apply barrier")
            .clone();
        if let Some(pause) = pause {
            pause.before_store.arrive_and_wait();
        }
    }
    pub(super) fn after_successful_apply_kura_store_for_test(&self) {
        let pause = self
            .test_failures
            .successful_frontier_pause
            .lock()
            .expect("read successful Apply barrier")
            .clone();
        if let Some(pause) = pause {
            pause.after_store.arrive_and_wait();
        }
    }
}
/// Test-only durable-application crash boundary.
pub(super) enum CrashPoint {
    /// After canonical block persistence.
    KuraStore,
    /// After the staged WSV checkpoint.
    WsvCheckpoint,
    /// After provider-ingest archive capture.
    ProviderIngestArchiveCapture,
    /// After reputation archive capture.
    ReputationArchiveCapture,
}
/// Persist the exact payload, exact execution input, and immutable recovery
/// record in crash-safe order after independently rebuilding every authority.
#[cfg(test)]
pub(crate) fn install_historical_autonomous_lane_recovery(
    state: &State,
    kura: &Kura,
    input: &HistoricalAutonomousReservationInstallV1,
) -> Result<HistoricalAutonomousLaneRecoveryInstallOutcome, V2ReservationLifecycleError> {
    let record = preflight_historical_autonomous_lane_recovery(state, kura, input)?;
    persist_preflighted_historical_autonomous_lane_recovery(kura, &record)
}
/// Persist one record whose complete State authority was already validated.
/// Kura performs its bounded namespace preflight, durable dependency checks,
/// and collision checks at the persistence boundary.
#[cfg(test)]
pub(crate) fn persist_preflighted_historical_autonomous_lane_recovery(
    kura: &Kura,
    record: &HistoricalAutonomousLaneRecoveryRecordV1,
) -> Result<HistoricalAutonomousLaneRecoveryInstallOutcome, V2ReservationLifecycleError> {
    kura.persist_lane_executable_payload(
        &record.payload,
        record.payload.network_id,
        record.payload.epoch,
    )?;
    persist_preflighted_historical_autonomous_lane_recoveries(kura, std::slice::from_ref(record))?
        .pop()
        .ok_or_else(|| {
            invalid_historical_autonomous_recovery(
                &record.installation_input(),
                "single historical recovery persistence produced no outcome",
            )
        })
}
impl AutonomousLaneQueueCarrierCleanupAuthorization {
    #[cfg(test)]
    fn from_projection_for_test(
        reservation_group: LaneQueueReservationGroupBindingV1,
        projection: ProductionInFlightFirstReleaseTransitionProjection,
    ) -> Result<Self, String> {
        Self::from_authenticated(AuthenticatedCarrierApplicationProjection {
            reservation_group,
            projection,
        })
    }
}
impl AutonomousLaneQueueCarrierCleanupAuthorization {
    #[cfg(test)]
    fn accepted_projection_for_test(&self) -> ProductionInFlightFirstReleaseTransitionProjection {
        *self.checked_apply_carrier.accepted_projection()
    }
}
impl V2ApplyService {
    #[cfg(test)]
    fn finish_durable_apply_completion(
        &self,
        evidence: DurableApplicationEvidence,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        let checked_application = check_production_application_transition(application_trace)
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"durable application does not refine its Decision completion",
                )
            })?;
        self.finish_durable_apply_completion_against(
            evidence,
            checked_application.into_projection(),
        )
    }
}
impl V2ApplyService {
    pub(super) fn inject_test_crash(&self, point: CrashPoint) -> Result<(), V2ApplyError> {
        let (requested, error) = match point {
            CrashPoint::KuraStore => (
                self.test_failures
                    .kura_store
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterKuraStore,
            ),
            CrashPoint::WsvCheckpoint => (
                self.test_failures
                    .wsv_checkpoint
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterWsvCheckpoint,
            ),
            CrashPoint::ProviderIngestArchiveCapture => (
                self.test_failures
                    .provider_ingest_archive_capture
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture,
            ),
            CrashPoint::ReputationArchiveCapture => (
                self.test_failures
                    .reputation_archive_capture
                    .swap(false, std::sync::atomic::Ordering::Relaxed),
                V2ApplyError::InjectedCrashAfterReputationArchiveCapture,
            ),
        };
        if requested { Err(error) } else { Ok(()) }
    }
    #[cfg(test)]
    pub(in crate::sumeragi) fn fail_after_kura_store_for_test(&self) {
        self.test_failures
            .kura_store
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(test)]
    fn fail_after_wsv_checkpoint_for_test(&self) {
        self.test_failures
            .wsv_checkpoint
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(test)]
    fn fail_after_provider_ingest_archive_capture_for_test(&self) {
        self.test_failures
            .provider_ingest_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(test)]
    fn fail_after_reputation_archive_capture_for_test(&self) {
        self.test_failures
            .reputation_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
#[cfg(test)]
pub(super) fn snapshot_mismatch_context(staged: &[u8], committed: &[u8]) -> String {
    let first_difference = staged
        .iter()
        .zip(committed)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| staged.len().min(committed.len()));
    let context_start = first_difference.saturating_sub(256);
    let staged_end = first_difference.saturating_add(768).min(staged.len());
    let committed_end = first_difference.saturating_add(768).min(committed.len());
    format!(
        "first_difference={first_difference}, staged_len={}, committed_len={}, \
         staged_context={:?}, committed_context={:?}",
        staged.len(),
        committed.len(),
        String::from_utf8_lossy(&staged[context_start..staged_end]),
        String::from_utf8_lossy(&committed[context_start..committed_end]),
    )
}
/// Execute the current typed reconciliation plan in focused no-network tests.
///
/// Production callers retain and handle every non-ready planning outcome;
/// this helper deliberately maps those outcomes to their exact test errors.
fn reconcile_lane_reservation_ownership(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    verified_active_context: &VerifiedHeightContext,
) -> Result<LaneReservationReconciliationSummary, V2ReservationLifecycleError> {
    match plan_lane_reservation_ownership(state, queue, kura, verified_active_context, None)? {
        LaneReservationReconciliationPlanning::AlreadyCompleted(observation) => {
            observe_completed_lane_reservation_reconciliation(queue, kura, observation)
        }
        LaneReservationReconciliationPlanning::Ready(plan) => {
            apply_lane_reservation_reconciliation_plan(state, queue, kura, plan)
        }
        LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
            let height = needs.first().map_or(0, |need| need.height);
            Err(V2ReservationLifecycleError::MissingCanonicalBody { height })
        }
        LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) => {
            let install = installs
                .first()
                .expect("historical recovery planning is never empty");
            Err(
                V2ReservationLifecycleError::HistoricalRecoveryInstallationMissing {
                    recovery_id: install.recovery_id,
                    lane_id: install.reservation_group.identity.lane_id,
                },
            )
        }
    }
}
#[test]
fn reservation_route_accepts_only_canonical_default_single_lane_coordinates() {
    let nexus = iroha_config::parameters::actual::Nexus::default();
    assert!(reservation_route_is_active(
        &nexus,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        1,
    ));
    assert!(!reservation_route_is_active(
        &nexus,
        LaneId::SINGLE,
        DataSpaceId::new(7),
        1,
    ));
    assert!(!reservation_route_is_active(
        &nexus,
        LaneId::new(1),
        DataSpaceId::UNIVERSAL,
        1,
    ));
}
/// Focused preflight harness for synthetic merge entries which deliberately
/// lack a durable Kura carrier. Production cleanup additionally requires the
/// canonical carrier/source-outcome authentication path.
fn finalize_certified_merge_reservations_for_test(
    state: &State,
    queue: &Queue,
    entry: &MergeLedgerEntry,
    applications: Vec<AuthenticatedCarrierApplicationProjection>,
) -> Result<usize, V2ReservationLifecycleError> {
    let groups = crate::state::certified_merge_queue_reservation_groups(entry)?;
    if groups.len() != applications.len() {
        return Err(
            V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                detail: "authenticated ApplyCarrier cardinality differs from canonical reservation groups"
                    .to_owned(),
            },
        );
    }
    for (transaction_hash, _) in groups.iter().flatten() {
        if !state.has_committed_entrypoint(*transaction_hash) {
            return Err(V2ReservationLifecycleError::UncommittedMergeEntrypoint {
                entrypoint_hash: *transaction_hash,
            });
        }
    }
    let mut authorized_groups = Vec::with_capacity(groups.len());
    for (group, application) in groups.into_iter().zip(applications) {
        let ordered_keys = group.into_iter().map(|(_, key)| key).collect::<Vec<_>>();
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).map_err(
                |reason| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: reason.to_owned(),
                },
            )?;
        if reservation_group != application.reservation_group {
            return Err(
                V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization {
                    detail: "authenticated ApplyCarrier names another ordered reservation group"
                        .to_owned(),
                },
            );
        }
        let authorization = application
            .queue_cleanup_authorization()
            .map_err(
                |detail| V2ReservationLifecycleError::InvalidCarrierCleanupAuthorization { detail },
            )?;
        authorized_groups.push((ordered_keys, authorization));
    }
    let cleanup = queue.commit_lane_reservation_groups_with_authorization(authorized_groups)?;
    let (finalized_reservations, _terminal_evidence) = cleanup.into_parts();
    Ok(finalized_reservations)
}
fn install_live_lifecycle_cursor_for_apply_test(
    kura: &Kura,
    generation: &crate::kura::AutonomousLifecycleProcessGenerationClaim,
    payload: &LaneExecutablePayloadV1,
    height_context_id: wire::HeightContextId,
    local_peer: &PeerId,
    signer: &KeyPair,
) -> LaneQueueReservationGroupBindingV1 {
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind apply lifecycle reservation group");
    let binding = crate::kura::AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        payload.origin_proposal.descriptor.lane_block_height,
        payload,
        reservation_group,
        local_peer,
    )
    .expect("bind apply lifecycle attempt");
    let validator_count = u8::try_from(binding.validator_set_identity().2)
        .expect("apply lifecycle validator count fits refinement width");
    let validator_mask = if validator_count == 128 {
        u128::MAX
    } else {
        (1_u128 << validator_count) - 1
    };
    let (_, local_actor) = binding.local_validator_identity();
    let producer = binding.producer_actor_projection();
    let live_state = ProductionInFlightFirstReleaseStateProjection {
        validator_count,
        producer,
        producer_selected_owner: producer,
        replicated_carrier_owners: validator_mask & !producer,
        payload_binding_a: producer | local_actor,
        binding_a: canonical_lane_queue_reservation_group_identity_projection(reservation_group),
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: local_actor,
            ..ProductionInFlightFirstReleaseCarrierProjection::default()
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: producer | local_actor,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v1: true,
            ever_reservation_v1: true,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    let cursor = crate::sumeragi::v2_lifecycle_recovery::sign_lifecycle_cursor(
        signer,
        local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        crate::kura::AutonomousLifecycleCursorPhaseV1::live(generation.generation(), live_state)
            .expect("construct apply lifecycle Live cursor"),
    )
    .expect("sign apply lifecycle Live cursor");
    let (_, lease) = kura
        .read_autonomous_lifecycle_cursor(payload, &binding, generation)
        .expect("read absent apply lifecycle cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(lease, cursor.clone())
            .expect("persist apply lifecycle Live cursor")
            .cursor(),
        Some(&cursor),
        "apply setup must read back the exact durable Live cursor",
    );
    reservation_group
}
include!("tests/v2_apply_unsealed_00.rs");
include!("tests/v2_apply_unsealed_01.rs");
include!("tests/v2_apply_unsealed_02.rs");

/// Canonical height-one material for exercising the recovered Decision Apply
/// worker from another Sumeragi unit-test module.
pub(in crate::sumeragi) struct ProductionRecoveredDecisionApplyFixtureV1 {
    /// Cryptographically verified immutable height context.
    pub(in crate::sumeragi) verified: VerifiedHeightContext,
    /// Exact canonical proposal manifest.
    pub(in crate::sumeragi) manifest: wire::PayloadManifest,
    /// Exact canonical `SignedBlockWire` proposal bytes.
    pub(in crate::sumeragi) canonical_wire: Vec<u8>,
    /// Body store retaining the production validation marker.
    pub(in crate::sumeragi) body_store: V2BodyStore,
    /// Exact durable body receipt selected by validation.
    pub(in crate::sumeragi) durable: DurableBodyReceipt,
    /// Real aggregate-signed CommitQC authorizing application.
    pub(in crate::sumeragi) commit_qc: wire::QuorumCertificate,
    /// Validator keys matching the frozen roster.
    pub(in crate::sumeragi) validator_keys: Vec<KeyPair>,
    /// Keeps the body-store directory alive through worker settlement.
    pub(in crate::sumeragi) directory: tempfile::TempDir,
}

/// Build one production-valid genesis, validation marker, and CommitQC for a
/// cross-module lifecycle Validate-to-Apply regression.
pub(in crate::sumeragi) fn production_recovered_decision_apply_fixture_v1()
-> ProductionRecoveredDecisionApplyFixtureV1 {
    let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
    assert!(
        fixture.state.matches_kura_instance(&fixture.kura),
        "production recovered Decision Apply fixture must retain the State Kura instance"
    );
    assert!(
        fixture.service.matches_lifecycle_launch(
            &fixture.state,
            &fixture.kura,
            &fixture.context,
            &fixture.service.validator_set_pops,
        ),
        "production recovered Decision Apply fixture must satisfy launch identity"
    );
    let body_store = fixture.reopen_body_store();
    let canonical_wire = fixture
        .body
        .encode_wire()
        .expect("encode canonical recovered Decision Apply fixture body");
    let durable = fixture.task.validated_receipt().durable().clone();
    let commit_qc = fixture.task.certificate().clone();
    let verified = VerifiedHeightContext::genesis(
        fixture.context.clone(),
        fixture.service.validator_set_pops.clone(),
    )
    .expect("verify recovered Decision Apply fixture height context");
    let ApplyFixture {
        manifest,
        service: _,
        body_root: directory,
        validator_keys,
        ..
    } = fixture;
    ProductionRecoveredDecisionApplyFixtureV1 {
        verified,
        manifest,
        canonical_wire,
        body_store,
        durable,
        commit_qc,
        validator_keys,
        directory,
    }
}
