//! No-clock production lifecycle for one interrupted canonical Kura tip.

use super::*;
use crate::sumeragi::v2_lifecycle_coordinator::{
    PendingKuraActivatedProductionLifecycleV1, PendingKuraProductionLifecycleV1,
    ProductionLifecycleLaunchInputsV1, ProductionPendingKuraApplyRecoveryProgressV1,
};

const PENDING_TIP_RECOVERY_DEADLINE_ROUNDS: u32 = 3;

/// Cadence-derived process-local deadline for closed-ingress interrupted-tip recovery.
#[derive(Clone, Copy, Debug)]
pub(super) struct PendingTipRecoveryDeadline {
    started_at: Instant,
    deadline: Instant,
    /// Total bounded recovery interval.
    pub(super) timeout: Duration,
}

impl PendingTipRecoveryDeadline {
    /// Derive one bounded deadline from the authenticated height cadence.
    pub(super) fn new(started_at: Instant, round_timeout: Duration) -> Result<Self, V2RunnerError> {
        let timeout = round_timeout
            .checked_mul(PENDING_TIP_RECOVERY_DEADLINE_ROUNDS)
            .ok_or(V2RunnerError::InvalidLimits)?;
        let deadline = started_at
            .checked_add(timeout)
            .ok_or(V2RunnerError::InvalidLimits)?;
        Ok(Self {
            started_at,
            deadline,
            timeout,
        })
    }

    /// Return whether the deadline has elapsed at `now`.
    pub(super) fn expired(self, now: Instant) -> bool {
        now >= self.deadline
    }

    /// Return the remaining bounded wait at `now`.
    pub(super) fn remaining(self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    /// Return elapsed recovery time at `now`.
    pub(super) fn elapsed(self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started_at)
    }
}

