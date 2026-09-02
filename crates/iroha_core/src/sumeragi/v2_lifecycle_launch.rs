//! Sealed production launch from recovered lifecycle ownership into live I/O.
use iroha_crypto::KeyPair;
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, consensus_v2 as wire},
    peer::PeerId,
};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;

#[path = "v2_lifecycle_pending_kura.rs"]
mod pending_kura;
#[path = "v2_lifecycle_preactivation.rs"]
mod preactivation;
#[path = "v2_lifecycle_turn_driver.rs"]
mod turn_driver;

#[cfg(test)]
#[path = "v2_lifecycle_turn_driver_tests.rs"]
mod turn_driver_tests;

pub(in crate::sumeragi) use pending_kura::{
    PendingKuraActivatedProductionLifecycleV1, PendingKuraProductionLifecycleV1,
    PreparedPendingKuraLaneRecoveryV1, ProductionPendingKuraApplyRecoveryErrorV1,
    ProductionPendingKuraApplyRecoveryProgressV1,
};
#[cfg(test)]
use preactivation::ProductionLifecyclePreActivationFailStopScopeV1;
pub(in crate::sumeragi) use preactivation::{
    ProductionLifecyclePreActivationErrorV1, ProductionPendingKuraApplyInstallErrorV1,
};
#[cfg(test)]
pub(in crate::sumeragi) use turn_driver::ProductionPreparedCertifiedServeTestSettlementV1;
pub(in crate::sumeragi) use turn_driver::{
    ProductionLifecycleCompletionPreGateV1, ProductionLifecycleCompletionSelectionV1,
    ProductionLifecycleCompletionTurnV1, ProductionLifecycleIngressSelectionV1,
    ProductionLifecycleIngressTurnV1, ProductionLifecycleReadyCompletionTurnV1,
    ProductionPreTimeoutLockedPrepareQcIngressTurnV1, ProductionPreparedOrdinaryIngressTurnV1,
    ProductionRecoveredLifecycleSignCompletionSelectionV1,
};

use super::{
    CertifiedFetchBodyPersistenceCompletionError, PreparedLifecycleIngressSelector,
    ProductionCompletionDispatchErrorV1, ProductionCompletionDispatchV1,
    ProductionIngressSchedulerInputsError, ProductionIngressTurnPreparation,
    ProductionLifecycleOwnerV1, ProductionRecoveredDecisionFetchPersistenceErrorV1,
    ProductionRecoveredDecisionFetchPersistenceV1, ProductionRecoveredLifecycleSignDispatchErrorV1,
    ProductionRecoveredLifecycleSignDispatchV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
    ProductionRecoveredLifecycleSignedBroadcastRefanoutV1, ReadyValidateSuccessorV1,
    RegisteredLifecycleValidateSidecarWaitV1,
    ingress_position::{FairIngressTurnContextCut, FairIngressTurnCut},
    work_registry::{
        LifecycleDecisionApplyTerminalPublicationErrorV1,
        RecoveredLifecycleSignCancellationPublicationError,
    },
};
use crate::sumeragi::v2_runner::LifecycleCurrentRunnerTurn;
#[cfg(test)]
use crate::sumeragi::v2_runner::LifecycleRunnerRankSnapshot;
use crate::{
    IrohaNetwork,
    kura::{Kura, KuraV2CommitReceipt},
    state::State,
    sumeragi::{
        FairV2Ingress, FairV2IngressDequeueDisposition, InboundBlockMessage,
        output_guard::ConsensusOutputGuard,
        serviced_candidate_store::{LeaderWireLifecycleRestore, LeaderWireLifecycleStoreGate},
        v2_apply::LifecycleDecisionApplyWorkerResultV1,
        v2_context::AuthenticatedGenesisBodyV1,
        v2_effects::{
            EffectExecutorError, EffectQueueConfig, PostFinalityCleanupOutcome, V2EffectExecutor,
        },
        v2_lane_work::{
            MergeSidecarDeferralDisposition, RetainedMergeSidecars, V2LaneWorkAdapter,
            V2LaneWorkError,
        },
        v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig, SerializedV2Runtime},
        v2_worker::{
            DurableExactOutputServiceOwner, KuraReplicaAdvertRefreshOwner,
            LifecycleCompletionTakeV1, LifecycleDecisionApplyDeferredRetryV1,
            PreparedCertifiedFetchBodyPersistenceCompletion,
            PreparedDeferredLifecycleValidateCompletionV1,
            PreparedLifecycleDecisionApplyCompletionV1, PreparedLifecycleValidateCompletionV1,
            PreparedRecoveredDecisionFetchBodyCompletionV1,
            PreparedRecoveredLifecycleSignCompletionV1, ProductionV2Services,
            RecoveredLifecycleProposalExactOutputCaptureV1, V2CleanupSupervisor,
        },
    },
};
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
    kagemusha_mint_finality_authority:
        Option<Arc<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1>>,
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
            kagemusha_mint_finality_authority: None,
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
    /// Attach the separately provisioned Pasta epoch authority used only for
    /// top-up-bearing Kagemusha V1 Commit votes.
    #[must_use]
    pub(in crate::sumeragi) fn with_kagemusha_mint_finality_authority(
        mut self,
        authority: Option<
            Arc<crate::zk::kagemusha_v1_recursion::KagemushaMintFinalityLocalAuthorityV1>,
        >,
    ) -> Self {
        self.kagemusha_mint_finality_authority = authority;
        self
    }
}
/// RAII owner of the exact durable leader-wire ingress gate for this launch.
///
/// Leader-wire recovery binds before runtime and service construction. The
/// ingress stays closed throughout this pre-activation tranche; any later
/// construction error, ordinary wrapper drop, or panic closes it before the
/// gate detaches.
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
        let Some(gate) = self.gate.as_ref().cloned() else {
            return Ok(());
        };
        self.ingress.retire_leader_wire_lifecycle_gate(&gate)?;
        self.gate = None;
        Ok(())
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
    // The exact Decision Fetch has already entered runtime ownership, but only
    // this seal may verify and dispatch it through interrupted-tip recovery.
    pending_kura_apply_replay: Option<super::super::v2::PreparedRecoveredPendingKuraApplyReplayV1>,
    // A recovered ProposalIntent has already consumed its sole startup Sign.
    // Preactivation must compare this opaque owner with the reducer directive
    // before live clocks can admit fresh local proposal work.
    recovered_local_proposal_attempt:
        Option<super::super::v2::RecoveredLifecycleLocalProposalAttemptV1>,
    // Exactly one guarded completion/retry owner may be parked. It drops only
    // after its exact service has stopped, so fail-stop queue identity remains
    // representable without parallel optional owner slots.
    pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>,
    // A single typed owner retains the exact selector and service-generation
    // fence. The variant fixes which Phase-A transaction may resume it.
    pending_ingress_capacity: Option<PendingIngressCapacityV1>,
    #[allow(dead_code)]
    completion_observer_activation: Option<ProductionV2CompletionObserverActivationPermitV1>,
    // Rust drops fields in declaration order. Keep this last so the service
    // worker has stopped before ingress closes and the durable gate detaches.
    #[allow(dead_code)]
    leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1,
}

