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
    PreparedLifecycleIngressSelector, ProductionLifecycleOwnerV1,
    ProductionRecoveredDecisionApplyDispatchErrorV1, ProductionRecoveredDecisionApplyDispatchV1,
    ProductionRecoveredDecisionFetchDispatchErrorV1, ProductionRecoveredDecisionFetchDispatchV1,
    ProductionRecoveredDecisionFetchPersistenceErrorV1,
    ProductionRecoveredDecisionFetchPersistenceV1, ProductionRecoveredLifecycleSignDispatchErrorV1,
    ProductionRecoveredLifecycleSignDispatchV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
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
            PreparedRecoveredDecisionApplyCompletionV1,
            PreparedRecoveredDecisionFetchBodyCompletionV1,
            PreparedRecoveredLifecycleSignCompletionV1, ProductionV2Services,
            RecoveredDecisionApplyDeferredRetryV1, RecoveredLifecycleProposalExactOutputCaptureV1,
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
    // Dedicated persisted Fetch completion drops after services have stopped
    // its worker, while retaining the queue Arc and fail-stop guard.
    recovered_decision_fetch_body_completion:
        Option<PreparedRecoveredDecisionFetchBodyCompletionV1>,
    // Services drop before this completion and stop the worker. This guard then
    // drops while its own retained queue Arc still represents the exact owner.
    recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>,
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

/// Result of settling one lifecycle-owned recovered Decision Fetch body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision Fetch settlement result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionFetchStoreSettlementV1 {
    /// No parked durable completion is currently present.
    None,
    /// Every pre-fsync owner remains parked unchanged for a later retry.
    Retry(ProductionRecoveredDecisionFetchStoreSettlementFailureV1),
    /// LedgerV1 publication was attempted and process restart is now required.
    RestartRequired,
    /// Fetch, Store, ingress, request, adapter, registry, and worker owners advanced.
    Applied,
}

/// Pre-publication adapter preparation for a recovered signed Broadcast.
///
/// This bounded seam proves the exact single-Broadcast reducer shape but does
/// not fsync, publish output, or acknowledge the worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Sign Broadcast preparation result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignBroadcastPreparationV1 {
    /// No parked recovered Sign completion is present.
    None,
    /// The guarded completion and adapter preview join the bounded shape exactly.
    Prepared,
    /// The completion or reducer successor is outside the single-Broadcast cut.
    Retry,
}

/// Result of durably settling one recovered Sign into its exact Broadcast child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Sign Broadcast settlement result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleSignBroadcastSettlementV1 {
    /// No parked recovered Sign completion is currently present.
    None,
    /// A pre-publication owner changed; every durable owner remains unchanged.
    Retry,
    /// LedgerV1 publication was attempted and process restart is now required.
    RestartRequired,
    /// Sign, Broadcast, adapter, registry, and worker owners advanced.
    ///
    /// The live Broadcast remains the durable output-debt source for the
    /// separate restart-safe refanout transaction.
    Applied,
}

/// Result of atomically settling a recovered Proposal into Broadcast plus Sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Proposal two-child settlement result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1 {
    /// No parked recovered Sign completion is currently present.
    None,
    /// Every pre-fsync owner remains unchanged and the completion was reparked.
    Retry,
    /// The aggregate Proposal control-and-chunk output corridor is currently full.
    CapacityUnavailable,
    /// Durable publication was attempted or output admission closed; restart is required.
    RestartRequired,
    /// Ledger, coordinator, registry, adapter, worker, and exact output advanced.
    Applied,
}

/// Result of settling one recovered Prepare Vote into Broadcast plus Commit Sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "recovered Vote two-child settlement result must be observed"]
pub(in crate::sumeragi) enum ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1 {
    /// No parked recovered Sign completion is currently present.
    None,
    /// Every pre-fsync owner remains unchanged and the completion was reparked.
    Retry,
    /// Durable publication was attempted or output admission closed; restart is required.
    RestartRequired,
    /// Ledger, coordinator, registry, adapter, and worker ownership advanced.
    /// The Ready Broadcast remains the durable source for typed refanout.
    Applied,
}

