//! Sealed production launch from recovered lifecycle ownership into live I/O.

use std::{
    collections::BTreeSet,
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
    kura::{Kura, KuraV2CommitReceipt},
    state::State,
    sumeragi::{
        FairV2Ingress,
        output_guard::ConsensusOutputGuard,
        serviced_candidate_store::{LeaderWireLifecycleRestore, LeaderWireLifecycleStoreGate},
        v2_apply::RecoveredDecisionApplyWorkerResultV1,
        v2_context::AuthenticatedGenesisBodyV1,
        v2_effects::{EffectQueueConfig, PostFinalityCleanupOutcome, V2EffectExecutor},
        v2_lane_work::{
            MergeSidecarDeferralDisposition, RetainedMergeSidecars, V2LaneWorkAdapter,
            V2LaneWorkError,
        },
        v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig, SerializedV2Runtime},
        v2_worker::{
            CertifiedServeIngressGate, DurableExactOutputServiceOwner,
            KuraReplicaAdvertRefreshOwner, PreparedRecoveredDecisionApplyCompletionV1,
            PreparedRecoveredDecisionFetchBodyCompletionV1,
            PreparedRecoveredLifecycleSignCompletionV1, ProductionV2Services,
            RecoveredDecisionApplyDeferredRetryV1, RecoveredLifecycleProposalExactOutputCaptureV1,
            V2CleanupSupervisor,
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

/// RAII owner of both exact durable ingress gates installed for this launch.
///
/// Leader-wire recovery binds before runtime and service construction. The
/// certified-Serve gate joins immediately after the exact service starts. The
/// ingress stays closed throughout this pre-activation tranche; any later
/// construction error, ordinary wrapper drop, or panic closes it before the
/// gates detach in one queue transaction.
struct ProductionLeaderWireIngressBindingV1 {
    ingress: Arc<FairV2Ingress>,
    gate: Option<Arc<LeaderWireLifecycleStoreGate>>,
    certified_serve_gate: Option<CertifiedServeIngressGate>,
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
            certified_serve_gate: None,
        })
    }

    /// Join the exact service-owned Serve gate to the retained leader gate.
    fn bind_certified_serve(mut self, gate: CertifiedServeIngressGate) -> Result<Self, String> {
        if self.gate.is_none() || self.certified_serve_gate.is_some() {
            self.ingress.close();
            return Err("production ingress binding changed its joint ownership".to_owned());
        }
        if let Err(error) = self.ingress.bind_certified_serve_gate(gate.clone()) {
            self.ingress.close();
            return Err(error);
        }
        self.certified_serve_gate = Some(gate);
        Ok(self)
    }

    fn retire(&mut self) -> Result<(), String> {
        match (self.gate.as_ref(), self.certified_serve_gate.as_ref()) {
            (None, None) => Ok(()),
            (Some(gate), None) => {
                self.ingress.close();
                self.ingress.unbind_leader_wire_lifecycle_gate(gate)?;
                self.gate = None;
                Ok(())
            }
            (None, Some(gate)) => {
                self.ingress.close();
                self.ingress.unbind_certified_serve_gate(gate)?;
                self.certified_serve_gate = None;
                Ok(())
            }
            (Some(leader_wire_gate), Some(certified_serve_gate)) => {
                self.ingress.close();
                if let Err(error) = self
                    .ingress
                    .unbind_height_ingress_gates(certified_serve_gate, leader_wire_gate)
                {
                    // Joint validation failed before mutation. Never fall back
                    // to split teardown across the shared carrier lanes.
                    self.certified_serve_gate = None;
                    self.gate = None;
                    return Err(error);
                }
                self.certified_serve_gate = None;
                self.gate = None;
                Ok(())
            }
        }
    }
}

impl Drop for ProductionLeaderWireIngressBindingV1 {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the sealed production height ingress gates"
            );
        }
    }
}

/// Opaque running stack produced by the sole consuming lifecycle launch.
///
/// Construction does not publish status or open ingress. The separate
/// consuming activation transition arms clocks, installs the completion
/// observer, opens the exact retained ingress, and publishes only through a
/// runner-owned status authority.
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

/// Move-only authority for refreshing the live Certified-Serve retirement cut.
///
/// Only the lifecycle finalization transaction can mint this value. The
/// production service must consume it while still coheld with the exact
/// payload-store owner, after the height output handoff has been sealed. This
/// prevents a sibling from reopening the store or authenticating a caller-
/// supplied signer against a stale startup snapshot.
#[must_use = "the Serve retirement permit must refresh the exact live payload census"]
pub(in crate::sumeragi) struct ProductionLifecycleServeRetirementAuthenticationPermitV1 {
    _seal: ProductionLifecycleServeRetirementAuthenticationPermitSealV1,
}

struct ProductionLifecycleServeRetirementAuthenticationPermitSealV1;

impl Drop for ProductionLifecycleServeRetirementAuthenticationPermitSealV1 {
    fn drop(&mut self) {}
}

/// Private proof that runner readiness and both durable ingress gates retired.
struct ProductionLifecycleRetiredIngressPermitV1 {
    _seal: ProductionLifecycleRetiredIngressPermitSealV1,
}

struct ProductionLifecycleRetiredIngressPermitSealV1;

impl Drop for ProductionLifecycleRetiredIngressPermitSealV1 {
    fn drop(&mut self) {}
}

/// Move-only authority for invoking the runner's exact finalized-output cut.
///
/// The runner helper is sibling-visible only so this lifecycle module can
/// reuse the established handoff transaction. Its private seal prevents any
/// other Sumeragi path from pairing raw services with arbitrary lane work.
#[must_use = "finalized-output rollover authority must cross the exact handoff"]
pub(in crate::sumeragi) struct ProductionLifecycleOutputRolloverPermitV1 {
    _seal: ProductionLifecycleOutputRolloverPermitSealV1,
}

struct ProductionLifecycleOutputRolloverPermitSealV1;

impl Drop for ProductionLifecycleOutputRolloverPermitSealV1 {
    fn drop(&mut self) {}
}

#[cfg(test)]
impl ProductionLifecycleServeRetirementAuthenticationPermitV1 {
    /// Mint the closed permit only for direct payload-store behavior fixtures.
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self {
            _seal: ProductionLifecycleServeRetirementAuthenticationPermitSealV1,
        }
    }
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
        let mut output = match services
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

        let Some(wal_permit) = output.prepare_wal_append_permit() else {
            drop(body);
            drop(preview);
            drop(output);
            restart!();
        };
        if preview
            .append_recovered_lifecycle_proposal_prepare_wal(wal_permit)
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