/// Sole parked lifecycle completion owner for this height.
#[allow(variant_size_differences)]
enum PendingLifecycleCompletionV1 {
    /// Lifecycle Decision Apply awaits its exact missing-sidecar retry.
    LifecycleDecisionApplyDeferred(RetainedLifecycleDecisionApplyDeferredV1),
    /// Ordinary certified-Fetch persistence awaits Phase B publication.
    CertifiedFetch(PreparedCertifiedFetchBodyPersistenceCompletion),
    /// Recovered Decision Fetch persistence awaits Store settlement.
    RecoveredDecisionFetch(PreparedRecoveredDecisionFetchBodyCompletionV1),
    /// Recovered Sign persistence awaits its adapter-family settlement.
    RecoveredSign(PreparedRecoveredLifecycleSignCompletionV1),
    /// One executed lifecycle Validate awaits same-address Ready publication.
    Validate(PreparedLifecycleValidateCompletionV1),
    /// One just-published Validate carrier must resolve at the same Ready
    /// address before any physical completion or Runtime turn can intervene.
    ReadyValidateSuccessor(ReadyValidateSuccessorV1),
    /// A missing-sidecar lifecycle Validate remains parked under its exact wait owner.
    DeferredValidate(PreparedDeferredLifecycleValidateCompletionV1),
    /// The exact missing-sidecar registration is fsynced and retains either
    /// the live guarded completion or its authenticated cold-open equivalent.
    RegisteredDeferredValidate(RegisteredLifecycleValidateSidecarWaitV1),
}

impl PendingLifecycleCompletionV1 {
    fn take_certified_fetch(
        slot: &mut Option<Self>,
    ) -> Option<PreparedCertifiedFetchBodyPersistenceCompletion> {
        match slot.take() {
            Some(Self::CertifiedFetch(completion)) => Some(completion),
            other => {
                *slot = other;
                None
            }
        }
    }

    fn take_recovered_decision_fetch(
        slot: &mut Option<Self>,
    ) -> Option<PreparedRecoveredDecisionFetchBodyCompletionV1> {
        match slot.take() {
            Some(Self::RecoveredDecisionFetch(completion)) => Some(completion),
            other => {
                *slot = other;
                None
            }
        }
    }

    fn take_recovered_sign(
        slot: &mut Option<Self>,
    ) -> Option<PreparedRecoveredLifecycleSignCompletionV1> {
        match slot.take() {
            Some(Self::RecoveredSign(completion)) => Some(completion),
            other => {
                *slot = other;
                None
            }
        }
    }

    fn recovered_sign(&self) -> Option<&PreparedRecoveredLifecycleSignCompletionV1> {
        match self {
            Self::RecoveredSign(completion) => Some(completion),
            Self::LifecycleDecisionApplyDeferred(_)
            | Self::CertifiedFetch(_)
            | Self::RecoveredDecisionFetch(_)
            | Self::Validate(_)
            | Self::ReadyValidateSuccessor(_)
            | Self::DeferredValidate(_)
            | Self::RegisteredDeferredValidate(_) => None,
        }
    }

    fn take_validate(slot: &mut Option<Self>) -> Option<PreparedLifecycleValidateCompletionV1> {
        match slot.take() {
            Some(Self::Validate(completion)) => Some(completion),
            other => {
                *slot = other;
                None
            }
        }
    }
}

/// Sole retained lifecycle ingress-capacity owner for this height.
#[allow(variant_size_differences)]
enum PendingIngressCapacityV1 {
    /// Resume through ordinary certified-Fetch persistence.
    CertifiedFetch(super::PreparedProductionIngressCapacityWait),
    /// Resume through recovered Decision-Fetch persistence.
    RecoveredDecisionFetch(super::PreparedProductionIngressCapacityWait),
    /// Resume current-height Certified-Serve after its worker fence advances.
    CertifiedServe(crate::sumeragi::v2_worker::LifecycleCertifiedServeCapacityWaitV1),
}

