//! Non-PendingKura process-height ownership for the production lifecycle runner.

use super::*;
use crate::sumeragi::v2_lifecycle_coordinator::{
    ActivatedProductionLifecycleV1, LaunchedProductionLifecycleV1,
    LaunchedRecoveredCompleteTipSuccessorLifecycleV1, ProductionLifecycleFinalizationOutcomeV1,
    ProductionLifecycleLaunchInputsV1, ProductionLifecyclePreActivationErrorV1,
    ProductionLifecyclePreparedLocalProposalStateV1, ProductionLifecycleShutdownErrorV1,
};

/// One-shot ownership of an authenticated successor's activation handoff.
///
/// Construction failure simply drops this token, leaving the predecessor's
/// `Running` work stage visible. The outer runner failure guard then closes
/// output and requires restart; only the lifecycle activation authority can
/// claim activation.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(super) enum PendingSuccessorActivation {
    /// Uninterrupted rollover whose published Applied predecessor owns the
    /// Running handoff.
    Applied {
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    },
    /// Process restart after recovery authenticated an exact complete durable
    /// tip; the process-local predecessor registry was intentionally cleared.
    RecoveredCompleteTip {
        authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    },
    /// First executable height derived from an authenticated audited snapshot.
    /// This carries no historical CommitQC or Kura finality receipt.
    SnapshotBootstrap {
        authority: SnapshotSuccessorActivationAuthority,
    },
}