/// Fail-stop failure while crossing the one-shot live-height boundary.
#[derive(Debug, Error)]
#[must_use = "failed lifecycle activation requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecycleActivationErrorV1 {
    /// The process-wide output barrier was already closed.
    #[error("canonical consensus output is closed")]
    OutputClosed,
    /// Pacemaker clocks could not be armed exactly once.
    #[error("live lifecycle clocks could not be armed: {0}")]
    RuntimeClock(#[source] super::super::v2_runtime::RuntimeClockError),
    /// The armed reducer could not produce its exact activation status.
    #[error("live lifecycle status could not be projected: {0}")]
    Status(#[source] super::super::v2::AdapterError),
    /// The launch lost its sole completion-observer permit or live worker.
    #[error("live lifecycle completion observer could not activate: {0}")]
    CompletionObserver(String),
    /// The runner-owned ingress/status authority rejected the launched stack.
    #[error("runner lifecycle activation failed: {0}")]
    Runner(String),
}

/// Fail-stop failure while consuming an activated height into final rollover.
#[derive(Debug, Error)]
#[must_use = "failed lifecycle finalization requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecycleFinalizationErrorV1 {
    /// Executor, lifecycle owner, or dedicated completion ownership is not quiescent.
    #[error("activated lifecycle height is not ready for final rollover")]
    NotReady,
    /// The runner readiness owner no longer names the launched ingress.
    #[error("activated lifecycle runner retirement failed: {0}")]
    Runner(String),
    /// The jointly bound ingress gates could not retire as one height owner.
    #[error("activated lifecycle ingress-gate retirement failed: {0}")]
    Ingress(String),
    /// The process-wide output barrier was already closed.
    #[error("canonical consensus output is closed during lifecycle finalization")]
    OutputClosed,
    /// The drained executor could not yield its exact Kura finality authority.
    #[error("effect executor finalization failed: {0}")]
    Executor(#[source] super::super::v2_effects::EffectExecutorError),
    /// The serialized reducer rejected the exact Kura receipt/artifact pair.
    #[error("serialized adapter finalization failed: {0}")]
    Adapter(#[source] super::super::v2::AdapterError),
    /// Lane/output rollover did not reach its one-shot durable handoff.
    #[error("finalized lifecycle output rollover failed: {0}")]
    OutputRollover(String),
    /// The post-handoff registry or Certified-Serve census changed.
    #[error("finalized lifecycle retirement census failed: {0}")]
    RetirementCensus(String),
}

/// Activated height after ingress retirement and reducer/WAL finalization.
///
/// Services and the complete lifecycle owner remain sealed here until the
/// existing lane/output transaction mints its durable handoff. There is no
/// service, receipt, artifact, or owner parts accessor.
#[must_use = "finalized lifecycle output rollover must be consumed"]
pub(in crate::sumeragi) struct FinalizedProductionLifecycleRolloverV1 {
    owner: ProductionLifecycleOwnerV1,
    services: ProductionV2Services,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    wal_retirement_warning: Option<String>,
    retired_ingress: ProductionLifecycleRetiredIngressPermitV1,
}

/// Height after the exact service/transport output handoff is sealed.
///
/// This intermediate state still owns the live lifecycle stores. Its next
/// consuming transition fsyncs all-row retirement before clean worker teardown
/// becomes available.
#[must_use = "post-handoff lifecycle stores must be retired"]
pub(in crate::sumeragi) struct ProductionLifecyclePostOutputHandoffV1 {
    owner: ProductionLifecycleOwnerV1,
    services: ProductionV2Services,
    receipt: KuraV2CommitReceipt,
    wal_retirement_warning: Option<String>,
    retired_ingress: ProductionLifecycleRetiredIngressPermitV1,
    retained_serve_payloads:
        BTreeSet<crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadId>,
}

/// Services whose output and lifecycle durability owners are fully retired.
#[must_use = "cleanup-ready lifecycle services must be explicitly finished"]
pub(in crate::sumeragi) struct ProductionLifecycleCleanupReadyV1 {
    services: ProductionV2Services,
    receipt: KuraV2CommitReceipt,
    wal_retirement_warning: Option<String>,
}

/// Final local-cleanup diagnostics after every consensus owner was retired.
#[derive(Clone, Debug)]
#[must_use = "post-finality cleanup diagnostics must be observed"]
pub(in crate::sumeragi) struct ProductionLifecycleFinalizationOutcomeV1 {
    cleanup: PostFinalityCleanupOutcome,
    wal_retirement_warning: Option<String>,
}

impl ProductionLifecycleFinalizationOutcomeV1 {
    /// Borrow the adapter WAL/serviced-candidate cleanup warning, if retained.
    pub(in crate::sumeragi) fn wal_retirement_warning(&self) -> Option<&str> {
        self.wal_retirement_warning.as_deref()
    }

    /// Borrow the ordered service/body/chunk cleanup diagnostics.
    pub(in crate::sumeragi) const fn cleanup(&self) -> &PostFinalityCleanupOutcome {
        &self.cleanup
    }
}

/// Opaque lifecycle stack after clocks, diagnostics, status, and ingress activate.
#[must_use = "the activated lifecycle stack owns the live height"]
pub(in crate::sumeragi) struct ActivatedProductionLifecycleV1 {
    // Drop readiness/ingress ownership before the launched stack unbinds its
    // durable gates. Finalization consumes the same authority explicitly.
    runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1,
    launched: LaunchedProductionLifecycleV1,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum ProductionLifecycleActivationPublicationV1 {
    Runner(super::super::v2_runner::ProductionLifecycleRunnerActivationV1),
    RecoveredCompleteTip {
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        retirement: super::ledger::RetiredRecoveredCompleteTipActivationAuthorityV1,
    },
}

impl ProductionLifecycleActivationPublicationV1 {
    fn open_and_publish(
        self,
        ingress: &Arc<FairV2Ingress>,
        status: wire::SumeragiV2Status,
    ) -> Result<
        super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1,
        ProductionLifecycleActivationErrorV1,
    > {
        let result = match self {
            Self::Runner(runner) => runner.open_and_publish(ingress, status),
            Self::RecoveredCompleteTip { runner, retirement } => {
                runner.open_and_publish(ingress, retirement, status)
            }
        };
        result.map_err(|error| ProductionLifecycleActivationErrorV1::Runner(error.to_string()))
    }
}

impl ProductionLifecycleOwnerV1 {
    #[cfg_attr(not(test), allow(dead_code))]
    fn refresh_live_serve_retirement_cut(
        &mut self,
        services: &ProductionV2Services,
        _retired_ingress: &ProductionLifecycleRetiredIngressPermitV1,
    ) -> Result<
        BTreeSet<crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadId>,
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServeRetirementAuthenticationErrorV1,
    >
    {
        let body_store_identity = self.body_store_identity.as_ref().ok_or(
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServeRetirementAuthenticationErrorV1::ForeignServiceOwner,
        )?;
        if !self
            .registry
            .registry_mut()
            .exactly_covers_finalization_work(&self.coordinator)
        {
            return Err(
                crate::sumeragi::v2_certified_serve_payload_store::CertifiedServeRetirementAuthenticationErrorV1::InvalidLifecycleCensus,
            );
        }
        let refreshed = services.authenticate_current_lifecycle_serve_retirement(
            ProductionLifecycleServeRetirementAuthenticationPermitV1 {
                _seal: ProductionLifecycleServeRetirementAuthenticationPermitSealV1,
            },
            &self.verified,
            &self.payload_store,
            body_store_identity,
        )?;
        let ledger = super::ledger::LifecycleLedgerV1::from_coordinator(&self.coordinator)
            .map_err(|_| {
                crate::sumeragi::v2_certified_serve_payload_store::CertifiedServeRetirementAuthenticationErrorV1::InvalidLifecycleCensus
            })?;
        let retained = super::open::authenticate_live_finalization_serve_census(
            &self.verified,
            &ledger,
            &self.coordinator,
            &refreshed,
        )
        .map_err(|_| {
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServeRetirementAuthenticationErrorV1::InvalidLifecycleCensus
        })?;
        self.serve_payloads = refreshed;
        Ok(retained)
    }
}

impl LaunchedProductionLifecycleV1 {
    /// Cross the ordinary/current/snapshot live-height boundary exactly once.
    #[allow(dead_code)]
    pub(in crate::sumeragi) fn activate(
        self,
        now: Instant,
        runner: super::super::v2_runner::ProductionLifecycleRunnerActivationV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        self.activate_with(
            now,
            ProductionLifecycleActivationPublicationV1::Runner(runner),
        )
    }

    /// Cross the CompleteTip boundary without separating retired H from H+1.
    #[allow(dead_code)]
    pub(super) fn activate_recovered_complete_tip(
        self,
        now: Instant,
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        retirement: super::ledger::RetiredRecoveredCompleteTipActivationAuthorityV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        self.activate_with(
            now,
            ProductionLifecycleActivationPublicationV1::RecoveredCompleteTip { runner, retirement },
        )
    }

    fn activate_with(
        mut self,
        now: Instant,
        publication: ProductionLifecycleActivationPublicationV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        let output_guard = self.services.lifecycle_output_guard();
        let activation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleActivationErrorV1::OutputClosed)?;
        self.executor
            .arm_live_clocks(now)
            .map_err(ProductionLifecycleActivationErrorV1::RuntimeClock)?;
        let status = self
            .executor
            .successor_activation_status_snapshot()
            .map_err(ProductionLifecycleActivationErrorV1::Status)?;
        let observer = self.completion_observer_activation.take().ok_or_else(|| {
            ProductionLifecycleActivationErrorV1::CompletionObserver(
                "launched lifecycle lost its one-shot observer permit".to_owned(),
            )
        })?;
        self.services
            .activate_effect_completion_observer(observer)
            .map_err(ProductionLifecycleActivationErrorV1::CompletionObserver)?;
        let runner_activation =
            publication.open_and_publish(&self.leader_wire_ingress_binding.ingress, status)?;
        activation.complete();
        Ok(ActivatedProductionLifecycleV1 {
            runner_activation,
            launched: self,
        })
    }
}

impl ActivatedProductionLifecycleV1 {
    /// Consume the activated height after executor and lifecycle work quiesce.
    ///
    /// Readiness closes before both ingress gates retire jointly. Only then is
    /// the executor consumed and the adapter's exact WAL retired under one
    /// fail-stop output operation. Every error consumes the height and leaves
    /// service teardown armed for restart.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_finalized_rollover(
        mut self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<FinalizedProductionLifecycleRolloverV1, ProductionLifecycleFinalizationErrorV1>
    {
        if !self.launched.executor.ready_to_finish()
            || self
                .launched
                .recovered_decision_fetch_body_completion
                .is_some()
            || self.launched.recovered_lifecycle_sign_completion.is_some()
            || self.launched.completion_observer_activation.is_some()
            || !self
                .launched
                .owner
                .registry
                .registry_mut()
                .exactly_covers_finalization_work(&self.launched.owner.coordinator)
        {
            return Err(ProductionLifecycleFinalizationErrorV1::NotReady);
        }

        let Self {
            runner_activation,
            mut launched,
        } = self;
        runner_activation
            .retire(&launched.leader_wire_ingress_binding.ingress)
            .map_err(|error| ProductionLifecycleFinalizationErrorV1::Runner(error.to_string()))?;
        launched
            .leader_wire_ingress_binding
            .retire()
            .map_err(ProductionLifecycleFinalizationErrorV1::Ingress)?;
        let retired_ingress = ProductionLifecycleRetiredIngressPermitV1 {
            _seal: ProductionLifecycleRetiredIngressPermitSealV1,
        };

        let LaunchedProductionLifecycleV1 {
            owner,
            executor,
            services,
            recovered_decision_fetch_body_completion,
            recovered_lifecycle_sign_completion,
            completion_observer_activation,
            leader_wire_ingress_binding,
        } = launched;
        debug_assert!(recovered_decision_fetch_body_completion.is_none());
        debug_assert!(recovered_lifecycle_sign_completion.is_none());
        debug_assert!(completion_observer_activation.is_none());
        drop(recovered_decision_fetch_body_completion);
        drop(recovered_lifecycle_sign_completion);
        drop(completion_observer_activation);
        drop(leader_wire_ingress_binding);

        let (runtime, receipt, artifact) = executor
            .into_finalized_parts()
            .map_err(ProductionLifecycleFinalizationErrorV1::Executor)?;
        let output_guard = services.lifecycle_output_guard();
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleFinalizationErrorV1::OutputClosed)?;
        let finalized = runtime
            .into_driver()
            .finish_height(&receipt, &artifact)
            .map_err(ProductionLifecycleFinalizationErrorV1::Adapter)?;
        operation.complete();

        Ok(FinalizedProductionLifecycleRolloverV1 {
            owner,
            services,
            receipt,
            artifact,
            wal_retirement_warning: finalized.into_wal_retirement_warning(),
            retired_ingress,
        })
    }

    /// Exercise the exact empty-output post-handoff retirement transaction.
    #[cfg(test)]
    pub(in crate::sumeragi) fn retire_lifecycle_stores_for_test(
        self,
        receipt: KuraV2CommitReceipt,
    ) -> Result<ProductionLifecycleCleanupReadyV1, String> {
        let Self {
            runner_activation,
            mut launched,
        } = self;
        runner_activation
            .retire(&launched.leader_wire_ingress_binding.ingress)
            .map_err(|error| error.to_string())?;
        launched
            .leader_wire_ingress_binding
            .retire()
            .map_err(|error| error.to_string())?;
        let retired_ingress = ProductionLifecycleRetiredIngressPermitV1 {
            _seal: ProductionLifecycleRetiredIngressPermitSealV1,
        };
        launched
            .services
            .seal_empty_exact_output_for_lifecycle_retirement_test()?;
        let retained_serve_payloads = launched
            .owner
            .refresh_live_serve_retirement_cut(&launched.services, &retired_ingress)
            .map_err(|error| error.to_string())?;
        let LaunchedProductionLifecycleV1 {
            owner,
            executor,
            services,
            recovered_decision_fetch_body_completion,
            recovered_lifecycle_sign_completion,
            completion_observer_activation,
            leader_wire_ingress_binding,
        } = launched;
        assert!(recovered_decision_fetch_body_completion.is_none());
        assert!(recovered_lifecycle_sign_completion.is_none());
        assert!(completion_observer_activation.is_none());
        drop(executor);
        drop(recovered_decision_fetch_body_completion);
        drop(recovered_lifecycle_sign_completion);
        drop(completion_observer_activation);
        drop(leader_wire_ingress_binding);
        ProductionLifecyclePostOutputHandoffV1 {
            owner,
            services,
            receipt,
            wal_retirement_warning: None,
            retired_ingress,
            retained_serve_payloads,
        }
        .retire_lifecycle_stores()
        .map_err(|error| error.to_string())
    }

    /// Borrow the live owner/runtime/service triple only from the serialized runner.
    ///
    /// The callback cannot outlive this borrow or move fields out of the opaque
    /// activated stack. This is the sole bridge intended for the ordinary live
    /// loop while its fixed operations are migrated behind lifecycle methods.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_runtime<R>(
        &mut self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        operation: impl FnOnce(
            &mut ProductionLifecycleOwnerV1,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> R,
    ) -> R {
        operation(
            &mut self.launched.owner,
            &mut self.launched.executor,
            &mut self.launched.services,
        )
    }
}

impl FinalizedProductionLifecycleRolloverV1 {
    /// Borrow the exact Kura/finality pair while constructing its successor.
    pub(in crate::sumeragi) const fn finality(
        &self,
    ) -> (&KuraV2CommitReceipt, &wire::finality::V2FinalityArtifact) {
        (&self.receipt, &self.artifact)
    }

    /// Seal every finalized output owner before touching lifecycle stores.
    #[allow(dead_code, clippy::too_many_arguments, clippy::result_large_err)]
    pub(in crate::sumeragi) fn rollover_outputs(
        self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        lane_work: V2LaneWorkAdapter,
        successor: &wire::HeightContext,
        control_queue_capacity: usize,
    ) -> Result<
        (
            ProductionLifecyclePostOutputHandoffV1,
            RetainedMergeSidecars,
        ),
        ProductionLifecycleFinalizationErrorV1,
    > {
        let Self {
            mut owner,
            services,
            receipt,
            artifact,
            wal_retirement_warning,
            retired_ingress,
        } = self;
        let retained = super::super::v2_runner::rollover_finalized_height_outputs_for_lifecycle(
            ProductionLifecycleOutputRolloverPermitV1 {
                _seal: ProductionLifecycleOutputRolloverPermitSealV1,
            },
            lane_work,
            &services,
            &receipt,
            &artifact,
            successor,
            control_queue_capacity,
        )
        .map_err(ProductionLifecycleFinalizationErrorV1::OutputRollover)?;
        let retained_serve_payloads = owner
            .refresh_live_serve_retirement_cut(&services, &retired_ingress)
            .map_err(|error| {
                ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string())
            })?;
        Ok((
            ProductionLifecyclePostOutputHandoffV1 {
                owner,
                services,
                receipt,
                wal_retirement_warning,
                retired_ingress,
                retained_serve_payloads,
            },
            retained,
        ))
    }
}