#[derive(Clone, Copy)]
enum PendingIngressCapacityKindV1 {
    CertifiedFetch,
    RecoveredDecisionFetch,
}
/// Result of draining one dedicated lifecycle Decision Apply worker completion.
#[allow(variant_size_differences)]
#[must_use = "a deferred lifecycle Decision Apply completion must remain retained"]
pub(in crate::sumeragi) enum ProductionLifecycleDecisionApplyCompletionV1 {
    /// Kura, LedgerV1, coordinator, registry, adapter, executor, and worker ack advanced.
    Applied,
    /// A guarded missing-sidecar result awaits exact fetch progress or queue re-entry.
    Deferred(RetainedLifecycleDecisionApplyDeferredV1),
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
#[must_use = "deferred lifecycle Decision Apply remains the sole retry owner"]
pub(in crate::sumeragi) struct RetainedLifecycleDecisionApplyDeferredV1 {
    completion: PreparedLifecycleDecisionApplyCompletionV1,
    sidecar: LifecycleDecisionApplySidecarWaitV1,
}
struct LifecycleDecisionApplySidecarWaitV1 {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    reference: CertifiedMergeLedgerReference,
}
impl LifecycleDecisionApplySidecarWaitV1 {
    fn register(
        &self,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> Result<MergeSidecarDeferralDisposition, V2LaneWorkError> {
        lane_work.defer_missing_lifecycle_decision_apply_sidecar(
            self.round,
            self.subject,
            self.reference.clone(),
        )
    }
}
/// Result of retrying one exact lifecycle Decision Apply after its merge sidecar arrives.
#[allow(variant_size_differences)]
#[must_use = "an unavailable retry still owns the lifecycle Decision Apply completion"]
pub(in crate::sumeragi) enum ProductionLifecycleDecisionApplyRetryV1 {
    /// The unchanged task was atomically returned to the dedicated worker FIFO.
    Requeued,
    /// Sidecar fetch progress or Consensus I/O capacity is pending; ownership is unchanged.
    Unavailable(RetainedLifecycleDecisionApplyDeferredV1),
    /// The dedicated worker index changed and consensus was closed for restart.
    RestartRequired,
}
impl RetainedLifecycleDecisionApplyDeferredV1 {
    /// Retry only after the exact authenticated sidecar is locally durable.
    ///
    /// Re-registering the sealed wait is idempotent. `Fetching` and
    /// `RetryLater` retain this complete owner; only `Available`, which
    /// reauthenticates the referenced Kura entry, may republish the task.
    fn retry_after_available(self) -> ProductionLifecycleDecisionApplyRetryV1 {
        let Self {
            completion,
            sidecar,
        } = self;
        match completion.retry_deferred() {
            LifecycleDecisionApplyDeferredRetryV1::Requeued => {
                ProductionLifecycleDecisionApplyRetryV1::Requeued
            }
            LifecycleDecisionApplyDeferredRetryV1::Unavailable(completion) => {
                ProductionLifecycleDecisionApplyRetryV1::Unavailable(Self {
                    completion,
                    sidecar,
                })
            }
            LifecycleDecisionApplyDeferredRetryV1::RestartRequired => {
                ProductionLifecycleDecisionApplyRetryV1::RestartRequired
            }
        }
    }
}
/// Fail-stop class while durably terminalizing a lifecycle Decision Apply completion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(in crate::sumeragi) enum ProductionLifecycleDecisionApplyCompletionErrorV1 {
    /// The owner had no exact active lifecycle Decision Apply lease and carrier.
    #[error("lifecycle Decision Apply completion lost its exact lifecycle owner")]
    Owner,
    /// The Kura result did not match the installed lifecycle Decision Apply authority.
    #[error("lifecycle Decision Apply completion changed its durable authority")]
    Completion,
    /// The exact decided merge-sidecar dependency could not be registered.
    #[error("lifecycle Decision Apply merge-sidecar recovery could not retain its exact owner")]
    Sidecar,
    /// The serialized adapter/executor retained conflicting live work.
    #[error("lifecycle Decision Apply completion overtook live reducer work")]
    Executor,
    /// The staged terminal was not the sole exact coordinator/registry successor.
    #[error("lifecycle Decision Apply terminal registry successor is not exact")]
    Registry,
    /// LedgerV1 exact-successor publication failed or became ambiguous.
    #[error("lifecycle Decision Apply terminal LedgerV1 publication failed")]
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

/// Move-only authority for crossing the ordinary live-clock boundary.
///
/// Only ordinary lifecycle activation can mint this permit. The dedicated
/// PendingKura lifecycle may borrow its executor for decided-lane recovery,
/// but it cannot manufacture the authority required to arm a pacemaker.
#[must_use = "ordinary lifecycle activation must consume the live-clock permit"]
pub(in crate::sumeragi) struct ProductionLifecycleLiveClockActivationPermitV1 {
    _seal: ProductionLifecycleLiveClockActivationPermitSealV1,
}

struct ProductionLifecycleLiveClockActivationPermitSealV1;

impl Drop for ProductionLifecycleLiveClockActivationPermitSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleLiveClockActivationPermitV1 {
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self {
            _seal: ProductionLifecycleLiveClockActivationPermitSealV1,
        }
    }
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

/// Private proof that runner readiness and durable leader-wire ingress retired.
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
            pending_lifecycle_completion,
            leader_wire_ingress_binding,
            ..
        } = self;
        let Some(completion) = PendingLifecycleCompletionV1::take_recovered_decision_fetch(
            pending_lifecycle_completion,
        ) else {
            return ProductionRecoveredDecisionFetchStoreSettlementV1::None;
        };
        macro_rules! retry {
            ($failure:expr) => {{
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),
                );
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
        let request_hash = completion.completion().request_hash();
        let wait_source = super::projection::certified_fetch_wait_source(request_hash);
        let response_hash = completion.completion().response_hash();
        let physical_ordinal = completion.completion().physical_admission_ordinal();
        let Some(body) = completion.completion().project_store_body_authority() else {
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Body);
        };
        let retry_marker =
            match executor.prepare_published_lifecycle_store_retry_marker(body.durable()) {
                Ok(marker) => marker,
                Err(_) => {
                    retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Executor)
                }
            };
        let selector = match executor.prepare_lifecycle_ingress_selector(
            &leader_wire_ingress_binding.ingress,
            physical_ordinal,
        ) {
            Ok(selector) => selector,
            Err(error) => {
                let reason = error.detail();
                iroha_logger::warn!(
                    %reason,
                    "recovered Fetch Store settlement could not recapture exact ingress"
                );
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
            Err(error) => {
                let reason = error.detail();
                iroha_logger::warn!(
                    %reason,
                    "recovered Fetch Store settlement could not lock exact ingress"
                );
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
                wait_source,
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
                wait_source,
                adapter,
            ) {
            Ok(successor) => successor,
            Err(_) => {
                drop(locked_dequeue);
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Registry)
            }
        };
        let retry_marker = match retry_marker
            .bind_store_successor(successor.store_effect(), successor.pending_effect_binding())
        {
            Ok(marker) => marker,
            Err(_) => {
                drop(successor);
                drop(locked_dequeue);
                retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Executor)
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
        let request_output_retirement = match services
            .prepare_recovered_decision_fetch_request_output_retirement(request_hash)
        {
            Ok(retirement) => retirement,
            Err(reason) => {
                iroha_logger::error!(
                    %reason,
                    "recovered Fetch request-output retirement preflight failed closed"
                );
                drop(transition);
                drop(locked_dequeue);
                output_guard.close_admission_for_restart();
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion = Some(
                    PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),
                );
                return ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired;
            }
        };
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(request_output_retirement);
            drop(transition);
            drop(locked_dequeue);
            retry!(ProductionRecoveredDecisionFetchStoreSettlementFailureV1::OutputClosed);
        };
        if transition.persist_exact_successor().is_err() {
            drop(request_output_retirement);
            drop(transition);
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            assert!(pending_lifecycle_completion.is_none());
            *pending_lifecycle_completion = Some(
                PendingLifecycleCompletionV1::RecoveredDecisionFetch(completion),
            );
            drop(locked_dequeue);
            drop(operation);
            return ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired;
        }
        transition.commit_after_publication();
        executor.commit_published_lifecycle_store_retry_marker(retry_marker);
        request_output_retirement.commit_after_publication(operation.permit());
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
    #[expect(
        dead_code,
        reason = "fixture-owned recovered-Sign dispatch remains source-audited until cold-start wiring lands"
    )]
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
        if self.pending_lifecycle_completion.is_some() {
            return Ok(false);
        }
        let drain = self.services.drain_recovered_lifecycle_sign_completion()?;
        let Some(completion) = drain.into_completion() else {
            return Ok(false);
        };
        self.pending_lifecycle_completion =
            Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
        Ok(true)
    }
    /// Durably cancel one authenticated recovered Sign whose reducer fence was
    /// superseded by certified progress.
    ///
    /// The current adapter state is already the certified successor and is not
    /// rewound. This transaction terminalizes only the exact claimed Sign row,
    /// removes its concrete carrier after LedgerV1 fsync, and then acknowledges
    /// the dedicated worker completion.
    fn settle_superseded_recovered_lifecycle_sign(&mut self) -> bool {
        let Self {
            owner,
            services,
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)
        else {
            iroha_logger::error!(
                "superseded recovered Sign cancellation lost its parked completion"
            );
            return false;
        };
        macro_rules! restart {
            ($reason:literal) => {{
                iroha_logger::error!($reason);
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                services
                    .lifecycle_output_guard()
                    .close_admission_for_restart();
                drop(completion);
                return false;
            }};
        }
        if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
            restart!("superseded recovered Sign cancellation lost its durable owner");
        }
        let Some(lease) = owner.coordinator.active_lease.clone() else {
            restart!("superseded recovered Sign cancellation lost its active lease");
        };
        let key = completion.dispatch_key();
        let Some(cancellation) = owner
            .registry
            .registry()
            .prepare_recovered_lifecycle_sign_cancellation(&owner.coordinator, &lease, key)
        else {
            restart!("superseded recovered Sign cancellation carrier preflight failed");
        };
        let mut staged = owner.coordinator.stage_durable_transaction();
        staged.reduce_cancel_superseded_sign(lease.clone());
        if staged.fault.is_some() {
            drop(cancellation);
            restart!("superseded recovered Sign cancellation staging failed");
        }
        let output_guard = services.lifecycle_output_guard();
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            drop(cancellation);
            restart!("superseded recovered Sign cancellation found output closed");
        };
        let published = owner
            .registry
            .registry_mut()
            .publish_recovered_lifecycle_sign_cancellation(
                cancellation,
                &owner.coordinator,
                &staged,
                &lease,
                || owner.coordinator.persist_exact_staged_successor(&staged),
            );
        match published {
            Ok(()) => {}
            Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(cancellation)) => {
                drop(cancellation);
                drop(operation);
                restart!("superseded recovered Sign cancellation changed before publication");
            }
            Err(RecoveredLifecycleSignCancellationPublicationError::Publication(
                error,
                cancellation,
            )) => {
                iroha_logger::error!(
                    %error,
                    "superseded recovered Sign cancellation LedgerV1 publication failed"
                );
                drop(cancellation);
                drop(operation);
                restart!("superseded recovered Sign cancellation failed closed");
            }
        }
        owner.coordinator = staged;
        completion.acknowledge_after_publication();
        operation.complete();
        true
    }
    /// Preflight the parked Sign's exact adapter successor.
    ///
    /// This deliberately drops the publication-inert preview before returning.
    /// Output is owned by the durable Broadcast child only after LedgerV1
    /// publication, through the separate typed refanout transaction.
    #[cfg_attr(
        test,
        expect(
            dead_code,
            reason = "standalone recovered-Sign preparation remains a source-audited cold-start seam"
        )
    )]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_broadcast(
        &mut self,
    ) -> ProductionRecoveredLifecycleSignBroadcastPreparationV1 {
        let Self {
            executor,
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) = pending_lifecycle_completion
            .as_ref()
            .and_then(PendingLifecycleCompletionV1::recovered_sign)
        else {
            return ProductionRecoveredLifecycleSignBroadcastPreparationV1::None;
        };
        let Some(authority) = completion.project_adapter_completion_authority() else {
            return ProductionRecoveredLifecycleSignBroadcastPreparationV1::Retry;
        };
        let preview = match executor.prepare_recovered_lifecycle_sign_completion(authority) {
            Ok(preview) => preview,
            Err(_) => return ProductionRecoveredLifecycleSignBroadcastPreparationV1::Retry,
        };
        if preview.shape()
            != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
        {
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
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)
        else {
            return ProductionRecoveredLifecycleSignBroadcastSettlementV1::None;
        };
        macro_rules! retry {
            () => {{
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
        if preview.shape()
            != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
        {
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
            assert!(pending_lifecycle_completion.is_none());
            *pending_lifecycle_completion =
                Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)
        else {
            return ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::None;
        };
        macro_rules! retry {
            () => {{
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            assert!(pending_lifecycle_completion.is_none());
            *pending_lifecycle_completion =
                Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)
        else {
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
            != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
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
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            assert!(pending_lifecycle_completion.is_none());
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
            pending_lifecycle_completion,
            ..
        } = self;
        let Some(completion) =
            PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)
        else {
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::None;
        };
        macro_rules! retry {
            () => {{
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
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
                assert!(pending_lifecycle_completion.is_none());
                *pending_lifecycle_completion =
                    Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
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
            assert!(pending_lifecycle_completion.is_none());
            *pending_lifecycle_completion =
                Some(PendingLifecycleCompletionV1::RecoveredSign(completion));
            drop(output);
            return ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired;
        }
        transition.commit_after_publication();
        completion.acknowledge_after_publication();
        output.commit_after_publication();
        ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied
    }
    /// Drive and retry one exact missing-sidecar lifecycle Decision Apply owner.
    ///
    /// The completion token no longer borrows the whole service owner: its
    /// stable dispatch key retains the exact worker completion accounting.
    /// This sealed method can therefore flush the sidecar request through the
    /// same service/lane instances before reprobing local Kura and queueing the
    /// unchanged task.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn drive_lifecycle_decision_apply_deferred(
        &mut self,
        deferred: RetainedLifecycleDecisionApplyDeferredV1,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> ProductionLifecycleDecisionApplyRetryV1 {
        if !deferred
            .completion
            .authorizes_sidecar_owner(&self.services, lane_work)
        {
            drop(deferred);
            return ProductionLifecycleDecisionApplyRetryV1::RestartRequired;
        }
        match deferred.sidecar.register(lane_work) {
            Ok(MergeSidecarDeferralDisposition::Available) => deferred.retry_after_available(),
            Ok(MergeSidecarDeferralDisposition::Fetching) => {
                if lane_work
                    .dispatch_next_lifecycle_decision_apply_sidecar_request(
                        &self.services,
                        &deferred.sidecar.reference,
                    )
                    .is_err()
                {
                    drop(deferred);
                    ProductionLifecycleDecisionApplyRetryV1::RestartRequired
                } else {
                    ProductionLifecycleDecisionApplyRetryV1::Unavailable(deferred)
                }
            }
            Ok(MergeSidecarDeferralDisposition::RetryLater) => {
                ProductionLifecycleDecisionApplyRetryV1::Unavailable(deferred)
            }
            Ok(MergeSidecarDeferralDisposition::Rejected(_)) | Err(_) => {
                drop(deferred);
                ProductionLifecycleDecisionApplyRetryV1::RestartRequired
            }
        }
    }
    /// Settle one already-classified lifecycle Decision Apply completion.
    ///
    /// The unified Completion driver is the only production caller which can
    /// supply this guarded token without probing the physical FIFO again.
    fn settle_lifecycle_decision_apply_completion_owner(
        &mut self,
        completion: PreparedLifecycleDecisionApplyCompletionV1,
        lane_work: &mut V2LaneWorkAdapter,
    ) -> Result<
        ProductionLifecycleDecisionApplyCompletionV1,
        ProductionLifecycleDecisionApplyCompletionErrorV1,
    > {
        let owner = &mut self.owner;
        let executor = &mut self.executor;
        let services = &mut self.services;
        macro_rules! restart {
            ($failure:expr) => {{
                owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err($failure);
            }};
        }
        if let LifecycleDecisionApplyWorkerResultV1::Deferred { task, reference } =
            completion.result()
        {
            if !completion.authorizes_sidecar_owner(services, lane_work) {
                drop(completion);
                restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Sidecar);
            }
            let sidecar = LifecycleDecisionApplySidecarWaitV1 {
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
                    return Ok(ProductionLifecycleDecisionApplyCompletionV1::Deferred(
                        RetainedLifecycleDecisionApplyDeferredV1 {
                            completion,
                            sidecar,
                        },
                    ));
                }
                Ok(MergeSidecarDeferralDisposition::Rejected(_)) | Err(_) => {
                    drop(completion);
                    restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Sidecar);
                }
            }
        }
        settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)
    }
}

/// Settle one already-classified Applied lifecycle Decision Apply completion.
///
/// This test-only seam exercises the same live active-height status publication
/// as the unified Completion driver without exposing recovered-only authority.
#[cfg(test)]
pub(in crate::sumeragi) fn settle_applied_live_lifecycle_decision_apply_completion_for_test(
    owner: &mut ProductionLifecycleOwnerV1,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    completion: PreparedLifecycleDecisionApplyCompletionV1,
) -> Result<
    ProductionLifecycleDecisionApplyCompletionV1,
    ProductionLifecycleDecisionApplyCompletionErrorV1,
> {
    if !matches!(
        completion.result(),
        LifecycleDecisionApplyWorkerResultV1::Applied(_)
    ) {
        owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
        return Err(ProductionLifecycleDecisionApplyCompletionErrorV1::Completion);
    }
    settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)
}

