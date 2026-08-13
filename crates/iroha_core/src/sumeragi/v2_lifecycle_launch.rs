//! Sealed production launch from recovered lifecycle ownership into live I/O.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_crypto::KeyPair;
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, consensus_v2 as wire},
    peer::PeerId,
};
use thiserror::Error;

use super::{
    ProductionLifecycleOwnerV1, ProductionRecoveredDecisionApplyDispatchErrorV1,
    ProductionRecoveredDecisionApplyDispatchV1,
    work_registry::RecoveredDecisionApplyTerminalPublicationError,
};
use crate::{
    IrohaNetwork,
    kura::Kura,
    state::State,
    sumeragi::{
        FairV2Ingress,
        output_guard::ConsensusOutputGuard,
        serviced_candidate_store::{LeaderWireLifecycleRestore, LeaderWireLifecycleStoreGate},
        v2_apply::RecoveredDecisionApplyWorkerResultV1,
        v2_context::AuthenticatedGenesisBodyV1,
        v2_effects::{EffectQueueConfig, V2EffectExecutor},
        v2_lane_work::{MergeSidecarDeferralDisposition, V2LaneWorkAdapter, V2LaneWorkError},
        v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig, SerializedV2Runtime},
        v2_worker::{
            DurableExactOutputServiceOwner, KuraReplicaAdvertRefreshOwner,
            PreparedRecoveredDecisionApplyCompletionV1, ProductionV2Services,
            RecoveredDecisionApplyDeferredRetryV1,
        },
    },
};

#[cfg(not(test))]
use crate::sumeragi::v2_runner::LifecycleCurrentRunnerTurn;
#[cfg(test)]
use crate::sumeragi::v2_runner::LifecycleRunnerRankSnapshot;

/// All non-lifecycle dependencies consumed by one production height launch.
///
/// The immutable height context, roster proofs, adapter, and body store are not
/// caller inputs: the recovered lifecycle owner supplies those exact values.
#[must_use = "production launch inputs must be consumed by the lifecycle owner"]
pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {
    runtime_started_at: Instant,
    round_timeout: Duration,
    runtime_queue: RuntimeQueueConfig,
    effect_queue: EffectQueueConfig,
    local_peer: PeerId,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: KeyPair,
    network: IrohaNetwork,
    state: Arc<State>,
    kura: Arc<Kura>,
    authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,
    consensus_io_capacity: usize,
    auxiliary_io_capacity: usize,
    orphan_chunk_capacity: usize,
    output_guard: Arc<ConsensusOutputGuard>,
    leader_wire_ingress: Arc<FairV2Ingress>,
    kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
}

impl ProductionLifecycleLaunchInputsV1 {
    /// Bind the runner-owned service dependencies for one consuming launch.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn new(
        runtime_started_at: Instant,
        round_timeout: Duration,
        runtime_queue: RuntimeQueueConfig,
        effect_queue: EffectQueueConfig,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        state: Arc<State>,
        kura: Arc<Kura>,
        authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Self {
        Self {
            runtime_started_at,
            round_timeout,
            runtime_queue,
            effect_queue,
            local_peer,
            local_validator,
            key_pair,
            network,
            state,
            kura,
            authenticated_genesis,
            consensus_io_capacity,
            auxiliary_io_capacity,
            orphan_chunk_capacity,
            output_guard,
            leader_wire_ingress,
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
        }
    }
}

/// RAII owner of the exact leader-wire gate installed for this sealed launch.
///
/// The ingress stays closed throughout this pre-activation tranche. Any later
/// construction error, ordinary wrapper drop, or panic closes it again before
/// detaching the exact gate, so no durable owner survives without its launch.
struct ProductionLeaderWireIngressBindingV1 {
    ingress: Arc<FairV2Ingress>,
    gate: Option<Arc<LeaderWireLifecycleStoreGate>>,
}

impl ProductionLeaderWireIngressBindingV1 {
    fn bind(
        ingress: Arc<FairV2Ingress>,
        gate: Arc<LeaderWireLifecycleStoreGate>,
        restore: LeaderWireLifecycleRestore,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
        context_id: wire::HeightContextId,
        height: wire::Height,
    ) -> Result<Self, String> {
        if let Err(error) = ingress.bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            lifecycle_ordinals,
            context_id,
            height,
        ) {
            ingress.close();
            return Err(error);
        }
        Ok(Self {
            ingress,
            gate: Some(gate),
        })
    }

    fn retire(&mut self) -> Result<(), String> {
        let Some(gate) = self.gate.as_ref() else {
            return Ok(());
        };
        self.ingress.close();
        self.ingress.unbind_leader_wire_lifecycle_gate(gate)?;
        self.gate = None;
        Ok(())
    }
}

impl Drop for ProductionLeaderWireIngressBindingV1 {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the sealed production leader-wire ingress gate"
            );
        }
    }
}

/// Opaque running stack produced by the sole consuming lifecycle launch.
///
/// Status publication and ingress activation are intentionally absent. A later
/// runner cut must add one sealed final activation transition after startup
/// effects, live-clock arming, and authenticated ingress opening all succeed.
#[must_use = "the launched lifecycle stack owns the active height"]
pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {
    owner: ProductionLifecycleOwnerV1,
    executor: V2EffectExecutor<SerializedV2Runtime>,
    services: ProductionV2Services,
    #[allow(dead_code)]
    completion_observer_activation: Option<ProductionV2CompletionObserverActivationPermitV1>,
    // Rust drops fields in declaration order. Keep this last so the service
    // worker has stopped before ingress closes and the durable gate detaches.
    #[allow(dead_code)]
    leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1,
}

/// Result of draining one dedicated recovered Apply worker completion.
#[allow(variant_size_differences)]
#[must_use = "a deferred recovered Apply completion must remain retained"]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionApplyCompletionV1 {
    /// The worker FIFO did not currently expose this lifecycle completion.
    None,
    /// Kura, LedgerV1, coordinator, registry, adapter, executor, and worker ack advanced.
    Applied,
    /// A guarded missing-sidecar result awaits exact fetch progress or queue re-entry.
    Deferred(RetainedRecoveredDecisionApplyDeferredV1),
}