/// Latch restart-required and report one exhausted recovery deadline.
pub(super) fn pending_tip_recovery_deadline_error(
    output_guard: &ConsensusOutputGuard,
    timeout: Duration,
    attempts: u64,
    stage: Option<PendingKuraApplyRecoveryStage>,
) -> V2RunnerError {
    output_guard.activate_restart_required();
    super::super::status::mark_v2_restart_required();
    V2RunnerError::PendingTipRecoveryDeadlineExceeded {
        timeout,
        attempts,
        stage,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingCanonicalRecoveryControlV1 {
    Complete,
    Shutdown,
}

struct PreparedPendingKuraSuccessorV1 {
    verified_context: crate::sumeragi::v2::VerifiedHeightContext,
    lifecycle_storage_authority: crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    pending_activation: PendingSuccessorActivation,
    receipt_height: u64,
    receipt_context_id: wire::HeightContextId,
    receipt_block_hash: HashOf<BlockHeader>,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn recover_pending_canonical_bodies(
    pending: &mut PendingKuraProductionLifecycleV1,
    setup_runner: &mut ProductionLifecyclePreActivationRunnerBorrowV1,
    activation: &mut ProductionLifecyclePendingKuraRunnerActivationV1,
    needs: &[CanonicalExecutedBlockNeedV1],
    context: &wire::HeightContext,
    local_peer: &PeerId,
    state: &Arc<State>,
    kura: &Arc<Kura>,
    output_guard: &Arc<ConsensusOutputGuard>,
    lane_work_limits: V2LaneWorkLimits,
    retransmit_interval: Duration,
    control_queue_capacity: usize,
    wake_rx: &std::sync::mpsc::Receiver<()>,
    shutdown_signal: &iroha_futures::supervisor::ShutdownSignal,
) -> Result<PendingCanonicalRecoveryControlV1, V2RunnerError> {
    if needs.is_empty() {
        return Err(V2RunnerError::Service(
            "pending Kura canonical executed-body recovery requested an empty body set".to_owned(),
        ));
    }
    pending.with_canonical_body_recovery_ingress(
        setup_runner,
        activation,
        |aperture, _executor, services| {
            let recovery_capacity = CanonicalExecutedBlockRecovery::need_capacity(lane_work_limits);
            let recovery_batches =
                canonical_executed_block_recovery_batches(needs, recovery_capacity)?;
            for bounded_needs in recovery_batches {
                let mut body_recovery = CanonicalExecutedBlockRecovery::new(
                    context.clone(),
                    local_peer.clone(),
                    Arc::clone(state),
                    Arc::clone(kura),
                    Arc::clone(output_guard),
                    lane_work_limits,
                    bounded_needs.to_vec(),
                )?;
                let mut next_retry = Instant::now();
                while body_recovery.has_pending() {
                    if output_guard.restart_required() {
                        return Err(V2RunnerError::RestartRequired);
                    }
                    if shutdown_signal.is_sent() {
                        return Ok(PendingCanonicalRecoveryControlV1::Shutdown);
                    }
                    let now = Instant::now();
                    if now >= next_retry {
                        body_recovery.service_next()?;
                        next_retry = deadline_after(now, retransmit_interval);
                    }
                    let drained = drain_canonical_executed_block_recovery_ingress(
                        aperture.ingress(),
                        &mut body_recovery,
                        control_queue_capacity,
                    )?;
                    if drained != 0 && body_recovery.has_pending() {
                        body_recovery.service_next()?;
                    }
                    let dispatched = dispatch_canonical_executed_block_recovery_effects(
                        &mut body_recovery,
                        services,
                        control_queue_capacity,
                    )?;
                    if body_recovery.has_pending() && drained == 0 && dispatched == 0 {
                        let wait = next_retry
                            .saturating_duration_since(Instant::now())
                            .min(IDLE_POLL);
                        if !wait.is_zero() {
                            let _ = wake_rx.recv_timeout(wait);
                        }
                    }
                }
            }
            Ok(PendingCanonicalRecoveryControlV1::Complete)
        },
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn reconcile_pending_lane_startup(
    mut pending: PendingKuraProductionLifecycleV1,
    setup_runner: &mut ProductionLifecyclePreActivationRunnerBorrowV1,
    activation: &mut ProductionLifecyclePendingKuraRunnerActivationV1,
    context: &wire::HeightContext,
    verified_context: &crate::sumeragi::v2::VerifiedHeightContext,
    state: &Arc<State>,
    queue: &Arc<Queue>,
    kura: &Arc<Kura>,
    local_peer: &PeerId,
    key_pair: &KeyPair,
    output_guard: &Arc<ConsensusOutputGuard>,
    lane_work_limits: V2LaneWorkLimits,
    retransmit_interval: Duration,
    control_queue_capacity: usize,
    wake_rx: &std::sync::mpsc::Receiver<()>,
    shutdown_signal: &iroha_futures::supervisor::ShutdownSignal,
    lifecycle_process_generation: Option<&AutonomousLifecycleProcessGenerationClaim>,
) -> Result<
    (
        PendingKuraProductionLifecycleV1,
        PendingCanonicalRecoveryControlV1,
    ),
    V2RunnerError,
> {
    let evidence_repair_queue_fence =
        LaneApplicationEvidenceRepairQueueFence::capture(queue.as_ref())?;
    loop {
        evidence_repair_queue_fence.revalidate(queue.as_ref())?;
        match plan_lane_application_evidence_repair(
            context,
            state.as_ref(),
            kura.as_ref(),
            lane_work_limits,
        )? {
            LaneApplicationEvidenceRepairPlanning::Ready(plan) if plan.is_empty() => break,
            LaneApplicationEvidenceRepairPlanning::Ready(plan) => {
                let planned_items = plan.item_count();
                let repair = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let summary =
                    apply_lane_application_evidence_repair(state.as_ref(), kura.as_ref(), plan)?;
                if planned_items == 0 || summary.publication_count() == 0 {
                    return Err(V2RunnerError::Service(
                        "pending Kura lane application repair made no bounded progress".to_owned(),
                    ));
                }
                repair.complete();
            }
            LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(needs) => {
                let control = recover_pending_canonical_bodies(
                    &mut pending,
                    setup_runner,
                    activation,
                    &needs,
                    context,
                    local_peer,
                    state,
                    kura,
                    output_guard,
                    lane_work_limits,
                    retransmit_interval,
                    control_queue_capacity,
                    wake_rx,
                    shutdown_signal,
                )?;
                if control == PendingCanonicalRecoveryControlV1::Shutdown {
                    return Ok((pending, control));
                }
            }
        }
    }

    let summary = loop {
        let deferred_terminal_recovery =
            reconcile_lifecycle_terminal_outcomes_before_queue_planning(
                output_guard,
                state.as_ref(),
                queue.as_ref(),
                kura.as_ref(),
                context,
            )?;
        let planning = plan_lane_reservation_ownership(
            state.as_ref(),
            queue.as_ref(),
            kura.as_ref(),
            verified_context,
            None,
        )?;
        let planning = match planning {
            LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) => {
                let planner_evidence = pre_lifecycle_plan.startup_snapshot_recovery_evidence()?;
                let lifecycle = reconcile_autonomous_lifecycle_startup(
                    state.as_ref(),
                    queue.as_ref(),
                    kura.as_ref(),
                    context,
                    planner_evidence,
                    deferred_terminal_recovery,
                    lifecycle_process_generation,
                    local_peer,
                    key_pair,
                )
                .map_err(V2RunnerError::Service)?;
                let completed_bootstraps = lifecycle.completed_bootstraps();
                let recovered_attempts = lifecycle.recovered_attempts();
                let replanned = plan_lane_reservation_ownership(
                    state.as_ref(),
                    queue.as_ref(),
                    kura.as_ref(),
                    verified_context,
                    Some(lifecycle),
                )?;
                if completed_bootstraps != 0 || recovered_attempts != 0 {
                    iroha_logger::info!(
                        completed_bootstraps,
                        recovered_attempts,
                        "reconciled signed autonomous lifecycle custody before Queue publication"
                    );
                }
                replanned
            }
            pending_plan => pending_plan,
        };
        match planning {
            LaneReservationReconciliationPlanning::Ready(plan) => {
                let recovery = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let summary = apply_lane_reservation_reconciliation_plan(
                    state.as_ref(),
                    queue.as_ref(),
                    kura.as_ref(),
                    plan,
                )?;
                recovery.complete();
                break summary;
            }
            LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
                if !queue.lane_reservation_startup_reconciliation_pending() {
                    return Err(V2RunnerError::Service(
                        "pending Kura reservation body recovery lost its Queue gate".to_owned(),
                    ));
                }
                let control = recover_pending_canonical_bodies(
                    &mut pending,
                    setup_runner,
                    activation,
                    &needs,
                    context,
                    local_peer,
                    state,
                    kura,
                    output_guard,
                    lane_work_limits,
                    retransmit_interval,
                    control_queue_capacity,
                    wake_rx,
                    shutdown_signal,
                )?;
                if control == PendingCanonicalRecoveryControlV1::Shutdown {
                    return Ok((pending, control));
                }
            }
            LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
                installs,
            ) => {
                if installs.is_empty() || !queue.lane_reservation_startup_reconciliation_pending() {
                    return Err(V2RunnerError::Service(
                        "pending Kura historical recovery lost its closed Queue boundary"
                            .to_owned(),
                    ));
                }
                let recovery = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let records = installs
                    .iter()
                    .map(|install| {
                        preflight_historical_autonomous_lane_recovery(
                            state.as_ref(),
                            kura.as_ref(),
                            install,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let process_generation = lifecycle_process_generation.ok_or_else(|| {
                    V2RunnerError::Service(
                        "pending Kura historical recovery requires a process generation".to_owned(),
                    )
                })?;
                for record in &records {
                    persist_canonical_historical_recovery_payload_custody(
                        kura.as_ref(),
                        process_generation,
                        key_pair,
                        local_peer,
                        record,
                    )?;
                }
                let _ = persist_preflighted_historical_autonomous_lane_recoveries(
                    kura.as_ref(),
                    &records,
                )?;
                validate_installed_historical_autonomous_lane_recoveries(kura.as_ref(), &records)?;
                recovery.complete();
            }
        }
    };
    if summary != Default::default() {
        iroha_logger::info!(
            recovered = summary.recovered,
            finalized_committed = summary.finalized_committed,
            retained_current = summary.retained_current,
            retained_certified = summary.retained_certified,
            retained_pending_merge = summary.retained_pending_merge,
            retained_historical_recovery = summary.retained_historical_recovery,
            released_strictly_absent = summary.released_strictly_absent,
            released_terminal_loser = summary.released_terminal_loser,
            resumed_retirement = summary.resumed_retirement,
            "reconciled pending Kura lane reservations through the lifecycle owner"
        );
    }
    Ok((pending, PendingCanonicalRecoveryControlV1::Complete))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn service_pending_certified_serve_barrier(
    serve_barrier: Option<super::super::v2_worker::CertifiedServeBarrier>,
    receiver: &Arc<FairV2Ingress>,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &Arc<ConsensusOutputGuard>,
    kura: &Kura,
    key_pair: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
) -> Result<bool, V2RunnerError> {
    let Some(serve_barrier) = serve_barrier else {
        return Ok(false);
    };
    let mut older_predecessor_remains = false;
    let completion_evidence = services
        .certified_serve_predecessor_completion_evidence(
            executor.remaining_completion_capacity() != 0,
            serve_barrier.scheduler_ordinal(),
        )
        .map_err(V2RunnerError::Service)?;
    let predecessor = executor.exact_serve_predecessor_observation(
        Instant::now(),
        serve_barrier.scheduler_ordinal(),
        completion_evidence,
    )?;
    let predecessor_admission = predecessor
        .should_open_predecessor_admission()
        .then(|| {
            services
                .open_certified_serve_predecessor_admission(serve_barrier)
                .map_err(V2RunnerError::Service)
        })
        .transpose()?;
    if let Some(predecessor_admission) = predecessor_admission {
        services
            .drain_exact_serve_runtime_predecessor(executor, serve_barrier.scheduler_ordinal())?;
        let completion_evidence = services
            .certified_serve_predecessor_completion_evidence(
                executor.remaining_completion_capacity() != 0,
                serve_barrier.scheduler_ordinal(),
            )
            .map_err(V2RunnerError::Service)?;
        let predecessor = executor.exact_serve_predecessor_observation(
            Instant::now(),
            serve_barrier.scheduler_ordinal(),
            completion_evidence,
        )?;
        if predecessor.has_runnable_predecessor()
            && services
                .certified_serve_predecessor_capacity_available(serve_barrier)
                .map_err(V2RunnerError::Service)?
        {
            output_guard.close_admission_for_restart();
            return Err(V2RunnerError::Service(
                "completed pending Kura recovery retained a runnable Serve predecessor".to_owned(),
            ));
        }
        let completion_evidence = services
            .certified_serve_predecessor_completion_evidence(
                executor.remaining_completion_capacity() != 0,
                serve_barrier.scheduler_ordinal(),
            )
            .map_err(V2RunnerError::Service)?;
        let predecessor = executor.exact_serve_predecessor_observation(
            Instant::now(),
            serve_barrier.scheduler_ordinal(),
            completion_evidence,
        )?;
        older_predecessor_remains = predecessor.has_runnable_predecessor();
        predecessor_admission
            .finish()
            .map_err(V2RunnerError::Service)?;
    }
    service_certified_serve_barrier_liveness_turn(true, |action| match action {
        CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix => {
            if let Some(timeout_recovery_cut) = executor.timeout_recovery_lifecycle_cut()? {
                services
                    .drain_timeout_recovery_prefix_completion(executor, timeout_recovery_cut)?;
            }
            Ok(())
        }
        CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode
        | CertifiedServeBarrierLivenessAction::Pacemaker => {
            output_guard.close_admission_for_restart();
            Err(V2RunnerError::Service(
                "pending Kura Serve barrier attempted pacemaker work".to_owned(),
            ))
        }
    })?;
    if !older_predecessor_remains {
        services.drain_certified_serve_predecessor_completion(executor)?;
        drain_decided_lane_recovery_ingress(
            receiver,
            executor,
            services,
            lane_work,
            executor.current_tag().view(),
            output_guard.as_ref(),
            kura,
            key_pair,
            block_sync_server,
        )?;
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_pending_active_height(
    mut activated: PendingKuraActivatedProductionLifecycleV1,
    mut active_runner: ProductionLifecycleActiveRunnerBorrowV1,
    mut committed_lane_status_publisher: CommittedLaneStatusPublisher,
    context: &wire::HeightContext,
    context_store: &crate::sumeragi::v2_context_store::V2ContextStore,
    state: &Arc<State>,
    kura: &Arc<Kura>,
    common_config: &iroha_config::parameters::actual::Common,
    receiver: &Arc<FairV2Ingress>,
    lane_relay_rx: &std::sync::mpsc::Receiver<crate::sumeragi::LaneRelayMessage>,
    wake_rx: &std::sync::mpsc::Receiver<()>,
    shutdown_signal: &iroha_futures::supervisor::ShutdownSignal,
    output_guard: &Arc<ConsensusOutputGuard>,
    cleanup_supervisor: &mut V2CleanupSupervisor,
    liveness_watchdog: &mut crate::sumeragi::status::V2LivenessWatchdog,
    block_sync_server: &mut V2BlockSyncServer,
    genesis_account: &AccountId,
    control_queue_capacity: usize,
    round_timeout: Duration,
    retransmit_interval: Duration,
) -> Result<Option<(PreparedPendingKuraSuccessorV1, RetainedMergeSidecars)>, V2RunnerError> {
    let mut next_lane_retransmit = deadline_after(Instant::now(), retransmit_interval);
    loop {
        activated.with_runner_runtime(&mut active_runner, |_executor, _services, lane_work| {
            committed_lane_status_publisher.publish_if_changed(lane_work)
        });
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            activated.into_clean_shutdown(&mut active_runner)?;
            return Ok(None);
        }
        liveness_watchdog.poll(Instant::now());
        let barrier = activated.with_runner_runtime(
            &mut active_runner,
            |executor, services, _lane_work| -> Result<_, V2RunnerError> {
                if executor.has_retained_certified_body_response() {
                    output_guard.close_admission_for_restart();
                    return Err(V2RunnerError::RestartRequired);
                }
                if let Some(scheduler_ordinal) = services
                    .dormant_certified_serve_ingress_scheduler_ordinal()
                    .map_err(V2RunnerError::Service)?
                {
                    let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
                    return Err(V2RunnerError::RestartRequired);
                }
                services
                    .certified_serve_barrier()
                    .map_err(V2RunnerError::Service)
            },
        )?;
        if activated.with_runner_runtime(&mut active_runner, |executor, services, lane_work| {
            service_pending_certified_serve_barrier(
                barrier,
                receiver,
                executor,
                services,
                lane_work,
                output_guard,
                kura.as_ref(),
                &common_config.key_pair,
                block_sync_server,
            )
        })? {
            activated.with_runner_runtime(&mut active_runner, |_executor, _services, lane_work| {
                committed_lane_status_publisher.publish_if_changed(lane_work)
            });
            let _ = wake_rx.recv_timeout(IDLE_POLL);
            continue;
        }

        let Some(certified_serve_producer_episode) = activated.with_runner_runtime(
            &mut active_runner,
            |_executor, services, _lane_work| {
                services
                    .try_begin_certified_serve_producer_episode()
                    .map_err(V2RunnerError::Service)
            },
        )?
        else {
            let _ = wake_rx.recv_timeout(IDLE_POLL);
            continue;
        };

        let ready = activated.with_runner_runtime(
            &mut active_runner,
            |executor, services, lane_work| -> Result<_, V2RunnerError> {
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let _ = services
                    .service_kura_replica_advert_refresh_turn(Instant::now())
                    .map_err(V2RunnerError::Service)?;
                services.drain_completions(executor)?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(executor, services)?;
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_decided_lane_recovery_ingress(
                    receiver,
                    executor,
                    services,
                    lane_work,
                    executor.current_tag().view(),
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server,
                )?;
                drain_lane_relay_ingress(
                    lane_relay_rx,
                    lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(executor, services, lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(lane_work)?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?;
                Ok(executor.ready_to_finish())
            },
        )?;
        activated.with_runner_runtime(&mut active_runner, |_executor, _services, lane_work| {
            committed_lane_status_publisher.publish_if_changed(lane_work)
        });
        if !ready {
            drop(certified_serve_producer_episode);
            let _ = wake_rx.recv_timeout(IDLE_POLL);
            continue;
        }

        let (finalized, lane_work) = activated.into_finalized_rollover(&mut active_runner)?;
        // Ingress is closed before releasing the producer exclusion episode.
        drop(certified_serve_producer_episode);
        let prepared_successor = {
            let (receipt, artifact) = finalized.finality();
            let predecessor = DurableV2PredecessorIdentity::authenticate(artifact, receipt)?;
            let artifact_hash = HashOf::new(artifact);
            let terminal_application =
                ProductionTerminalApplicationWithoutSuccessorActivationProjection {
                    context_id: successor_context_refinement_projection(context.id()),
                    context_height: context.height,
                    receipt_context_id: successor_context_refinement_projection(
                        receipt.context_id(),
                    ),
                    receipt_height: receipt.height(),
                    receipt_block_hash: successor_block_refinement_projection(receipt.block_hash()),
                    receipt_artifact_hash: CanonicalIdentityProjection::from_bytes(
                        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                        IDENTITY_KIND_FINALITY_ARTIFACT,
                        *receipt.artifact_hash().as_ref(),
                    ),
                    artifact_context_id: successor_context_refinement_projection(
                        artifact.context_id(),
                    ),
                    artifact_height: artifact.height,
                    artifact_block_hash: successor_block_refinement_projection(artifact.block_hash),
                    artifact_hash: CanonicalIdentityProjection::from_bytes(
                        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                        IDENTITY_KIND_FINALITY_ARTIFACT,
                        *artifact_hash.as_ref(),
                    ),
                    predecessor: predecessor.refinement_projection(),
                    pending_successor_activation_present: false,
                };
            let Some(checked_application) =
                check_production_terminal_application_transition(terminal_application)
            else {
                return Err(V2RunnerError::SuccessorRefinementRejected);
            };
            let _authorized_application = checked_application.into_projection();
            let activation = PendingSuccessorConstruction::begin(predecessor)?;
            let construction = output_guard
                .begin_fail_stop_operation()
                .ok_or(V2RunnerError::RestartRequired)?;
            let successor =
                build_verified_successor(state.as_ref(), context_store, artifact, receipt)?;
            construction.complete();
            let (next_verified_context, successor_authority, next_lifecycle_storage_authority) =
                successor
                    .into_parts_with_lifecycle_storage_authority(kura.as_ref(), genesis_account)?;
            let next_context = next_verified_context.context().clone();
            let pending_activation = activation.bind(successor_authority)?;
            (
                next_context,
                PreparedPendingKuraSuccessorV1 {
                    verified_context: next_verified_context,
                    lifecycle_storage_authority: next_lifecycle_storage_authority,
                    pending_activation,
                    receipt_height: receipt.height(),
                    receipt_context_id: receipt.context_id(),
                    receipt_block_hash: receipt.block_hash(),
                },
            )
        };
        let (next_context, prepared_successor) = prepared_successor;
        let (post_output, retained_merge_sidecars) = finalized.rollover_outputs(
            &mut active_runner,
            lane_work,
            &next_context,
            control_queue_capacity,
        )?;
        let cleanup_ready = post_output.retire_lifecycle_stores()?;
        let cleanup = cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor);
        if let Some(warning) = cleanup.wal_retirement_warning() {
            iroha_logger::warn!(
                height = prepared_successor.receipt_height,
                context_id = ?prepared_successor.receipt_context_id,
                block_hash = %prepared_successor.receipt_block_hash,
                cleanup_target = PostFinalityCleanupTarget::SafetyWal.as_str(),
                reason = warning,
                "pending Kura lifecycle finalized with retained local WAL cleanup state"
            );
        }
        for warning in cleanup.cleanup().warnings() {
            iroha_logger::warn!(
                height = prepared_successor.receipt_height,
                context_id = ?prepared_successor.receipt_context_id,
                block_hash = %prepared_successor.receipt_block_hash,
                cleanup_target = warning.target().as_str(),
                reason = warning.reason(),
                "pending Kura lifecycle finalized with retained local cleanup state"
            );
        }
        return Ok(Some((prepared_successor, retained_merge_sidecars)));
    }
}

/// Recover one interrupted Kura tip without clocks, then enter the ordinary lifecycle loop.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn run_pending_kura_lifecycle_height(
    config: iroha_config::parameters::actual::Sumeragi,
    common_config: iroha_config::parameters::actual::Common,
    events_sender: crate::EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    provider_ingest_finalized_archive: Option<
        Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
    >,
    reputation_finalized_archive: Option<
        Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
    >,
    network: crate::IrohaNetwork,
    block_rx: Arc<FairV2Ingress>,
    lane_relay_rx: std::sync::mpsc::Receiver<crate::sumeragi::LaneRelayMessage>,
    wake_rx: std::sync::mpsc::Receiver<()>,
    shutdown_signal: iroha_futures::supervisor::ShutdownSignal,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
    consensus_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    verified_context: crate::sumeragi::v2::VerifiedHeightContext,
    context_store: crate::sumeragi::v2_context_store::V2ContextStore,
    signature_policy: BlockSignaturePolicy,
    lifecycle_storage_authority: crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    mut first_height_authenticated_genesis: Option<
        crate::sumeragi::v2_context::AuthenticatedGenesisBodyV1,
    >,
    pending_kura_apply: super::super::v2_recovery::PendingKuraApply,
    pending_successor_activation: Option<PendingSuccessorActivation>,
    staged_genesis_nexus_amx_context: Option<
        crate::sumeragi::v2_context::StagedGenesisNexusAmxContext,
    >,
    _first_height_genesis: Option<SignedBlock>,
    genesis_account: AccountId,
    block_cadence: Duration,
    round_timeout: Duration,
    retransmit_interval: Duration,
    lifecycle_process_generation: Option<AutonomousLifecycleProcessGenerationClaim>,
    reservation_reconciliation_pending: bool,
    _eager_block_sync: bool,
    mut cleanup_supervisor: V2CleanupSupervisor,
    mut liveness_watchdog: crate::sumeragi::status::V2LivenessWatchdog,
    deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    mut retained_merge_sidecars: Option<RetainedMergeSidecars>,
    kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
    mut block_sync_server: Option<V2BlockSyncServer>,
) -> Result<(), V2RunnerError> {
    if pending_successor_activation.is_some()
        || staged_genesis_nexus_amx_context.is_some()
        || !reservation_reconciliation_pending
    {
        return Err(V2RunnerError::SuccessorRefinementRejected);
    }
    cleanup_supervisor.reap_finished();
    if output_guard.restart_required() {
        return Err(V2RunnerError::RestartRequired);
    }
    if shutdown_signal.is_sent() {
        return Ok(());
    }
    let local_peer = common_config.peer.id().clone();
    let context = verified_context.context().clone();
    close_ingress_for_rollover(&ingress_ready, &block_rx);
    block_rx
        .configure_roster_for_context(
            context
                .roster
                .iter()
                .map(|validator| validator.validator.clone()),
            &context.network_id,
            context.da_layout,
        )
        .map_err(ingress_capacity_error)?;
    super::super::status::set_v2_network_ingress(context.id(), context.height, &block_rx);
    let shared_config = config.v2_config(block_cadence, context.mode)?;
    let fingerprints = adapter_fingerprints(&local_peer, &shared_config);
    let control_queue_capacity = usize::try_from(shared_config.limits.control_queue_capacity)?;
    let chunk_queue_capacity = usize::try_from(shared_config.limits.chunk_queue_capacity)?;
    let certified_request_capacity =
        usize::try_from(shared_config.limits.certified_request_capacity)?;
    let effect_work_capacity = usize::try_from(shared_config.limits.effect_work_capacity)?;
    let runtime_queue = runtime_queue_config(&shared_config)?;
    let effect_queue = effect_queue_config(&shared_config)?;
    let serviced_candidate_capacity_geometry = ServicedCandidateCapacityGeometry::new(
        usize::try_from(shared_config.limits.runtime_command_capacity)?,
        effect_work_capacity,
    );
    let lane_work_limits = lane_work_limits(
        &shared_config,
        network.reply_route_source_capacity(),
        consensus_frame_byte_capacity,
        block_sync_frame_byte_capacity,
        retransmit_interval,
        round_timeout,
    )?;
    let local_validator = local_validator_index(&context, &local_peer, config.role)?;
    if block_sync_server.is_none() {
        block_sync_server = Some(V2BlockSyncServer::new(
            context.network_id,
            certified_request_capacity,
        )?);
    }
    let consensus_key_hash: [u8; 32] =
        Hash::new(common_config.key_pair.public_key().encode()).into();
    let storage_root = kura.sumeragi_v2_storage_root();
    let body_store = V2BodyStore::open_with_policy(
        storage_root.join("bodies"),
        context.clone(),
        signature_policy,
    )
    .map_err(|error| {
        V2RunnerError::Effect(super::super::v2_effects::EffectExecutorError::BodyStore(
            error.to_string(),
        ))
    })?
    .into_quarantined_recovered_startup()
    .map_err(|error| {
        V2RunnerError::Effect(super::super::v2_effects::EffectExecutorError::BodyStore(
            error.to_string(),
        ))
    })?;
    let wal_authority = kura
        .mint_safety_wal_directory_authority()
        .map_err(|error| V2RunnerError::Service(error.to_string()))?;
    let recovered = SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(
        kura.as_ref(),
        wal_authority,
        verified_context.clone(),
        local_validator,
        Generation::INITIAL,
        consensus_key_hash,
        fingerprints,
        serviced_candidate_capacity_geometry,
        deferred_admission_ordinals.clone(),
    )?;
    let authenticated = recovered
        .bind_pending_kura_apply(pending_kura_apply)
        .map_err(|(error, _retained)| V2RunnerError::Adapter(error))?
        .authenticate_final_wal_startup_authority()?;
    let factory = authenticated.bind_production_lifecycle_owner_factory_inputs_v1(
        RecoveredLifecycleOwnerFactoryDependencyPermitV1::mint_for_recovered_runner(
            common_config.key_pair.clone(),
            block_cadence,
        ),
        lifecycle_storage_authority,
        Arc::clone(&state),
        Arc::clone(&queue),
        Arc::clone(&kura),
        provider_ingest_finalized_archive.clone(),
        reputation_finalized_archive.clone(),
        events_sender.clone(),
    )?;
    let owner = authenticated.open_production_lifecycle_owner_v1(
        &shared_config,
        network.reply_route_source_capacity(),
        factory,
        body_store,
    )?;
    let (exact_output_service_owner, exact_output_transport_owner) =
        durable_exact_output_handoff_owner_pair();
    let launch_inputs = ProductionLifecycleLaunchInputsV1::new(
        Instant::now(),
        round_timeout,
        runtime_queue,
        effect_queue,
        local_peer.clone(),
        local_validator,
        common_config.key_pair.clone(),
        network.clone(),
        Arc::clone(&state),
        Arc::clone(&kura),
        first_height_authenticated_genesis.take(),
        effect_work_capacity,
        certified_request_capacity,
        chunk_queue_capacity,
        Arc::clone(&output_guard),
        Arc::clone(&block_rx),
        Arc::clone(&kura_replica_advert_refresh),
        exact_output_service_owner,
    );
    let launched = owner.launch(launch_inputs)?;
    let mut setup_runner =
        ProductionLifecyclePreActivationRunnerBorrowV1::mint_for_recovered_runner();
    let mut activation =
        ProductionLifecyclePendingKuraRunnerActivationV1::mint_for_recovered_runner(
            Arc::clone(&ingress_ready),
            Arc::clone(&block_rx),
        );
    let mut pending = launched.install_pending_kura_apply(&mut setup_runner)?;
    pending.with_runner_setup(
        &mut setup_runner,
        |executor, services| -> Result<_, V2RunnerError> {
            let _ = reconcile_executor_locked_body(executor, services)?;
            Ok(())
        },
    )?;
    let recovery_deadline = PendingTipRecoveryDeadline::new(Instant::now(), round_timeout)?;
    let (mut recovery_attempts, mut recovery_stage) = (0_u64, None);
    loop {
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            pending.into_clean_shutdown(activation)?;
            return Ok(());
        }
        let now = Instant::now();
        if recovery_deadline.expired(now) {
            pending.with_runner_setup(
                &mut setup_runner,
                |executor, services| -> Result<_, V2RunnerError> {
                    executor.record_pending_tip_recovery_deadline_exceeded(services)?;
                    Ok(())
                },
            )?;
            return Err(pending_tip_recovery_deadline_error(
                output_guard.as_ref(),
                recovery_deadline.timeout,
                recovery_attempts,
                recovery_stage,
            ));
        }
        match pending.drive_apply_recovery_turn(&mut setup_runner, control_queue_capacity)? {
            ProductionPendingKuraApplyRecoveryProgressV1::Completed { attempts } => {
                recovery_attempts = attempts;
                break;
            }
            ProductionPendingKuraApplyRecoveryProgressV1::Advanced {
                attempts, stage, ..
            } => {
                recovery_attempts = attempts;
                recovery_stage = Some(stage);
            }
            ProductionPendingKuraApplyRecoveryProgressV1::Waiting { attempts, stage } => {
                recovery_attempts = attempts;
                recovery_stage = Some(stage);
                let remaining = recovery_deadline.remaining(Instant::now());
                if !remaining.is_zero() {
                    let _ = wake_rx.recv_timeout(remaining.min(IDLE_POLL));
                }
            }
        }
    }
    iroha_logger::info!(
        height = context.height,
        elapsed = ?recovery_deadline.elapsed(Instant::now()),
        attempts = recovery_attempts,
        "finished lifecycle-owned interrupted-tip local Apply recovery"
    );

    let (pending, control) = reconcile_pending_lane_startup(
        pending,
        &mut setup_runner,
        &mut activation,
        &context,
        &verified_context,
        &state,
        &queue,
        &kura,
        &local_peer,
        &common_config.key_pair,
        &output_guard,
        lane_work_limits,
        retransmit_interval,
        control_queue_capacity,
        &wake_rx,
        &shutdown_signal,
        lifecycle_process_generation.as_ref(),
    )?;
    if control == PendingCanonicalRecoveryControlV1::Shutdown {
        pending.into_clean_shutdown(activation)?;
        return Ok(());
    }
    let mut prepared = pending.prepare_lane_recovery(
        &mut setup_runner,
        &queue,
        |expected, _executor, _services| {
            V2LaneWorkAdapter::new_with_output_guard_and_transport(
                &verified_context,
                local_peer.clone(),
                common_config.key_pair.clone(),
                config.role == NodeRole::Validator,
                Arc::clone(&state),
                Arc::clone(&kura),
                lane_work_limits,
                None,
                Some(expected),
                Arc::clone(&output_guard),
                exact_output_transport_owner,
                retained_merge_sidecars.take(),
                lifecycle_process_generation.clone(),
            )
            .map_err(V2RunnerError::from)
        },
    )?;
    let mut committed_lane_status_publisher = CommittedLaneStatusPublisher::default();
    prepared.with_runner_setup(
        &mut setup_runner,
        |lane_work, executor, services| -> Result<_, V2RunnerError> {
            if let Some(scheduler_ordinal) = services
                .dormant_certified_serve_ingress_scheduler_ordinal()
                .map_err(V2RunnerError::Service)?
            {
                let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
                return Err(V2RunnerError::RestartRequired);
            }
            let directive = reconcile_executor_locked_body(executor, services)?;
            lane_work.retain_merge_sidecars_for_global_view(
                directive.tag().view(),
                directive.locked_subject(),
                directive.decided_subject(),
            )?;
            if directive.decided_subject().is_none()
                && let Some((locked_round, locked)) = directive.locked_body()
            {
                let _ = lane_work.mark_global_body_locked(locked_round, locked)?;
            }
            dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?;
            committed_lane_status_publisher.publish_if_changed(lane_work);
            Ok(())
        },
    )?;
    let activated = prepared.activate_no_clock(activation)?;
    let active_runner = ProductionLifecycleActiveRunnerBorrowV1::mint_for_recovered_runner();
    let Some((successor, retained_merge_sidecars)) = run_pending_active_height(
        activated,
        active_runner,
        committed_lane_status_publisher,
        &context,
        &context_store,
        &state,
        &kura,
        &common_config,
        &block_rx,
        &lane_relay_rx,
        &wake_rx,
        &shutdown_signal,
        &output_guard,
        &mut cleanup_supervisor,
        &mut liveness_watchdog,
        block_sync_server
            .as_mut()
            .expect("pending Kura historical Serve server initialized above"),
        &genesis_account,
        control_queue_capacity,
        round_timeout,
        retransmit_interval,
    )?
    else {
        return Ok(());
    };

    super::lifecycle_run_inner::run_non_pending_lifecycle_loop(
        config,
        common_config,
        events_sender,
        state,
        queue,
        kura,
        provider_ingest_finalized_archive,
        reputation_finalized_archive,
        network,
        block_rx,
        lane_relay_rx,
        wake_rx,
        shutdown_signal,
        ingress_ready,
        output_guard,
        consensus_frame_byte_capacity,
        block_sync_frame_byte_capacity,
        successor.verified_context,
        context_store,
        BlockSignaturePolicy::RotatingLeader,
        successor.lifecycle_storage_authority,
        None,
        Some(successor.pending_activation),
        None,
        None,
        genesis_account,
        block_cadence,
        round_timeout,
        retransmit_interval,
        lifecycle_process_generation,
        false,
        true,
        cleanup_supervisor,
        liveness_watchdog,
        deferred_admission_ordinals,
        Some(retained_merge_sidecars),
        kura_replica_advert_refresh,
        block_sync_server,
    )
}