impl ProductionLifecyclePostOutputHandoffV1 {
    /// Retire every payload, ledger row, logical owner, and concrete carrier.
    ///
    /// Payload tombstones publish first. The exact all-row LedgerV1 successor
    /// then fsyncs before the assertion-only registry/coordinator consumption. An
    /// armed output operation spans the complete cut, so an ambiguous payload
    /// or ledger publication cannot return a retryable live owner.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn retire_lifecycle_stores(
        self,
    ) -> Result<ProductionLifecycleCleanupReadyV1, ProductionLifecycleFinalizationErrorV1> {
        let Self {
            owner,
            services,
            receipt,
            wal_retirement_warning,
            retired_ingress,
            retained_serve_payloads,
        } = self;
        let output_guard = services.lifecycle_output_guard();
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleFinalizationErrorV1::OutputClosed)?;
        let ProductionLifecycleOwnerV1 {
            verified,
            coordinator,
            registry,
            mut payload_store,
            serve_payloads,
            body_store,
            body_store_identity,
            kura_binding,
            apply_service,
            adapter_startup,
        } = owner;
        let current =
            super::ledger::LifecycleLedgerV1::from_coordinator(&coordinator).map_err(|error| {
                ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string())
            })?;
        let refreshed = payload_store
            .retire_authenticated_cut(serve_payloads, &retained_serve_payloads)
            .map_err(|error| {
                ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string())
            })?;
        let reconciliation =
            super::open::reconcile_complete_tip_serve_retirement(&current, refreshed).map_err(
                |error| ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string()),
            )?;
        let staged = current
            .stage_finalized_height_all_row_retirement(reconciliation)
            .map_err(|error| {
                ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string())
            })?;
        let publication = coordinator
            .persist_exact_finalization_successor(staged)
            .map_err(|error| {
                ProductionLifecycleFinalizationErrorV1::RetirementCensus(error.to_string())
            })?;

        publication.consume_owners(registry);
        drop(retired_ingress);
        drop(verified);
        drop(payload_store);
        drop(body_store);
        drop(body_store_identity);
        drop(kura_binding);
        drop(apply_service);
        drop(adapter_startup);
        operation.complete();
        Ok(ProductionLifecycleCleanupReadyV1 {
            services,
            receipt,
            wal_retirement_warning,
        })
    }
}