/// Opaque guarded missing-sidecar result.
///
/// There is intentionally no parts or acknowledgement API. The sole retry
/// method either republishes the unchanged task under its existing queue key
/// or returns this complete guarded owner when capacity remains unavailable.
#[must_use = "deferred recovered Apply remains the sole retry owner"]
pub(in crate::sumeragi) struct RetainedRecoveredDecisionApplyDeferredV1 {
    completion: PreparedRecoveredDecisionApplyCompletionV1,
    sidecar: RecoveredDecisionApplySidecarWaitV1,
}

struct RecoveredDecisionApplySidecarWaitV1 {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    reference: CertifiedMergeLedgerReference,
}

impl RecoveredDecisionApplySidecarWaitV1 {
    fn register(
        &self,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> Result<MergeSidecarDeferralDisposition, V2LaneWorkError> {
        lane_work.defer_missing_recovered_decision_apply_sidecar(
            self.round,
            self.subject,
            self.reference.clone(),
        )
    }
}

/// Result of retrying one exact recovered Apply after its merge sidecar arrives.
#[allow(variant_size_differences)]
#[must_use = "an unavailable retry still owns the recovered Apply completion"]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionApplyRetryV1 {
    /// The unchanged task was atomically returned to the dedicated worker FIFO.
    Requeued,
    /// Sidecar fetch progress or Consensus I/O capacity is pending; ownership is unchanged.
    Unavailable(RetainedRecoveredDecisionApplyDeferredV1),
    /// The dedicated worker index changed and consensus was closed for restart.
    RestartRequired,
}

impl RetainedRecoveredDecisionApplyDeferredV1 {
    /// Retry only after the exact authenticated sidecar is locally durable.
    ///
    /// Re-registering the sealed wait is idempotent. `Fetching` and
    /// `RetryLater` retain this complete owner; only `Available`, which
    /// reauthenticates the referenced Kura entry, may republish the task.
    fn retry_after_available(self) -> ProductionRecoveredDecisionApplyRetryV1 {
        let Self {
            completion,
            sidecar,
        } = self;
        match completion.retry_deferred() {
            RecoveredDecisionApplyDeferredRetryV1::Requeued => {
                ProductionRecoveredDecisionApplyRetryV1::Requeued
            }
            RecoveredDecisionApplyDeferredRetryV1::Unavailable(completion) => {
                ProductionRecoveredDecisionApplyRetryV1::Unavailable(Self {
                    completion,
                    sidecar,
                })
            }
            RecoveredDecisionApplyDeferredRetryV1::RestartRequired => {
                ProductionRecoveredDecisionApplyRetryV1::RestartRequired
            }
        }
    }
}

/// Fail-stop class while durably terminalizing a recovered Apply completion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionApplyCompletionErrorV1 {
    /// The owner had no exact active recovered Apply lease and carrier.
    #[error("recovered Apply completion lost its exact lifecycle owner")]
    Owner,
    /// The Kura result did not match the installed recovered Apply authority.
    #[error("recovered Apply completion changed its durable authority")]
    Completion,
    /// The exact decided merge-sidecar dependency could not be registered.
    #[error("recovered Apply merge-sidecar recovery could not retain its exact owner")]
    Sidecar,
    /// The serialized adapter/executor retained conflicting live work.
    #[error("recovered Apply completion overtook live reducer work")]
    Executor,
    /// The staged terminal was not the sole exact coordinator/registry successor.
    #[error("recovered Apply terminal registry successor is not exact")]
    Registry,
    /// LedgerV1 exact-successor publication failed or became ambiguous.
    #[error("recovered Apply terminal LedgerV1 publication failed")]
    Ledger,
}

/// Move-only authority reserved for the final one-shot runner activation.
///
/// Launch construction retains this authority, but only this launch module
/// can mint it. The eventual runner cut must consume it in the same sealed
/// transition that arms clocks, opens authenticated ingress, and publishes the
/// first live status snapshot.
#[must_use = "completion-observer authority must remain sealed until runner activation"]
pub(in crate::sumeragi) struct ProductionV2CompletionObserverActivationPermitV1 {
    _seal: ProductionV2CompletionObserverActivationPermitSealV1,
}

struct ProductionV2CompletionObserverActivationPermitSealV1;

impl Drop for ProductionV2CompletionObserverActivationPermitSealV1 {
    fn drop(&mut self) {}
}