fn settle_applied_lifecycle_decision_apply_completion(
    owner: &mut ProductionLifecycleOwnerV1,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    completion: PreparedLifecycleDecisionApplyCompletionV1,
) -> Result<
    ProductionLifecycleDecisionApplyCompletionV1,
    ProductionLifecycleDecisionApplyCompletionErrorV1,
> {
    macro_rules! restart {
        ($failure:expr) => {{
            owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
            return Err($failure);
        }};
    }
    let LifecycleDecisionApplyWorkerResultV1::Applied(applied) = completion.result() else {
        unreachable!("deferred Apply completion cannot enter the applied settlement cut")
    };
    if owner.coordinator.fault.is_some() || owner.coordinator.ledger_store.is_none() {
        restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Owner);
    }
    let Some(lease) = owner.coordinator.active_lease.clone() else {
        restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Owner);
    };
    let Some((transition, authority)) = owner
        .registry
        .prepare_lifecycle_decision_apply_terminal_transition(&owner.coordinator, &lease, applied)
    else {
        restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Completion);
    };
    let adapter = match executor.prepare_lifecycle_decision_apply_completion(authority) {
        Ok(adapter) => adapter,
        Err(error) => {
            iroha_logger::error!(
                %error,
                status = ?executor.status(),
                "lifecycle Decision Apply executor completion preflight failed closed"
            );
            restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Executor)
        }
    };
    let mut staged = owner.coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None);
    if staged.fault.is_some() {
        restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Registry);
    }
    let published = owner
        .registry
        .publish_lifecycle_decision_apply_terminal_transition(
            transition,
            &owner.coordinator,
            &staged,
            &lease,
            || owner.coordinator.persist_exact_staged_successor(&staged),
        );
    match published {
        Ok(()) => {}
        Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Preflight(transition)) => {
            iroha_logger::error!(
                "lifecycle Decision Apply terminal registry preflight failed closed"
            );
            drop(transition);
            restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Registry);
        }
        Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Publication(error, transition)) => {
            iroha_logger::error!(
                %error,
                "lifecycle Decision Apply terminal LedgerV1 publication failed closed"
            );
            drop(transition);
            restart!(ProductionLifecycleDecisionApplyCompletionErrorV1::Ledger);
        }
    }
    owner.coordinator = staged;
    let finality = adapter.commit_after_durable_settlement();
    let status = executor.commit_lifecycle_decision_apply_finality(finality);
    let settled = completion.acknowledge_after_owner_settlement();
    assert!(
        matches!(settled, LifecycleDecisionApplyWorkerResultV1::Applied(_)),
        "borrowed lifecycle Decision Apply result cannot change before acknowledgement"
    );
    super::super::status::set_v2_status(status);
    Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied)
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
    /// A durable Validate sidecar registration could not be authenticated and rebound.
    #[error("Validate sidecar registration recovery failed: {0}")]
    ValidateSidecarRegistration(String),
    /// A post-construction process-identity check failed.
    #[error("launched lifecycle stack lost exact process ownership")]
    OwnershipMismatch,
}