impl ProductionLifecycleCleanupReadyV1 {
    /// Permit normal worker cleanup only after every durable handoff completed.
    pub(in crate::sumeragi) fn finish_cleanup(
        mut self,
        cleanup_timeout: Duration,
        supervisor: &mut V2CleanupSupervisor,
    ) -> ProductionLifecycleFinalizationOutcomeV1 {
        self.services.allow_clean_shutdown();
        let cleanup = self
            .services
            .finish_height(self.receipt, cleanup_timeout, supervisor);
        ProductionLifecycleFinalizationOutcomeV1 {
            cleanup,
            wal_retirement_warning: self.wal_retirement_warning,
        }
    }
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
        let payload_store_identity = self.payload_store.instance_identity();
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
            payload_store_identity.clone(),
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
            || !services.matches_lifecycle_payload_store(&payload_store_identity)
        {
            return Err(ProductionLifecycleLaunchErrorV1::OwnershipMismatch);
        }
        let certified_serve_gate = services
            .certified_serve_ingress_gate()
            .map_err(ProductionLifecycleLaunchErrorV1::Services)?;
        let leader_wire_ingress_binding = leader_wire_ingress_binding
            .bind_certified_serve(certified_serve_gate)
            .map_err(ProductionLifecycleLaunchErrorV1::Services)?;
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
        ingress.require_certified_serve_gate();
        ingress.require_leader_wire_lifecycle_gate();
        ingress.state.lock().leader_wire_max_chunk_count = 2;