impl LaunchedProductionLifecycleV1 {
    /// Reserve, claim, and queue the recovered Apply at the live Completion cursor.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_apply(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionApplyDispatchV1,
        ProductionRecoveredDecisionApplyDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_decision_apply(&self.services, &self.executor, runner)
    }

    /// Drive and retry one exact missing-sidecar recovered Apply owner.
    ///
    /// The completion token no longer borrows the whole service owner: its
    /// stable dispatch key retains the exact worker completion accounting.
    /// This sealed method can therefore flush the sidecar request through the
    /// same service/lane instances before reprobing local Kura and queueing the
    /// unchanged task.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_recovered_decision_apply_deferred(
        &mut self,
        deferred: RetainedRecoveredDecisionApplyDeferredV1,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionRecoveredDecisionApplyRetryV1 {
        if !deferred
            .completion
            .authorizes_sidecar_owner(&self.services, lane_work)
        {
            drop(deferred);
            return ProductionRecoveredDecisionApplyRetryV1::RestartRequired;
        }
        match deferred.sidecar.register(lane_work) {
            Ok(MergeSidecarDeferralDisposition::Available) => deferred.retry_after_available(),
            Ok(MergeSidecarDeferralDisposition::Fetching) => {
                if lane_work
                    .dispatch_next_recovered_apply_sidecar_request(
                        &self.services,
                        &deferred.sidecar.reference,
                    )
                    .is_err()
                {
                    drop(deferred);
                    ProductionRecoveredDecisionApplyRetryV1::RestartRequired
                } else {
                    ProductionRecoveredDecisionApplyRetryV1::Unavailable(deferred)
                }
            }
            Ok(MergeSidecarDeferralDisposition::RetryLater) => {
                ProductionRecoveredDecisionApplyRetryV1::Unavailable(deferred)
            }
            Ok(MergeSidecarDeferralDisposition::Rejected(_)) | Err(_) => {
                drop(deferred);
                ProductionRecoveredDecisionApplyRetryV1::RestartRequired
            }
        }
    }

    /// Exercise recovered Apply dispatch with a fixture-owned runner observation.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_apply(
        &mut self,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionApplyDispatchV1,
        ProductionRecoveredDecisionApplyDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_decision_apply(&self.services, &self.executor, runner)
    }

    /// Drain and durably terminalize one lifecycle-owned recovered Apply.
    ///
    /// Applied results are rejoined to the exact claimed carrier before the
    /// adapter is previewed. LedgerV1 fsync precedes every volatile move; the
    /// post-fsync tail contains only coordinator/adapter/executor moves,
    /// registry removal, worker acknowledgement, and status publication.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn settle_recovered_decision_apply_completion(
        &mut self,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> Result<
        ProductionRecoveredDecisionApplyCompletionV1,
        ProductionRecoveredDecisionApplyCompletionErrorV1,
    > {
        let owner = &mut self.owner;
        let executor = &mut self.executor;
        let services = &mut self.services;
        let drain = services
            .drain_recovered_decision_apply_completion()
            .map_err(|_| ProductionRecoveredDecisionApplyCompletionErrorV1::Owner)?;
        let Some(completion) = drain.into_completion() else {
            return Ok(ProductionRecoveredDecisionApplyCompletionV1::None);
        };

        macro_rules! restart {
            ($failure:expr) => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err($failure);
            }};
        }

        if let RecoveredDecisionApplyWorkerResultV1::Deferred { task, reference } =
            completion.result()
        {
            if !completion.authorizes_sidecar_owner(services, lane_work) {
                drop(completion);
                restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Sidecar);
            }
            let sidecar = RecoveredDecisionApplySidecarWaitV1 {
                round: task.validated_receipt().durable().round(),
                subject: task.subject(),
                reference: reference.clone(),
            };
            match sidecar.register(lane_work) {
                Ok(
                    MergeSidecarDeferralDisposition::Fetching
                    | MergeSidecarDeferralDisposition::Available
                    | MergeSidecarDeferralDisposition::RetryLater,
                ) => {
                    return Ok(ProductionRecoveredDecisionApplyCompletionV1::Deferred(
                        RetainedRecoveredDecisionApplyDeferredV1 {
                            completion,
                            sidecar,
                        },
                    ));
                }
                Ok(MergeSidecarDeferralDisposition::Rejected(_)) | Err(_) => {
                    drop(completion);
                    restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Sidecar);
                }
            }
        }
        let RecoveredDecisionApplyWorkerResultV1::Applied(applied) = completion.result() else {
            unreachable!("recovered Apply result variants are exhausted above")
        };

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Owner);
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Owner);
        };
        let Some((transition, authority)) = owner
            .registry
            .prepare_recovered_decision_apply_terminal_transition(
                &owner.coordinator,
                &lease,
                applied,
            )
        else {
            restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Completion);
        };
        let adapter = match executor.prepare_recovered_decision_apply_completion(authority) {
            Ok(adapter) => adapter,
            Err(_) => restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Executor),
        };
        let mut staged = owner.coordinator.stage_durable_transaction();
        staged.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None);
        if staged.fault.is_some() {
            restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Registry);
        }
        let published = owner
            .registry
            .publish_recovered_decision_apply_terminal_transition(
                transition,
                &owner.coordinator,
                &staged,
                &lease,
                || owner.coordinator.persist_exact_staged_successor(&staged),
            );
        match published {
            Ok(()) => {}
            Err(RecoveredDecisionApplyTerminalPublicationError::Preflight(_)) => {
                restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Registry);
            }
            Err(RecoveredDecisionApplyTerminalPublicationError::Publication(_, _)) => {
                restart!(ProductionRecoveredDecisionApplyCompletionErrorV1::Ledger);
            }
        }

        owner.coordinator = staged;
        let finality = adapter.commit_after_durable_settlement();
        let status = executor.commit_recovered_decision_apply_finality(finality);
        let settled = completion.acknowledge_after_owner_settlement();
        assert!(
            matches!(settled, RecoveredDecisionApplyWorkerResultV1::Applied(_)),
            "borrowed recovered Apply result cannot change before acknowledgement"
        );
        super::super::status::set_v2_status(status);
        Ok(ProductionRecoveredDecisionApplyCompletionV1::Applied)
    }
}