/// Settle at most one owner-held cold output through the live guarded service.
///
/// A structural or LedgerV1 publication failure closes output admission for
/// restart. Service failures are already closed by the executor bridge.
pub(in crate::sumeragi) fn settle_one_recovered_lifecycle_output(
    owner: &mut ProductionLifecycleOwnerV1,
    executor: &mut V2EffectExecutor<SerializedV2Runtime>,
    services: &mut ProductionV2Services,
) -> Result<super::RecoveredLifecycleOutputSettlementV1, EffectExecutorError> {
    match owner.settle_next_recovered_lifecycle_output(|effect| {
        executor.execute_recovered_lifecycle_output_service(effect, services)
    }) {
        Ok(settlement) => Ok(settlement),
        Err(super::open::RecoveredLifecycleOutputSettlementErrorV1::Service(error)) => Err(error),
        Err(super::open::RecoveredLifecycleOutputSettlementErrorV1::InvalidAuthority(reason)) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            Err(EffectExecutorError::Contract(reason.to_owned()))
        }
        Err(super::open::RecoveredLifecycleOutputSettlementErrorV1::Durability) => {
            services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            Err(EffectExecutorError::Contract(
                "recovered lifecycle output terminal publication failed".to_owned(),
            ))
        }
    }
}
/// Fail-stop failure while crossing the one-shot live-height boundary.
#[derive(Debug, Error)]
#[must_use = "failed lifecycle activation requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecycleActivationErrorV1 {
    /// The process-wide output barrier was already closed.
    #[error("canonical consensus output is closed")]
    OutputClosed,
    /// Interrupted-tip recovery must use its dedicated no-clock live state.
    #[error("pending Kura recovery cannot arm ordinary live lifecycle clocks")]
    PendingKuraApply,
    /// The interrupted-tip replay has not completed its exact local Apply.
    #[error("pending Kura recovery is not ready for no-clock lifecycle activation")]
    PendingKuraApplyNotReady,
    /// A recovered Proposal Sign was not joined to runner-local proposal state.
    #[error("recovered local Proposal ownership was not initialized before activation")]
    LocalProposalReplayUninitialized,
    /// The reducer could not reproject the prepared local-Proposal directive.
    #[error("live lifecycle could not revalidate its prepared local Proposal: {0}")]
    LocalProposalDirective(#[source] super::super::v2_effects::EffectExecutorError),
    /// Prepared runner state no longer names this exact context and directive.
    #[error("prepared runner local Proposal state does not match lifecycle activation")]
    LocalProposalPreparationMismatch,
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

/// Fail-stop failure while consuming a live or unpublished lifecycle height.
#[derive(Debug, Error)]
#[must_use = "failed lifecycle shutdown requires cold restart"]
pub(in crate::sumeragi) enum ProductionLifecycleShutdownErrorV1 {
    /// The process-wide output barrier was already closed.
    #[error("canonical consensus output is closed during lifecycle shutdown")]
    OutputClosed,
    /// The runner readiness owner no longer names the launched ingress.
    #[error("lifecycle shutdown runner retirement failed: {0}")]
    Runner(String),
    /// The jointly bound ingress gates could not retire as one height owner.
    #[error("lifecycle shutdown ingress-gate retirement failed: {0}")]
    Ingress(String),
}

fn lifecycle_activation_recovery_blocker(
    pending_kura_replay: bool,
    pending_kura_evidence: bool,
    recovered_local_proposal: bool,
) -> Option<ProductionLifecycleActivationErrorV1> {
    if pending_kura_replay || pending_kura_evidence {
        Some(ProductionLifecycleActivationErrorV1::PendingKuraApply)
    } else if recovered_local_proposal {
        Some(ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized)
    } else {
        None
    }
}

/// Fail-stop failure while consuming an activated height into final rollover.
#[derive(Debug, Error)]
#[must_use = "failed lifecycle finalization requires process restart"]
pub(in crate::sumeragi) enum ProductionLifecycleFinalizationErrorV1 {
    /// Executor, lifecycle owner, or dedicated completion ownership is not quiescent.
    #[error("activated lifecycle height is not ready for final rollover")]
    NotReady,
    /// Executor and registry retained different immutable Store publications.
    #[error("published lifecycle Store finalization census failed: {0}")]
    StoreMarkerCensus(String),
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
    /// The retired ordinal floor could not initialize its exact successor.
    #[error("finalized lifecycle successor ordinal floor failed: {0}")]
    SuccessorFloor(#[source] super::ledger::LifecycleLedgerError),
}

/// Activated height after ingress retirement and reducer finalization.
///
/// Services and the complete lifecycle owner remain sealed here until the
/// existing lane/output transaction mints its durable handoff. The safety WAL
/// stays owned here so a crash before that handoff can still recover the
/// finalized body and its lane-only completion. There is no service, receipt,
/// artifact, or owner parts accessor.
#[must_use = "finalized lifecycle output rollover must be consumed"]
pub(in crate::sumeragi) struct FinalizedProductionLifecycleRolloverV1 {
    owner: ProductionLifecycleOwnerV1,
    services: ProductionV2Services,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    finalized_adapter: super::super::v2::FinalizedV2Height,
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
    retained_floor: super::super::v2::FinalizedLifecycleRetainedFloorV1,
}

/// Final cleanup diagnostics plus the still-sealed successor ordinal floor.
///
/// Production consumes the floor into H+1 storage before inspecting the
/// diagnostics. Focused finalization tests may inspect and then drop this
/// unpublished capability without opening successor work.
#[must_use = "post-finality cleanup diagnostics must be observed"]
pub(in crate::sumeragi) struct ProductionLifecycleFinalizationOutcomeV1 {
    cleanup: PostFinalityCleanupOutcome,
    wal_retirement_warning: Option<String>,
    retained_floor: Option<super::super::v2::FinalizedLifecycleRetainedFloorV1>,
    output_guard: Arc<ConsensusOutputGuard>,
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

    /// Consume the live finalized floor into the already-derived H+1 storage seal.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_successor_storage(
        mut self,
        storage: super::super::v2::RecoveredLifecycleStorageAuthorityV1,
    ) -> Result<
        (Self, super::super::v2::RecoveredLifecycleStorageAuthorityV1),
        ProductionLifecycleFinalizationErrorV1,
    > {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleFinalizationErrorV1::OutputClosed)?;
        let floor = self.retained_floor.take().ok_or_else(|| {
            ProductionLifecycleFinalizationErrorV1::RetirementCensus(
                "finalized lifecycle outcome lost its retained ordinal floor".to_owned(),
            )
        })?;
        let storage = storage
            .bind_finalized_predecessor_floor(floor)
            .map_err(ProductionLifecycleFinalizationErrorV1::SuccessorFloor)?;
        operation.complete();
        Ok((self, storage))
    }
}