impl PendingSuccessorActivation {
    /// Authenticate one recovered successor and retain its exact activation authority.
    pub(super) fn recovered(
        authority: RecoveredSuccessorActivationAuthority,
        local_signer: &KeyPair,
    ) -> Result<Self, V2RunnerError> {
        let (transition, authority_kind, status_height) = match &authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => (
                SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
                SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
                authority.predecessor().height(),
            ),
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => (
                SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
                SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
                authority.snapshot_anchor_height(),
            ),
        };
        let published_height = super::super::status::v2_status().map_or(0, |status| status.height);
        let lifecycle = ProductionSuccessorStartupLifecycleProjection {
            transition_kind: transition,
            authority_kind,
            status_height,
            stage_before: SUCCESSOR_STAGE_NONE,
            stage_after: SUCCESSOR_STAGE_NONE,
            published_height_before: published_height,
            published_height_after: published_height,
            restart_required_before: false,
            restart_required_after: false,
        };
        let Some(checked_lifecycle) =
            check_production_successor_startup_lifecycle_transition(lifecycle)
        else {
            return Err(V2RunnerError::SuccessorRefinementRejected);
        };
        let _authorized_lifecycle = checked_lifecycle.into_projection();
        Ok(match authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => {
                let expected_predecessor = authority.predecessor();
                let retired = authority
                    .into_canonical_predecessor_storage(local_signer)?
                    .retire()?;
                if retired.predecessor() != expected_predecessor {
                    return Err(V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                        expected: expected_predecessor,
                        actual: retired.predecessor(),
                    });
                }
                Self::RecoveredCompleteTip { authority: retired }
            }
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => {
                Self::SnapshotBootstrap { authority }
            }
        })
    }

    /// Reauthenticate retained restart storage before constructing live H+1 services.
    pub(super) fn preflight_recovered_startup(&self) -> Result<(), V2RunnerError> {
        match self {
            Self::RecoveredCompleteTip { authority }
                if !authority.authorizes_retained_successor() =>
            {
                Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                    predecessor: authority.predecessor(),
                })
            }
            Self::Applied { .. }
            | Self::RecoveredCompleteTip { .. }
            | Self::SnapshotBootstrap { .. } => Ok(()),
        }
    }

    /// Bind the prepared status to its retained restart authority before ingress opens.
    #[cfg(test)]
    pub(super) fn preflight_ingress_open(
        &self,
        successor: &wire::SumeragiV2Status,
    ) -> Result<(), V2RunnerError> {
        match self {
            Self::RecoveredCompleteTip { authority }
                if !authority.authorizes_successor_status(successor) =>
            {
                Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid {
                    predecessor: authority.predecessor(),
                })
            }
            Self::Applied { .. }
            | Self::RecoveredCompleteTip { .. }
            | Self::SnapshotBootstrap { .. } => Ok(()),
        }
    }

    /// Publish one authenticated successor status through its exact retained authority.
    #[cfg(test)]
    pub(super) fn publish(self, successor: wire::SumeragiV2Status) -> Result<(), V2RunnerError> {
        match self {
            Self::Applied {
                expected_predecessor,
                authority,
            } => {
                super::super::status::activate_v2_successor_height(
                    expected_predecessor,
                    authority,
                    successor,
                )?;
            }
            Self::RecoveredCompleteTip { authority } => {
                super::super::status::activate_recovered_complete_tip_v2_height(
                    authority, successor,
                )?;
            }
            Self::SnapshotBootstrap { authority } => {
                super::super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CanonicalRecoveryControlV1 {
    Complete,
    Shutdown,
}

struct FinalizedLifecycleHeightV1 {
    verified_context: crate::sumeragi::v2::VerifiedHeightContext,
    lifecycle_storage_authority: crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    pending_successor_activation: PendingSuccessorActivation,
    retained_merge_sidecars: RetainedMergeSidecars,
    eager_block_sync: bool,
}

struct PreparedLifecycleSuccessorV1 {
    verified_context: crate::sumeragi::v2::VerifiedHeightContext,
    lifecycle_storage_authority: crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    pending_activation: PendingSuccessorActivation,
    receipt_height: u64,
    receipt_context_id: wire::HeightContextId,
    receipt_block_hash: HashOf<BlockHeader>,
}

/// Unpublished lifecycle height together with its sole runner activation.
///
/// CompleteTip retains the retired predecessor inside its sealed launched
/// wrapper until activation or orderly shutdown consumes the whole join.
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum ProductionLifecyclePreActivationHeightV1 {
    Ordinary {
        launched: Box<LaunchedProductionLifecycleV1>,
        activation: ProductionLifecycleRunnerActivationV1,
    },
    CompleteTip {
        launched: Box<LaunchedRecoveredCompleteTipSuccessorLifecycleV1>,
        activation: ProductionLifecycleCompleteTipRunnerActivationV1,
    },
}

impl ProductionLifecyclePreActivationHeightV1 {
    fn with_runner_setup<R>(
        &mut self,
        runner: &mut ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, V2RunnerError>,
    ) -> Result<R, V2RunnerError> {
        let mut operation = Some(operation);
        match self {
            Self::Ordinary { launched, .. } => {
                launched.with_runner_setup(runner, |executor, services| {
                    operation
                        .take()
                        .expect("one preactivation branch consumes the setup operation")(
                        executor, services,
                    )
                })
            }
            Self::CompleteTip { launched, .. } => {
                launched.with_runner_setup(runner, |executor, services| {
                    operation
                        .take()
                        .expect("one CompleteTip branch consumes the setup operation")(
                        executor, services,
                    )
                })
            }
        }
    }

    fn with_canonical_body_recovery_ingress<R>(
        &mut self,
        runner: &mut ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> Result<R, V2RunnerError>,
    ) -> Result<R, V2RunnerError> {
        let mut operation = Some(operation);
        match self {
            Self::Ordinary {
                launched,
                activation,
            } => launched.with_canonical_body_recovery_ingress(
                runner,
                activation,
                |ingress, executor, services| {
                    operation
                        .take()
                        .expect("one preactivation branch consumes the recovery operation")(
                        ingress, executor, services,
                    )
                },
            ),
            Self::CompleteTip {
                launched,
                activation,
            } => launched.with_canonical_body_recovery_ingress(
                runner,
                activation,
                |ingress, executor, services| {
                    operation
                        .take()
                        .expect("one CompleteTip branch consumes the recovery operation")(
                        ingress, executor, services,
                    )
                },
            ),
        }
    }

    fn initialize_recovered_local_proposal(
        &mut self,
        runner: ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<
        (
            LocalProposalDirective,
            ProductionLifecyclePreparedLocalProposalStateV1,
        ),
        ProductionLifecyclePreActivationErrorV1,
    > {
        match self {
            Self::Ordinary { launched, .. } => launched.initialize_recovered_local_proposal(runner),
            Self::CompleteTip { launched, .. } => {
                launched.initialize_recovered_local_proposal(runner)
            }
        }
    }

    fn activate(
        self,
        now: Instant,
        local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,
    ) -> Result<
        ActivatedProductionLifecycleV1,
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleActivationErrorV1,
    > {
        match self {
            Self::Ordinary {
                launched,
                activation,
            } => launched.activate(now, activation, local_proposal),
            Self::CompleteTip {
                launched,
                activation,
            } => launched.activate(now, activation, local_proposal),
        }
    }

    fn into_clean_shutdown(self) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        match self {
            Self::Ordinary {
                launched,
                activation,
            } => launched.into_clean_shutdown(activation),
            Self::CompleteTip {
                launched,
                activation,
            } => launched.into_clean_shutdown(activation),
        }
    }
}

#[inline(never)]
fn launch_ordinary_lifecycle_height(
    owner: crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    inputs: ProductionLifecycleLaunchInputsV1,
    activation: ProductionLifecycleRunnerActivationV1,
) -> Result<ProductionLifecyclePreActivationHeightV1, V2RunnerError> {
    Ok(ProductionLifecyclePreActivationHeightV1::Ordinary {
        launched: owner.launch(inputs)?,
        activation,
    })
}

#[inline(never)]
fn launch_recovered_complete_tip_lifecycle_height(
    owner: crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    inputs: ProductionLifecycleLaunchInputsV1,
    authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    ingress_ready: &Arc<AtomicBool>,
    block_ingress: &Arc<FairV2Ingress>,
) -> Result<ProductionLifecyclePreActivationHeightV1, V2RunnerError> {
    let predecessor = authority.predecessor();
    let bound = authority
        .bind_successor_owner(owner)
        .map_err(|_| V2RunnerError::CompleteTipSuccessorAuthorityInvalid { predecessor })?;
    Ok(ProductionLifecyclePreActivationHeightV1::CompleteTip {
        launched: bound.launch(inputs)?,
        activation: ProductionLifecycleCompleteTipRunnerActivationV1::mint_for_recovered_runner(
            Arc::clone(ingress_ready),
            Arc::clone(block_ingress),
        ),
    })
}

#[inline(never)]
fn launch_non_pending_lifecycle_height(
    owner: crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    inputs: ProductionLifecycleLaunchInputsV1,
    activation: Option<PendingSuccessorActivation>,
    ingress_ready: &Arc<AtomicBool>,
    block_ingress: &Arc<FairV2Ingress>,
) -> Result<ProductionLifecyclePreActivationHeightV1, V2RunnerError> {
    match activation {
        None => launch_ordinary_lifecycle_height(
            owner,
            inputs,
            ProductionLifecycleRunnerActivationV1::current_height(
                Arc::clone(ingress_ready),
                Arc::clone(block_ingress),
            ),
        ),
        Some(PendingSuccessorActivation::Applied {
            expected_predecessor,
            authority,
        }) => launch_ordinary_lifecycle_height(
            owner,
            inputs,
            ProductionLifecycleRunnerActivationV1::applied(
                Arc::clone(ingress_ready),
                Arc::clone(block_ingress),
                expected_predecessor,
                authority,
            ),
        ),
        Some(PendingSuccessorActivation::SnapshotBootstrap { authority }) => {
            launch_ordinary_lifecycle_height(
                owner,
                inputs,
                ProductionLifecycleRunnerActivationV1::snapshot_bootstrap(
                    Arc::clone(ingress_ready),
                    Arc::clone(block_ingress),
                    authority,
                ),
            )
        }
        Some(PendingSuccessorActivation::RecoveredCompleteTip { authority }) => {
            launch_recovered_complete_tip_lifecycle_height(
                owner,
                inputs,
                authority,
                ingress_ready,
                block_ingress,
            )
        }
    }
}

#[cfg(test)]
/// Launch and cleanly retire one ordinary or CompleteTip lifecycle fixture.
pub(in crate::sumeragi) fn launch_non_pending_lifecycle_height_and_shutdown_for_test(
    owner: crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    inputs: ProductionLifecycleLaunchInputsV1,
    complete_tip: Option<RetiredRecoveredCompleteTipActivationAuthorityV1>,
    ingress_ready: &Arc<AtomicBool>,
    block_ingress: &Arc<FairV2Ingress>,
) -> Result<wire::HeightContextId, V2RunnerError> {
    let activation = complete_tip
        .map(|authority| PendingSuccessorActivation::RecoveredCompleteTip { authority });
    let mut preactivation = launch_non_pending_lifecycle_height(
        owner,
        inputs,
        activation,
        ingress_ready,
        block_ingress,
    )?;
    let mut setup_runner = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    let context_id = preactivation.with_runner_setup(&mut setup_runner, |executor, services| {
        if !services.matches_lifecycle_executor_output_guard(executor) {
            return Err(V2RunnerError::RestartRequired);
        }
        Ok(executor.context().id())
    })?;
    preactivation.into_clean_shutdown()?;
    Ok(context_id)
}

#[cfg(test)]
/// Launch and activate one ordinary or CompleteTip lifecycle fixture.
///
/// The returned owner has crossed the real runner publication boundary. Tests
/// must consume it through finalized rollover or orderly active shutdown.
pub(in crate::sumeragi) fn launch_non_pending_lifecycle_height_and_activate_for_test(
    owner: crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
    inputs: ProductionLifecycleLaunchInputsV1,
    complete_tip: Option<RetiredRecoveredCompleteTipActivationAuthorityV1>,
    ingress_ready: &Arc<AtomicBool>,
    block_ingress: &Arc<FairV2Ingress>,
) -> Result<(ActivatedProductionLifecycleV1, wire::HeightContextId), V2RunnerError> {
    let activation = complete_tip
        .map(|authority| PendingSuccessorActivation::RecoveredCompleteTip { authority });
    let mut preactivation = launch_non_pending_lifecycle_height(
        owner,
        inputs,
        activation,
        ingress_ready,
        block_ingress,
    )?;
    let mut setup_runner = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
    let context_id = preactivation.with_runner_setup(&mut setup_runner, |executor, services| {
        if !services.matches_lifecycle_executor_output_guard(executor) {
            return Err(V2RunnerError::RestartRequired);
        }
        Ok(executor.context().id())
    })?;
    let (_directive, local_proposal) =
        preactivation.initialize_recovered_local_proposal(setup_runner)?;
    let activated = preactivation.activate(Instant::now(), local_proposal)?;
    Ok((activated, context_id))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn recover_canonical_bodies_before_activation(
    preactivation: &mut ProductionLifecyclePreActivationHeightV1,
    setup_runner: &mut ProductionLifecyclePreActivationRunnerBorrowV1,
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
) -> Result<CanonicalRecoveryControlV1, V2RunnerError> {
    if needs.is_empty() {
        return Err(V2RunnerError::Service(
            "canonical executed-body recovery requested an empty body set".to_owned(),
        ));
    }
    preactivation.with_canonical_body_recovery_ingress(
        setup_runner,
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
                while canonical_recovery_source_work_remains(
                    body_recovery.has_pending(),
                    body_recovery.effect_count(),
                ) {
                    if output_guard.restart_required() {
                        return Err(V2RunnerError::RestartRequired);
                    }
                    if shutdown_signal.is_sent() {
                        return Ok(CanonicalRecoveryControlV1::Shutdown);
                    }
                    let now = Instant::now();
                    if body_recovery.has_pending() && now >= next_retry {
                        let request_queued = body_recovery.service_next()?;
                        let serviced_at = Instant::now();
                        refresh_canonical_recovery_retry_deadline(
                            &mut next_retry,
                            serviced_at,
                            retransmit_interval,
                            request_queued,
                        );
                    }
                    let ingress = if body_recovery.has_pending() {
                        drain_canonical_executed_block_recovery_ingress(
                            aperture.ingress(),
                            &mut body_recovery,
                            control_queue_capacity,
                        )?
                    } else {
                        CanonicalRecoveryIngressDrain::default()
                    };
                    if ingress.exact_response_progress && body_recovery.has_pending() {
                        let request_queued = body_recovery.service_next()?;
                        let serviced_at = Instant::now();
                        refresh_canonical_recovery_retry_deadline_after_progress(
                            &mut next_retry,
                            serviced_at,
                            retransmit_interval,
                            request_queued,
                        );
                    }
                    let dispatch = dispatch_canonical_executed_block_recovery_effects(
                        &mut body_recovery,
                        services,
                        control_queue_capacity,
                    )?;
                    if dispatch.request_dispatched {
                        refresh_canonical_recovery_retry_deadline(
                            &mut next_retry,
                            Instant::now(),
                            retransmit_interval,
                            true,
                        );
                    }
                    if canonical_recovery_source_work_remains(
                        body_recovery.has_pending(),
                        body_recovery.effect_count(),
                    ) && ingress.drained == 0
                        && dispatch.handled == 0
                    {
                        let wait = canonical_recovery_idle_wait(next_retry, Instant::now());
                        let _ = wake_rx.recv_timeout(wait);
                    }
                }
            }
            Ok(CanonicalRecoveryControlV1::Complete)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn service_retained_certified_response(
    receiver: &Arc<FairV2Ingress>,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &Arc<ConsensusOutputGuard>,
    kura: &Kura,
    key_pair: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    npos_vrf: &mut V2NposVrfLifecycle,
) -> Result<bool, V2RunnerError> {
    if !executor.has_retained_certified_body_response() {
        return Ok(false);
    }
    let target_ordinal = executor
        .retained_certified_body_response_scheduler_ordinal()?
        .ok_or_else(|| {
            V2RunnerError::Service(
                "retained certified body response lost its exact scheduler position".to_owned(),
            )
        })?;
    if executor
        .older_runtime_lifecycle_predates_retained_response(Instant::now(), target_ordinal)?
    {
        advance_executor_once_before_exact_serve(receiver, executor, services)?;
    }
    if let Some(timeout_recovery_cut) = executor.timeout_recovery_lifecycle_cut()? {
        services.drain_timeout_recovery_prefix_completion(executor, timeout_recovery_cut)?;
    }
    let response_backpressured = match executor.retry_retained_certified_body_response(services) {
        Ok(_) => false,
        Err(EffectTransportError::Backpressure) => true,
        Err(EffectTransportError::FailClosed(reason)) => {
            return Err(V2RunnerError::Service(reason));
        }
        Err(error) => {
            iroha_logger::debug!(%error, "rejected retained certified body response");
            false
        }
    };
    if response_backpressured {
        if executor.retained_response_may_admit_certified_fence_escape() {
            drain_v2_ingress(
                receiver,
                executor,
                services,
                lane_work,
                output_guard,
                kura,
                key_pair,
                block_sync_server,
                block_sync,
                block_sync_request,
                npos_vrf,
                V2IngressDrainMode::CertifiedFenceEscape,
                1,
            )?;
        }
        drain_v2_ingress(
            receiver,
            executor,
            services,
            lane_work,
            output_guard,
            kura,
            key_pair,
            block_sync_server,
            block_sync,
            block_sync_request,
            npos_vrf,
            V2IngressDrainMode::TimeoutVoteEpisode,
            1,
        )?;
        executor.reconcile_retained_response_certified_fence_escape_phase();
        advance_pacemaker_once(receiver, executor, services)?;
        executor.reconcile_retained_response_certified_fence_escape_phase();
    }
    Ok(true)
}

/// Consume one ready lifecycle height through exact output handoff, store
/// retirement, and worker cleanup around a caller-authenticated successor.
pub(in crate::sumeragi) fn finalize_lifecycle_height<T>(
    activated: ActivatedProductionLifecycleV1,
    active_runner: &mut ProductionLifecycleActiveRunnerBorrowV1,
    mut lane_work: V2LaneWorkAdapter,
    control_queue_capacity: usize,
    cleanup_supervisor: &mut V2CleanupSupervisor,
    prepare_successor: impl FnOnce(
        &KuraV2CommitReceipt,
        &wire::finality::V2FinalityArtifact,
        &mut V2LaneWorkAdapter,
    ) -> Result<(wire::HeightContext, T), V2RunnerError>,
) -> Result<
    (
        T,
        RetainedMergeSidecars,
        ProductionLifecycleFinalizationOutcomeV1,
    ),
    V2RunnerError,
> {
    let finalized = activated.into_finalized_rollover(active_runner)?;
    let (next_context, prepared_successor) = {
        let (receipt, artifact) = finalized.finality();
        prepare_successor(receipt, artifact, &mut lane_work)?
    };
    let (post_output, retained_merge_sidecars) = finalized.rollover_outputs(
        active_runner,
        lane_work,
        &next_context,
        control_queue_capacity,
    )?;
    let cleanup_ready = post_output.retire_lifecycle_stores()?;
    let cleanup = cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor);
    Ok((prepared_successor, retained_merge_sidecars, cleanup))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_lifecycle_active_height(
    mut activated: ActivatedProductionLifecycleV1,
    mut active_runner: ProductionLifecycleActiveRunnerBorrowV1,
    mut lane_work: V2LaneWorkAdapter,
    mut committed_lane_status_publisher: CommittedLaneStatusPublisher,
    context: &wire::HeightContext,
    context_store: &crate::sumeragi::v2_context_store::V2ContextStore,
    state: &Arc<State>,
    queue: &Arc<Queue>,
    kura: &Arc<Kura>,
    common_config: &iroha_config::parameters::actual::Common,
    events_sender: &crate::EventsSender,
    receiver: &Arc<FairV2Ingress>,
    lane_relay_rx: &std::sync::mpsc::Receiver<crate::sumeragi::LaneRelayMessage>,
    pending_queue_plan_admission_dirty: &Arc<AtomicBool>,
    wake_rx: &std::sync::mpsc::Receiver<()>,
    shutdown_signal: &iroha_futures::supervisor::ShutdownSignal,
    output_guard: &Arc<ConsensusOutputGuard>,
    cleanup_supervisor: &mut V2CleanupSupervisor,
    liveness_watchdog: &mut crate::sumeragi::status::V2LivenessWatchdog,
    npos_vrf: &mut V2NposVrfLifecycle,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_server: &mut V2BlockSyncServer,
    eager_block_sync: &mut bool,
    candidate_limits: CandidateLimits,
    local_validator: Option<wire::ValidatorIndex>,
    control_queue_capacity: usize,
    body_queue_capacity: usize,
    height_started_at: Instant,
    block_cadence: Duration,
    round_timeout: Duration,
    retransmit_interval: Duration,
    first_height_genesis: Option<&SignedBlock>,
    genesis_account: &AccountId,
) -> Result<Option<FinalizedLifecycleHeightV1>, V2RunnerError> {
    let mut next_block_sync_attempt =
        initial_block_sync_deadline(height_started_at, round_timeout, *eager_block_sync);
    let mut next_lane_retransmit = deadline_after(height_started_at, retransmit_interval);
    let mut next_npos_vrf_retransmit = deadline_after(height_started_at, retransmit_interval);
    let mut block_sync_request = None;
    let mut admitted_discovered_commit_qc = false;
    let mut producer_claim = LifecycleProducerClaimDispositionV1::initial();

    loop {
        committed_lane_status_publisher.publish_if_changed(&lane_work);
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            activated.into_clean_shutdown(&mut active_runner)?;
            return Ok(None);
        }
        liveness_watchdog.poll(Instant::now());

        let retained_response = activated.with_runner_runtime(
            &mut active_runner,
            |_owner, executor, services, _local_proposal| {
                let now = Instant::now();
                if now >= next_npos_vrf_retransmit {
                    broadcast_npos_vrf_messages(
                        npos_vrf.retransmission(),
                        output_guard.as_ref(),
                        services,
                    )?;
                    next_npos_vrf_retransmit = deadline_after(now, retransmit_interval);
                }
                let retained_response = service_retained_certified_response(
                    receiver,
                    executor,
                    services,
                    &mut lane_work,
                    output_guard,
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server,
                    block_sync,
                    &mut block_sync_request,
                    npos_vrf,
                )?;
                Ok::<_, V2RunnerError>(retained_response)
            },
        )?;
        if retained_response {
            committed_lane_status_publisher.publish_if_changed(&lane_work);
            let _ = wake_rx.recv_timeout(IDLE_POLL);
            continue;
        }

        let discovery_was_outstanding = activated.with_runner_runtime(
            &mut active_runner,
            |_owner, executor, services, local_proposal| {
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let advert_refresh = services
                    .service_kura_replica_advert_refresh_turn(Instant::now())
                    .map_err(V2RunnerError::Service)?;
                if advert_refresh.fanout_attempted {
                    iroha_logger::debug!(
                        height = context.height,
                        probes = advert_refresh.probes,
                        retained_source = advert_refresh.retained_source,
                        scan_active = advert_refresh.scan_active,
                        "advanced bounded Kura replica-advert refresh"
                    );
                }
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(executor, services)?;
                local_proposal
                    .state
                    .reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drive_merge_sidecar_recovery(executor, services, &mut lane_work)?;
                services
                    .replay_buffered_chunks(executor)
                    .map_err(V2RunnerError::Service)?;
                while let Some(rejection) = services.take_validation_rejection() {
                    let current = executor.local_proposal_directive()?;
                    let expected_round = round_for_tag(context, current.tag())?;
                    match local_proposal.state.handle_validation_rejection(
                        LocalProposalOwner::from(current),
                        expected_round,
                        rejection.round(),
                        rejection.subject(),
                    ) {
                        LocalValidationDisposition::Ignored => {}
                        LocalValidationDisposition::FatalNonEmpty => {
                            return Err(V2RunnerError::LocalNonEmptyRetryRejected(
                                rejection.reason().to_owned(),
                            ));
                        }
                        LocalValidationDisposition::RetryNonEmpty => {
                            iroha_logger::warn!(
                                reason = rejection.reason(),
                                "local Sumeragi v2 candidate rejected; retrying with non-empty work only"
                            );
                        }
                    }
                }
                if directive.decided_subject().is_none() {
                    drive_block_sync(
                        Instant::now(),
                        &mut next_block_sync_attempt,
                        retransmit_interval,
                        &mut block_sync_request,
                        block_sync,
                        &common_config.key_pair,
                        output_guard.as_ref(),
                        services,
                    )?;
                }
                Ok::<_, V2RunnerError>(block_sync_request.is_some())
            },
        )?;

        let drain_disposition = drain_lifecycle_v2_ingress(
            &mut activated,
            &mut active_runner,
            receiver,
            &mut lane_work,
            kura.as_ref(),
            &common_config.key_pair,
            block_sync_server,
            block_sync,
            &mut block_sync_request,
            npos_vrf,
            body_queue_capacity,
            producer_claim,
        )?;
        producer_claim = drain_disposition.producer_claim();
        if discovery_was_outstanding && block_sync_request.is_none() {
            admitted_discovered_commit_qc = true;
        }
        let retained_response = activated.with_runner_runtime(
            &mut active_runner,
            |_owner, executor, _services, _local_proposal| {
                executor.has_retained_certified_body_response()
            },
        );
        if drain_disposition.requires_yield() || retained_response {
            committed_lane_status_publisher.publish_if_changed(&lane_work);
            let _ = wake_rx.recv_timeout(IDLE_POLL);
            continue;
        }

        let ready_to_finish = activated.with_runner_runtime(
            &mut active_runner,
            |_owner, executor, services, local_proposal| {
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(executor, services)?;
                local_proposal
                    .state
                    .reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_lane_relay_ingress(
                    lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(executor, services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(&mut lane_work)?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(&mut lane_work, services, control_queue_capacity)?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    services,
                    control_queue_capacity,
                )?;
                advance_executor(receiver, executor, services, control_queue_capacity)?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(executor, services)?;
                local_proposal
                    .state
                    .reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                if directive.decided_subject().is_none()
                    && let Some((locked_round, locked)) = directive.locked_body()
                {
                    let lock_outcome = lane_work.mark_global_body_locked(locked_round, locked)?;
                    if lock_outcome == GlobalBodyLockOutcome::Inserted && local_validator.is_some()
                    {
                        services
                            .request_locked_candidate(executor.current_tag(), locked_round, locked)
                            .map_err(V2RunnerError::Service)?;
                    }
                }
                while let Some(prepared) = services.take_prepared_candidate() {
                    let current = executor.local_proposal_directive()?;
                    if let Some(events) = local_proposal.state.take_prepared_events(
                        LocalProposalOwner::from(current),
                        prepared.tag(),
                        prepared.subject(),
                    ) {
                        let Some(_permit) = output_guard.acquire() else {
                            return Err(V2RunnerError::RestartRequired);
                        };
                        for event in events {
                            let _ = events_sender.send(EventBox::Pipeline(event));
                        }
                    }
                }
                services
                    .replay_buffered_chunks(executor)
                    .map_err(V2RunnerError::Service)?;
                Ok::<_, V2RunnerError>(executor.ready_to_finish())
            },
        )?;
        committed_lane_status_publisher.publish_if_changed(&lane_work);

        if pending_queue_plan_admission_dirty.swap(false, Ordering::AcqRel) {
            let active_view = activated.with_runner_runtime(
                &mut active_runner,
                |_owner, executor, _services, _local_proposal| {
                    executor
                        .local_proposal_directive()
                        .map(|directive| directive.tag().view())
                },
            )?;
            if !lane_work.refresh_pending_queue_plan_admission_handoffs(active_view)? {
                pending_queue_plan_admission_dirty.store(true, Ordering::Release);
            }
        }

        let producer_turn =
            match activated.claim_producer_turn_for_local_proposal(&mut active_runner) {
                Ok(claimed) => claimed,
                Err(error) => {
                    iroha_logger::error!(?error, "Sumeragi v2 ProducerTurn claim failed closed");
                    output_guard.close_admission_for_restart();
                    return Err(V2RunnerError::RestartRequired);
                }
            };
        if !ready_to_finish || producer_turn.is_some() {
            let scheduled = activated.with_runner_runtime(
                &mut active_runner,
                |_owner, executor, services, local_proposal| {
                    schedule_local_proposal(
                        candidate_limits,
                        context,
                        local_validator,
                        &common_config.key_pair,
                        output_guard.as_ref(),
                        state.as_ref(),
                        queue,
                        kura.as_ref(),
                        first_height_genesis,
                        height_started_at,
                        block_cadence,
                        &mut local_proposal.state,
                        executor,
                        services,
                        &mut lane_work,
                        npos_vrf,
                        retransmit_interval,
                    )?;
                    dispatch_lane_work_effects(&mut lane_work, services, control_queue_capacity)
                },
            );
            if let Err(error) = scheduled {
                output_guard.close_admission_for_restart();
                drop(producer_turn);
                return Err(error);
            }
        }
        if let Some(claimed) = producer_turn {
            let attempted =
                claimed.into_attempted(super::producer_turn_attempt_permit(&mut active_runner));
            if let Err(error) =
                activated.settle_producer_turn_after_local_proposal(&mut active_runner, attempted)
            {
                iroha_logger::error!(
                    failure = ?error.failure(),
                    "ProducerTurn terminal settlement requires restart"
                );
                output_guard.close_admission_for_restart();
                return Err(V2RunnerError::RestartRequired);
            }
        }

        if ready_to_finish {
            let (prepared_successor, retained_merge_sidecars, cleanup) = finalize_lifecycle_height(
                activated,
                &mut active_runner,
                lane_work,
                control_queue_capacity,
                cleanup_supervisor,
                |receipt, artifact, _lane_work| {
                    let predecessor =
                        DurableV2PredecessorIdentity::authenticate(artifact, receipt)?;
                    let artifact_hash = HashOf::new(artifact);
                    let terminal_application =
                        ProductionTerminalApplicationWithoutSuccessorActivationProjection {
                            context_id: successor_context_refinement_projection(context.id()),
                            context_height: context.height,
                            receipt_context_id: successor_context_refinement_projection(
                                receipt.context_id(),
                            ),
                            receipt_height: receipt.height(),
                            receipt_block_hash: successor_block_refinement_projection(
                                receipt.block_hash(),
                            ),
                            receipt_artifact_hash: CanonicalIdentityProjection::from_bytes(
                                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                                IDENTITY_KIND_FINALITY_ARTIFACT,
                                *receipt.artifact_hash().as_ref(),
                            ),
                            artifact_context_id: successor_context_refinement_projection(
                                artifact.context_id(),
                            ),
                            artifact_height: artifact.height,
                            artifact_block_hash: successor_block_refinement_projection(
                                artifact.block_hash,
                            ),
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
                    let successor_construction = output_guard
                        .begin_fail_stop_operation()
                        .ok_or(V2RunnerError::RestartRequired)?;
                    let successor =
                        build_verified_successor(state.as_ref(), context_store, artifact, receipt)?;
                    successor_construction.complete();
                    let (
                        next_verified_context,
                        successor_authority,
                        next_lifecycle_storage_authority,
                    ) = successor.into_parts_with_lifecycle_storage_authority(
                        kura.as_ref(),
                        genesis_account,
                    )?;
                    let next_context = next_verified_context.context().clone();
                    let pending_activation = activation.bind(successor_authority)?;
                    Ok((
                        next_context,
                        PreparedLifecycleSuccessorV1 {
                            verified_context: next_verified_context,
                            lifecycle_storage_authority: next_lifecycle_storage_authority,
                            pending_activation,
                            receipt_height: receipt.height(),
                            receipt_context_id: receipt.context_id(),
                            receipt_block_hash: receipt.block_hash(),
                        },
                    ))
                },
            )?;
            let PreparedLifecycleSuccessorV1 {
                verified_context,
                lifecycle_storage_authority,
                pending_activation,
                receipt_height,
                receipt_context_id,
                receipt_block_hash,
            } = prepared_successor;
            let (cleanup, lifecycle_storage_authority) =
                cleanup.bind_successor_storage(lifecycle_storage_authority)?;
            let prepared_successor = PreparedLifecycleSuccessorV1 {
                verified_context,
                lifecycle_storage_authority,
                pending_activation,
                receipt_height,
                receipt_context_id,
                receipt_block_hash,
            };
            if let Some(warning) = cleanup.wal_retirement_warning() {
                iroha_logger::warn!(
                    height = prepared_successor.receipt_height,
                    context_id = ?prepared_successor.receipt_context_id,
                    block_hash = %prepared_successor.receipt_block_hash,
                    cleanup_target = PostFinalityCleanupTarget::SafetyWal.as_str(),
                    reason = warning,
                    "Sumeragi v2 finalized with retained local WAL cleanup state"
                );
            }
            for warning in cleanup.cleanup().warnings() {
                iroha_logger::warn!(
                    height = prepared_successor.receipt_height,
                    context_id = ?prepared_successor.receipt_context_id,
                    block_hash = %prepared_successor.receipt_block_hash,
                    cleanup_target = warning.target().as_str(),
                    reason = warning.reason(),
                    "Sumeragi v2 finalized with retained local cleanup state"
                );
            }
            *eager_block_sync = retain_eager_block_sync(false, admitted_discovered_commit_qc);
            return Ok(Some(FinalizedLifecycleHeightV1 {
                verified_context: prepared_successor.verified_context,
                lifecycle_storage_authority: prepared_successor.lifecycle_storage_authority,
                pending_successor_activation: prepared_successor.pending_activation,
                retained_merge_sidecars,
                eager_block_sync: *eager_block_sync,
            }));
        }

        committed_lane_status_publisher.publish_if_changed(&lane_work);
        let _ = wake_rx.recv_timeout(IDLE_POLL);
    }
}

/// Run every ordinary, applied, snapshot, and CompleteTip height through one
/// lifecycle-owned adapter/executor/service stack.
///
/// PendingKura never enters this function. Its dedicated no-clock lifecycle
/// transfers only the verified successor and fresh lifecycle-storage
/// authority here after finalization.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn run_non_pending_lifecycle_loop(
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
    pending_queue_plan_admission_dirty: Arc<AtomicBool>,
    wake_rx: std::sync::mpsc::Receiver<()>,
    shutdown_signal: iroha_futures::supervisor::ShutdownSignal,
    ingress_ready: Arc<AtomicBool>,
    output_guard: Arc<ConsensusOutputGuard>,
    consensus_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    mut verified_context: crate::sumeragi::v2::VerifiedHeightContext,
    context_store: crate::sumeragi::v2_context_store::V2ContextStore,
    mut signature_policy: BlockSignaturePolicy,
    mut lifecycle_storage_authority: crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    mut first_height_authenticated_genesis: Option<
        crate::sumeragi::v2_context::AuthenticatedGenesisBodyV1,
    >,
    mut pending_successor_activation: Option<PendingSuccessorActivation>,
    mut staged_genesis_nexus_amx_context: Option<
        crate::sumeragi::v2_context::StagedGenesisNexusAmxContext,
    >,
    mut first_height_genesis: Option<SignedBlock>,
    genesis_account: AccountId,
    block_cadence: Duration,
    round_timeout: Duration,
    retransmit_interval: Duration,
    lifecycle_process_generation: Option<AutonomousLifecycleProcessGenerationClaim>,
    mut reservation_reconciliation_pending: bool,
    mut eager_block_sync: bool,
    mut cleanup_supervisor: V2CleanupSupervisor,
    mut liveness_watchdog: crate::sumeragi::status::V2LivenessWatchdog,
    deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    mut retained_merge_sidecars: Option<RetainedMergeSidecars>,
    kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
    mut block_sync_server: Option<V2BlockSyncServer>,
) -> Result<(), V2RunnerError> {
    let local_peer = common_config.peer.id().clone();

    loop {
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            return Ok(());
        }
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
        let body_queue_capacity = usize::try_from(shared_config.limits.body_queue_capacity)?;
        let chunk_queue_capacity = usize::try_from(shared_config.limits.chunk_queue_capacity)?;
        let certified_request_capacity =
            usize::try_from(shared_config.limits.certified_request_capacity)?;
        let effect_work_capacity = usize::try_from(shared_config.limits.effect_work_capacity)?;
        validate_deadline_duration(CANDIDATE_WORK_RECHECK)?;
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
        let candidate_limits = candidate_limits(&context, &shared_config)?;
        let local_validator = local_validator_index(&context, &local_peer, config.role)?;
        let mut npos_vrf = V2NposVrfLifecycle::open(
            &context,
            state.as_ref(),
            local_validator,
            &common_config.key_pair,
        )?;
        let new_block_sync_server = block_sync_server
            .is_none()
            .then(|| V2BlockSyncServer::new(context.network_id, certified_request_capacity))
            .transpose()?;
        let mut block_sync = V2BlockSyncDiscovery::new(
            context.clone(),
            local_peer.clone(),
            certified_request_capacity,
        )?;
        if let Some(server) = new_block_sync_server {
            block_sync_server = Some(server);
        }
        let consensus_key_hash: [u8; 32] =
            Hash::new(common_config.key_pair.public_key().encode()).into();
        let storage_root = kura.sumeragi_v2_storage_root();
        let body_store = V2BodyStore::open_with_policy(
            storage_root.join("bodies"),
            context.clone(),
            signature_policy.clone(),
        )
        .map_err(|error| {
            V2RunnerError::Effect(super::super::v2_effects::EffectExecutorError::BodyStore(
                error.to_string(),
            ))
        })?;
        let body_store = body_store
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
            .authenticate_final_wal_startup_authority()
            .map_err(|(error, _retained)| V2RunnerError::Adapter(error))?;
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
        let runtime_started_at = Instant::now();
        let launch_inputs = ProductionLifecycleLaunchInputsV1::new(
            runtime_started_at,
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
        let mut preactivation = launch_non_pending_lifecycle_height(
            owner,
            launch_inputs,
            pending_successor_activation.take(),
            &ingress_ready,
            &block_rx,
        )?;
        let mut setup_runner =
            ProductionLifecyclePreActivationRunnerBorrowV1::mint_for_recovered_runner();

        if reservation_reconciliation_pending {
            let evidence_repair_queue_fence =
                LaneApplicationEvidenceRepairQueueFence::capture(queue.as_ref())?;
            loop {
                evidence_repair_queue_fence.revalidate(queue.as_ref())?;
                match plan_lane_application_evidence_repair(
                    &context,
                    state.as_ref(),
                    kura.as_ref(),
                    lane_work_limits,
                )? {
                    LaneApplicationEvidenceRepairPlanning::Ready(plan) if plan.is_empty() => break,
                    LaneApplicationEvidenceRepairPlanning::Ready(plan) => {
                        let planned_items = plan.item_count();
                        let evidence_repair = output_guard
                            .begin_fail_stop_operation()
                            .ok_or(V2RunnerError::RestartRequired)?;
                        let summary = apply_lane_application_evidence_repair(
                            state.as_ref(),
                            kura.as_ref(),
                            plan,
                        )?;
                        if planned_items == 0 || summary.publication_count() == 0 {
                            return Err(V2RunnerError::Service(
                                "lane application evidence startup repair made no bounded progress"
                                    .to_owned(),
                            ));
                        }
                        evidence_repair.complete();
                    }
                    LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(needs) => {
                        if recover_canonical_bodies_before_activation(
                            &mut preactivation,
                            &mut setup_runner,
                            &needs,
                            &context,
                            &local_peer,
                            &state,
                            &kura,
                            &output_guard,
                            lane_work_limits,
                            retransmit_interval,
                            control_queue_capacity,
                            &wake_rx,
                            &shutdown_signal,
                        )? == CanonicalRecoveryControlV1::Shutdown
                        {
                            preactivation.into_clean_shutdown()?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        if reservation_reconciliation_pending {
            let summary = loop {
                let deferred_terminal_recovery =
                    reconcile_lifecycle_terminal_outcomes_before_queue_planning(
                        &output_guard,
                        state.as_ref(),
                        queue.as_ref(),
                        kura.as_ref(),
                        &context,
                    )?;
                let planning = plan_lane_reservation_ownership(
                    state.as_ref(),
                    queue.as_ref(),
                    kura.as_ref(),
                    &verified_context,
                    None,
                )?;
                let planning = match planning {
                    LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) => {
                        let planner_evidence =
                            pre_lifecycle_plan.startup_snapshot_recovery_evidence()?;
                        let lifecycle = reconcile_autonomous_lifecycle_startup(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            &context,
                            planner_evidence,
                            deferred_terminal_recovery,
                            lifecycle_process_generation.as_ref(),
                            &local_peer,
                            &common_config.key_pair,
                        )
                        .map_err(V2RunnerError::Service)?;
                        let completed_bootstraps = lifecycle.completed_bootstraps();
                        let recovered_attempts = lifecycle.recovered_attempts();
                        let replanned = plan_lane_reservation_ownership(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            &verified_context,
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
                    pending => pending,
                };
                match planning {
                    LaneReservationReconciliationPlanning::Ready(plan) => {
                        let reservation_recovery = output_guard
                            .begin_fail_stop_operation()
                            .ok_or(V2RunnerError::RestartRequired)?;
                        let summary = apply_lane_reservation_reconciliation_plan(
                            state.as_ref(),
                            queue.as_ref(),
                            kura.as_ref(),
                            plan,
                        )?;
                        reservation_recovery.complete();
                        break summary;
                    }
                    LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
                        if !queue.lane_reservation_startup_reconciliation_pending() {
                            return Err(V2RunnerError::Service(
                                "reservation body recovery was requested after the Queue startup gate opened"
                                    .to_owned(),
                            ));
                        }
                        if recover_canonical_bodies_before_activation(
                            &mut preactivation,
                            &mut setup_runner,
                            &needs,
                            &context,
                            &local_peer,
                            &state,
                            &kura,
                            &output_guard,
                            lane_work_limits,
                            retransmit_interval,
                            control_queue_capacity,
                            &wake_rx,
                            &shutdown_signal,
                        )? == CanonicalRecoveryControlV1::Shutdown
                        {
                            preactivation.into_clean_shutdown()?;
                            return Ok(());
                        }
                    }
                    LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
                        installs,
                    ) => {
                        if installs.is_empty()
                            || !queue.lane_reservation_startup_reconciliation_pending()
                        {
                            return Err(V2RunnerError::Service(
                                "historical reservation recovery lost its closed Queue boundary"
                                    .to_owned(),
                            ));
                        }
                        let historical_recovery = output_guard
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
                        let process_generation =
                            lifecycle_process_generation.as_ref().ok_or_else(|| {
                                V2RunnerError::Service(
                                    "historical recovery requires a validator process generation"
                                        .to_owned(),
                                )
                            })?;
                        for record in &records {
                            persist_canonical_historical_recovery_payload_custody(
                                kura.as_ref(),
                                process_generation,
                                &common_config.key_pair,
                                &local_peer,
                                record,
                            )?;
                        }
                        let _ = persist_preflighted_historical_autonomous_lane_recoveries(
                            kura.as_ref(),
                            &records,
                        )?;
                        validate_installed_historical_autonomous_lane_recoveries(
                            kura.as_ref(),
                            &records,
                        )?;
                        historical_recovery.complete();
                    }
                }
            };
            reservation_reconciliation_pending = false;
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
                    "reconciled durable lane reservations through the lifecycle owner"
                );
            }
        }

        let authenticated_genesis_nexus_amx_context = staged_genesis_nexus_amx_context
            .take()
            .map(AuthenticatedGenesisNexusAmxContext::Staged);
        let lane_work =
            preactivation.with_runner_setup(&mut setup_runner, |executor, services| {
                let mut lane_work =
                    construct_after_pending_tip_application_recovery(false, false, || {
                        V2LaneWorkAdapter::new_with_output_guard_and_transport(
                            &verified_context,
                            local_peer.clone(),
                            common_config.key_pair.clone(),
                            config.role == NodeRole::Validator,
                            Arc::clone(&state),
                            Arc::clone(&kura),
                            lane_work_limits,
                            authenticated_genesis_nexus_amx_context,
                            None,
                            Arc::clone(&output_guard),
                            exact_output_transport_owner,
                            retained_merge_sidecars.take(),
                            lifecycle_process_generation.clone(),
                        )
                        .map_err(V2RunnerError::from)
                    })?;
                lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
                lane_work.activate_after_lane_drain_queue_install(&queue)?;
                let _ = reconcile_executor_locked_body(executor, services)?;
                let startup_directive = executor.local_proposal_directive()?;
                lane_work.retain_merge_sidecars_for_global_view(
                    startup_directive.tag().view(),
                    startup_directive.locked_subject(),
                    startup_directive.decided_subject(),
                )?;
                if startup_directive.decided_subject().is_none()
                    && let Some((locked_round, locked)) = startup_directive.locked_body()
                {
                    let _ = lane_work.mark_global_body_locked(locked_round, locked)?;
                }
                dispatch_lane_work_effects(&mut lane_work, services, control_queue_capacity)?;
                Ok(lane_work)
            })?;
        let mut committed_lane_status_publisher = CommittedLaneStatusPublisher::default();
        committed_lane_status_publisher.publish_if_changed(&lane_work);
        let (_initial_directive, local_proposal) =
            preactivation.initialize_recovered_local_proposal(setup_runner)?;
        // Startup repair is not live-height cadence. Arm activation, proposal,
        // and discovery deadlines only after every closed-ingress recovery and
        // lane setup transaction has completed.
        let height_started_at = Instant::now();
        let mut activated = preactivation.activate(height_started_at, local_proposal)?;
        let mut active_runner =
            ProductionLifecycleActiveRunnerBorrowV1::mint_for_recovered_runner();
        activated.with_runner_runtime(
            &mut active_runner,
            |_owner, _executor, services, _local_proposal| {
                broadcast_npos_vrf_messages(
                    npos_vrf.take_outbound(),
                    output_guard.as_ref(),
                    services,
                )
            },
        )?;

        let finalized = run_lifecycle_active_height(
            activated,
            active_runner,
            lane_work,
            committed_lane_status_publisher,
            &context,
            &context_store,
            &state,
            &queue,
            &kura,
            &common_config,
            &events_sender,
            &block_rx,
            &lane_relay_rx,
            &pending_queue_plan_admission_dirty,
            &wake_rx,
            &shutdown_signal,
            &output_guard,
            &mut cleanup_supervisor,
            &mut liveness_watchdog,
            &mut npos_vrf,
            &mut block_sync,
            block_sync_server
                .as_mut()
                .expect("block-sync server is installed before lifecycle activation"),
            &mut eager_block_sync,
            candidate_limits,
            local_validator,
            control_queue_capacity,
            body_queue_capacity,
            height_started_at,
            block_cadence,
            round_timeout,
            retransmit_interval,
            first_height_genesis.as_ref(),
            &genesis_account,
        )?;
        let Some(finalized) = finalized else {
            return Ok(());
        };
        verified_context = finalized.verified_context;
        lifecycle_storage_authority = finalized.lifecycle_storage_authority;
        pending_successor_activation = Some(finalized.pending_successor_activation);
        retained_merge_sidecars = Some(finalized.retained_merge_sidecars);
        eager_block_sync = finalized.eager_block_sync;
        signature_policy = BlockSignaturePolicy::RotatingLeader;
        first_height_authenticated_genesis = None;
        first_height_genesis = None;
        staged_genesis_nexus_amx_context = None;
    }
}