        let (first_serve_gate, first_ordinals) =
            crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
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
            first_ordinals,
            context_id,
            HEIGHT,
        )
        .expect("bind the exact launch gate")
        .bind_certified_serve(first_serve_gate.clone())
        .expect("join the exact certified Serve gate");
        assert!(
            ingress
                .state
                .lock()
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &first_gate))
        );
        assert!(
            ingress
                .state
                .lock()
                .certified_serve_gate
                .as_ref()
                .is_some_and(|bound| bound.ptr_eq(&first_serve_gate))
        );
        binding
            .retire()
            .expect("explicit retirement detaches both exact launch gates");
        binding
            .retire()
            .expect("explicit retirement remains idempotent");
        {
            let state = ingress.state.lock();
            assert!(
                state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none()
            );
        }

        let (drop_serve_gate, drop_ordinals) =
            crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
        let (drop_gate, drop_restore) = empty_leader_wire_gate_for_binding_test(
            &directory, "drop.wal", context_id, HEIGHT, &validator,
        );
        let binding = ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&ingress),
            Arc::clone(&drop_gate),
            drop_restore,
            drop_ordinals,
            context_id,
            HEIGHT,
        )
        .expect("rebind the exact launch gate")
        .bind_certified_serve(drop_serve_gate)
        .expect("rejoin the certified Serve gate");
        drop(binding);
        {
            let state = ingress.state.lock();
            assert!(
                state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none(),
                "Drop must detach both exact launch gates"
            );
        }

        let (mismatched_serve_gate, _) =
            crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
        let (mismatch_gate, mismatch_restore) = empty_leader_wire_gate_for_binding_test(
            &directory,
            "mismatch.wal",
            context_id,
            HEIGHT,
            &validator,
        );
        let mismatch = match ProductionLeaderWireIngressBindingV1::bind(
            Arc::clone(&ingress),
            mismatch_gate,
            mismatch_restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            HEIGHT,
        )
        .expect("bind the leader gate before the mismatched Serve join")
        .bind_certified_serve(mismatched_serve_gate)
        {
            Ok(_) => panic!("a foreign lifecycle ordinal source passed the joint join"),
            Err(error) => error,
        };
        assert!(mismatch.contains("actor-global lifecycle ordinal source"));
        {
            let state = ingress.state.lock();
            assert!(
                state.leader_wire_lifecycle_gate.is_none() && state.certified_serve_gate.is_none(),
                "a failed joint join must drop the retained leader binding"
            );
        }

        let (incumbent_gate, incumbent_restore) = empty_leader_wire_gate_for_binding_test(
            &directory,
            "incumbent.wal",
            context_id,
            HEIGHT,
            &validator,
        );
        let (incumbent_serve_gate, incumbent_ordinals) =
            crate::sumeragi::v2_worker::tests::certified_serve_ingress_gate_fixture();
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&incumbent_gate),
                incumbent_restore,
                incumbent_ordinals,
                context_id,
                HEIGHT,
            )
            .expect("bind the incumbent gate");
        ingress
            .bind_certified_serve_gate(incumbent_serve_gate.clone())
            .expect("bind the incumbent certified Serve gate");
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
            .unbind_height_ingress_gates(&incumbent_serve_gate, &incumbent_gate)
            .expect("clean up both incumbent bindings");
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
        let finalized_output_source = include_str!("v2_runner/finalized_output_rollover.rs");
        let runner_tests_source = include_str!("v2_runner_tests.rs");
        let coordinator_source = include_str!("v2_lifecycle_coordinator.rs");
        let ledger_source = include_str!("v2_lifecycle_ledger.rs");
        let payload_store_source = include_str!("v2_certified_serve_payload_store.rs");
        let lifecycle_open_source = include_str!("v2_lifecycle_open.rs");
        let registry_validate_source =
            include_str!("v2_lifecycle_work_registry_validate_recovery.rs");
        let lifecycle_startup_test_source =
            include_str!("tests/v2_adapter_04b_lifecycle_startup.rs");
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
        let activation_impl = bound_launch
            .find("impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1")
            .expect("the launched H+1 join has one consuming activation implementation");
        let activation_consume = bound_launch
            .find("let Self {\n            launched,\n            retirement,\n        } = self;")
            .expect(
                "CompleteTip activation consumes the still-joined launched owner and retirement",
            );
        let typed_activation = bound_launch
            .find("launched.activate_recovered_complete_tip(now, runner, retirement)")
            .expect("CompleteTip activation enters only the typed publication boundary");
        assert!(
            bind < launch
                && launch < consume
                && consume < generic_launch
                && generic_launch < retained
                && retained < wrapper
                && wrapper < activation_impl
                && activation_impl < activation_consume
                && activation_consume < typed_activation
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
        let certified_serve_gate = launch
            .find("services\n            .certified_serve_ingress_gate()")
            .expect("launch obtains the exact service-owned Serve gate");
        let joint_ingress_bind = launch
            .find(".bind_certified_serve(certified_serve_gate)")
            .expect("launch joins both durable ingress gates before success");
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
                && worker < certified_serve_gate
                && certified_serve_gate < joint_ingress_bind
                && joint_ingress_bind < identity
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
        assert!(source.contains("certified_serve_gate: Option<CertifiedServeIngressGate>"));
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
        let joint_unbind = leader_wire_drop
            .find(".unbind_height_ingress_gates(certified_serve_gate, leader_wire_gate)")
            .expect("completed launch retirement detaches both exact gates atomically");
        assert!(close < joint_unbind);
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

        let lifecycle_activation = source
            .split_once("fn activate_with(")
            .expect("the launched lifecycle has one consuming activation transaction")
            .1
            .split_once("impl ActivatedProductionLifecycleV1")
            .expect("activation ends before the runner-borrowed live type state")
            .0;
        let activation_guard = lifecycle_activation
            .find("begin_fail_stop_operation()")
            .expect("activation arms the process-wide fail-stop boundary");
        let clocks = lifecycle_activation
            .find("arm_live_clocks(now)")
            .expect("activation arms live clocks");
        let status = lifecycle_activation
            .find("successor_activation_status_snapshot()")
            .expect("activation projects status only after clocks arm");
        let observer = lifecycle_activation
            .find("completion_observer_activation.take()")
            .expect("activation consumes the sole observer permit");
        let register_observer = lifecycle_activation
            .find("activate_effect_completion_observer(observer)")
            .expect("activation installs the completion observer");
        let publish = lifecycle_activation
            .find("publication.open_and_publish(")
            .expect("activation delegates ingress and status to runner authority");
        let complete = lifecycle_activation
            .find("activation.complete()")
            .expect("activation releases output only after publication");
        let activated = lifecycle_activation
            .find("ActivatedProductionLifecycleV1 {\n            runner_activation,\n            launched: self,")
            .expect("activation returns the sole opaque live owner");
        assert!(
            activation_guard < clocks
                && clocks < status
                && status < observer
                && observer < register_observer
                && register_observer < publish
                && publish < complete
                && complete < activated
        );
        assert!(!lifecycle_activation.contains("set_v2_status"));
        assert!(!lifecycle_activation.contains("into_parts"));

        let activated_owner = source
            .split_once("struct ActivatedProductionLifecycleV1")
            .expect("activation returns one opaque owner type state")
            .1
            .split_once("enum ProductionLifecycleActivationPublicationV1")
            .expect("the activated owner declaration ends before publication authority")
            .0;
        assert!(activated_owner.contains(
            "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1"
        ));
        assert!(activated_owner.contains("launched: LaunchedProductionLifecycleV1"));
        assert!(
            activated_owner.find("runner_activation:").unwrap()
                < activated_owner.find("launched:").unwrap(),
            "runner readiness must drop before the launched stack unbinds durable gates"
        );
        for forbidden in [
            "pub launched:",
            "pub(crate) launched:",
            "pub(in crate::sumeragi) launched:",
            "pub runner_activation:",
            "pub(crate) runner_activation:",
            "pub(in crate::sumeragi) runner_activation:",
            "impl Clone for ActivatedProductionLifecycleV1",
            "impl Copy for ActivatedProductionLifecycleV1",
        ] {
            assert!(!activated_owner.contains(forbidden));
        }
        let activated_borrow = source
            .split_once("impl ActivatedProductionLifecycleV1")
            .expect("the activated owner has one runner-borrow surface")
            .1
            .split_once("impl ProductionLifecycleOwnerV1")
            .expect("the activated owner surface ends before launch helpers")
            .0;
        for required in [
            "fn with_runner_runtime<R>(",
            "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
            "&mut self.launched.owner",
            "&mut self.launched.executor",
            "&mut self.launched.services",
        ] {
            assert!(activated_borrow.contains(required));
        }
        for forbidden in [
            "into_parts",
            "fn into_owner(",
            "fn into_executor(",
            "fn into_services(",
            "pub launched:",
            "pub(crate) launched:",
        ] {
            assert!(!activated_borrow.contains(forbidden));
        }

        let serve_retirement = source
            .split_once("fn refresh_live_serve_retirement_cut(")
            .expect("live Serve retirement has one launch-private join")
            .1
            .split_once("/// Cross the ordinary/current/snapshot live-height boundary")
            .expect("live Serve retirement stays bounded before activation")
            .0;
        let registry_census = serve_retirement
            .find("exactly_covers_finalization_work(&self.coordinator)")
            .expect("retirement rejoins the exact live concrete registry");
        let service_census = serve_retirement
            .find("authenticate_current_lifecycle_serve_retirement(")
            .expect("retirement authenticates through the exact launched service");
        let ledger = serve_retirement
            .find("LifecycleLedgerV1::from_coordinator(&self.coordinator)")
            .expect("retirement derives the current ledger from the same owner");
        let payload_census = serve_retirement
            .find("authenticate_live_finalization_serve_census(")
            .expect("retirement joins ledger rows and admission-wait payloads");
        let install = serve_retirement
            .find("self.serve_payloads = refreshed")
            .expect("retirement replaces the stale startup cut only after authentication");
        assert!(
            registry_census < service_census
                && service_census < ledger
                && ledger < payload_census
                && payload_census < install
        );
        assert!(
            serve_retirement
                .contains("_retired_ingress: &ProductionLifecycleRetiredIngressPermitV1")
        );
        assert!(!serve_retirement.contains("CertifiedServePayloadStoreV1::open("));
        for authority in [
            "ProductionLifecycleServeRetirementAuthenticationPermitV1",
            "ProductionLifecycleRetiredIngressPermitV1",
        ] {
            assert!(!source.contains(&format!("impl Clone for {authority}")));
            assert!(!source.contains(&format!("impl Copy for {authority}")));
        }
        let fixture_retirement = activated_borrow
            .split_once("fn retire_lifecycle_stores_for_test(")
            .expect("activation behavior has one consuming retirement fixture")
            .1
            .split_once("/// Borrow the live owner/runtime/service triple")
            .expect("retirement fixture ends before the ordinary runner borrow")
            .0;
        let readiness_retire = fixture_retirement
            .find("runner_activation\n            .retire(")
            .expect("retirement clears runner readiness first");
        let gates_retire = fixture_retirement
            .find("leader_wire_ingress_binding\n            .retire()")
            .expect("retirement detaches both ingress gates second");
        let output_handoff = fixture_retirement
            .find("seal_empty_exact_output_for_lifecycle_retirement_test()")
            .expect("fixture seals its exact empty output handoff");
        let refresh = fixture_retirement
            .find("refresh_live_serve_retirement_cut(&launched.services, &retired_ingress)")
            .expect("fixture refreshes Serve only after output handoff");
        let retirement = fixture_retirement
            .find(".retire_lifecycle_stores()")
            .expect("fixture exercises the post-handoff durable retirement tail");
        assert!(
            readiness_retire < gates_retire
                && gates_retire < output_handoff
                && output_handoff < refresh
                && refresh < retirement
        );

        let activated_finalization = activated_borrow
            .split_once("fn into_finalized_rollover(")
            .expect("activated owner has one consuming finalization")
            .1
            .split_once("/// Exercise the exact empty-output post-handoff retirement transaction")
            .expect("production finalization ends before its behavior fixture")
            .0;
        let executor_ready = activated_finalization
            .find("executor.ready_to_finish()")
            .expect("finalization first proves exact executor quiescence");
        let registry_ready = activated_finalization
            .find("exactly_covers_finalization_work")
            .expect("finalization first proves exact lifecycle-owner quiescence");
        let runner_retire = activated_finalization
            .find("runner_activation\n            .retire(")
            .expect("finalization clears runner readiness and ingress");
        let gate_retire = activated_finalization
            .find("leader_wire_ingress_binding\n            .retire()")
            .expect("finalization jointly retires both durable ingress gates");
        let executor_consume = activated_finalization
            .find("executor\n            .into_finalized_parts()")
            .expect("finalization consumes the exact executor after gate retirement");
        let operation = activated_finalization
            .find("begin_fail_stop_operation()")
            .expect("adapter finalization is fail-stop guarded");
        let adapter_finish = activated_finalization
            .find(".finish_height(&receipt, &artifact)")
            .expect("the serialized adapter consumes exact Kura finality");
        let operation_complete = activated_finalization
            .find("operation.complete()")
            .expect("adapter finalization completes the fail-stop operation last");
        assert!(executor_ready < runner_retire && registry_ready < runner_retire);
        assert!(
            runner_retire < gate_retire
                && gate_retire < executor_consume
                && executor_consume < operation
                && operation < adapter_finish
                && adapter_finish < operation_complete
        );

        let rollover = source
            .split_once("impl FinalizedProductionLifecycleRolloverV1")
            .expect("finalized owner has one output-rollover implementation")
            .1
            .split_once("impl ProductionLifecyclePostOutputHandoffV1")
            .expect("output rollover ends before lifecycle-store retirement")
            .0;
        let sealed_output = rollover
            .find("rollover_finalized_height_outputs_for_lifecycle(")
            .expect("finalized owner invokes the existing exact output handoff");
        let output_permit = rollover
            .find("ProductionLifecycleOutputRolloverPermitV1 {")
            .expect("only the finalized owner mints the sibling-call permit");
        let serve_refresh = rollover
            .find("refresh_live_serve_retirement_cut(&services, &retired_ingress)")
            .expect("Serve census refresh follows durable output handoff");
        assert!(sealed_output < output_permit && output_permit < serve_refresh);
        assert!(finalized_output_source.contains(
            "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1"
        ));

        let store_retirement = source
            .split_once("impl ProductionLifecyclePostOutputHandoffV1")
            .expect("post-output owner has one lifecycle-store implementation")
            .1
            .split_once("impl ProductionLifecycleCleanupReadyV1")
            .expect("store retirement ends before clean worker teardown")
            .0;
        let retirement_operation = store_retirement
            .find("begin_fail_stop_operation()")
            .expect("store retirement arms process-wide fail-stop ownership");
        let payload_retire = store_retirement
            .find(".retire_authenticated_cut(serve_payloads, &retained_serve_payloads)")
            .expect("the exact live payload cut retires before LedgerV1");
        let ledger_stage = store_retirement
            .find(".stage_finalized_height_all_row_retirement(reconciliation)")
            .expect("all rows stage from the refreshed Serve cut");
        let ledger_publish = store_retirement
            .find(".persist_exact_finalization_successor(staged)")
            .expect("the opaque staged successor fsyncs exactly once");
        let owner_consume = store_retirement
            .find("publication.consume_owners(registry)")
            .expect("only the published token consumes logical and concrete owners");
        let retirement_complete = store_retirement
            .find("operation.complete()")
            .expect("store retirement releases fail-stop ownership last");
        assert!(
            retirement_operation < payload_retire
                && payload_retire < ledger_stage
                && ledger_stage < ledger_publish
                && ledger_publish < owner_consume
                && owner_consume < retirement_complete
        );
        let cleanup = source
            .split_once("impl ProductionLifecycleCleanupReadyV1")
            .expect("cleanup-ready owner has one consuming cleanup")
            .1
            .split_once("impl ProductionLifecycleOwnerV1")
            .expect("cleanup-ready surface ends before launch construction")
            .0;
        assert!(
            cleanup
                .find("self.services.allow_clean_shutdown()")
                .expect("only cleanup-ready state permits normal service Drop")
                < cleanup
                    .find(".finish_height(self.receipt, cleanup_timeout, supervisor)")
                    .expect("clean service teardown follows explicit permission")
        );

        for state in [
            "FinalizedProductionLifecycleRolloverV1",
            "ProductionLifecyclePostOutputHandoffV1",
            "ProductionLifecycleCleanupReadyV1",
            "StagedFinalizationRetirementV1",
            "PublishedFinalizationRetirementV1",
            "ProductionLifecycleOutputRolloverPermitV1",
        ] {
            assert!(!source.contains(&format!("impl Clone for {state}")));
            assert!(!source.contains(&format!("impl Copy for {state}")));
            assert!(!ledger_source.contains(&format!("impl Clone for {state}")));
            assert!(!ledger_source.contains(&format!("impl Copy for {state}")));
            let declaration_source = if source.contains(&format!("struct {state}")) {
                source
            } else {
                ledger_source
            };
            let start = declaration_source
                .find(&format!("struct {state}"))
                .unwrap_or_else(|| panic!("missing opaque finalization state {state}"));
            let prefix = &declaration_source[..start];
            let declaration_start = prefix.rfind("\n\n").unwrap_or(0);
            let declaration_end = declaration_source[start..]
                .find("\n}")
                .map(|offset| start + offset)
                .expect("opaque finalization declaration is closed");
            let declaration = &declaration_source[declaration_start..declaration_end];
            assert!(!declaration.contains("Clone"));
            assert!(!declaration.contains("Copy"));
            assert!(!declaration.contains("pub owner:"));
            assert!(!declaration.contains("pub coordinator:"));
            assert!(!declaration.contains("pub services:"));
            assert!(!declaration.contains("pub current:"));
            assert!(!declaration.contains("pub retired:"));
        }
        let published_retirement = ledger_source
            .split_once("fn persist_exact_finalization_successor(")
            .expect("coordinator has one consuming finalization publication")
            .1
            .split_once("#[cfg(test)]")
            .expect("finalization publication ends before test helpers")
            .0;
        let consume_coordinator = published_retirement
            .find("self,")
            .expect("publication consumes the exact coordinator instance");
        let exact_source = published_retirement
            .find("LifecycleLedgerV1::from_coordinator(&self)? != current")
            .expect("publication rejoins the staged source to that coordinator");
        let persist = published_retirement
            .find("store.persist_exact_successor(&current, &retired)?")
            .expect("publication fsyncs the exact staged successor");
        let reload = published_retirement
            .find("store.load()? != retired")
            .expect("publication revalidates the linked store after fsync");
        let sealed = published_retirement
            .find("coordinator: self")
            .expect("published token retains the exact consumed coordinator");
        assert!(
            consume_coordinator < exact_source
                && exact_source < persist
                && persist < reload
                && reload < sealed
        );
        assert!(
            lifecycle_startup_test_source
                .contains("production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout")
        );
        assert!(
            lifecycle_startup_test_source
                .contains(".retire_lifecycle_stores_for_test(finality_receipt)")
        );
        assert!(
            lifecycle_startup_test_source
                .contains("cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)")
        );
        let finalization_behavior = lifecycle_startup_test_source
            .split_once(
                "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
            )
            .expect("marker replay has one production finalization behavior fixture")
            .1
            .split_once("fn expect_recovered_open_error")
            .expect("production finalization behavior ends before recovery helpers")
            .0;
        let status_guard = finalization_behavior
            .find("let _status_guard = crate::sumeragi::status::rbc_status_test_guard()")
            .expect("the production finalization fixture serializes global status mutation");
        let genesis_transaction = finalization_behavior
            .find("TransactionBuilder::new_genesis(")
            .expect("the production finalization fixture uses a genesis-domain transaction");
        let genesis_key = finalization_behavior
            .find("Algorithm::Ed25519")
            .expect("the genesis transaction uses an allowed non-consensus signing key");
        let genesis_da = finalization_behavior
            .find("block_builder.set_da_proof_policies(Some(proof_policy_bundle))")
            .expect("the production finalization genesis seals its active DA policy");
        let genesis_signature = finalization_behavior
            .find(".try_build_with_signature(0, genesis_key.private_key())")
            .expect("the configured genesis authority signs at index zero");
        let genesis_policy = finalization_behavior
            .find("BlockSignaturePolicy::GenesisAuthority(")
            .expect("the recovered body store retains the genesis signature policy");
        let decision = finalization_behavior
            .find("WalRecordV2::Decision(decision)")
            .expect("the finalization fixture starts from a durable Decision");
        let launch = finalization_behavior
            .find("let mut launched = owner")
            .expect("the recovered Decision owner launches through production");
        let dispatch = finalization_behavior
            .find(".dispatch_recovered_decision_apply(")
            .expect("the recovered Apply uses the lifecycle scheduler");
        let settle = finalization_behavior
            .find("settle_recovered_decision_apply_completion(&mut lane_work)")
            .expect("the recovered Apply publishes exact finality");
        let activation = finalization_behavior
            .find("let activated = launched")
            .expect("the completed recovered height activates through the runner seal");
        let finalize = finalization_behavior
            .find(".into_finalized_rollover(&mut runner)")
            .expect("the activated owner runs the production finalization transition");
        let retain_decision = finalization_behavior
            .find(".retain_merge_sidecars_for_global_view(")
            .expect("the lane owner retains the ordinary exact Decision carrier");
        let output = finalization_behavior
            .find(".rollover_outputs(&mut runner, lane_work, &successor, 64)")
            .expect("the exact service and lane owners seal output together");
        let stores = finalization_behavior
            .find(".retire_lifecycle_stores()")
            .expect("lifecycle stores retire only after output handoff");
        let workers = finalization_behavior
            .find("cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)")
            .expect("clean worker teardown is the final behavior step");
        assert!(
            status_guard < genesis_key
                && genesis_key < genesis_transaction
                && genesis_transaction < genesis_da
                && genesis_da < genesis_signature
                && genesis_signature < genesis_policy
                && genesis_policy < decision
                && decision < launch
                && launch < dispatch
                && dispatch < settle
                && settle < activation
                && activation < finalize
                && finalize < retain_decision
                && retain_decision < output
                && output < stores
                && stores < workers
        );
        assert!(registry_validate_source.contains("broadcast.is_unpaired()"));
        assert!(
            registry_validate_source
                .contains("carrier.pairs_exact_next_sign(next_sign, next_sign_digest)")
        );

        let current_payload_census = payload_store_source
            .split_once("fn authenticate_current_for_lifecycle_retirement(")
            .expect("Serve store has one current retirement census")
            .1
            .split_once("/// Compare this opened payload owner")
            .expect("current retirement census stays bounded")
            .0;
        for required in [
            "self.reload_payload_census_strict()?",
            "payloads.keys().copied().collect::<BTreeSet<_>>() != self.indexed",
            ".authenticate_for_complete_tip_retirement(verified, local_signer)",
            "self.validate_authenticated_cut(&authenticated)?",
        ] {
            assert!(current_payload_census.contains(required));
        }
        let live_serve_join = lifecycle_open_source
            .split_once("fn authenticate_live_finalization_serve_census(")
            .expect("Serve retirement has one ledger/wait join")
            .1
            .split_once("/// Seal the final post-mutation Serve cut")
            .expect("live Serve join stays bounded")
            .0;
        for required in [
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "authenticate_complete_tip_serve_census(ledger, recovered)?",
            "WaitSource::Capacity(class)",
            "receipt.exactly_matches_pending(payload.request())",
            "prepare_certified_serve_admission(",
            "candidate != waiting.candidate",
            "owned != recovered_ids",
        ] {
            assert!(live_serve_join.contains(required));
        }
        let finalization_registry = registry_validate_source
            .split_once("fn exactly_covers_finalization_work(")
            .expect("registry has one finalization-only census")
            .1
            .split_once("fn exactly_covers_ready_work_with_extra(")
            .expect("finalization census delegates to the shared exact coverage")
            .0;
        assert!(
            finalization_registry
                .contains("exactly_covers_ready_work_with_extra(coordinator, extra, None, true)")
        );
        assert!(
            registry_validate_source.contains("broadcast.matches_current_finalization_record(")
        );

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
        let ordinary_activation = runner_dependency_permit
            .split_once("struct ProductionLifecycleRunnerActivationV1")
            .expect("runner retains one ordinary activation authority")
            .1
            .split_once("struct ProductionLifecycleCompleteTipRunnerActivationV1")
            .expect("ordinary activation ends before the CompleteTip authority")
            .0;
        for required in [
            "_seal: ProductionLifecycleRunnerActivationSealV1",
            "ingress_ready: Arc<AtomicBool>",
            "block_ingress: Arc<FairV2Ingress>",
            "status: ProductionLifecycleRunnerStatusAuthorityV1",
            "struct ProductionLifecycleRunnerActivationSealV1",
            "impl Drop for ProductionLifecycleRunnerActivationSealV1",
            "fn current_height(",
            "fn applied(",
            "fn snapshot_bootstrap(",
            "CurrentHeight",
            "Applied",
            "SnapshotBootstrap",
            "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
            "self.ingress_ready.store(false, Ordering::Release)",
            "self.block_ingress.open()",
            "status::set_v2_status(successor)",
            "status::activate_v2_successor_height(",
            "status::activate_snapshot_bootstrap_v2_height(",
            "self.block_ingress.close()",
            "self.ingress_ready.store(true, Ordering::Release)",
            "ProductionLifecycleActivatedRunnerAuthorityV1 {",
            "ingress_ready: self.ingress_ready",
            "block_ingress: self.block_ingress",
        ] {
            assert!(ordinary_activation.contains(required));
        }
        let close_readiness = ordinary_activation
            .find("self.ingress_ready.store(false, Ordering::Release)")
            .unwrap();
        let exact_ingress = ordinary_activation.find("Arc::ptr_eq").unwrap();
        let reject_close = ordinary_activation
            .find("self.block_ingress.close()")
            .unwrap();
        let open_ingress = ordinary_activation
            .find("self.block_ingress.open()")
            .unwrap();
        let publish_status = ordinary_activation
            .find("let publication = match self.status")
            .unwrap();
        let release_readiness = ordinary_activation
            .rfind("self.ingress_ready.store(true, Ordering::Release)")
            .unwrap();
        assert!(
            close_readiness < exact_ingress
                && exact_ingress < reject_close
                && reject_close < open_ingress
                && open_ingress < publish_status
                && publish_status < release_readiness
        );
        for forbidden in [
            "impl Clone for ProductionLifecycleRunnerActivationV1",
            "impl Copy for ProductionLifecycleRunnerActivationV1",
            "pub(in crate::sumeragi) fn current_height(",
            "pub(crate) fn current_height(",
            "pub fn current_height(",
            "pub(in crate::sumeragi) fn applied(",
            "pub(in crate::sumeragi) fn snapshot_bootstrap(",
            "fn into_parts(",
        ] {
            assert!(!ordinary_activation.contains(forbidden));
        }

        let complete_tip_activation = runner_dependency_permit
            .split_once("struct ProductionLifecycleCompleteTipRunnerActivationV1")
            .expect("runner retains one CompleteTip activation authority")
            .1
            .split_once("struct ProductionLifecycleActivatedRunnerAuthorityV1")
            .expect("CompleteTip activation ends before the live runner borrow key")
            .0;
        for required in [
            "_seal: ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "struct ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1",
            "fn mint_for_recovered_runner(",
            "ProductionLifecycleActivatedRunnerAuthorityV1 {",
            "ingress_ready: self.ingress_ready",
            "block_ingress: self.block_ingress",
        ] {
            assert!(complete_tip_activation.contains(required));
        }
        let close_readiness = complete_tip_activation
            .find("self.ingress_ready.store(false, Ordering::Release)")
            .unwrap();
        let exact_ingress = complete_tip_activation.find("Arc::ptr_eq").unwrap();
        let retirement_join = complete_tip_activation
            .find("retirement.authorizes_successor_status(&successor)")
            .unwrap();
        let open_ingress = complete_tip_activation
            .find("self.block_ingress.open()")
            .unwrap();
        let publish_status = complete_tip_activation
            .find("status::activate_recovered_complete_tip_v2_height(retirement, successor)")
            .unwrap();
        let release_readiness = complete_tip_activation
            .find("self.ingress_ready.store(true, Ordering::Release)")
            .unwrap();
        assert!(
            close_readiness < exact_ingress
                && exact_ingress < retirement_join
                && retirement_join < open_ingress
                && open_ingress < publish_status
                && publish_status < release_readiness
        );
        assert_eq!(
            complete_tip_activation
                .matches("self.block_ingress.close()")
                .count(),
            3,
            "mismatch, invalid retirement, and publication failure each close exact ingress"
        );
        for forbidden in [
            "impl Clone for ProductionLifecycleCompleteTipRunnerActivationV1",
            "impl Copy for ProductionLifecycleCompleteTipRunnerActivationV1",
            "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
            "pub(crate) fn mint_for_recovered_runner(",
            "pub fn mint_for_recovered_runner(",
            "fn into_parts(",
        ] {
            assert!(!complete_tip_activation.contains(forbidden));
        }
        let activated_runner = runner_dependency_permit
            .split_once("struct ProductionLifecycleActivatedRunnerAuthorityV1")
            .expect("activation retains one exact readiness/ingress owner")
            .1
            .split_once("struct ProductionLifecycleActiveRunnerBorrowV1")
            .expect("activated runner ownership ends before the live borrow key")
            .0;
        for required in [
            "_seal: ProductionLifecycleActivatedRunnerAuthoritySealV1",
            "ingress_ready: Arc<AtomicBool>",
            "block_ingress: Arc<FairV2Ingress>",
            "impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1",
            "fn retire(",
            "self.ingress_ready.store(false, Ordering::Release)",
            "self.block_ingress.close()",
            "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
            "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
        ] {
            assert!(activated_runner.contains(required));
        }
        for forbidden in [
            "impl Clone for ProductionLifecycleActivatedRunnerAuthorityV1",
            "impl Copy for ProductionLifecycleActivatedRunnerAuthorityV1",
            "fn into_parts(",
            "pub ingress_ready:",
            "pub block_ingress:",
        ] {
            assert!(!activated_runner.contains(forbidden));
        }
        assert_eq!(
            activated_runner
                .matches("self.ingress_ready.store(false, Ordering::Release)")
                .count(),
            2
        );
        assert_eq!(
            activated_runner
                .matches("self.block_ingress.close()")
                .count(),
            2
        );
        let runner_borrow = runner_dependency_permit
            .split_once("struct ProductionLifecycleActiveRunnerBorrowV1")
            .expect("runner owns one live borrow key")
            .1;
        assert!(runner_borrow.contains("fn mint_for_recovered_runner() -> Self"));
        assert!(!runner_borrow.contains("pub(in crate::sumeragi) fn mint_for_recovered_runner"));
        assert!(!runner_borrow.contains("fn into_parts("));
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
        assert_recovered_proposal_prepare_wal_settlement_is_restart_closed();
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
                "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
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

    fn assert_recovered_proposal_prepare_wal_settlement_is_restart_closed() {
        let source = include_str!("v2_lifecycle_launch.rs");
        let settlement = source
            .split_once(
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(",
            )
            .expect("initial recovered Proposal has one WAL-first transaction")
            .1
            .split_once(
                "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
            )
            .expect("initial Proposal WAL transaction stays bounded")
            .0;
        let completion = settlement
            .find("recovered_lifecycle_sign_completion.take()")
            .expect("take the guarded Proposal completion once");
        let body = settlement
            .find("prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)")
            .expect("preflight the exact future Prepare Sign and body");
        let shape = settlement
            .find("RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal")
            .expect("accept only the initial Proposal persistence shape");
        let output_projection = settlement
            .find("preview.project_proposal_exact_output_authority()")
            .expect("seal output from the same pre-WAL preview");
        let output_capture = settlement
            .find("capture_recovered_lifecycle_proposal_exact_output(output_authority)")
            .expect("reserve Proposal control and chunks before WAL I/O");
        let wal_permit = settlement
            .find("output.prepare_wal_append_permit()")
            .expect("borrow the WAL authority from the still-armed output reservation");
        let wal = settlement
            .find("append_recovered_lifecycle_proposal_prepare_wal(wal_permit)")
            .expect("append and fsync the exact PrepareIntent");
        let registry = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(")
            .expect("seal the post-WAL two-child registry successor");
        let transition = settlement
            .find("prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(")
            .expect("stage the exact two-child Ledger successor");
        let fsync = settlement
            .find("transition.persist_exact_successor().is_err()")
            .expect("fsync the two-child Ledger successor");
        let transition_commit = settlement
            .find("transition.commit_after_publication();")
            .expect("publish coordinator, registry, and adapter after Ledger fsync");
        let worker_commit = settlement
            .find("completion.acknowledge_after_publication();")
            .expect("retire the guarded worker after durable publication");
        let output_commit = settlement
            .find("output.commit_after_publication();")
            .expect("enqueue the pre-WAL output reservation last");
        assert!(
            completion < body
                && body < shape
                && shape < output_projection
                && output_projection < output_capture
                && output_capture < wal_permit
                && wal_permit < wal
                && wal < registry
                && registry < transition
                && transition < fsync
                && fsync < transition_commit
                && transition_commit < worker_commit
                && worker_commit < output_commit
        );
        assert!(
            settlement
                .contains("RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)")
        );
        assert!(settlement.contains("*recovered_lifecycle_sign_completion = Some(completion)"));
        assert!(!settlement.contains("output.abort_before_publication()"));
        let post_wal = &settlement[wal..transition_commit];
        assert!(post_wal.matches("drop(output);").count() >= 3);
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