/// Move-only runner state joined to one exact launched reducer directive.
///
/// Only [`LaunchedProductionLifecycleV1::initialize_recovered_local_proposal`]
/// can construct this value. Ordinary and CompleteTip activation consume it,
/// revalidate the exact context/directive under fail-stop, and retain the real
/// runner scheduler state until readiness and ingress retire.
#[must_use = "prepared local-Proposal state must enter lifecycle activation"]
pub(in crate::sumeragi) struct ProductionLifecyclePreparedLocalProposalStateV1 {
    runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    context_id: wire::HeightContextId,
    directive: super::super::v2::LocalProposalDirective,
}

impl ProductionLifecyclePreparedLocalProposalStateV1 {
    fn exactly_matches(
        &self,
        context_id: wire::HeightContextId,
        directive: super::super::v2::LocalProposalDirective,
    ) -> bool {
        self.context_id == context_id
            && self.directive == directive
            && self
                .runner
                .prepared_local_proposal_exactly_matches(directive)
    }

    /// Check whether the prepared state owns the exact recovered attempt.
    #[cfg(test)]
    pub(in crate::sumeragi) fn already_attempted(
        &self,
        directive: super::super::v2::LocalProposalDirective,
    ) -> bool {
        self.runner.already_attempted(directive)
    }
}

/// Opaque lifecycle stack after clocks, diagnostics, status, and ingress activate.
#[must_use = "the activated lifecycle stack owns the live height"]
pub(in crate::sumeragi) struct ActivatedProductionLifecycleV1 {
    // Drop readiness/ingress ownership before the launched stack unbinds its
    // durable gates. Finalization consumes the same authority explicitly.
    runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1,
    // The exact runner-local Proposal state remains live until readiness and
    // ingress retire. Dropping the prepared owner before this boundary would
    // have made activation impossible.
    local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,
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
    /// Reauthenticate a CompleteTip successor after recovered-output launch settlement.
    pub(super) fn reauthenticate_recovered_complete_tip_successor(
        &mut self,
        retirement: &mut super::ledger::RetiredRecoveredCompleteTipActivationAuthorityV1,
    ) -> Result<(), super::ledger::CompleteTipSuccessorOwnerBindErrorV1> {
        let launched_ownership_matches = self
            .owner
            .body_store_identity
            .as_ref()
            .is_some_and(|identity| self.services.matches_lifecycle_body_store(identity))
            && self
                .services
                .matches_lifecycle_payload_store(&self.owner.payload_store.instance_identity())
            && self
                .services
                .matches_lifecycle_executor_output_guard(&self.executor);
        let result = launched_ownership_matches
            .then(|| retirement.reauthenticate_launched_successor_owner(&mut self.owner))
            .unwrap_or(Err(super::ledger::CompleteTipSuccessorOwnerBindErrorV1));
        if result.is_err() {
            self.services
                .lifecycle_output_guard()
                .close_admission_for_restart();
        }
        result
    }

    fn finish_clean_shutdown(
        mut self,
        operation: Option<crate::sumeragi::output_guard::ConsensusFailStopOperation<'_>>,
        runner_retirement: Result<(), super::super::v2_runner::V2RunnerError>,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let ingress_retirement = self.leader_wire_ingress_binding.retire();
        if let Err(error) = runner_retirement {
            return Err(ProductionLifecycleShutdownErrorV1::Runner(
                error.to_string(),
            ));
        }
        if let Err(error) = ingress_retirement {
            return Err(ProductionLifecycleShutdownErrorV1::Ingress(error));
        }
        let Some(operation) = operation else {
            return Err(ProductionLifecycleShutdownErrorV1::OutputClosed);
        };
        self.services.allow_clean_shutdown();
        operation.complete();
        Ok(())
    }

    /// Consume an unpublished height during an orderly operator shutdown.
    ///
    /// This does not claim finality or retire durable lifecycle rows. It closes
    /// runner admission, detaches durable leader-wire ingress, and permits the
    /// worker to stop normally; any parked affine recovery owner may still
    /// require cold replay when this stack subsequently drops.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        runner: super::super::v2_runner::ProductionLifecycleRunnerActivationV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let output_guard = self.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress);
        self.finish_clean_shutdown(operation, runner_retirement)
    }

    /// Consume a sealed CompleteTip successor without publishing H+1 status.
    #[allow(dead_code, clippy::result_large_err)]
    pub(super) fn into_complete_tip_clean_shutdown(
        self,
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        retirement: super::ledger::RetiredRecoveredCompleteTipActivationAuthorityV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let output_guard = self.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress);
        drop(retirement);
        self.finish_clean_shutdown(operation, runner_retirement)
    }

    /// Cross the ordinary/current/snapshot live-height boundary exactly once.
    #[allow(dead_code)]
    pub(in crate::sumeragi) fn activate(
        self,
        now: Instant,
        runner: super::super::v2_runner::ProductionLifecycleRunnerActivationV1,
        local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        self.activate_with(
            now,
            ProductionLifecycleActivationPublicationV1::Runner(runner),
            local_proposal,
        )
    }

    /// Cross the CompleteTip boundary without separating retired H from H+1.
    #[allow(dead_code)]
    pub(super) fn activate_recovered_complete_tip(
        self,
        now: Instant,
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        retirement: super::ledger::RetiredRecoveredCompleteTipActivationAuthorityV1,
        local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        self.activate_with(
            now,
            ProductionLifecycleActivationPublicationV1::RecoveredCompleteTip { runner, retirement },
            local_proposal,
        )
    }

    fn activate_with(
        mut self,
        now: Instant,
        publication: ProductionLifecycleActivationPublicationV1,
        local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,
    ) -> Result<ActivatedProductionLifecycleV1, ProductionLifecycleActivationErrorV1> {
        if let Some(error) = lifecycle_activation_recovery_blocker(
            self.pending_kura_apply_replay.is_some(),
            self.executor
                .pending_kura_apply_recovery_evidence()
                .is_some(),
            self.recovered_local_proposal_attempt.is_some(),
        ) {
            self.services
                .lifecycle_output_guard()
                .close_admission_for_restart();
            return Err(error);
        }
        let output_guard = self.services.lifecycle_output_guard();
        let activation = output_guard
            .begin_fail_stop_operation()
            .ok_or(ProductionLifecycleActivationErrorV1::OutputClosed)?;
        let current_directive = self
            .executor
            .local_proposal_directive()
            .map_err(ProductionLifecycleActivationErrorV1::LocalProposalDirective)?;
        if !local_proposal.exactly_matches(self.executor.context().id(), current_directive) {
            return Err(ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch);
        }
        let clock_activation = ProductionLifecycleLiveClockActivationPermitV1 {
            _seal: ProductionLifecycleLiveClockActivationPermitSealV1,
        };
        self.executor
            .arm_live_clocks(clock_activation, now)
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
            local_proposal,
            launched: self,
        })
    }

    fn verify_published_store_marker_finalization_census(&self) -> Result<(), String> {
        let executor = self.executor.published_lifecycle_store_retry_census()?;
        let registry = self
            .owner
            .registry
            .registry()
            .published_lifecycle_store_retry_census()?;
        if executor != registry {
            return Err(
                "executor markers differ from authenticated lifecycle Store rows".to_owned(),
            );
        }
        Ok(())
    }

    fn ready_for_finalized_rollover(
        &mut self,
    ) -> Result<bool, ProductionLifecycleFinalizationErrorV1> {
        let locally_ready = self.executor.ready_to_finish()
            && !self.owner.has_recovered_lifecycle_outputs()
            && self.pending_kura_apply_replay.is_none()
            && self.recovered_local_proposal_attempt.is_none()
            && self.pending_lifecycle_completion.is_none()
            && self.pending_ingress_capacity.is_none()
            && self.completion_observer_activation.is_none()
            && self
                .owner
                .registry
                .registry_mut()
                .exactly_covers_finalization_work(&self.owner.coordinator);
        if !locally_ready {
            return Ok(false);
        }
        self.verify_published_store_marker_finalization_census()
            .map_err(ProductionLifecycleFinalizationErrorV1::StoreMarkerCensus)?;
        Ok(true)
    }
}