/// Stable pre-fsync settlement diagnostic; it contains no completion parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionRecoveredDecisionFetchStoreSettlementFailureV1 {
    /// The claimed coordinator or dedicated registry carrier changed.
    Owner,
    /// The fsynced completion does not bind one exact recovered body frame.
    Body,
    /// Fresh fair-ingress selector recapture or exact locking failed.
    Ingress,
    /// The dedicated request/response owner changed.
    Executor,
    /// The reducer preview did not produce the exact Store successor.
    Adapter,
    /// Store projection or child address preflight failed.
    Registry,
    /// Consensus output was already closed before publication.
    OutputClosed,
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
    /// Sign, reserve, claim, and publish the recovered Decision Fetch request.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_fetch(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionFetchDispatchV1,
        ProductionRecoveredDecisionFetchDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_decision_fetch(&self.services, &mut self.executor, runner)
    }

    /// Exercise recovered Decision Fetch request dispatch with a fixture cursor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_decision_fetch(
        &mut self,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionFetchDispatchV1,
        ProductionRecoveredDecisionFetchDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_decision_fetch(&self.services, &mut self.executor, runner)
    }

    /// Persist one selected recovered Decision Fetch response at the current Ingress cursor.
    #[cfg(not(test))]
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn persist_recovered_decision_fetch_response(
        &mut self,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        self.owner.persist_recovered_decision_fetch_response(
            &self.services,
            &mut self.executor,
            selector,
            runner,
        )
    }

    /// Exercise recovered Decision Fetch Phase A with a fixture Ingress cursor.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn persist_recovered_decision_fetch_response(
        &mut self,
        selector: PreparedLifecycleIngressSelector,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredDecisionFetchPersistenceV1,
        ProductionRecoveredDecisionFetchPersistenceErrorV1,
    > {
        self.owner.persist_recovered_decision_fetch_response(
            &self.services,
            &mut self.executor,
            selector,
            runner,
        )
    }

    /// Park at most one guarded durable recovered Decision Fetch body.
    /// No raw completion or acknowledgement surface exists; the fixed Store
    /// settlement below is the only consuming transaction.
    pub(in crate::sumeragi) fn retain_recovered_decision_fetch_body_completion(
        &mut self,
    ) -> Result<bool, String> {
        if self.recovered_decision_fetch_body_completion.is_some() {
            return Ok(false);
        }
        let drain = self
            .services
            .drain_recovered_decision_fetch_body_completion()?;
        let Some(completion) = drain.into_completion() else {
            return Ok(false);
        };
        self.recovered_decision_fetch_body_completion = Some(completion);
        Ok(true)
    }

    /// Settle the parked recovered Decision Fetch into one durable Store successor.
    ///
    /// Every semantic, owner, and exact-queue check precedes LedgerV1 fsync.
    /// The live Fetch remains payload-free; the derived Store alone retains the
    /// body frame. After publication the remaining operations are assertion-only
    /// moves under the prelocked fair-ingress occurrence and fail-stop output cut.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(
        &mut self,
    ) -> ProductionRecoveredDecisionFetchStoreSettlementV1 {
        let Self {
            owner,
            executor,
            services,
            recovered_decision_fetch_body_completion,
            leader_wire_ingress_binding,
            ..
        } = self;
        let Some(completion) = recovered_decision_fetch_body_completion.take() else {
            return ProductionRecoveredDecisionFetchStoreSettlementV1::None;
        };

        macro_rules! retry {
            ($failure:expr) => {{
                assert!(recovered_decision_fetch_body_completion.is_none());
                *recovered_decision_fetch_body_completion = Some(completion);
                return ProductionRecoveredDecisionFetchStoreSettlementV1::Retry($failure);
            }};
        }

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Owner);
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Owner);
        };
        let key = completion.completion().dispatch_key();
        let response_hash = completion.completion().response_hash();
        let physical_ordinal = completion.completion().physical_admission_ordinal();
        let Some(body) = completion.completion().project_store_body_authority() else {
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Body);
        };

        let selector = match executor.prepare_lifecycle_ingress_selector(
            &leader_wire_ingress_binding.ingress,
            physical_ordinal,
        ) {
            Ok(selector) => selector,
            Err(_) => {
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Ingress)
            }
        };
        let retirement =
            match executor.prepare_recovered_decision_fetch_owner_retirement(key, response_hash) {
                Ok(retirement) => retirement,
                Err(_) => {
                    retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Executor)
                }
            };
        let locked_dequeue = match selector.into_locked_recovered_decision_fetch_dequeue(
            executor,
            &leader_wire_ingress_binding.ingress,
            completion.completion(),
        ) {
            Ok(locked) => locked,
            Err(_) => {
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Ingress)
            }
        };
        let adapter_authority = match owner
            .registry
            .registry_mut()
            .prepare_recovered_decision_fetch_store_adapter_authority(
                &owner.coordinator,
                &lease,
                key,
                body,
            ) {
            Ok(authority) => authority,
            Err(_) => {
                drop(locked_dequeue);
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Registry)
            }
        };
        let adapter =
            match executor.prepare_recovered_decision_fetch_store_adapter(adapter_authority) {
                Ok(adapter) => adapter,
                Err(_) => {
                    drop(locked_dequeue);
                    retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Adapter)
                }
            };
        let successor = match owner
            .registry
            .registry_mut()
            .prepare_recovered_decision_fetch_store_successor(
                &owner.coordinator,
                &lease,
                &owner.verified,
                key,
                adapter,
            ) {
            Ok(successor) => successor,
            Err(_) => {
                drop(locked_dequeue);
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Registry)
            }
        };
        let transition = match owner
            .coordinator
            .prepare_recovered_decision_fetch_store_transition(&lease, &owner.verified, successor)
        {
            Ok(transition) => transition,
            Err(_) => {
                drop(locked_dequeue);
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Registry)
            }
        };
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(transition);
            drop(locked_dequeue);
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::OutputClosed);
        };

        if transition.persist_exact_successor().is_err() {
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(recovered_decision_fetch_body_completion.is_none());
            *recovered_decision_fetch_body_completion = Some(completion);
            drop(locked_dequeue);
            drop(operation);
            return ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired;
        }

        transition.commit_after_publication();
        executor.commit_recovered_decision_fetch_owner_retirement(retirement);
        locked_dequeue.commit();
        completion.acknowledge_after_publication();
        operation.complete();
        ProductionRecoveredDecisionFetchStoreSettlementV1::Applied
    }

    /// Reserve, claim, and queue one recovered Sign at the current Completion cursor.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn dispatch_recovered_lifecycle_sign(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredLifecycleSignDispatchV1,
        ProductionRecoveredLifecycleSignDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_lifecycle_sign(&self.services, &self.executor, runner)
    }

    /// Exercise recovered Sign dispatch with a fixture-owned Completion cursor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn dispatch_recovered_lifecycle_sign(
        &mut self,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredLifecycleSignDispatchV1,
        ProductionRecoveredLifecycleSignDispatchErrorV1,
    > {
        self.owner
            .dispatch_recovered_lifecycle_sign(&self.services, &self.executor, runner)
    }

    /// Park at most one guarded recovered Sign completion under this owner.
    ///
    /// Repeated calls retain the existing token and generic completion drains
    /// cannot acknowledge it. The bounded Vote/Timeout completion may enter
    /// the durable Broadcast settlement; other successor shapes remain guarded.
    pub(in crate::sumeragi) fn retain_recovered_lifecycle_sign_completion(
        &mut self,
    ) -> Result<bool, String> {
        if self.recovered_lifecycle_sign_completion.is_some() {
            return Ok(false);
        }
        let drain = self.services.drain_recovered_lifecycle_sign_completion()?;
        let Some(completion) = drain.into_completion() else {
            return Ok(false);
        };
        self.recovered_lifecycle_sign_completion = Some(completion);
        Ok(true)
    }

    /// Preflight the parked Sign's exact adapter successor.
    ///
    /// This deliberately drops the publication-inert preview before returning.
    /// Output is owned by the durable Broadcast child only after LedgerV1
    /// publication, through the separate typed refanout transaction.
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_broadcast(
        &mut self,
    ) -> ProductionRecoveredLifecycleSignBroadcastPreparationV1 {
        let Self {
            executor,
            recovered_lifecycle_sign_completion,
            ..
        } = self;
        let Some(completion) = recovered_lifecycle_sign_completion.as_ref() else {
            return ProductionRecoveredLifecycleSignBroadcastPreparationV1::None;
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            return ProductionRecoveredLifecycleSignBroadcastPreparationV1::Retry;
        };
        let preview = match executor.prepare_recovered_lifecycle_sign_completion(authority) {
            Ok(preview) => preview,
            Err(_) => return ProductionRecoveredLifecycleSignBroadcastPreparationV1::Retry,
        };
        if preview.shape() != super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast {
            return ProductionRecoveredLifecycleSignBroadcastPreparationV1::Retry;
        }
        drop(preview);
        ProductionRecoveredLifecycleSignBroadcastPreparationV1::Prepared
    }

    /// Settle the parked recovered Sign into one durable signed Broadcast.
    ///
    /// The claimed Sign carrier, adapter preview, and Broadcast child remain
    /// borrow-bound until the staged LedgerV1 successor is fsynced. Only
    /// assertion-only in-memory moves follow publication. The live Broadcast
    /// then becomes the sole restart-recoverable source for typed refanout.
    /// Proposal and follow-on-Sign reducer shapes remain outside this bounded
    /// transaction and fail-stop for restart.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(
        &mut self,
    ) -> ProductionRecoveredLifecycleSignBroadcastSettlementV1 {
        let Self {
            owner,
            executor,
            services,
            recovered_lifecycle_sign_completion,
            ..
        } = self;
        let Some(completion) = recovered_lifecycle_sign_completion.take() else {
            return ProductionRecoveredLifecycleSignBroadcastSettlementV1::None;
        };

        macro_rules! retry {
            () => {{
                assert!(recovered_lifecycle_sign_completion.is_none());
                *recovered_lifecycle_sign_completion = Some(completion);
                return ProductionRecoveredLifecycleSignBroadcastSettlementV1::Retry;
            }};
        }
        macro_rules! restart {
            () => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                drop(completion);
                return ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired;
            }};
        }

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!();
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!();
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            restart!();
        };
        let preview = match executor.prepare_recovered_lifecycle_sign_completion(authority) {
            Ok(preview) => preview,
            Err(_) => restart!(),
        };
        if preview.shape() != super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast {
            drop(preview);
            restart!();
        }
        let key = preview.dispatch_key();
        let successor = match owner
            .registry
            .registry_mut()
            .prepare_recovered_lifecycle_sign_broadcast_successor(
                &owner.coordinator,
                &lease,
                &owner.verified,
                key,
                preview,
            ) {
            Ok(successor) => successor,
            Err(_) => retry!(),
        };
        let transition = match owner
            .coordinator
            .prepare_recovered_lifecycle_sign_broadcast_transition(
                &lease,
                &owner.verified,
                successor,
            ) {
            Ok(transition) => transition,
            Err(_) => retry!(),
        };
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(transition);
            restart!();
        };

        if transition.persist_exact_successor().is_err() {
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(recovered_lifecycle_sign_completion.is_none());
            *recovered_lifecycle_sign_completion = Some(completion);
            drop(operation);
            return ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired;
        }

        transition.commit_after_publication();
        completion.acknowledge_after_publication();
        operation.complete();
        ProductionRecoveredLifecycleSignBroadcastSettlementV1::Applied
    }

    /// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.
    ///
    /// The next Vote body/WAL authority and both registry children remain
    /// borrow-bound through one LedgerV1 fsync. No network output is published
    /// here: the resulting Broadcast stays Ready and the typed refanout driver
    /// must transfer that durable debt before the adjacent Commit Sign runs.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(
        &mut self,
    ) -> ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1 {
        let Self {
            owner,
            executor,
            services,
            recovered_lifecycle_sign_completion,
            ..
        } = self;
        let Some(completion) = recovered_lifecycle_sign_completion.take() else {
            return ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::None;
        };

        macro_rules! retry {
            () => {{
                assert!(recovered_lifecycle_sign_completion.is_none());
                *recovered_lifecycle_sign_completion = Some(completion);
                return ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Retry;
            }};
        }
        macro_rules! restart {
            () => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(completion);
                return ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::RestartRequired;
            }};
        }

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!();
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!();
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            restart!();
        };
        let (preview, body) = match services
            .prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)
        {
            Ok(prepared) => prepared,
            Err(_) => restart!(),
        };
        if !preview.is_vote_broadcast_and_sign_shape() {
            drop(body);
            drop(preview);
            restart!();
        }
        let key = preview.dispatch_key();
        let successor = match owner
            .registry
            .registry_mut()
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(
                &owner.coordinator,
                &lease,
                &owner.verified,
                key,
                preview,
                body,
            ) {
            Ok(successor) => successor,
            Err(_) => retry!(),
        };
        let transition = match owner
            .coordinator
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(&lease, successor)
        {
            Ok(transition) => transition,
            Err(_) => retry!(),
        };
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(transition);
            restart!();
        };
        if transition.persist_exact_successor().is_err() {
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(recovered_lifecycle_sign_completion.is_none());
            *recovered_lifecycle_sign_completion = Some(completion);
            drop(operation);
            return ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::RestartRequired;
        }

        transition.commit_after_publication();
        completion.acknowledge_after_publication();
        operation.complete();
        ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Applied
    }

    /// Fsync an initial Proposal `PrepareIntent`, then publish both successors.
    ///
    /// The Proposal control message and canonical chunks are reserved before
    /// the WAL append.  A successful append changes the retry boundary: every
    /// later failure closes output and requires cold recovery, whose exact
    /// WAL-before-Ledger and two-child Ledger cuts are authenticated by the
    /// same Proposal/Prepare body and replay seals.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(
        &mut self,
    ) -> ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1 {
        let Self {
            owner,
            executor,
            services,
            recovered_lifecycle_sign_completion,
            ..
        } = self;
        let Some(completion) = recovered_lifecycle_sign_completion.take() else {
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::None;
        };

        macro_rules! restart {
            () => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(completion);
                return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired;
            }};
        }

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!();
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!();
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            restart!();
        };
        let (mut preview, body) = match services
            .prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)
        {
            Ok(prepared) => prepared,
            Err(_) => restart!(),
        };
        if preview.shape()
            != super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
        {
            drop(body);
            drop(preview);
            restart!();
        }
        let key = preview.dispatch_key();
        let output_authority = match preview.project_proposal_exact_output_authority() {
            Ok(authority) => authority,
            Err(_) => {
                drop(body);
                drop(preview);
                restart!();
            }
        };
        let output = match services
            .capture_recovered_lifecycle_proposal_exact_output(output_authority)
        {
            Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(output)) => output,
            Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)) => {
                drop(authority);
                drop(body);
                drop(preview);
                assert!(recovered_lifecycle_sign_completion.is_none());
                *recovered_lifecycle_sign_completion = Some(completion);
                return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable;
            }
            Err(_) => {
                drop(body);
                drop(preview);
                restart!();
            }
        };

        if preview
            .append_recovered_lifecycle_proposal_prepare_wal()
            .is_err()
        {
            drop(body);
            drop(preview);
            drop(output);
            restart!();
        }
        let successor = match owner
            .registry
            .registry_mut()
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(
                &owner.coordinator,
                &lease,
                &owner.verified,
                key,
                preview,
                body,
            ) {
            Ok(successor) => successor,
            Err(_) => {
                drop(output);
                restart!();
            }
        };
        let transition = match owner
            .coordinator
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(&lease, successor)
        {
            Ok(transition) => transition,
            Err(_) => {
                drop(output);
                restart!();
            }
        };
        if transition.persist_exact_successor().is_err() {
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(recovered_lifecycle_sign_completion.is_none());
            drop(output);
            drop(completion);
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired;
        }

        transition.commit_after_publication();
        completion.acknowledge_after_publication();
        output.commit_after_publication();
        ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied
    }

    /// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.
    ///
    /// The signed Proposal and its canonical chunks reserve one aggregate
    /// exact-output batch before the two-child LedgerV1 successor is fsynced.
    /// Publication then installs both registry children, advances the adapter,
    /// acknowledges the worker, and finally enqueues both fanouts. The
    /// Broadcast is parked only in process-local state while the output owner
    /// is live; LedgerV1 remains the restart source for that output debt.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(
        &mut self,
    ) -> ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1 {
        let Self {
            owner,
            executor,
            services,
            recovered_lifecycle_sign_completion,
            ..
        } = self;
        let Some(completion) = recovered_lifecycle_sign_completion.take() else {
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::None;
        };

        macro_rules! retry {
            () => {{
                assert!(recovered_lifecycle_sign_completion.is_none());
                *recovered_lifecycle_sign_completion = Some(completion);
                return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Retry;
            }};
        }
        macro_rules! restart {
            () => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(completion);
                return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired;
            }};
        }

        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!();
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!();
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            restart!();
        };
        let (mut preview, body) = match services
            .prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)
        {
            Ok(prepared) => prepared,
            Err(_) => restart!(),
        };
        if preview.shape()
            != super::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
        {
            drop(body);
            drop(preview);
            restart!();
        }
        let key = preview.dispatch_key();
        let output_authority = match preview.project_proposal_exact_output_authority() {
            Ok(authority) => authority,
            Err(_) => {
                drop(body);
                drop(preview);
                restart!();
            }
        };
        let output = match services
            .capture_recovered_lifecycle_proposal_exact_output(output_authority)
        {
            Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(output)) => output,
            Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)) => {
                drop(authority);
                drop(body);
                drop(preview);
                assert!(recovered_lifecycle_sign_completion.is_none());
                *recovered_lifecycle_sign_completion = Some(completion);
                return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable;
            }
            Err(_) => {
                drop(body);
                drop(preview);
                restart!();
            }
        };
        let successor = match owner
            .registry
            .registry_mut()
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(
                &owner.coordinator,
                &lease,
                &owner.verified,
                key,
                preview,
                body,
            ) {
            Ok(successor) => successor,
            Err(_) => {
                drop(output.abort_before_publication());
                retry!();
            }
        };
        let transition = match owner
            .coordinator
            .prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(&lease, successor)
        {
            Ok(transition) => transition,
            Err(_) => {
                drop(output.abort_before_publication());
                retry!();
            }
        };

        if transition.persist_exact_successor().is_err() {
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(recovered_lifecycle_sign_completion.is_none());
            *recovered_lifecycle_sign_completion = Some(completion);
            drop(output);
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired;
        }

        transition.commit_after_publication();
        completion.acknowledge_after_publication();
        output.commit_after_publication();
        ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied
    }

    /// Refanout one durable recovered signed Broadcast at the Completion cursor.
    ///
    /// Success parks only the live coordinator row. LedgerV1 deliberately
    /// remains Ready, so a hard crash reconstructs the exact output debt while
    /// the current process lets the exact-output corridor own all retries.
    #[cfg(not(test))]
    pub(in crate::sumeragi) fn refanout_recovered_lifecycle_signed_broadcast(
        &mut self,
        runner: LifecycleCurrentRunnerTurn<'_>,
    ) -> Result<
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    > {
        self.owner
            .refanout_recovered_lifecycle_signed_broadcast(&self.services, runner)
    }

    /// Exercise durable recovered Broadcast refanout with a fixture-owned cursor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn refanout_recovered_lifecycle_signed_broadcast(
        &mut self,
        runner: LifecycleRunnerRankSnapshot,
    ) -> Result<
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    > {
        self.owner
            .refanout_recovered_lifecycle_signed_broadcast(&self.services, runner)
    }

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
    fn launch_local_identity_matches(
        roster: &[wire::ValidatorPower],
        local_peer: &PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: &KeyPair,
    ) -> bool {
        if local_peer.public_key() != key_pair.public_key() {
            return false;
        }
        let roster_position = roster
            .iter()
            .position(|entry| &entry.validator == local_peer)
            .and_then(|position| u32::try_from(position).ok());
        local_validator.is_none_or(|observed| roster_position == Some(observed))
    }

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
            || !Self::launch_local_identity_matches(
                &context.roster,
                &inputs.local_peer,
                inputs.local_validator,
                &inputs.key_pair,
            )
            || !self.kura_binding.as_ref().is_some_and(|binding| {
                binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)
            })
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
            recovered_decision_fetch_body_completion: None,
            recovered_lifecycle_sign_completion: None,
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

    #[test]
    fn launch_local_identity_requires_the_bound_key_and_exact_roster_position() {
        let key_pair = KeyPair::random();
        let local_peer = PeerId::new(key_pair.public_key().clone());
        let roster = vec![wire::ValidatorPower {
            validator: local_peer.clone(),
            power: 1,
        }];
        assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
            &roster,
            &local_peer,
            Some(0),
            &key_pair,
        ));
        assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
            &roster,
            &local_peer,
            None,
            &key_pair,
        ));
        assert!(!ProductionLifecycleOwnerV1::launch_local_identity_matches(
            &roster,
            &local_peer,
            Some(1),
            &key_pair,
        ));
        let foreign_key = KeyPair::random();
        assert!(!ProductionLifecycleOwnerV1::launch_local_identity_matches(
            &roster,
            &local_peer,
            Some(0),
            &foreign_key,
        ));
        let observer_key = KeyPair::random();
        let observer_peer = PeerId::new(observer_key.public_key().clone());
        assert!(ProductionLifecycleOwnerV1::launch_local_identity_matches(
            &roster,
            &observer_peer,
            None,
            &observer_key,
        ));
    }

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
        let runner_tests_source = include_str!("v2_runner_tests.rs");
        let coordinator_source = include_str!("v2_lifecycle_coordinator.rs");
        let ledger_source = include_str!("v2_lifecycle_ledger.rs");
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
            "queue: Arc<Queue>",
            "provider_ingest_finalized_archive:",
            "reputation_finalized_archive:",
            "block_cadence: Duration",
            "events_sender: EventsSender",
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
        let local_identity = launch
            .find("Self::launch_local_identity_matches(")
            .expect("launch checks local peer, validator index, and bound signer before I/O");
        let kura_check = launch
            .find("binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)")
            .expect("launch rejoins the owner with the exact recovery Kura and local signer");
        let apply_identity = launch
            .find("service.matches_lifecycle_launch(")
            .expect("launch verifies the retained replay service before taking owner cuts");
        let registry_check = launch
            .find("exactly_covers_recovered_ready_work(&self.coordinator)")
            .unwrap();
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
        let take_apply = launch
            .find(".apply_service\n            .take()")
            .expect("launch consumes the exact marker-replay service once");
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
        let worker = launch
            .find("ProductionV2Services::start_with_apply_service(")
            .expect("launch transfers the exact marker-replay service to the worker");
        let worker_permit = launch
            .find("super::ProductionLifecycleApplyServiceLaunchPermitV1 {")
            .expect("launch mints the sole private Apply-service transfer permit");
        assert_eq!(
            launch.matches("inputs.auxiliary_io_capacity,").count(),
            2,
            "Serve restore and service startup must share the exact certified-request capacity"
        );
        let identity = launch
            .rfind("self.body_store_identity = Some(body_store_identity)")
            .unwrap();
        let complete = launch.rfind("construction.complete()").unwrap();
        assert!(
            arm < owner_check
                && owner_check < local_identity
                && local_identity < kura_check
                && kura_check < apply_identity
                && apply_identity < registry_check
        );
        assert!(
            apply_identity < storage_paths
                && storage_paths < adapter_wal
                && adapter_wal < restore_ordinals
                && restore_ordinals < producer_high_water
                && producer_high_water < open_gate
                && open_gate < body_receipts
                && body_receipts < gate_high_water
                && open_gate < gate_high_water
                && gate_high_water < bind_gate
                && bind_gate < take
                && take <= take_apply
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
        assert!(take_apply < worker_permit && worker_permit < worker);
        assert!(!launch.contains("inputs.block_cadence"));
        assert!(!launch.contains("genesis_account_for_launch"));
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
        let sign_completion_field = launched_fields
            .find(
                "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
            )
            .expect("launched wrapper retains the guarded recovered Sign completion");
        let binding_field = launched_fields
            .find("leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1")
            .expect("launched wrapper retains leader-wire binding ownership");
        assert!(
            services_field < sign_completion_field && sign_completion_field < binding_field,
            "Rust field drop order must stop services before dropping the Sign guard and unbinding leader-wire ingress"
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
        let legacy_start = worker_start
            .split_once(
                "/// Start with the exact application service used for recovered marker replay.",
            )
            .expect("legacy construction ends before the sealed transfer seam")
            .0;
        assert!(legacy_start.contains("let apply_service = V2ApplyService::new("));
        assert!(legacy_start.contains("Self::start_inner("));
        assert!(!legacy_start.contains("Self::start_with_apply_service("));
        let transferred_start = worker_start
            .split_once("pub(in crate::sumeragi) fn start_with_apply_service(")
            .expect("worker has one sealed recovered-service transfer seam")
            .1
            .split_once("fn start_inner(")
            .expect("sealed transfer validation precedes the shared constructor")
            .0;
        assert!(transferred_start.contains(
            "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1"
        ));
        let service_identity = transferred_start
            .find("apply_service.matches_lifecycle_launch(")
            .expect("worker rechecks exact recovered service identity");
        let enter_inner = transferred_start
            .find("Self::start_inner(")
            .expect("worker transfers only the checked service");
        assert!(service_identity < enter_inner);
        assert!(!transferred_start.contains("create_dir_all"));
        assert_eq!(
            worker_source
                .matches("ProductionLifecycleApplyServiceLaunchPermitV1")
                .count(),
            1,
            "only the sealed worker parameter may name the launch permit"
        );
        assert_eq!(
            source
                .matches("ProductionLifecycleApplyServiceLaunchPermitV1 {")
                .count(),
            1,
            "only lifecycle launch may construct the private permit"
        );
        assert!(coordinator_source.contains(
            "pub(in crate::sumeragi) struct ProductionLifecycleApplyServiceLaunchPermitV1 {\n    _seal: ProductionLifecycleApplyServiceLaunchPermitSealV1,\n}"
        ));
        assert!(
            coordinator_source
                .contains("impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1")
        );
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
        let runner_dependency_permit = runner_source
            .split_once(
                "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
            )
            .expect("runner owns the recovered lifecycle dependency permit")
            .1
            .split_once("/// Cadence-derived process-local deadline")
            .expect("runner dependency permit stays a bounded source region")
            .0;
        for required in [
            "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
            "local_signer: KeyPair",
            "fn mint_for_recovered_runner(local_signer: KeyPair) -> Self",
            "#[cfg(test)]",
            "fn for_test(local_signer: KeyPair) -> Self",
            "fn into_local_signer(self) -> KeyPair",
            "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
        ] {
            assert!(runner_dependency_permit.contains(required));
        }
        for forbidden in [
            "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
            "pub(crate) fn mint_for_recovered_runner(",
            "pub fn mint_for_recovered_runner(",
            "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
            "fn into_parts(",
        ] {
            assert!(!runner_dependency_permit.contains(forbidden));
        }
        assert!(runner_tests_source.contains(
            "fn recovered_lifecycle_factory_dependency_permit_retains_the_exact_local_signer()"
        ));
        let factory_bind = adapter_source
            .split_once("fn bind_production_lifecycle_owner_factory_inputs_v1(")
            .expect("adapter has one sealed lifecycle factory-input bind")
            .1
            .split_once("/// Consume all recovered adapter and storage authority")
            .expect("factory-input bind remains a bounded source region")
            .0;
        assert!(factory_bind.contains(
            "permit: super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1"
        ));
        assert!(factory_bind.contains("let local_signer = permit.into_local_signer();"));
        assert!(!source.contains("fn body_store("));
        assert!(!source.contains("fn adapter("));
        assert!(!source.contains("debug_assert!(startup_effects.is_empty())"));
    }

    #[test]
    fn recovered_lifecycle_sign_dispatch_source_is_sealed_and_restart_closed() {
        let scheduler_source = include_str!("v2_lifecycle_scheduler_inputs.rs");
        let registry_source = include_str!("v2_lifecycle_work_registry.rs");
        let coordinator_source = include_str!("v2_lifecycle_coordinator.rs");
        let worker_source = include_str!("v2_worker.rs");
        let launch_source = include_str!("v2_lifecycle_launch.rs");
        let effects_source = include_str!("v2_effects.rs");

        let dispatch = scheduler_source
            .split_once("fn dispatch_recovered_lifecycle_sign_with_runner_debt(")
            .expect("production owner has one recovered Sign dispatch transaction")
            .1
            .split_once(
                "/// Refanout one durable recovered signed Broadcast at the live Completion cursor.",
            )
            .expect("recovered Sign dispatch stays a bounded source region")
            .0;
        let body_owner = dispatch
            .find("let Some(body_store_identity) = self.body_store_identity.as_ref()")
            .expect("dispatch requires its launched body-store identity");
        let service_owner = dispatch
            .find("services.matches_lifecycle_body_store(body_store_identity)")
            .expect("dispatch rejoins the exact launched body store");
        let output_owner = dispatch
            .find("services.matches_lifecycle_executor_output_guard(executor)")
            .expect("dispatch rejoins service and executor output ownership");
        let attest = dispatch
            .find("attest_ready_recovered_lifecycle_sign")
            .expect("dispatch authenticates one current Ready carrier");
        let reserve = dispatch
            .find("capture_recovered_lifecycle_sign_capacity(dispatch_key)")
            .expect("dispatch reserves dedicated capacity before claiming");
        let claim = dispatch
            .find("self.coordinator.plan_turn(inputs)")
            .expect("dispatch claims only after capacity is held");
        let broadcast_reservation = dispatch
            .find("reservation.class() == CapacityClass::Consensus")
            .expect("the claimed Sign retains its mandatory Broadcast reservation");
        let projection = dispatch
            .find("prepare_recovered_lifecycle_sign_dispatch")
            .expect("the claimed carrier projects directly into its opaque task");
        let preflight = dispatch
            .find("reservation.preflight(&prepared)")
            .expect("queue identity is rechecked before publication");
        let publish = dispatch
            .find("reservation.commit(prepared)")
            .expect("the reserved queue cut performs the sole publication");
        assert!(
            body_owner < service_owner
                && service_owner < output_owner
                && output_owner < attest
                && attest < reserve
                && reserve < claim
                && claim < broadcast_reservation
                && broadcast_reservation < projection
                && projection < preflight
                && preflight < publish
        );
        assert_eq!(
            dispatch
                .matches("self.coordinator.rollback_unpublished_turn(&lease)")
                .count(),
            1,
            "the polymorphic unexpected-plan branch retains the unreserved rollback"
        );
        assert_eq!(
            dispatch
                .matches("rollback_unpublished_reserved_turn(&lease")
                .count(),
            3,
            "every reserved post-claim failure must release the coordinator overlay"
        );
        assert_eq!(
            dispatch.matches("reservation.cancel_uncommitted()").count(),
            6,
            "every reserved prepublication failure must release its capacity owner"
        );
        for forbidden in [
            "AdapterEffect",
            "PendingRuntimeEffectBinding",
            "RuntimeEffectOwnership",
            "EffectWorkId",
            "into_parts",
        ] {
            assert!(
                !dispatch.contains(forbidden),
                "recovered Sign dispatch exposes forbidden raw authority {forbidden}"
            );
        }

        let phase_carrier = registry_source
            .split_once("impl DurableRecoveredWalSignWork {")
            .expect("PhaseVote carrier has one exactness implementation")
            .1
            .split_once(
                "/// Whether one concrete registry row is still an executable adapter effect",
            )
            .expect("PhaseVote carrier exactness stays a bounded source region")
            .0;
        assert_eq!(
            phase_carrier
                .matches("self.matches_current_terminal_parent(coordinator)")
                .count(),
            2,
            "Ready and Claimed PhaseVote checks must rejoin the current terminal Validate parent"
        );
        assert_eq!(
            phase_carrier
                .matches("metadata.continuation == super::schema::DurableContinuation::None")
                .count(),
            2,
            "Ready and Claimed Sign children must remain standalone durable carriers"
        );
        for required in [
            "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
            "metadata.matches_admission(parent)",
            "super::schema::DurableContinuation::successor(",
            "coordinator.key_index.get(&parent.key)",
            "coordinator.owner_index.get(&parent.causal_root)",
        ] {
            assert!(
                phase_carrier.contains(required),
                "PhaseVote parent rejoin omitted {required}"
            );
        }

        let identity = registry_source
            .split_once("impl RecoveredLifecycleSignDispatchIdentityV1 {")
            .expect("recovered Sign identity has one sealed implementation")
            .1
            .split_once("/// Read-only coordinates of one exact Waiting Fetch incumbent.")
            .expect("recovered Sign identity stays a bounded source region")
            .0;
        assert!(identity.contains("&AdapterEffect::Sign {"));
        assert!(identity.contains("request: request.clone()"));
        assert!(!identity.contains("tag.view() =="));
        assert!(!identity.contains("vote.round.view"));

        let task = worker_source
            .split_once("pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {")
            .expect("worker has one opaque recovered Sign task")
            .1
            .split_once("enum V2IoCommand {")
            .expect("recovered Sign task/result stay a bounded source region")
            .0;
        for required in [
            "identity: RecoveredLifecycleSignDispatchIdentityV1",
            "prepared_candidate: Option<PreparedCandidateBody>",
            "self.task.prepared_candidate == expected_prepared",
            "outbound_payload: Option<EncodedV2Payload>",
            "authorizes_request(self.task.tag, &self.task.request)",
        ] {
            assert!(
                task.contains(required),
                "opaque Sign task omitted {required}"
            );
        }
        for forbidden in [
            "pub tag:",
            "pub request:",
            "pub signature:",
            "pub outbound_payload:",
            "fn into_parts(",
            "fn into_result(",
            "fn into_task(",
            "fn request(",
            "fn prepared_candidate(",
            "fn result(",
            "fn acknowledgement(",
            "fn acknowledge(",
            "fn signature(",
            "fn outbound_payload(",
        ] {
            assert!(
                !task.contains(forbidden),
                "opaque Sign task/result expose forbidden surface {forbidden}"
            );
        }
        let parked_completion = worker_source
            .split_once(
                "pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {",
            )
            .expect("worker has one opaque parked recovered Sign completion")
            .1
            .split_once("/// Result of atomically returning one guarded missing-sidecar Apply")
            .expect("parked recovered Sign completion stays a bounded source region")
            .0;
        for forbidden in [
            "fn into_parts(",
            "fn into_result(",
            "fn into_task(",
            "fn request(",
            "fn prepared_candidate(",
            "fn result(",
            "fn acknowledgement(",
            "fn acknowledge(",
            "fn signature(",
            "fn outbound_payload(",
            "fn settle(",
        ] {
            assert!(
                !parked_completion.contains(forbidden),
                "parked recovered Sign completion exposes forbidden surface {forbidden}"
            );
        }
        let signer = worker_source
            .split_once("fn sign_recovered_lifecycle_task(")
            .expect("worker has one fixed recovered Sign implementation")
            .1
            .split_once("fn recover_outbound_proposal_payload(")
            .expect("fixed recovered Sign stays a bounded source region")
            .0;
        assert!(!signer.contains("prepared_candidates"));
        assert!(!signer.contains("register_outbound_payload"));
        let capacity = worker_source
            .split_once("fn capture_recovered_lifecycle_sign_capacity<'a>(")
            .expect("worker has one dedicated recovered Sign capacity capture")
            .1
            .split_once("fn begin_decision_serve_reconciliation(")
            .expect("recovered Sign capacity capture stays a bounded source region")
            .0;
        assert_eq!(capacity.matches("operation.complete()").count(), 5);
        assert!(!capacity.contains("drop(operation)"));

        let rollback = coordinator_source
            .split_once("fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {")
            .expect("coordinator has one unpublished-claim rollback")
            .1
            .split_once("/// Rebuild records after seeding the ordinal high-water mark.")
            .expect("unpublished rollback stays a bounded source region")
            .0;
        assert!(rollback.contains("lease.output_reservation.is_some()"));
        assert!(rollback.contains("assert!(\n            inserted,"));
        assert!(!rollback.contains("debug_assert!"));

        for regression in [
            "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
            "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
            "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
            "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
        ] {
            assert!(
                worker_source.contains(regression) || coordinator_source.contains(regression),
                "recovered Sign prerequisite omitted behavior regression {regression}"
            );
        }
        assert!(effects_source.contains("owner.dispatch_recovered_lifecycle_sign("));
        assert!(effects_source.contains(
            "Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)"
        ));
        assert!(effects_source.contains(
            "a non-Completion runner cursor cannot claim or mutate a recovered Sign owner"
        ));

        let settlement = launch_source
            .split_once("pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(")
            .expect("recovered Sign has one durable Broadcast settlement")
            .1
            .split_once("/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.")
            .expect("recovered Sign settlement stays a bounded source region")
            .0;
        let completion = settlement
            .find("recovered_lifecycle_sign_completion.take()")
            .expect("settlement takes the guarded completion once");
        let preview = settlement
            .find("prepare_recovered_lifecycle_sign_completion(authority)")
            .expect("settlement previews the exact signed reducer successor");
        let registry = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_successor(")
            .expect("settlement seals the exact registry child");
        let transition = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_transition(")
            .expect("settlement stages one exact LedgerV1 successor");
        let operation = settlement
            .find("output_guard.begin_fail_stop_operation()")
            .expect("settlement arms the shared output guard before fsync");
        let fsync = settlement
            .find("transition.persist_exact_successor().is_err()")
            .expect("settlement fsyncs the exact successor");
        let coordinator_commit = settlement
            .find("transition.commit_after_publication();")
            .expect("coordinator, registry, and adapter commit after fsync");
        let worker_commit = settlement
            .find("completion.acknowledge_after_publication();")
            .expect("the worker owner retires last");
        let operation_commit = settlement
            .find("operation.complete();")
            .expect("the fail-stop operation completes after every owner commit");
        assert!(
            completion < preview
                && preview < registry
                && registry < transition
                && transition < operation
                && operation < fsync
                && fsync < coordinator_commit
                && coordinator_commit < worker_commit
                && worker_commit < operation_commit
        );
        assert!(!settlement.contains("capture_recovered_lifecycle_signed_broadcast_refanout"));
        assert!(!settlement.contains("commit_after_publication();\n        output"));
        let tail = &settlement[coordinator_commit..];
        assert!(!tail.contains("return "));
        assert!(!tail.contains(".is_err()"));

        let refanout = scheduler_source
            .split_once("fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(")
            .expect("durable Broadcast has one typed refanout transaction")
            .1
            .split_once("/// Sign, reserve, claim, and publish the sole recovered Decision Fetch")
            .expect("durable Broadcast refanout stays a bounded source region")
            .0;
        let census = refanout
            .find("if exact_ready != self.coordinator.ready_index")
            .expect("refanout authenticates the complete Ready census");
        let target = refanout
            .find("work_class == LifecycleWorkClass::Broadcast")
            .expect("refanout selects one Broadcast without requiring a two-row census");
        let retained_pair = refanout
            .find("recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal")
            .expect("pair recognition comes from the Broadcast carrier's retained child seal");
        let attest = refanout
            .find("attest_ready_recovered_lifecycle_signed_broadcast")
            .expect("refanout authenticates the durable Ready carrier");
        let full_rows = refanout
            .find("for ready_ordinal in &exact_ready")
            .expect("all unrelated Ready work remains in scheduler ranking");
        let ordinary_sign = refanout
            .find("attest_ready_recovered_lifecycle_sign(")
            .expect("an unrelated adjacent Sign uses its ordinary carrier attestation");
        let claim = refanout
            .find("self.coordinator.plan_turn(inputs)")
            .expect("refanout claims through the lifecycle scheduler");
        let projection = refanout
            .find("project_claimed_recovered_lifecycle_signed_broadcast_output")
            .expect("refanout rechecks the claimed durable carrier");
        let capture = refanout
            .find("capture_recovered_lifecycle_signed_broadcast_refanout(authority)")
            .expect("refanout reserves the exact network corridor");
        let wait = refanout
            .find("settle_turn(lease, super::TurnOutcome::Blocked(wait))")
            .expect("successful refanout parks only volatile scheduler state");
        let commit = refanout
            .find("output.commit_after_publication()")
            .expect("fanout commits only after the durable row is parked");
        assert!(
            census < target
                && target < retained_pair
                && retained_pair < attest
                && attest < full_rows
                && full_rows < ordinary_sign
                && ordinary_sign < claim
                && claim < projection
                && projection < capture
                && capture < wait
        );
        assert!(wait < commit);
        assert!(refanout.contains("rollback_unpublished_turn(&lease)"));
        assert!(
            refanout.contains("attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(")
        );
        assert!(!refanout.contains("exact_ready.len() == 2"));
        assert!(!refanout.contains("exact_ready.len() != 2"));
        assert!(!refanout.contains("persist_exact_successor"));
        assert!(!refanout.contains("TurnOutcome::Terminal"));

        let launched = launch_source
            .split_once("pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {")
            .expect("launched stack has one retained-owner declaration")
            .1
            .split_once("\n}")
            .expect("launched stack declaration is closed")
            .0;
        let services = launched.find("services: ProductionV2Services").unwrap();
        let completion = launched
            .find(
                "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
            )
            .unwrap();
        let ingress = launched
            .find("leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1")
            .unwrap();
        assert!(services < completion && completion < ingress);
        assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed();
        assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed();
    }

    fn assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed() {
        let source = include_str!("v2_lifecycle_launch.rs");
        let settlement = source
            .split_once(
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
            )
            .expect("recovered Prepare Vote has one combined settlement")
            .1
            .split_once(
                "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
            )
            .expect("combined Vote settlement stays bounded")
            .0;
        let completion = settlement
            .find("recovered_lifecycle_sign_completion.take()")
            .expect("take the guarded worker completion once");
        let body = settlement
            .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
            .expect("join the exact launched body owner");
        let mode = settlement
            .find("preview.is_vote_broadcast_and_sign_shape()")
            .expect("accept only Prepare-Broadcast then Commit-Sign");
        let registry = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
            .expect("seal the exact two-child registry successor");
        let transition = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
            .expect("stage the exact two-child Ledger successor");
        let operation = settlement
            .find("output_guard.begin_fail_stop_operation()")
            .expect("arm fail-stop output before fsync");
        let fsync = settlement
            .find("transition.persist_exact_successor().is_err()")
            .expect("fsync the two-child successor once");
        let transition_commit = settlement
            .find("transition.commit_after_publication();")
            .expect("publish coordinator, registry, and adapter after fsync");
        let worker_commit = settlement
            .find("completion.acknowledge_after_publication();")
            .expect("retire the guarded worker after publication");
        let operation_commit = settlement
            .find("operation.complete();")
            .expect("complete fail-stop ownership last");
        assert!(
            completion < body
                && body < mode
                && mode < registry
                && registry < transition
                && transition < operation
                && operation < fsync
                && fsync < transition_commit
                && transition_commit < worker_commit
                && worker_commit < operation_commit
        );
        assert!(!settlement.contains("project_proposal_exact_output_authority"));
        assert!(!settlement.contains("capture_recovered_lifecycle_proposal_exact_output"));
        assert!(!settlement.contains("output.commit_after_publication()"));
        let tail = &settlement[transition_commit..];
        assert!(!tail.contains("return "));
        assert!(!tail.contains(".is_err()"));
        assert!(!tail.contains('?'));
    }

    fn assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed() {
        let source = include_str!("v2_lifecycle_launch.rs");
        let settlement = source
            .split_once(
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
            )
            .expect("recovered Proposal has one combined settlement")
            .1
            .split_once("/// Refanout one durable recovered signed Broadcast")
            .expect("combined Proposal settlement stays bounded")
            .0;
        let completion = settlement
            .find("recovered_lifecycle_sign_completion.take()")
            .expect("take the guarded worker completion once");
        let body = settlement
            .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
            .expect("join the exact launched body owner");
        let output_projection = settlement
            .find("preview.project_proposal_exact_output_authority()")
            .expect("project output only from the same adapter preview");
        let output_capture = settlement
            .find("capture_recovered_lifecycle_proposal_exact_output(output_authority)")
            .expect("reserve Proposal control and chunks atomically");
        let registry = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
            .expect("seal the exact two-child registry successor");
        let transition = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
            .expect("stage the exact two-child Ledger successor");
        let fsync = settlement
            .find("transition.persist_exact_successor().is_err()")
            .expect("fsync the two-child successor once");
        let transition_commit = settlement
            .find("transition.commit_after_publication();")
            .expect("publish coordinator, registry, and adapter after fsync");
        let worker_commit = settlement
            .find("completion.acknowledge_after_publication();")
            .expect("retire the guarded worker after publication");
        let output_commit = settlement
            .find("output.commit_after_publication();")
            .expect("enqueue the reserved atomic batch last");
        assert!(
            completion < body
                && body < output_projection
                && output_projection < output_capture
                && output_capture < registry
                && registry < transition
                && transition < fsync
                && fsync < transition_commit
                && transition_commit < worker_commit
                && worker_commit < output_commit
        );
        assert_eq!(
            settlement
                .matches("output.abort_before_publication()")
                .count(),
            2,
            "every fallible post-reservation pre-fsync branch must release the batch"
        );
        assert!(
            settlement
                .contains("RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)")
        );
        assert!(settlement.contains("*recovered_lifecycle_sign_completion = Some(completion)"));
        assert!(settlement.contains("drop(output);"));
        let tail = &settlement[transition_commit..];
        assert!(!tail.contains("return "));
        assert!(!tail.contains(".is_err()"));
        assert!(!tail.contains("?"));
    }

    #[test]
    fn recovered_decision_fetch_dispatch_reserves_capacity_before_claim_and_failures_leave_no_mutation()
     {
        let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
        let dispatch = scheduler
            .split_once("fn dispatch_recovered_decision_fetch_with_runner_debt(")
            .expect("recovered Fetch has one request-dispatch transaction")
            .1
            .split_once("/// Persist one selected recovered Decision Fetch response")
            .expect("request dispatch stays a bounded source region")
            .0;
        let output = dispatch
            .find("capture_recovered_decision_fetch_exact_output(&owner)")
            .expect("exact output is captured");
        let executor = dispatch
            .find("prepare_recovered_decision_fetch_request_registration(owner)")
            .expect("executor vacancy is reserved");
        let claim = dispatch
            .find("self.coordinator.plan_turn(inputs)")
            .expect("coordinator claim exists");
        let commit = dispatch
            .find("registration.commit(prepared)")
            .expect("request owner has one commit tail");
        assert!(output < executor && executor < claim && claim < commit);
        assert!(dispatch.contains("output.abort_before_claim();"));
        assert!(dispatch.contains("rollback_unpublished_turn(&lease)"));
    }

    #[test]
    fn recovered_decision_fetch_queue_parks_generic_drain_and_extracts_only_dedicated_completion() {
        let worker = include_str!("v2_worker.rs");
        let generic = worker
            .split_once("fn take_io_completion(")
            .expect("generic completion selector exists")
            .1
            .split_once("fn take_recovered_decision_apply_completion(")
            .expect("generic selector stays bounded")
            .0;
        assert!(generic.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)"));
        assert!(generic.contains("self.held_io_completion = Some(completion);"));
        let dedicated = worker
            .split_once("fn take_recovered_decision_fetch_body_completion(")
            .expect("dedicated recovered Fetch extractor exists")
            .1
            .split_once("fn take_next_completion(")
            .expect("dedicated extractor stays bounded")
            .0;
        assert!(dedicated.contains("RecoveredDecisionFetchBodyPersisted"));
        assert!(worker.contains("tracked.state = V2IoWorkState::Active;"));
        assert!(worker.contains("tracked.state = V2IoWorkState::CompletionPending;"));
        assert!(worker.contains("drain_recovered_decision_fetch_body_completion"));
    }

    #[test]
    fn recovered_decision_fetch_phase_a_rejects_foreign_ingress_cursor_before_mutation() {
        let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
        let wrapper = scheduler
            .split_once("pub(crate) fn persist_recovered_decision_fetch_response(")
            .expect("production Phase-A wrapper exists")
            .1
            .split_once("/// Exercise Phase A with a fixture-owned current Ingress snapshot.")
            .expect("production cursor check stays isolated")
            .0;
        let cursor = wrapper
            .find("runner.target() != LifecycleRunnerRankTarget::Ingress")
            .expect("Phase A requires the Ingress cursor");
        let reject = wrapper
            .find("ForeignRunnerObservation")
            .expect("foreign cursor rejects explicitly");
        let handoff = wrapper
            .find("persist_recovered_decision_fetch_response_after_runner")
            .expect("mutation lives behind cursor validation");
        assert!(cursor < reject && reject < handoff);
        assert!(!wrapper[..handoff].contains("capture_lifecycle_capacity_rank"));
        assert!(!wrapper[..handoff].contains("prepare_recovered_decision_fetch_response_claim"));
    }

    #[test]
    fn recovered_decision_fetch_response_claim_precedes_assertion_only_queue_publication() {
        let effects = include_str!("v2_effects.rs");
        let commit = effects
            .split_once("pub(in crate::sumeragi) fn commit_with_queue(")
            .expect("recovered response has one composite commit")
            .1
            .split_once("impl RecoveredDecisionFetchResponseCandidateV1")
            .expect("composite commit stays bounded")
            .0;
        let claim = commit
            .find("owner.commit_exact_response_claim(response_hash)")
            .expect("exact response claim is installed");
        let queue = commit
            .find("queue.commit_recovered_decision_fetch_body_persistence(task)")
            .expect("dedicated persistence is published");
        assert!(claim < queue);
        assert!(commit.contains("assert!(owner.matches_response_claim_preflight"));
        let worker = include_str!("v2_worker.rs");
        let queue_commit = worker
            .split_once("fn commit_recovered_decision_fetch_body_persistence(")
            .expect("dedicated queue commit exists")
            .1
            .split_once("#[cfg(test)]")
            .expect("queue commit stays bounded")
            .0;
        assert!(queue_commit.contains("assert!("));
        assert!(!queue_commit.contains("return Err"));
    }

    #[test]
    fn recovered_decision_fetch_store_settlement_is_restart_closed_and_tail_infallible() {
        let launch = include_str!("v2_lifecycle_launch.rs");
        let settlement = launch
            .split_once("pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(")
            .expect("recovered Fetch has one Store settlement transaction")
            .1
            .split_once("/// Reserve, claim, and queue one recovered Sign")
            .expect("recovered Fetch Store settlement stays bounded")
            .0;
        let selector = settlement
            .find("prepare_lifecycle_ingress_selector(")
            .expect("fresh selector preflight exists");
        let request = settlement
            .find("prepare_recovered_decision_fetch_owner_retirement(")
            .expect("request/response retirement preflight exists");
        let ingress = settlement
            .find("into_locked_recovered_decision_fetch_dequeue(")
            .expect("exact ingress occurrence is locked");
        let carrier = settlement
            .find("prepare_recovered_decision_fetch_store_adapter_authority(")
            .expect("claimed recovered carrier preflight exists");
        let adapter = settlement
            .find("prepare_recovered_decision_fetch_store_adapter(")
            .expect("fixed reducer preview exists");
        let registry = settlement
            .find("prepare_recovered_decision_fetch_store_successor(")
            .expect("dedicated Store carrier preflight exists");
        let transition = settlement
            .find("prepare_recovered_decision_fetch_store_transition(")
            .expect("Fetch-to-Store coordinator successor is staged");
        let output = settlement
            .find("begin_fail_stop_operation()")
            .expect("output fail-stop cut precedes publication");
        let fsync = settlement
            .find("transition.persist_exact_successor().is_err()")
            .expect("exact LedgerV1 successor is fsynced once");
        let coordinator_commit = settlement
            .find("transition.commit_after_publication();")
            .expect("coordinator/registry/adapter tail exists");
        let request_commit = settlement
            .find("commit_recovered_decision_fetch_owner_retirement(retirement);")
            .expect("dedicated request owner retires after publication");
        let ingress_commit = settlement
            .find("locked_dequeue.commit();")
            .expect("locked ingress occurrence retires after publication");
        let worker_commit = settlement
            .find("completion.acknowledge_after_publication();")
            .expect("worker owner retires and disarms after publication");
        let output_commit = settlement
            .find("operation.complete();")
            .expect("output fail-stop cut closes last");
        assert!(
            selector < request
                && request < ingress
                && ingress < carrier
                && carrier < adapter
                && adapter < registry
                && registry < transition
                && transition < output
                && output < fsync
                && fsync < coordinator_commit
                && coordinator_commit < request_commit
                && request_commit < ingress_commit
                && ingress_commit < worker_commit
                && worker_commit < output_commit
        );
        let tail = &settlement[coordinator_commit..];
        assert!(!tail.contains("return "));
        assert!(!tail.contains("Result<"));
        assert!(!tail.contains(".is_err()"));

        let worker = include_str!("v2_worker.rs");
        let guarded = worker
            .split_once("impl GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {")
            .expect("recovered Fetch completion has one armed guard")
            .1
            .split_once("impl GuardedCertifiedFetchBodyPersistenceCompletion")
            .expect("recovered Fetch guard stays bounded")
            .0;
        assert!(guarded.contains("let _completion = self"));
        assert!(guarded.contains(".take()"));
        assert!(guarded.contains("self.drop_guard.disarm();"));
        let prepared = worker
            .split_once("impl PreparedRecoveredDecisionFetchBodyCompletionV1 {")
            .expect("parked recovered Fetch completion has one consuming acknowledgement")
            .1
            .split_once("impl PreparedRecoveredLifecycleSignCompletionV1")
            .expect("parked recovered Fetch acknowledgement stays bounded")
            .0;
        let index = prepared
            .find("acknowledge_recovered_decision_fetch_body(key, id, response_hash);")
            .expect("exact worker index is removed");
        let disarm = prepared
            .find("self.guarded.acknowledge_after_publication();")
            .expect("restart guard is disarmed after index removal");
        assert!(index < disarm);

        let ledger = include_str!("v2_lifecycle_ledger.rs");
        let open = include_str!("v2_lifecycle_open.rs");
        let registry_source = include_str!("v2_lifecycle_work_registry_validate_recovery.rs");
        for required in [
            "authenticate_recovered_decision_fetch_store",
            "open_recovered_decision_store_startup",
            "stage_recovered_decision_apply_projection",
            "successor_records_after_live_store",
        ] {
            assert!(ledger.contains(required), "cold restart omitted {required}");
        }
        assert!(open.contains("RecoveredWalStartupProjectionV1::DecisionStore"));
        assert!(registry_source.contains("install_recovered_wal_decision_store"));
    }
}