/// Fail-stop failure while consuming the recovered lifecycle owner into I/O.
#[derive(Debug, Error)]
#[must_use = "a failed consuming launch requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecycleLaunchErrorV1 {
    /// The owner was already launched or its retained cuts disagreed.
    #[error("recovered lifecycle owner is not an exact unlaunched owner")]
    InvalidOwner,
    /// The canonical output corridor was already fail-stop closed.
    #[error("canonical consensus output is closed")]
    OutputClosed,
    /// Durable ordinal restoration or leader-wire gate binding failed.
    #[error("sealed leader-wire launch failed: {0}")]
    LeaderWire(String),
    /// The sealed adapter could not enter the serialized runtime.
    #[error("serialized runtime launch failed: {0}")]
    Runtime(#[source] super::super::v2_runtime::RuntimeConfigError),
    /// Exact-body recovery could not enter the effect executor.
    #[error("effect executor launch failed: {0}")]
    Executor(#[source] super::super::v2_effects::EffectExecutorError),
    /// The ordered I/O worker could not start with the transferred store.
    #[error("production I/O launch failed: {0}")]
    Services(String),
    /// A post-construction process-identity check failed.
    #[error("launched lifecycle stack lost exact process ownership")]
    OwnershipMismatch,
}

impl ProductionLifecycleOwnerV1 {
    /// Consume the sealed adapter and exact body store into one running stack.
    ///
    /// One armed fail-stop operation spans runtime, executor, and worker
    /// construction. Success leaves only the body-store instance identity in
    /// the lifecycle owner. This transition never publishes adapter status.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn launch(
        mut self,
        inputs: ProductionLifecycleLaunchInputsV1,
    ) -> Result<LaunchedProductionLifecycleV1, ProductionLifecycleLaunchErrorV1> {
        let construction_guard = Arc::clone(&inputs.output_guard);
        let construction = construction_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleLaunchErrorV1::OutputClosed)?;
        let context = self.verified.context().clone();
        let validator_set_pops = self.verified.proofs_of_possession().to_vec();
        if self.body_store.is_none()
            || self.body_store_identity.is_some()
            || !self
                .kura_binding
                .as_ref()
                .is_some_and(|binding| binding.matches_kura(inputs.kura.as_ref()))
            || !self.apply_service.as_ref().is_some_and(|service| {
                service.matches_lifecycle_launch(
                    &inputs.state,
                    &inputs.kura,
                    &context,
                    &validator_set_pops,
                )
            })
            || self.adapter_startup.is_none()
            || self.coordinator.active_context()
                != super::projection::lifecycle_context(self.verified.context())
        {
            return Err(ProductionLifecycleLaunchErrorV1::InvalidOwner);
        }
        if {
            let registry = self.registry.registry_mut();
            !registry.exactly_covers_recovered_ready_work(&self.coordinator)
                && !registry
                    .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
        } {
            return Err(ProductionLifecycleLaunchErrorV1::InvalidOwner);
        }
        let launch_storage = self
            .kura_binding
            .as_ref()
            .and_then(|binding| binding.storage_paths_for_launch(inputs.kura.as_ref()))
            .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?;
        let leader_wire_launch = self
            .adapter_startup
            .as_mut()
            .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?
            .prepare_leader_wire_launch(launch_storage.wal_path())
            .map_err(|error| ProductionLifecycleLaunchErrorV1::LeaderWire(error.to_owned()))?;
        let lifecycle_ordinals = ProductionV2Services::restore_lifecycle_ordinal_source(
            &context,
            launch_storage.chunk_root(),
            inputs.network.reply_route_source_capacity().max(1),
            inputs.auxiliary_io_capacity,
        )
        .map_err(ProductionLifecycleLaunchErrorV1::LeaderWire)?;
        if let Some(high_watermark) = leader_wire_launch.restored_producer_ordinal_high_watermark()
        {
            lifecycle_ordinals
                .advance_past(high_watermark)
                .map_err(ProductionLifecycleLaunchErrorV1::LeaderWire)?;
        }
        let (leader_wire_gate, leader_wire_restore, leader_wire_recovery_authority) =
            leader_wire_launch
                .open_gate(
                    &context,
                    self.body_store
                        .as_ref()
                        .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?,
                )
                .map_err(ProductionLifecycleLaunchErrorV1::LeaderWire)?;
        lifecycle_ordinals
            .advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())
            .map_err(ProductionLifecycleLaunchErrorV1::LeaderWire)?;
        let leader_wire_ingress_binding = ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&inputs.leader_wire_ingress),
            Arc::clone(&leader_wire_gate),
            leader_wire_restore,
            lifecycle_ordinals.clone(),
            context.id(),
            context.height,
        )
        .map_err(ProductionLifecycleLaunchErrorV1::LeaderWire)?;

        let adapter_startup = self
            .adapter_startup
            .take()
            .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?;
        let body_store = self
            .body_store
            .take()
            .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?;
        let apply_service = self
            .apply_service
            .take()
            .ok_or(ProductionLifecycleLaunchErrorV1::InvalidOwner)?;
        let body_store_identity = body_store.instance_identity();
        let runtime = adapter_startup
            .into_serialized_runtime(
                inputs.runtime_started_at,
                inputs.round_timeout,
                inputs.runtime_queue,
                lifecycle_ordinals.clone(),
            )
            .map_err(ProductionLifecycleLaunchErrorV1::Runtime)?;
        let (mut executor, body_store) = V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            context.clone(),
            inputs.local_peer.clone(),
            inputs.local_validator,
            Arc::clone(&inputs.output_guard),
            inputs.effect_queue,
        )
        .map_err(ProductionLifecycleLaunchErrorV1::Executor)?;
        if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref() {
            executor
                .install_authenticated_genesis_body(authenticated_genesis.signed_block())
                .map_err(ProductionLifecycleLaunchErrorV1::Executor)?;
        }
        if !body_store
            .instance_identity()
            .same_instance(&body_store_identity)
        {
            return Err(ProductionLifecycleLaunchErrorV1::OwnershipMismatch);
        }
        let initial_tag = executor.current_tag();
        let durable_decided_subject = executor
            .local_proposal_directive()
            .map_err(|error| {
                ProductionLifecycleLaunchErrorV1::Executor(
                    super::super::v2_effects::EffectExecutorError::Runtime(error.to_string()),
                )
            })?
            .decided_subject();
        let services = ProductionV2Services::start_with_apply_service(
            super::ProductionLifecycleApplyServiceLaunchPermitV1 {
                _seal: super::ProductionLifecycleApplyServiceLaunchPermitSealV1,
            },
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            inputs.local_peer,
            inputs.local_validator,
            inputs.key_pair,
            inputs.network,
            launch_storage.into_chunk_root(),
            body_store,
            inputs.state,
            inputs.kura,
            apply_service,
            inputs.consensus_io_capacity,
            inputs.auxiliary_io_capacity,
            inputs.orphan_chunk_capacity,
            lifecycle_ordinals,
            Arc::clone(&inputs.output_guard),
            inputs.leader_wire_ingress,
            inputs.kura_replica_advert_refresh,
            leader_wire_recovery_authority,
            inputs.exact_output_handoff_owner,
        )
        .map_err(ProductionLifecycleLaunchErrorV1::Services)?;
        if !services.matches_lifecycle_executor_output_guard(&executor)
            || !services.matches_lifecycle_body_store(&body_store_identity)
        {
            return Err(ProductionLifecycleLaunchErrorV1::OwnershipMismatch);
        }
        self.body_store_identity = Some(body_store_identity);
        construction.complete();
        Ok(LaunchedProductionLifecycleV1 {
            owner: self,
            executor,
            services,
            completion_observer_activation: Some(
                ProductionV2CompletionObserverActivationPermitV1 {
                    _seal: ProductionV2CompletionObserverActivationPermitSealV1,
                },
            ),
            leader_wire_ingress_binding,
        })
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::v2_lifecycle_coordinator::reviewed_lifecycle_ledger_source_for_test;

    fn empty_leader_wire_gate_for_binding_test(
        directory: &TempDir,
        filename: &str,
        context_id: wire::HeightContextId,
        height: wire::Height,
        validator: &PeerId,
    ) -> (
        Arc<LeaderWireLifecycleStoreGate>,
        LeaderWireLifecycleRestore,
    ) {
        let owner = [0xA7; 32];
        let max_chunk_count = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(1, max_chunk_count)
            .expect("finite leader-wire binding fixture capacity");
        let recovery_authority =
            crate::sumeragi::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context_id,
                height,
                owner,
                0,
                false,
            );
        LeaderWireLifecycleStoreGate::open(
            &directory.path().join(filename),
            context_id,
            height,
            owner,
            [validator.clone()].into_iter().collect(),
            capacity,
            max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open empty leader-wire binding fixture")
    }

    #[test]
    fn production_leader_wire_binding_retires_explicitly_on_drop_and_closes_on_failure() {
        const HEIGHT: wire::Height = 7;
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"production-leader-wire-launch-binding",
        )));
        let validator = PeerId::new(KeyPair::random().public_key().clone());
        let directory = TempDir::new().expect("temporary launch binding directory");
        let ingress = Arc::new(FairV2Ingress::new(16, 1 << 20, 1 << 18, 0, 0));
        ingress
            .configure_roster([validator.clone()])
            .expect("one-validator launch binding geometry");
        ingress.require_leader_wire_lifecycle_gate();
        ingress.state.lock().leader_wire_max_chunk_count = 2;

        let (first_gate, first_restore) = empty_leader_wire_gate_for_binding_test(
            &directory,
            "explicit.wal",
            context_id,
            HEIGHT,
            &validator,
        );
        let mut binding = ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&ingress),
            Arc::clone(&first_gate),
            first_restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            HEIGHT,
        )
        .expect("bind the exact launch gate");
        assert!(
            ingress
                .state
                .lock()
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &first_gate))
        );
        binding
            .retire()
            .expect("explicit retirement detaches the exact launch gate");
        binding
            .retire()
            .expect("explicit retirement remains idempotent");
        assert!(ingress.state.lock().leader_wire_lifecycle_gate.is_none());

        let (drop_gate, drop_restore) = empty_leader_wire_gate_for_binding_test(
            &directory, "drop.wal", context_id, HEIGHT, &validator,
        );
        let binding = ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&ingress),
            Arc::clone(&drop_gate),
            drop_restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            HEIGHT,
        )
        .expect("rebind the exact launch gate");
        drop(binding);
        assert!(
            ingress.state.lock().leader_wire_lifecycle_gate.is_none(),
            "Drop must detach the exact launch gate"
        );

        let (incumbent_gate, incumbent_restore) = empty_leader_wire_gate_for_binding_test(
            &directory,
            "incumbent.wal",
            context_id,
            HEIGHT,
            &validator,
        );
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&incumbent_gate),
                incumbent_restore,
                RuntimeLifecycleOrdinalSource::after_high_watermark(0),
                context_id,
                HEIGHT,
            )
            .expect("bind the incumbent gate");
        ingress.open().expect("open the incumbent ingress");
        let (foreign_gate, foreign_restore) = empty_leader_wire_gate_for_binding_test(
            &directory,
            "foreign.wal",
            context_id,
            HEIGHT,
            &validator,
        );
        let error = match ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&ingress),
            foreign_gate,
            foreign_restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            HEIGHT,
        ) {
            Ok(_) => panic!("an open, already-bound ingress accepted a foreign launch gate"),
            Err(error) => error,
        };
        assert!(error.contains("empty closed ingress"));
        assert!(
            !ingress.state.lock().open,
            "failed binding must close ingress"
        );
        ingress
            .unbind_leader_wire_lifecycle_gate(&incumbent_gate)
            .expect("clean up the incumbent binding");
    }

    #[test]
    fn launch_source_keeps_status_sealed_and_orders_store_transfer() {
        let source = include_str!("v2_lifecycle_launch.rs");
        let adapter_source = include_str!("v2.rs");
        let safety_wal_source = include_str!("safety_wal.rs");
        let kura_source = concat!(
            include_str!("../kura.rs"),
            include_str!("../kura/bound_progress_and_retained_support.rs")
        );
        let adjacent_store_source = include_str!("serviced_candidate_store.rs");
        let worker_source = include_str!("v2_worker.rs");
        let runner_source = include_str!("v2_runner.rs");
        let ledger_source = reviewed_lifecycle_ledger_source_for_test();
        let bound_launch = ledger_source
            .split_once("// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_BEGIN")
            .expect("the bound CompleteTip launch has one sealed source region")
            .1
            .split_once("// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_END")
            .expect("the bound CompleteTip launch region has one end")
            .0;

        let bind = bound_launch
            .find("impl BoundRecoveredCompleteTipSuccessorOwnerV1")
            .expect("the bound H+1 owner has one launch implementation");
        let launch = bound_launch
            .find("pub(in crate::sumeragi) fn launch(")
            .expect("the bound H+1 owner exposes one consuming launch");
        let consume = bound_launch
            .find("let Self { owner, retirement } = self;")
            .expect("launch consumes both halves of the exact join");
        let generic_launch = bound_launch
            .find("let launched = owner.launch(inputs)?;")
            .expect("the bound owner enters the sole generic launch transaction");
        let retained = bound_launch
            .find("LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n            launched,\n            retirement,")
            .expect("the successful launch retains its retirement authority");
        let wrapper = bound_launch
            .find("struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1")
            .expect("the typed post-launch wrapper stays opaque");
        assert!(bound_launch.contains(
            "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {\n    launched: super::launch::LaunchedProductionLifecycleV1,\n    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,\n}"
        ));
        assert!(
            bind < launch
                && launch < consume
                && consume < generic_launch
                && generic_launch < retained
                && retained < wrapper
        );
        for forbidden in [
            "set_v2_status",
            "publish_status(",
            "successor_activation_status",
            "activate_effect_completion_observer",
            "into_owner",
            "into_parts",
            "fn owner(",
            "fn retirement(",
            "fn launched(",
            "fn into_launched(",
            "fn into_retirement(",
            "-> ProductionLifecycleOwnerV1",
            "-> super::launch::LaunchedProductionLifecycleV1",
            "-> RetiredRecoveredCompleteTipActivationAuthorityV1",
            "pub launched:",
            "pub retirement:",
            "pub(crate) launched:",
            "pub(crate) retirement:",
            "pub(in crate::sumeragi) launched:",
            "pub(in crate::sumeragi) retirement:",
        ] {
            assert!(
                !bound_launch.contains(forbidden),
                "bound CompleteTip launch exposes forbidden surface {forbidden}"
            );
        }
        assert_eq!(bound_launch.matches("owner.launch(inputs)?").count(), 1);

        assert!(source.contains("authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,"));
        assert_eq!(
            source
                .matches("authenticated_genesis: Option<AuthenticatedGenesisBodyV1>")
                .count(),
            2,
            "the move-only genesis seal must occur only in the launch input field and constructor"
        );
        assert!(!source.contains("authenticated_genesis: Option<SignedBlock>"));
        let raw_genesis_account_input = ["genesis_account", ": AccountId"].concat();
        assert!(
            !source.contains(&raw_genesis_account_input),
            "launch inputs must not accept a caller-selected genesis validation authority"
        );
        let inputs = source
            .split_once("pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {")
            .expect("launch inputs have one declaration")
            .1
            .split_once("\n}")
            .expect("launch input declaration is closed")
            .0;
        for forbidden in [
            "chunk_root: PathBuf",
            "wal_path: PathBuf",
            "lifecycle_ordinals: RuntimeLifecycleOrdinalSource",
            "durable_bodies:",
            "recovered_body_receipts:",
        ] {
            assert!(
                !inputs.contains(forbidden),
                "launch inputs expose caller-selected durable authority {forbidden}"
            );
        }

        let launch = source
            .split_once("pub(in crate::sumeragi) fn launch(")
            .expect("the owner has one consuming launch")
            .1
            .split_once("\n}\n\n#[cfg(test)]")
            .expect("the consuming launch ends before its source guards")
            .0;
        assert!(!source.contains("publish_status("));
        assert!(!launch.contains("set_v2_effect_completion_observer"));
        let arm = launch.find("begin_fail_stop_operation()").unwrap();
        let owner_check = launch.find("if self.body_store.is_none()").unwrap();
        let kura_check = launch
            .find("binding.matches_kura(inputs.kura.as_ref())")
            .expect("launch rejoins the owner with the exact recovery Kura");
        let registry_check = launch
            .find("exactly_covers_recovered_ready_work(&self.coordinator)")
            .unwrap();
        let genesis_authority = launch
            .find("binding.genesis_account_for_launch(inputs.kura.as_ref())")
            .expect("launch derives the Apply authority from the recovery-owned Kura seal");
        let storage_paths = launch
            .find("binding.storage_paths_for_launch(inputs.kura.as_ref())")
            .expect("launch derives paths from the exact recovery-owned Kura seal");
        let body_receipts = launch
            .find("self.body_store\n                        .as_ref()")
            .expect("launch derives exact receipts from its owner-held body store");
        let adapter_wal = launch
            .find(".prepare_leader_wire_launch(launch_storage.wal_path())")
            .expect("launch rejoins adapter authority to the recovery-sealed WAL");
        let restore_ordinals = launch
            .find("ProductionV2Services::restore_lifecycle_ordinal_source(")
            .expect("launch restores its sole lifecycle ordinal source internally");
        assert!(launch.contains(
            "inputs.network.reply_route_source_capacity().max(1),\n            inputs.auxiliary_io_capacity,"
        ));
        let producer_high_water = launch
            .find("leader_wire_launch.restored_producer_ordinal_high_watermark()")
            .expect("launch folds the adapter producer high-watermark");
        let open_gate = launch
            .find(".open_gate(")
            .expect("launch opens the gate with exact owner-store receipts");
        let gate_high_water = launch
            .find("leader_wire_restore.scheduler_ordinal_high_watermark()")
            .expect("launch folds the restored leader-wire high-watermark");
        let bind_gate = launch
            .find("ProductionLeaderWireIngressBindingV1::bind(")
            .expect("launch binds the exact gate before runtime construction");
        let take = launch.find(".body_store\n            .take()").unwrap();
        let runtime = launch
            .find(".into_serialized_runtime(")
            .expect("launch consumes the adapter into the serialized runtime");
        let executor = launch
            .find("V2EffectExecutor::open_with_body_store(")
            .unwrap();
        let genesis_gate = launch
            .find("if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()")
            .expect("fresh-genesis installation stays behind the owned optional seal");
        let genesis = launch
            .find(
                "executor\n                .install_authenticated_genesis_body(authenticated_genesis.signed_block())",
            )
            .expect("authenticated genesis enters the executor before worker start");
        let worker = launch.find("ProductionV2Services::start(").unwrap();
        assert_eq!(
            launch.matches("inputs.auxiliary_io_capacity,").count(),
            2,
            "Serve restore and service startup must share the exact certified-request capacity"
        );
        let identity = launch
            .rfind("self.body_store_identity = Some(body_store_identity)")
            .unwrap();
        let complete = launch.rfind("construction.complete()").unwrap();
        assert!(arm < owner_check && owner_check < kura_check && arm < registry_check);
        assert!(
            kura_check < genesis_authority
                && genesis_authority < storage_paths
                && storage_paths < adapter_wal
                && adapter_wal < restore_ordinals
                && restore_ordinals < producer_high_water
                && producer_high_water < open_gate
                && open_gate < body_receipts
                && body_receipts < gate_high_water
                && open_gate < gate_high_water
                && gate_high_water < bind_gate
                && bind_gate < take
                && take < runtime
        );
        let gate_open = adapter_source
            .split_once("pub(in crate::sumeragi) fn open_gate(")
            .expect("adapter projection has one consuming gate open")
            .1
            .split_once("impl ProductionLifecycleAdapterStartupV1")
            .expect("gate open ends before adapter startup methods")
            .0;
        assert!(gate_open.contains("body_store: &super::v2_body_store::V2BodyStore"));
        assert!(gate_open.contains("body_store.matches_context(context)"));
        assert!(gate_open.contains("body_store\n            .recovery_catalog()"));
        assert!(gate_open.contains(".map(|(_, receipt)| receipt)"));
        assert!(!gate_open.contains("durable_bodies: &[DurableBodyReceipt]"));
        assert!(gate_open.contains(
            "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(\n            self.storage,"
        ));
        let adapter_launch = adapter_source
            .split_once("pub(in crate::sumeragi) fn prepare_leader_wire_launch(")
            .expect("adapter startup has one leader-wire projection")
            .1
            .split_once("/// Consume the sealed adapter startup directly")
            .expect("leader-wire projection ends before runtime consumption")
            .0;
        for required in [
            "&mut self",
            "!*leader_wire_launch_prepared",
            "adapter.wal.matches_path(expected_wal_path)",
            "adapter\n                    .mint_leader_wire_store_authority(expected_wal_path)",
            "*leader_wire_launch_prepared = true",
        ] {
            assert!(
                adapter_launch.contains(required),
                "one-shot adapter leader-wire projection omitted {required}"
            );
        }
        let runtime_conversion = adapter_source
            .split_once("pub(in crate::sumeragi) fn into_serialized_runtime(")
            .expect("adapter startup has one runtime conversion")
            .1
            .split_once("#[cfg(test)]\n    pub(in crate::sumeragi) const fn fixture_for_test")
            .expect("runtime conversion ends before fixture helpers")
            .0;
        assert!(runtime_conversion.contains("leader_wire_launch_prepared: true"));
        let adapter_open = adapter_source
            .split_once("fn open_with_aggregator_and_publication_with_capacity(")
            .expect("adapter has one production recovery open")
            .1
            .split_once("/// Return the tag which must accompany a new asynchronous operation")
            .expect("adapter recovery open ends before projections")
            .0;
        let safety_open = adapter_open
            .find("let (wal_path, wal) = match wal_target")
            .expect("adapter selects one sealed WAL open target first");
        let kura_open = adapter_open
            .find("SafetyWal::open_with_kura_authority(")
            .expect("production adapter consumes the Kura-root authority");
        let fixture_open = adapter_open
            .find("SafetyWalOpenTarget::FixturePath(wal_path)")
            .expect("legacy pathname opening is explicitly test-only");
        let serviced_mint = adapter_open
            .find("wal.mint_serviced_candidate_store_authority(&wal_path)?")
            .expect("adapter mints the fixed serviced-candidate authority");
        let serviced_open = adapter_open
            .find("ServicedCandidateStore::open_with_safety_wal_authority(")
            .expect("adapter consumes the serviced-candidate authority");
        let wal_replay = adapter_open
            .find("let entries = wal\n            .recovered_records()")
            .expect("adapter replays the bound WAL after adjacent recovery");
        assert!(safety_open < kura_open && kura_open < fixture_open);
        assert!(fixture_open < serviced_mint && serviced_mint < serviced_open);
        assert!(serviced_open < wal_replay);

        for capability in [
            "SafetyWalServicedCandidateStoreAuthority",
            "SafetyWalLeaderWireStoreAuthority",
        ] {
            let declaration = safety_wal_source
                .split_once(&format!("pub(crate) struct {capability} {{"))
                .unwrap_or_else(|| panic!("missing {capability}"))
                .1
                .split_once("\n}")
                .expect("capability declaration is closed")
                .0;
            assert!(declaration.contains("entry: BoundSafetyWalAdjacentEntry"));
            assert!(!safety_wal_source.contains(&format!("impl Clone for {capability}")));
            assert!(!safety_wal_source.contains(&format!("impl Copy for {capability}")));
        }
        for required in [
            "#[cfg(any(test, not(all(unix, not(target_os = \"espidf\")))))]\nuse std::fs::OpenOptions;",
            "direct_lexical_directory_metadata(expected_path)?",
            "open_canonical_directory_nofollow(&canonical_path)?",
            "let metadata = fs::symlink_metadata(expected_path)?;",
            "fs::symlink_metadata(&self.expected_path)",
            "let linked = fs::symlink_metadata(self.expected_path.join(name))?;",
            "rustix::fs::OFlags::CREATE\n                        | rustix::fs::OFlags::EXCL",
            "unix_file_identity(&opened) != expected_identity",
            "fn write_all(&mut self, bytes: &[u8])",
            "fn sync_data(&mut self)",
            "self.directory.verify_leaf(self.file, self.wal_name)",
            "let durable = rustix::fs::statat(",
            "promoted adjacent snapshot changed across directory sync",
            "BoundSafetyWalDirectory::from_kura_authority(kura, authority)",
            "safety-WAL authority belongs to a different Kura instance",
            "#[cfg(test)]\n    fn bind(expected_path: &Path)",
            "#[cfg(test)]\n    pub(crate) fn open(",
        ] {
            assert!(
                safety_wal_source.contains(required),
                "opened WAL-directory authority omitted {required}"
            );
        }
        for required in [
            "store_root_directory: BoundProgressDirectory",
            "Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?",
            "KuraSafetyWalDirectoryAuthority",
            "#[derive(Debug)]\n#[must_use = \"the Kura-bound safety-WAL directory authority must open one WAL\"]",
        ] {
            assert!(
                kura_source.contains(required),
                "Kura storage owner omitted {required}"
            );
        }
        assert!(!kura_source.contains("impl Clone for KuraSafetyWalDirectoryAuthority"));
        assert!(!kura_source.contains("impl Copy for KuraSafetyWalDirectoryAuthority"));
        for required in [
            "pub(crate) fn mint_safety_wal_directory_authority(",
            "rustix::fs::openat(\n                &root.file,\n                STORE_ROOT_LOCK_FILE_NAME,",
            "Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)",
            "rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU)",
            "Self::open_bound_progress_child_directory(",
            "kura_identity: self.instance_identity()",
        ] {
            assert!(
                kura_source.contains(required),
                "Kura-root WAL authority omitted {required}"
            );
        }
        assert_eq!(
            safety_wal_source
                .matches("Err(SafetyWalError::UnsupportedStorageBinding {")
                .count(),
            3,
            "the production Kura-root open and both adjacent authority mints must reject on non-Unix"
        );
        assert_eq!(
            safety_wal_source
                .matches("snapshot storage is unsupported on this platform")
                .count(),
            3,
            "non-Unix adjacent read, publication, and retirement must have no path fallback"
        );
        assert!(
            adjacent_store_source.contains("storage: SafetyWalServicedCandidateStoreAuthority")
        );
        assert!(adjacent_store_source.contains("storage: SafetyWalLeaderWireStoreAuthority"));
        assert!(adjacent_store_source.contains("pub(crate) fn open_with_safety_wal_authority("));
        assert!(adjacent_store_source.contains(
            "#[cfg(test)]\n    #[allow(clippy::too_many_arguments)]\n    pub(crate) fn open("
        ));
        let runner_leader_mint = runner_source
            .find("adapter.mint_leader_wire_store_authority(&wal_path)?")
            .expect("legacy runner consumes the adapter-minted sibling authority");
        let runner_gate_open = runner_source
            .find("LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(")
            .expect("legacy runner opens only through the typed authority");
        assert!(runner_leader_mint < runner_gate_open);
        assert!(runner_source.contains("kura\n            .mint_safety_wal_directory_authority()"));
        assert!(runner_source.contains("kura.as_ref(),\n                wal_authority,"));
        assert!(
            take < executor
                && executor < genesis_gate
                && genesis_gate < genesis
                && genesis < worker
                && worker < identity
                && identity < complete
        );
        assert!(launch.contains("inputs.block_cadence,\n            genesis_account,"));
        assert!(launch.contains(
            "completion_observer_activation: Some(\n                ProductionV2CompletionObserverActivationPermitV1"
        ));
        assert!(launch.contains("leader_wire_ingress_binding,"));
        assert!(source.contains("impl Drop for ProductionLeaderWireIngressBindingV1"));
        let launched_fields = source
            .split_once("pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {")
            .expect("launched wrapper has one declaration")
            .1
            .split_once("\n}")
            .expect("launched wrapper declaration is closed")
            .0;
        let services_field = launched_fields
            .find("services: ProductionV2Services")
            .expect("launched wrapper retains the service worker");
        let binding_field = launched_fields
            .find("leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1")
            .expect("launched wrapper retains leader-wire binding ownership");
        assert!(
            services_field < binding_field,
            "Rust field drop order must stop services before unbinding leader-wire ingress"
        );
        let leader_wire_drop = source
            .split_once("impl ProductionLeaderWireIngressBindingV1 {")
            .expect("leader-wire launch binding has one implementation")
            .1
            .split_once("impl Drop for ProductionLeaderWireIngressBindingV1")
            .expect("leader-wire binding Drop follows its implementation")
            .0;
        let close = leader_wire_drop
            .find("self.ingress.close()")
            .expect("leader-wire retirement closes ingress first");
        let unbind = leader_wire_drop
            .find("self.ingress.unbind_leader_wire_lifecycle_gate(gate)?")
            .expect("leader-wire retirement unbinds the exact retained gate");
        assert!(close < unbind);
        assert!(
            source.contains("impl Drop for ProductionV2CompletionObserverActivationPermitSealV1")
        );
        let worker_start = worker_source
            .split_once("pub(crate) fn start(")
            .expect("production services have one constructor")
            .1
            .split_once("/// Sign and retain all canonical chunks")
            .expect("service construction ends before outbound registration")
            .0;
        let status_publication = worker_source
            .split_once("fn publish_effect_status(")
            .expect("production services have one effect-status publisher")
            .1
            .split_once("fn fail_closed(")
            .expect("effect-status publication ends before fail-stop handling")
            .0;
        assert!(!worker_start.contains("set_v2_effect_completion_observer"));
        assert!(!worker_start.contains("activate_effect_completion_observer"));
        assert!(!worker_start.contains("publish_effect_status"));
        assert!(!status_publication.contains("set_v2_effect_completion_observer"));
        let observer_activation = worker_source
            .split_once("fn activate_effect_completion_observer(")
            .expect("the completion observer has one sealed activation seam")
            .1
            .split_once("/// Atomically reserve the selected lifecycle carrier")
            .expect("the sealed activation seam stays narrow")
            .0;
        assert!(observer_activation.contains("ProductionV2CompletionObserverActivationPermitV1"));
        let activation_arm = observer_activation
            .find("begin_fail_stop_operation()")
            .unwrap();
        let live_worker = observer_activation
            .find(".io\n            .as_ref()")
            .unwrap();
        let register = observer_activation
            .find("set_v2_effect_completion_observer")
            .unwrap();
        let activation_complete = observer_activation.find("activation.complete()").unwrap();
        assert!(
            activation_arm < live_worker
                && live_worker < register
                && register < activation_complete
        );
        assert_eq!(
            worker_source
                .matches("set_v2_effect_completion_observer(")
                .count(),
            1
        );
        assert!(!worker_source.contains("ProductionV2CompletionObserverActivationPermitV1 {"));
        assert!(!launch.contains("activate_effect_completion_observer("));
        assert!(!runner_source.contains("activate_effect_completion_observer("));
        assert!(!source.contains("fn body_store("));
        assert!(!source.contains("fn adapter("));
        assert!(!source.contains("debug_assert!(startup_effects.is_empty())"));
    }
}