impl ActivatedProductionLifecycleV1 {
    /// Return whether every executor and lifecycle owner can cross final rollover now.
    ///
    /// Keeping this predicate shared with the consuming transition lets the
    /// active runner stutter through another Completion turn while authenticated
    /// lifecycle work remains schedulable, without weakening the finalizer's
    /// fail-closed check.
    pub(in crate::sumeragi) fn ready_for_finalized_rollover(
        &mut self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<bool, ProductionLifecycleFinalizationErrorV1> {
        self.launched.ready_for_finalized_rollover()
    }

    /// Close new physical ingress while retaining the activated lifecycle for
    /// a finite terminal-recovery drain before finalized rollover.
    pub(in crate::sumeragi) fn close_runner_ingress_for_finalized_drain(
        &self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
        receiver: &Arc<FairV2Ingress>,
    ) -> Result<(), super::super::v2_runner::V2RunnerError> {
        self.runner_activation.close_ingress(receiver)?;
        if !Arc::ptr_eq(receiver, &self.launched.leader_wire_ingress_binding.ingress) {
            return Err(super::super::v2_runner::V2RunnerError::LifecycleActivationIngressMismatch);
        }
        Ok(())
    }

    /// Consume an active, possibly non-final height for orderly operator exit.
    ///
    /// Durable WAL, body, and lifecycle rows remain untouched for cold replay.
    /// Runner readiness closes first, followed by the retained local Proposal
    /// state and durable leader-wire ingress. This path never mints finality or
    /// finalized-output rollover authority.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<(), ProductionLifecycleShutdownErrorV1> {
        let Self {
            launched,
            local_proposal,
            runner_activation,
        } = self;
        let output_guard = launched.services.lifecycle_output_guard();
        let operation = output_guard.begin_fail_stop_operation();
        let runner_retirement =
            runner_activation.retire(&launched.leader_wire_ingress_binding.ingress);
        drop(local_proposal);
        launched.finish_clean_shutdown(operation, runner_retirement)
    }

    /// Consume the activated height after executor and lifecycle work quiesce.
    ///
    /// Readiness closes before durable leader-wire ingress retires. Only then is
    /// the executor consumed and the adapter closed under one fail-stop output
    /// operation. Its exact WAL remains owned by the finalized rollover until
    /// lane/output durability completes. Every error consumes the height and
    /// leaves service teardown armed for restart.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_finalized_rollover(
        mut self,
        _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    ) -> Result<FinalizedProductionLifecycleRolloverV1, ProductionLifecycleFinalizationErrorV1>
    {
        if !self.launched.ready_for_finalized_rollover()? {
            return Err(ProductionLifecycleFinalizationErrorV1::NotReady);
        }

        let Self {
            mut launched,
            local_proposal,
            runner_activation,
        } = self;
        runner_activation
            .retire(&launched.leader_wire_ingress_binding.ingress)
            .map_err(|error| ProductionLifecycleFinalizationErrorV1::Runner(error.to_string()))?;
        drop(local_proposal);
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
            pending_kura_apply_replay,
            recovered_local_proposal_attempt,
            pending_lifecycle_completion,
            pending_ingress_capacity,
            completion_observer_activation,
            leader_wire_ingress_binding,
        } = launched;
        debug_assert!(pending_kura_apply_replay.is_none());
        debug_assert!(recovered_local_proposal_attempt.is_none());
        debug_assert!(pending_lifecycle_completion.is_none());
        debug_assert!(pending_ingress_capacity.is_none());
        debug_assert!(completion_observer_activation.is_none());
        drop(pending_lifecycle_completion);
        drop(pending_kura_apply_replay);
        drop(recovered_local_proposal_attempt);
        drop(pending_ingress_capacity);
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
            finalized_adapter: finalized,
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
            mut launched,
            local_proposal,
            runner_activation,
        } = self;
        runner_activation
            .retire(&launched.leader_wire_ingress_binding.ingress)
            .map_err(|error| error.to_string())?;
        drop(local_proposal);
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
            pending_kura_apply_replay,
            recovered_local_proposal_attempt,
            pending_lifecycle_completion,
            pending_ingress_capacity,
            completion_observer_activation,
            leader_wire_ingress_binding,
        } = launched;
        assert!(pending_kura_apply_replay.is_none());
        assert!(recovered_local_proposal_attempt.is_none());
        assert!(pending_lifecycle_completion.is_none());
        assert!(pending_ingress_capacity.is_none());
        assert!(completion_observer_activation.is_none());
        drop(executor);
        drop(pending_kura_apply_replay);
        drop(recovered_local_proposal_attempt);
        drop(pending_lifecycle_completion);
        drop(pending_ingress_capacity);
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

    /// Borrow the live owner/runtime/service/local-Proposal owners only from the runner.
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
            &mut super::super::v2_runner::ProductionLifecycleLocalProposalStateV1,
        ) -> R,
    ) -> R {
        let local_proposal = self
            .local_proposal
            .runner
            .prepared_local_proposal_mut()
            .expect("activated lifecycle retains its prepared local-Proposal owner");
        operation(
            &mut self.launched.owner,
            &mut self.launched.executor,
            &mut self.launched.services,
            local_proposal,
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
            finalized_adapter,
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
        let wal_retirement_warning = finalized_adapter.retire_after_output_handoff();
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
            recovered_lifecycle_outputs,
            mut payload_store,
            serve_payloads,
            body_store,
            body_store_identity,
            kura_binding,
            apply_service,
            adapter_startup,
            timeout_supersession_successor: _,
        } = owner;
        debug_assert!(
            recovered_lifecycle_outputs
                .as_ref()
                .is_none_or(super::open::PreparedLifecycleOutputRecoveryV1::is_empty)
        );
        drop(recovered_lifecycle_outputs);
        let kura_binding = kura_binding.ok_or_else(|| {
            ProductionLifecycleFinalizationErrorV1::RetirementCensus(
                "finalized lifecycle owner lost its recovered Kura binding".to_owned(),
            )
        })?;
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

        let published_floor = publication.consume_owners(registry);
        let retained_floor = kura_binding.bind_finalized_lifecycle_floor(published_floor);
        drop(retired_ingress);
        drop(verified);
        drop(payload_store);
        drop(body_store);
        drop(body_store_identity);
        drop(apply_service);
        drop(adapter_startup);
        operation.complete();
        Ok(ProductionLifecycleCleanupReadyV1 {
            services,
            receipt,
            wal_retirement_warning,
            retained_floor,
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
        let output_guard = self.services.lifecycle_output_guard();
        self.services.allow_clean_shutdown();
        let cleanup = self
            .services
            .finish_height(self.receipt, cleanup_timeout, supervisor);
        ProductionLifecycleFinalizationOutcomeV1 {
            cleanup,
            wal_retirement_warning: self.wal_retirement_warning,
            retained_floor: Some(self.retained_floor),
            output_guard,
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
    #[inline(never)]
    pub(in crate::sumeragi) fn launch(
        mut self,
        inputs: ProductionLifecycleLaunchInputsV1,
    ) -> Result<Box<LaunchedProductionLifecycleV1>, ProductionLifecycleLaunchErrorV1> {
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
        let Some(owner_held_outputs) = self.exact_lifecycle_output_ordinals_for_registry_census()
        else {
            return Err(ProductionLifecycleLaunchErrorV1::InvalidOwner);
        };
        if {
            let registry = self.registry.registry_mut();
            !registry.exactly_covers_recovered_ready_work_with_owner_held_outputs(
                &self.coordinator,
                &owner_held_outputs,
            ) && !registry
                .exactly_covers_recovered_ready_work_and_wal_authority_with_owner_held_outputs(
                    &self.coordinator,
                    &owner_held_outputs,
                )
        } {
            return Err(ProductionLifecycleLaunchErrorV1::InvalidOwner);
        }
        let recovered_validate_sidecar =
            RegisteredLifecycleValidateSidecarWaitV1::recover_at_launch(
                &mut self.coordinator,
                &mut self.registry,
            )
            .map_err(|error| {
                ProductionLifecycleLaunchErrorV1::ValidateSidecarRegistration(error.to_string())
            })?;
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
        // One paired authority covers durable coordinator admission, runtime,
        // and fair ingress. Seed it after the opened LedgerV1 floor, then
        // advance the shared cursor through every leader-wire recovery cut
        // before either admission surface opens.
        let (runtime_ordinal_authority, coordinator_ordinal_authority) =
            super::authority::lifecycle_ordinal_authorities_after_high_watermark(
                self.coordinator.high_water(),
            );
        let lifecycle_ordinals =
            RuntimeLifecycleOrdinalSource::from_authority(runtime_ordinal_authority);
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
        self.coordinator
            .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
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
        let (runtime, mut pending_kura_apply_replay, recovered_local_proposal_attempt) =
            adapter_startup
                .into_serialized_runtime(
                    inputs.runtime_started_at,
                    inputs.round_timeout,
                    inputs.runtime_queue,
                    lifecycle_ordinals.clone(),
                )
                .map_err(ProductionLifecycleLaunchErrorV1::Runtime)?;
        let replayed_decision = runtime.replayed_decision_key().map_err(|error| {
            ProductionLifecycleLaunchErrorV1::Executor(EffectExecutorError::Runtime(
                error.to_string(),
            ))
        })?;
        let recovered_validate_retry_census = self
            .registry
            .project_recovered_durable_validate_retry_census(&self.coordinator, replayed_decision)
            .map_err(|error| {
                ProductionLifecycleLaunchErrorV1::Executor(EffectExecutorError::Contract(format!(
                    "cold Validate retry authority failed closed: {error:?}"
                )))
            })?;
        let (mut executor, body_store) = V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            recovered_validate_retry_census,
            pending_kura_apply_replay.as_mut(),
            context.clone(),
            inputs.local_peer.clone(),
            inputs.local_validator,
            Arc::clone(&inputs.output_guard),
            inputs.effect_queue,
        )
        .map_err(ProductionLifecycleLaunchErrorV1::Executor)?;
        if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref() {
            executor
                .install_authenticated_genesis_body(authenticated_genesis)
                .map_err(ProductionLifecycleLaunchErrorV1::Executor)?;
        }
        for (effect, pending, durable_receipt) in self
            .registry
            .registry()
            .recovered_published_store_retry_markers()
        {
            executor
                .install_recovered_published_lifecycle_store_retry_marker(
                    effect,
                    pending,
                    durable_receipt,
                )
                .map_err(ProductionLifecycleLaunchErrorV1::Executor)?;
        }
        for (effect, pending, durable_receipt, lifecycle_ordinal) in self
            .registry
            .registry()
            .recovered_published_validate_retry_markers()
        {
            executor
                .install_recovered_published_lifecycle_validate_retry_marker(
                    effect,
                    pending,
                    durable_receipt,
                    lifecycle_ordinal,
                )
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
        let mut services = ProductionV2Services::start_with_apply_service(
            super::ProductionLifecycleApplyServiceLaunchPermitV1 {
                _seal: super::ProductionLifecycleApplyServiceLaunchPermitSealV1,
            },
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            inputs.local_peer,
            inputs.local_validator,
            inputs.kagemusha_mint_finality_authority,
            inputs.key_pair,
            inputs.network,
            body_store,
            payload_store_identity.clone(),
            inputs.state,
            inputs.kura,
            apply_service,
            inputs.consensus_io_capacity,
            inputs.auxiliary_io_capacity,
            inputs.orphan_chunk_capacity,
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
        loop {
            match settle_one_recovered_lifecycle_output(&mut self, &mut executor, &mut services)
                .map_err(ProductionLifecycleLaunchErrorV1::Executor)?
            {
                super::RecoveredLifecycleOutputSettlementV1::Completed => {}
                super::RecoveredLifecycleOutputSettlementV1::Empty
                | super::RecoveredLifecycleOutputSettlementV1::Deferred
                | super::RecoveredLifecycleOutputSettlementV1::SourceRetained => break,
            }
        }
        self.body_store_identity = Some(body_store_identity);
        construction.complete();
        Ok(Box::new(LaunchedProductionLifecycleV1 {
            owner: self,
            executor,
            services,
            pending_kura_apply_replay,
            recovered_local_proposal_attempt,
            pending_lifecycle_completion: recovered_validate_sidecar
                .map(PendingLifecycleCompletionV1::RegisteredDeferredValidate),
            pending_ingress_capacity: None,
            completion_observer_activation: Some(
                ProductionV2CompletionObserverActivationPermitV1 {
                    _seal: ProductionV2CompletionObserverActivationPermitSealV1,
                },
            ),
            leader_wire_ingress_binding,
        }))
    }
}

#[cfg(test)]
mod tests {
    include!("v2_lifecycle_launch_tests.rs");
}
