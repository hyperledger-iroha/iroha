//! Sealed executor join for exact fair-ingress selector debt.
#[cfg(test)]
use super::{
    CapacityClass, OwnerId, PhysicalSlotId,
    work_registry::{ConcreteLifecycleWork, ConcreteWorkAddress},
};
use super::{
    LifecycleCoordinator, LifecycleWorkRegistryHolder,
    ingress_position::{
        FairIngressQueueCut, FairIngressQueueCutError, FairIngressQueuePositions,
        LockedPreparedFairIngressExactDequeue, PendingFairIngressIdentity,
        PreparedFairIngressQueueWitness,
    },
    projection::{
        certified_fetch_lifecycle_key, certified_fetch_wait_source, pending_effect_causal_root,
    },
    schema::{
        CausalRoot, LifecycleContext, LifecycleDigest, LifecycleKey, LifecyclePhase,
        LifecycleState, LifecycleWorkClass, ReadyEvent, WaitSource,
    },
    work_registry::{
        CertifiedFetchCompletionError, CertifiedFetchWaitingLocation,
        ConcreteLifecycleWorkRegistry, PreparedCertifiedFetchCompletion,
    },
};
#[cfg(test)]
use crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding;
use crate::sumeragi::{
    FairV2Ingress, FairV2IngressClass, FairV2IngressDequeueDisposition,
    FairV2IngressQueueGateVerdict, FairV2IngressSourceClass, InboundBlockMessage,
    message::BlockMessage,
    v2_body_store::{
        DurableCertifiedFetchBodyReceipt, RecoveredDecisionFetchStoreBodyAuthorityV1, V2BodyStore,
        V2BodyStoreError,
    },
    v2_effects::{
        CertifiedResponsePriorityCandidate, CertifiedResponsePriorityProbe, EffectExecutorError,
        EffectTransportError, EffectWorkId, RecoveredDecisionFetchResponseCandidateV1,
        V2EffectExecutor, v2_ingress_head_can_drain,
    },
    v2_runtime::SerializedV2Runtime,
    v2_transport::AuthenticatedCertifiedBodyResponse,
    v2_transport::V2TransportError,
    v2_worker::{PreparedCertifiedFetchBodyPersistenceCompletion, ProductionV2Services},
};
use iroha_crypto::HashOf;
use iroha_data_model::block::consensus_v2 as wire;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
/// Exact typed reason why one drainable occurrence owns selector priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleIngressPriorityAuthority {
    /// A formally untrusted physical completion predates an active request cut.
    RequestFencedCompletion,
    /// The lowest physical occurrence in one authenticated response family.
    ClaimedResponseFamily,
}
/// Complete classification of one exact pre-cut physical occurrence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LifecycleIngressOccurrenceVerdict {
    request_fenced_completion: bool,
    claimed_response_family: bool,
}
impl LifecycleIngressOccurrenceVerdict {
    const NOT_PRIORITY: Self = Self {
        request_fenced_completion: false,
        claimed_response_family: false,
    };
    const fn with_authority(mut self, authority: LifecycleIngressPriorityAuthority) -> Self {
        match authority {
            LifecycleIngressPriorityAuthority::RequestFencedCompletion => {
                self.request_fenced_completion = true;
            }
            LifecycleIngressPriorityAuthority::ClaimedResponseFamily => {
                self.claimed_response_family = true;
            }
        }
        self
    }
    const fn is_priority(self) -> bool {
        self.request_fenced_completion || self.claimed_response_family
    }
}
/// Why the executor could not seal an exact selector snapshot.
#[derive(Debug)]
pub(crate) enum LifecycleIngressSelectorError {
    /// The queue could not mint an exact pre-cut witness for the target.
    QueueCutCapture,
    /// The target queue cut belongs to another height context.
    ForeignContext,
    /// The queue changed while the executor classified its immutable carriers.
    QueueCutChanged,
    /// The queue cut lost an exact ingress ownership carrier.
    InvalidOccurrenceIdentity {
        /// Exact physical occurrence whose ownership was absent.
        ordinal: u64,
    },
    /// A prepared claimed-response candidate changed during exact re-probing.
    CandidateRevalidationDrift {
        /// Exact response occurrence whose candidate no longer matched.
        ordinal: u64,
    },
    /// Exact executor ownership or fail-stop validation failed.
    ExecutorAuthority {
        /// Exact physical occurrence, or `None` for a whole-executor cut.
        ordinal: Option<u64>,
        /// Typed executor validation failure.
        error: Box<EffectTransportError>,
    },
    /// The reducer-owned terminal snapshot could not be read before selection.
    ExecutorState(Box<EffectExecutorError>),
    /// The complete occurrence key set or cardinality was not representable.
    InvalidCensus,
}
/// Opaque exact identity of one bounded certified-Fetch persistence command.
///
/// The physical queue identity remains nested and cannot be reconstructed from
/// a work id, response hash, or caller-supplied ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct CertifiedFetchBodyPersistenceId {
    ingress_identity: PendingFairIngressIdentity,
    work_id: EffectWorkId,
}
/// Move-only storage-worker command prepared from one consumed selector.
///
/// It owns the sole retained [`AuthenticatedCertifiedBodyResponse`] minted by
/// the winning executor probe and deliberately retains no queue witness.
#[derive(Debug)]
#[must_use = "the exact authenticated response must be persisted or returned"]
pub(in crate::sumeragi) struct CertifiedFetchBodyPersistenceTask {
    id: CertifiedFetchBodyPersistenceId,
    authenticated: AuthenticatedCertifiedBodyResponse,
}
impl CertifiedFetchBodyPersistenceTask {
    /// Return the exact indexed command identity used by the bounded I/O FIFO.
    pub(in crate::sumeragi) const fn id(&self) -> CertifiedFetchBodyPersistenceId {
        self.id
    }
    /// Return whether this command came from the exact queue-selected carrier.
    pub(in crate::sumeragi) fn matches_ingress_identity(
        &self,
        identity: PendingFairIngressIdentity,
    ) -> bool {
        self.id.ingress_identity == identity
    }
    /// Return the existing executor work id; this does not allocate a runtime ordinal.
    pub(in crate::sumeragi) const fn work_id(&self) -> EffectWorkId {
        self.id.work_id
    }
    /// Hash the complete response for the existing exact-command descriptor.
    pub(in crate::sumeragi) fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        HashOf::new(self.authenticated.response())
    }
    /// Persist through the one height-local body-store owner.
    ///
    /// Failure returns this whole move-only task. Success carries the exact
    /// queue identity and authenticated response forward beside the sealed
    /// durable receipt, but never carries the stale selector witness.
    pub(in crate::sumeragi) fn persist(
        self,
        body_store: &mut V2BodyStore,
    ) -> Result<CertifiedFetchBodyPersistenceCompletion, (V2BodyStoreError, Self)> {
        let receipt =
            match body_store.persist_authenticated_certified_fetch_response(&self.authenticated) {
                Ok(receipt) => receipt,
                Err(error) => return Err((error, self)),
            };
        Ok(CertifiedFetchBodyPersistenceCompletion {
            id: self.id,
            authenticated: self.authenticated,
            receipt,
        })
    }
}
/// Move-only ordinary I/O completion for one exact persisted response body.
///
/// This value is the only input to the fresh-selector Phase-B transaction. It
/// exposes neither response parts nor a receipt constructor.
#[derive(Debug)]
#[must_use = "the durable response must enter the fresh-selector transaction"]
pub(crate) struct CertifiedFetchBodyPersistenceCompletion {
    id: CertifiedFetchBodyPersistenceId,
    authenticated: AuthenticatedCertifiedBodyResponse,
    receipt: DurableCertifiedFetchBodyReceipt,
}
impl CertifiedFetchBodyPersistenceCompletion {
    /// Return the exact indexed command identity for completion acknowledgement.
    pub(in crate::sumeragi) const fn id(&self) -> CertifiedFetchBodyPersistenceId {
        self.id
    }
    /// Return the existing executor work id without exposing queue coordinates.
    pub(in crate::sumeragi) const fn work_id(&self) -> EffectWorkId {
        self.id.work_id
    }
    /// Hash the complete authenticated response for exact command acknowledgement.
    pub(in crate::sumeragi) fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        HashOf::new(self.authenticated.response())
    }
}
/// Opaque physical-ingress and lifecycle identity of one recovered Decision
/// Fetch body persistence command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDecisionFetchBodyPersistenceIdV1 {
    ingress_identity: PendingFairIngressIdentity,
    dispatch_key: super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
}
/// Move-only body-store command owned by the recovered Decision Fetch carrier.
#[derive(Debug)]
#[must_use = "the recovered Decision Fetch response must be durably persisted"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchBodyPersistenceTaskV1 {
    id: RecoveredDecisionFetchBodyPersistenceIdV1,
    claim_preflight: crate::sumeragi::v2_transport::CertifiedBodyResponseClaimPreflight,
    authenticated: AuthenticatedCertifiedBodyResponse,
}
impl RecoveredDecisionFetchBodyPersistenceTaskV1 {
    /// Build one exact authenticated persistence task behind a sealed test target.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        target: &LifecycleIngressIoTargetSeal,
        dispatch_key: super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
        authenticated: AuthenticatedCertifiedBodyResponse,
    ) -> Self {
        assert_eq!(
            target.kind(),
            LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
        );
        assert!(target.matches_recovered_decision_fetch_key(dispatch_key));
        Self {
            id: RecoveredDecisionFetchBodyPersistenceIdV1 {
                ingress_identity: target.ingress_identity(),
                dispatch_key,
            },
            claim_preflight:
                crate::sumeragi::v2_transport::CertifiedBodyResponseClaimPreflight::Vacant,
            authenticated,
        }
    }
    /// Return the dedicated lifecycle/ingress queue identity.
    pub(in crate::sumeragi) const fn id(&self) -> RecoveredDecisionFetchBodyPersistenceIdV1 {
        self.id
    }
    /// Return the recovered lifecycle dispatch owner.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> super::work_registry::RecoveredDecisionFetchDispatchKeyV1 {
        self.id.dispatch_key
    }
    /// Return the exact queue ordinal solely for fresh selector recapture.
    pub(in crate::sumeragi) const fn physical_admission_ordinal(&self) -> u64 {
        self.id.ingress_identity.physical_admission_ordinal()
    }
    /// Match the exact selected physical ingress occurrence.
    pub(in crate::sumeragi) fn matches_ingress_identity(
        &self,
        identity: PendingFairIngressIdentity,
    ) -> bool {
        self.id.ingress_identity == identity
    }
    /// Hash the complete authenticated response without exposing its parts.
    pub(in crate::sumeragi) fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        HashOf::new(self.authenticated.response())
    }
    /// Return the response-family state observed by the final exact probe.
    pub(in crate::sumeragi) const fn claim_preflight(
        &self,
    ) -> crate::sumeragi::v2_transport::CertifiedBodyResponseClaimPreflight {
        self.claim_preflight
    }
    /// Persist the authenticated response through the crash-safe body store.
    pub(in crate::sumeragi) fn persist(
        self,
        body_store: &mut V2BodyStore,
    ) -> Result<RecoveredDecisionFetchBodyPersistenceCompletionV1, (V2BodyStoreError, Self)> {
        let receipt =
            match body_store.persist_authenticated_certified_fetch_response(&self.authenticated) {
                Ok(receipt) => receipt,
                Err(error) => return Err((error, self)),
            };
        Ok(RecoveredDecisionFetchBodyPersistenceCompletionV1 {
            id: self.id,
            authenticated: self.authenticated,
            receipt,
        })
    }
}
/// Durable body receipt retained under the recovered Decision Fetch owner.
#[derive(Debug)]
#[must_use = "the recovered Decision Fetch completion must enter the fixed Store settlement"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchBodyPersistenceCompletionV1 {
    id: RecoveredDecisionFetchBodyPersistenceIdV1,
    authenticated: AuthenticatedCertifiedBodyResponse,
    receipt: DurableCertifiedFetchBodyReceipt,
}
impl RecoveredDecisionFetchBodyPersistenceCompletionV1 {
    /// Return the exact dedicated queue identity.
    pub(in crate::sumeragi) const fn id(&self) -> RecoveredDecisionFetchBodyPersistenceIdV1 {
        self.id
    }
    /// Return the lifecycle dispatch owner without exposing response parts.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> super::work_registry::RecoveredDecisionFetchDispatchKeyV1 {
        self.id.dispatch_key
    }
    /// Return the queue ordinal solely for fresh exact-selector recapture.
    pub(in crate::sumeragi) const fn physical_admission_ordinal(&self) -> u64 {
        self.id.ingress_identity.physical_admission_ordinal()
    }
    /// Hash the exact durable authenticated response.
    pub(in crate::sumeragi) fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        HashOf::new(self.authenticated.response())
    }
    /// Project the fixed body-frame authority without exposing response or receipt parts.
    pub(in crate::sumeragi) fn project_store_body_authority(
        &self,
    ) -> Option<RecoveredDecisionFetchStoreBodyAuthorityV1> {
        RecoveredDecisionFetchStoreBodyAuthorityV1::from_persisted_certified_response(
            &self.authenticated,
            &self.receipt,
        )
    }
}
/// Why a recovered selector could not become one dedicated persistence task.
#[derive(Debug)]
pub(in crate::sumeragi) enum RecoveredDecisionFetchBodyPersistencePreparationFailureV1 {
    /// The selected occurrence was not the exact recovered response family.
    Selector(CertifiedFetchReadyPublicationError),
    /// The executor candidate changed during the final equality re-probe.
    Executor(EffectTransportError),
}
/// Ownership-preserving recovered persistence preparation failure.
#[must_use = "the unchanged selector remains available for capacity rollback"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
    _failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1,
    prepared: PreparedLifecycleIngressSelector,
}
impl RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
    /// Recover the unchanged complete selector for reservation rollback.
    pub(in crate::sumeragi) fn into_prepared(self) -> PreparedLifecycleIngressSelector {
        self.prepared
    }
}
/// Why a selector could not be consumed into one storage-worker command.
#[derive(Debug)]
#[allow(variant_size_differences)]
pub(crate) enum CertifiedFetchBodyPersistencePreparationFailure {
    /// The selected occurrence did not retain exact certified-Fetch authority.
    Selector(CertifiedFetchReadyPublicationError),
    /// The executor changed before the selector's final equality re-probe.
    Executor(EffectTransportError),
}
/// Ownership-preserving Phase-A preparation failure.
#[must_use = "the unchanged selector remains available for retry or drop"]
pub(crate) struct CertifiedFetchBodyPersistencePreparationError {
    failure: CertifiedFetchBodyPersistencePreparationFailure,
    prepared: PreparedLifecycleIngressSelector,
}
impl CertifiedFetchBodyPersistencePreparationError {
    /// Return the closed failure classification.
    pub(crate) const fn failure(&self) -> &CertifiedFetchBodyPersistencePreparationFailure {
        &self.failure
    }
    /// Recover the complete unchanged selector preparation.
    pub(crate) fn into_prepared(self) -> PreparedLifecycleIngressSelector {
        self.prepared
    }
}
/// Retryable failure before the LedgerV1 publication call begins.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum CertifiedFetchBodyPersistenceRetryFailure {
    FreshSelector(LifecycleIngressSelectorError),
    Selector(CertifiedFetchReadyPublicationError),
    CompletionIdentity,
    Executor(EffectTransportError),
    CoordinatorStutter,
    Registry(CertifiedFetchCompletionError),
    Service(String),
    OutputClosed,
}
/// Ownership-preserving failure from the pre-LedgerV1 half of Phase B.
///
/// The only recovery surface returns the complete opaque ordinary-completion
/// outcome. It never exposes the authenticated response, receipt, work ack,
/// queue coordinates, or decomposed response parts.
#[must_use = "the persisted response still owns its exact retry authority"]
pub(crate) struct CertifiedFetchBodyPersistenceRetryError {
    failure: CertifiedFetchBodyPersistenceRetryFailure,
    completion: PreparedCertifiedFetchBodyPersistenceCompletion,
}
impl CertifiedFetchBodyPersistenceRetryError {
    /// Stable diagnostic category for the retryable pre-ledger rejection.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            CertifiedFetchBodyPersistenceRetryFailure::FreshSelector(_) => "fresh selector",
            CertifiedFetchBodyPersistenceRetryFailure::Selector(_) => "selector authority",
            CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity => {
                "persistence completion identity"
            }
            CertifiedFetchBodyPersistenceRetryFailure::Executor(_) => "executor preflight",
            CertifiedFetchBodyPersistenceRetryFailure::CoordinatorStutter => "coordinator mutation",
            CertifiedFetchBodyPersistenceRetryFailure::Registry(_) => "registry preflight",
            CertifiedFetchBodyPersistenceRetryFailure::Service(_) => "service preflight",
            CertifiedFetchBodyPersistenceRetryFailure::OutputClosed => "consensus output closed",
        }
    }
    /// Recover the whole move-only completion for a later fresh-selector retry.
    pub(crate) fn into_completion(self) -> PreparedCertifiedFetchBodyPersistenceCompletion {
        self.completion
    }
    /// Preserve the underlying typed error for diagnostics without exposing it.
    pub(crate) fn detail(&self) -> String {
        match &self.failure {
            CertifiedFetchBodyPersistenceRetryFailure::FreshSelector(error) => {
                format!("{error:?}")
            }
            CertifiedFetchBodyPersistenceRetryFailure::Selector(error) => format!("{error:?}"),
            CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity => {
                "persisted completion differs from the fresh exact selector".to_owned()
            }
            CertifiedFetchBodyPersistenceRetryFailure::Executor(error) => error.to_string(),
            CertifiedFetchBodyPersistenceRetryFailure::CoordinatorStutter => {
                "waiting Fetch did not stage one new Ready successor".to_owned()
            }
            CertifiedFetchBodyPersistenceRetryFailure::Registry(error) => format!("{error:?}"),
            CertifiedFetchBodyPersistenceRetryFailure::Service(error) => error.clone(),
            CertifiedFetchBodyPersistenceRetryFailure::OutputClosed => {
                "consensus output admission is closed".to_owned()
            }
        }
    }
}
/// Exact fresh queue witness retained after LedgerV1 may have advanced.
struct PreparedCertifiedFetchExactDequeue {
    context: LifecycleContext,
    ingress_identity: PendingFairIngressIdentity,
    queue_witness: PreparedFairIngressQueueWitness,
}
/// Closed pre-fsync failure for recovered exact-ingress locking.
#[derive(Debug)]
pub(in crate::sumeragi) enum RecoveredDecisionFetchExactDequeueErrorV1 {
    /// The fresh selected family differs from the parked durable completion.
    CompletionIdentity,
    /// The dedicated executor request owner changed during the final re-probe.
    Executor(EffectTransportError),
    /// The frozen queue prefix changed before the service lock was acquired.
    Queue(FairIngressQueueCutError),
}
/// Prevalidated recovered response dequeue held across LedgerV1 fsync.
#[must_use = "recovered Decision Fetch ingress occurrence has not been acknowledged"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchExactDequeueV1<'a> {
    locked: LockedPreparedFairIngressExactDequeue<'a>,
}
impl PreparedRecoveredDecisionFetchExactDequeueV1<'_> {
    /// Assertion-remove the exact selected response after durable publication.
    pub(in crate::sumeragi) fn commit(self) {
        let (inbound, disposition) = self.locked.commit();
        assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit);
        drop(inbound);
    }
}
impl PreparedCertifiedFetchExactDequeue {
    const fn physical_admission_ordinal(&self) -> u64 {
        self.ingress_identity.physical_admission_ordinal()
    }
    fn commit(
        self,
        ingress: &FairV2Ingress,
    ) -> Result<CertifiedFetchDequeuedResponse, (FairIngressQueueCutError, Self)> {
        let Self {
            context,
            ingress_identity,
            queue_witness,
        } = self;
        match queue_witness.commit_exact_dequeue_retaining(
            ingress,
            context,
            ingress_identity.physical_admission_ordinal(),
        ) {
            Ok((inbound, disposition)) => Ok(CertifiedFetchDequeuedResponse {
                ingress_identity,
                inbound,
                disposition,
            }),
            Err((error, queue_witness)) => Err((
                error,
                Self {
                    context,
                    ingress_identity,
                    queue_witness,
                },
            )),
        }
    }
}
/// Restart-only failure after LedgerV1 publication was invoked.
#[derive(Debug)]
#[allow(variant_size_differences)]
enum CertifiedFetchBodyPersistenceRestartFailure {
    Ledger(String),
    Queue(FairIngressQueueCutError),
}
/// Sealed post-ledger authority which may never be retried in-process.
///
/// Both the still-indexed I/O completion and fresh exact queue witness remain
/// owned for diagnostics and destruction. The fail-stop operation has already
/// closed output before this value can reach its caller.
#[must_use = "post-ledger failure requires process restart"]
pub(crate) struct CertifiedFetchBodyPersistenceRestartError {
    failure: CertifiedFetchBodyPersistenceRestartFailure,
    completion: PreparedCertifiedFetchBodyPersistenceCompletion,
    exact_dequeue: PreparedCertifiedFetchExactDequeue,
}
impl CertifiedFetchBodyPersistenceRestartError {
    /// Stable diagnostic category for the restart-only boundary.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            CertifiedFetchBodyPersistenceRestartFailure::Ledger(_) => "LedgerV1 publication",
            CertifiedFetchBodyPersistenceRestartFailure::Queue(_) => "post-ledger queue CAS",
        }
    }
    /// Preserve the exact post-ledger error for restart diagnostics.
    pub(crate) fn detail(&self) -> String {
        match &self.failure {
            CertifiedFetchBodyPersistenceRestartFailure::Ledger(error) => error.clone(),
            CertifiedFetchBodyPersistenceRestartFailure::Queue(error) => format!("{error:?}"),
        }
    }
    /// Return the still-indexed existing executor work identity.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.completion.work_id()
    }
    /// Return the retained fresh physical queue occurrence for diagnostics.
    pub(crate) const fn physical_admission_ordinal(&self) -> u64 {
        self.exact_dequeue.physical_admission_ordinal()
    }
}
/// Closed Phase-B status split at the LedgerV1 durability boundary.
#[must_use = "retryable and restart-only failures have different ownership rules"]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(crate) enum CertifiedFetchBodyPersistenceCompletionError {
    /// No ledger publication was invoked; the whole completion may be retried.
    Retry(CertifiedFetchBodyPersistenceRetryError),
    /// Ledger publication was invoked; output is closed and retry is forbidden.
    RestartRequired(CertifiedFetchBodyPersistenceRestartError),
}
/// Typed reason an authenticated selected response could not wake its exact
/// existing certified-Fetch record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum CertifiedFetchReadyPublicationError {
    /// The coordinator had already latched an unrelated fail-closed fault.
    CoordinatorFaulted,
    /// The selected ingress occurrence is not its family's unique winner.
    SelectedOccurrenceNotClaimedResponse,
    /// The queue-minted selected occurrence lost its exact identity binding.
    InvalidSelectedOccurrence,
    /// The executor candidate or pending runtime binding lost exact Fetch semantics.
    InvalidCandidateBinding,
    /// Prepared, candidate, statement, or coordinator height contexts disagree.
    ForeignContext,
    /// No existing lifecycle row owns the exact certified-Fetch semantic key.
    MissingLifecycleKey,
    /// Coordinator indexes no longer identify exactly one immutable row.
    InvalidCoordinatorIndex,
    /// The exact key belongs to another authenticated causal root.
    ForeignCausalRoot,
    /// The exact key no longer names Fetch work.
    WrongWorkClass,
    /// The existing Fetch row cannot accept one equal-address response carrier.
    InvalidPhysicalReplacement,
    /// The existing Fetch row waits on another external source.
    WrongWaitSource,
    /// The existing Fetch row waits on another observed generation.
    WrongWaitGeneration,
    /// More than one waiting row would be changed by this source generation.
    AmbiguousWaitSource,
    /// The existing Fetch row is already owned by an active lease.
    ClaimedRecord,
}
/// Result of an exact certified-Fetch response readiness publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
enum CertifiedFetchReadyPublication {
    /// The original waiting row became ready.
    Published,
    /// The exact row was already ready; no generation changed.
    StutterReady,
    /// The exact row was already terminal; no generation changed.
    StutterTerminal,
}
/// Move-only logical mutation prepared before the fallible queue CAS.
struct PreparedCertifiedFetchReadyMutation<'a> {
    target: &'a mut LifecycleCoordinator,
    next: LifecycleCoordinator,
    location: CertifiedFetchWaitingLocation,
}
impl PreparedCertifiedFetchReadyMutation<'_> {
    #[cfg(test)]
    fn target_for_test(&self) -> &LifecycleCoordinator {
        self.target
    }
    fn commit(self) {
        *self.target = self.next;
    }
    fn persist_exact_staged_successor(&self) -> Result<(), super::ledger::LifecycleLedgerError> {
        self.target.persist_exact_staged_successor(&self.next)
    }
}
enum PreparedCertifiedFetchReadyTransition<'a> {
    Mutation(PreparedCertifiedFetchReadyMutation<'a>),
    Stutter(CertifiedFetchReadyPublication),
}
#[derive(Debug)]
#[allow(dead_code)]
enum CertifiedFetchCompletionPreparationError {
    ReadyAuthority(CertifiedFetchReadyPublicationError),
    Registry(CertifiedFetchCompletionError),
}
/// One executor-authenticated claimed-response family winner.
///
/// The queue identity remains distinct across byte-identical retransmissions.
/// The executor candidate is opaque and has been equality re-probed, but no
/// response claim, runtime capacity, service handoff, or composite queue CAS
/// is exposed yet.
#[derive(Debug)]
struct PreparedClaimedResponseFamily {
    ingress_identity: PendingFairIngressIdentity,
    inbound: Arc<InboundBlockMessage>,
    candidate: PreparedCertifiedResponseCandidate,
}
#[derive(Debug)]
enum PreparedCertifiedResponseCandidate {
    Ordinary(Box<CertifiedResponsePriorityCandidate>),
    Recovered(Box<RecoveredDecisionFetchResponseCandidateV1>),
}
impl PreparedCertifiedResponseCandidate {
    fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        match self {
            Self::Ordinary(candidate) => candidate.request_hash(),
            Self::Recovered(candidate) => candidate.request_hash(),
        }
    }
    fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        match self {
            Self::Ordinary(candidate) => candidate.response_hash(),
            Self::Recovered(candidate) => candidate.response_hash(),
        }
    }
    fn matches_authenticated_response(
        &self,
        response: &wire::CertifiedBodyResponse,
        responder: &iroha_data_model::peer::PeerId,
    ) -> bool {
        match self {
            Self::Ordinary(candidate) => {
                candidate.matches_authenticated_response(response, responder)
            }
            Self::Recovered(candidate) => {
                candidate.matches_authenticated_response(response, responder)
            }
        }
    }
    fn ordinary(&self) -> Option<&CertifiedResponsePriorityCandidate> {
        match self {
            Self::Ordinary(candidate) => Some(candidate),
            Self::Recovered(_) => None,
        }
    }
    fn recovered(&self) -> Option<&RecoveredDecisionFetchResponseCandidateV1> {
        match self {
            Self::Ordinary(_) => None,
            Self::Recovered(candidate) => Some(candidate),
        }
    }
}
impl PreparedClaimedResponseFamily {
    fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.candidate.request_hash()
    }
    fn authenticated_response(
        &self,
    ) -> Option<(
        &wire::CertifiedBodyResponse,
        &iroha_data_model::peer::PeerId,
    )> {
        let BlockMessage::V2(message) = self.inbound.message() else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
        else {
            return None;
        };
        Some((response, self.inbound.sender()?))
    }
}
/// Sealed semantic authority derived only from one selected family winner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CertifiedFetchReadyAuthority {
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    key: LifecycleKey,
    causal_root: CausalRoot,
}
/// Exact response owner returned only by the consuming queue witness.
///
/// The registry may inspect this sealed carrier, but no sibling can construct
/// it from a cloned envelope or caller-supplied queue coordinates.
#[derive(Debug)]
#[must_use = "the dequeued response must enter the exact registry completion"]
pub(super) struct CertifiedFetchDequeuedResponse {
    ingress_identity: PendingFairIngressIdentity,
    inbound: InboundBlockMessage,
    disposition: FairV2IngressDequeueDisposition,
}
impl CertifiedFetchDequeuedResponse {
    /// Return the queue-minted identity transferred by checked dequeue.
    pub(super) const fn ingress_identity(&self) -> PendingFairIngressIdentity {
        self.ingress_identity
    }
    /// Borrow the exact owned authenticated wire carrier.
    pub(super) fn inbound(&self) -> &InboundBlockMessage {
        &self.inbound
    }
    /// Return the frozen ordinary dequeue disposition.
    pub(super) const fn disposition(&self) -> FairV2IngressDequeueDisposition {
        self.disposition
    }
}
impl CertifiedFetchReadyAuthority {
    fn wait_source(self) -> WaitSource {
        certified_fetch_wait_source(self.request_hash)
    }
}
/// Unforgeable selector-minted authority for one concrete certified-Fetch
/// completion preflight.
///
/// All fields and the production mint remain private to this module. The
/// concrete registry may inspect this value only after the selector has bound
/// one exact queue identity, signed response family, authenticated responder,
/// and executor candidate. In particular, no sibling can reconstruct it from
/// decomposed hashes or a caller-supplied pending binding.
pub(super) struct CertifiedFetchCompletionAuthority<'a> {
    ready: CertifiedFetchReadyAuthority,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: &'a iroha_data_model::peer::PeerId,
    authenticated_response: &'a wire::CertifiedBodyResponse,
    candidate_pending: &'a crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding,
}
impl CertifiedFetchCompletionAuthority<'_> {
    /// Return the queue-minted identity of the selected physical occurrence.
    pub(super) const fn ingress_identity(&self) -> PendingFairIngressIdentity {
        self.ready.ingress_identity
    }
    /// Return the exact signed-request family selected by the executor join.
    pub(super) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.ready.request_hash
    }
    /// Return the hash of the authenticated selected response.
    pub(super) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Return the authenticated causal root retained by the pending Fetch.
    pub(super) const fn causal_root(&self) -> CausalRoot {
        self.ready.causal_root
    }
    /// Borrow the authenticated outer responder bound by the selector.
    pub(super) const fn authenticated_responder(&self) -> &iroha_data_model::peer::PeerId {
        self.authenticated_responder
    }
    /// Borrow the complete signed response retained by the queue witness.
    pub(super) const fn authenticated_response(&self) -> &wire::CertifiedBodyResponse {
        self.authenticated_response
    }
    /// Borrow the executor-minted exact pending-effect authority.
    pub(super) const fn candidate_pending(
        &self,
    ) -> &crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding {
        self.candidate_pending
    }
}
/// Borrow-free exact selector preparation from one validated queue/executor cut.
///
/// This value is not `SchedulerInputs` or rank authority. Phase A consumes it
/// into one bounded persistence command; Phase B captures a new instance and
/// consumes it inside the LedgerV1/output-permitted transaction. Both paths
/// release every retained family `Arc` after the final candidate probe so an
/// exact checked dequeue can recover exclusive ownership of the envelope.
#[must_use = "the prepared selector must enter one consuming lifecycle transaction"]
pub(crate) struct PreparedLifecycleIngressSelector {
    context: LifecycleContext,
    request_fence_active: bool,
    queue_witness: PreparedFairIngressQueueWitness,
    io_target: PreparedLifecycleIngressIoTarget,
    verdicts: BTreeMap<u64, LifecycleIngressOccurrenceVerdict>,
    priority_owners: BTreeSet<u64>,
    claimed_response_families:
        BTreeMap<HashOf<wire::CertifiedBodyRequest>, PreparedClaimedResponseFamily>,
    selector_debt: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedLifecycleIngressIoTarget {
    CertifiedServe { request: LifecycleDigest },
    CertifiedFetchBodyPersistence,
    RecoveredDecisionFetchBodyPersistence,
    Unsupported,
}
/// Closed I/O command family derived from one authenticated selected carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleIngressIoTargetKind {
    /// Serve one durably admitted certified-body request on the auxiliary lane.
    CertifiedServe,
    /// Persist one authenticated certified-Fetch response on the consensus lane.
    CertifiedFetchBodyPersistence,
    /// Persist one lifecycle-recovered Decision Fetch response without ordinary work ownership.
    RecoveredDecisionFetchBodyPersistence,
}
/// Opaque binding between a selected fair-ingress occurrence and its I/O target.
///
/// No constructor or scalar debt accessor is exposed. The production service
/// may use the seal only to reserve the matching hierarchical FIFO class, and
/// the lifecycle owner may use it only to bind the exact Ready row.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LifecycleIngressIoTargetSeal {
    context: LifecycleContext,
    ingress_identity: PendingFairIngressIdentity,
    kind: LifecycleIngressIoTargetKind,
    certified_serve_request: Option<LifecycleDigest>,
    certified_fetch_work_id: Option<EffectWorkId>,
    recovered_decision_fetch_key: Option<super::work_registry::RecoveredDecisionFetchDispatchKeyV1>,
    _linearity: LifecycleIngressIoTargetSealLinearity,
}
#[derive(Debug, PartialEq, Eq)]
struct LifecycleIngressIoTargetSealLinearity;
impl Drop for LifecycleIngressIoTargetSealLinearity {
    fn drop(&mut self) {}
}
impl LifecycleIngressIoTargetSeal {
    /// Construct a closed target around fixture-owned height coordinates.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        context: &wire::HeightContext,
        kind: LifecycleIngressIoTargetKind,
        physical_admission_ordinal: u64,
    ) -> Self {
        let context = lifecycle_context_from_wire(context);
        Self {
            context,
            ingress_identity: PendingFairIngressIdentity::for_test(
                context,
                LifecycleDigest::new([0xA7; 32]),
                physical_admission_ordinal,
            ),
            kind,
            certified_serve_request: None,
            certified_fetch_work_id: matches!(
                kind,
                LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence
            )
            .then(|| EffectWorkId::for_test(physical_admission_ordinal)),
            recovered_decision_fetch_key: None,
            _linearity: LifecycleIngressIoTargetSealLinearity,
        }
    }
    /// Construct a fixture-only recovered Fetch target with its exact owner key.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_recovered_decision_fetch_test(
        context: &wire::HeightContext,
        dispatch_key: super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
        physical_admission_ordinal: u64,
    ) -> Self {
        let context = lifecycle_context_from_wire(context);
        Self {
            context,
            ingress_identity: PendingFairIngressIdentity::for_test(
                context,
                LifecycleDigest::new([0xA8; 32]),
                physical_admission_ordinal,
            ),
            kind: LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence,
            certified_serve_request: None,
            certified_fetch_work_id: None,
            recovered_decision_fetch_key: Some(dispatch_key),
            _linearity: LifecycleIngressIoTargetSealLinearity,
        }
    }
    /// Construct a fixture-only Serve target carrying the selector-derived
    /// exact signed-request digest and no Fetch identity.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_certified_serve_test(
        context: &wire::HeightContext,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        physical_admission_ordinal: u64,
    ) -> Self {
        let context = lifecycle_context_from_wire(context);
        let mut digest = [0_u8; 32];
        digest.copy_from_slice(request_hash.as_ref());
        Self {
            context,
            ingress_identity: PendingFairIngressIdentity::for_test(
                context,
                LifecycleDigest::new(digest),
                physical_admission_ordinal,
            ),
            kind: LifecycleIngressIoTargetKind::CertifiedServe,
            certified_serve_request: Some(LifecycleDigest::new(digest)),
            certified_fetch_work_id: None,
            recovered_decision_fetch_key: None,
            _linearity: LifecycleIngressIoTargetSealLinearity,
        }
    }
    /// Return the immutable height context authenticated by the selector.
    pub(in crate::sumeragi) const fn context(&self) -> LifecycleContext {
        self.context
    }
    /// Return the selected queue-minted occurrence identity.
    pub(in crate::sumeragi) const fn ingress_identity(&self) -> PendingFairIngressIdentity {
        self.ingress_identity
    }
    /// Return the closed command family authenticated by the carrier.
    pub(in crate::sumeragi) const fn kind(&self) -> LifecycleIngressIoTargetKind {
        self.kind
    }
    /// Return whether a tracked executor id is the selected Fetch command id.
    ///
    /// This comparison keeps the sealed id opaque: the service may reject an
    /// existing owner but cannot extract or reconstruct the underlying value.
    pub(in crate::sumeragi) fn matches_certified_fetch_work_id(
        &self,
        candidate: EffectWorkId,
    ) -> bool {
        self.kind == LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence
            && self.certified_fetch_work_id == Some(candidate)
    }
    /// Compare the dedicated recovered Decision Fetch owner key without exposing it.
    pub(in crate::sumeragi) fn matches_recovered_decision_fetch_key(
        &self,
        candidate: super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
    ) -> bool {
        self.kind == LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
            && self.recovered_decision_fetch_key == Some(candidate)
    }
    /// Return whether this selector-owned target names one exact signed
    /// Certified-Serve request. The stored digest remains opaque and no reply
    /// route or queue position is reconstructed.
    pub(in crate::sumeragi) fn matches_certified_serve_request(
        &self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> bool {
        let mut digest = [0_u8; 32];
        digest.copy_from_slice(request_hash.as_ref());
        self.kind == LifecycleIngressIoTargetKind::CertifiedServe
            && self.certified_serve_request == Some(LifecycleDigest::new(digest))
            && self.certified_fetch_work_id.is_none()
            && self.recovered_decision_fetch_key.is_none()
    }
}
/// Failure to derive an I/O command family from the selected ingress carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleIngressIoTargetError {
    /// The selected carrier does not dispatch through the ordered I/O worker.
    UnsupportedCarrier,
    /// A certified-Fetch response lost its exact authenticated family binding.
    InvalidCertifiedFetch,
}
/// Failure to join the selected Fetch family to its exact waiting coordinator
/// row and concrete registry incumbent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LifecycleIngressSchedulerCarrierError {
    /// The selected I/O family is not a certified-Fetch persistence target.
    UnsupportedCarrier,
    /// Selector or coordinator authority rejected the waiting Fetch identity.
    InvalidWaitingFetch,
    /// The concrete registry incumbent did not match the waiting Fetch slot.
    InvalidRegistryIncumbent,
}
/// Opaque exact Waiting-Fetch generation transition authenticated by the
/// selector, coordinator, and concrete registry together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LifecycleIngressSchedulerFetchSeal {
    owner: super::OwnerId,
    ordinal: u128,
    key: LifecycleKey,
    slot: super::PhysicalSlotId,
    incumbent_digest: LifecycleDigest,
    wake_generation: (WaitSource, u64),
    post_submit_wait: super::WaitToken,
}
impl LifecycleIngressSchedulerFetchSeal {
    /// Return the exact waiting Fetch ordinal.
    pub(super) const fn ordinal(self) -> u128 {
        self.ordinal
    }
    /// Reauthenticate the exact Waiting Fetch row used to mint this seal.
    pub(super) fn matches_waiting_record(self, record: &super::LifecycleRecord) -> bool {
        let LifecycleState::Waiting(wait) = record.state else {
            return false;
        };
        let (source, generation) = self.wake_generation;
        record.owner == self.owner
            && record.ordinal == self.ordinal
            && record.key == self.key
            && record.work_class == LifecycleWorkClass::Fetch
            && record
                .work_class
                .accepts_stage(record.key.phase(), record.stage)
            && record.physical_slots.len() == 1
            && record.physical_slots.get(&self.slot) == Some(&self.incumbent_digest)
            && wait.source() == source
            && certified_fetch_scheduler_generation(wait) == Some(generation)
            && self.post_submit_wait == super::WaitToken::new(source, generation)
    }
    /// Return the authenticated prospective-ready generation advancement.
    pub(super) const fn wake_generation(self) -> (WaitSource, u64) {
        self.wake_generation
    }
    /// Return the same-source fence installed after reserved submission.
    pub(super) const fn post_submit_wait(self) -> super::WaitToken {
        self.post_submit_wait
    }
}
impl PreparedLifecycleIngressSelector {
    /// Return the exact height context shared by the queue and executor.
    pub(super) const fn context(&self) -> LifecycleContext {
        self.context
    }
    /// Return the cardinality of exact concrete priority occurrences.
    pub(super) const fn selector_debt(&self) -> u64 {
        self.selector_debt
    }
    /// Return the target's exact one-based lane/source rank components.
    pub(super) const fn selected_positions(&self) -> FairIngressQueuePositions {
        self.queue_witness.selected_positions()
    }
    /// Return the queue-minted selected physical identity.
    pub(super) const fn selected_identity(&self) -> &PendingFairIngressIdentity {
        self.queue_witness.selected_identity()
    }
    /// Seal the selected carrier's exact I/O command family without exposing
    /// an admission class, queue depth, or caller-constructible identity.
    pub(crate) fn take_lifecycle_io_target(
        &mut self,
    ) -> Result<LifecycleIngressIoTargetSeal, LifecycleIngressIoTargetError> {
        let (kind, certified_serve_request, certified_fetch_work_id, recovered_decision_fetch_key) =
            match self.io_target {
                PreparedLifecycleIngressIoTarget::CertifiedServe { request } => (
                    LifecycleIngressIoTargetKind::CertifiedServe,
                    Some(request),
                    None,
                    None,
                ),
                PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence => {
                    let family = self
                        .selected_claimed_response_family()
                        .map_err(|_| LifecycleIngressIoTargetError::InvalidCertifiedFetch)?;
                    let work_id = family
                        .candidate
                        .ordinary()
                        .map(CertifiedResponsePriorityCandidate::work_id)
                        .ok_or(LifecycleIngressIoTargetError::InvalidCertifiedFetch)?;
                    (
                        LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
                        None,
                        Some(work_id),
                        None,
                    )
                }
                PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence => {
                    let family = self
                        .selected_claimed_response_family()
                        .map_err(|_| LifecycleIngressIoTargetError::InvalidCertifiedFetch)?;
                    let key = family
                        .candidate
                        .recovered()
                        .map(RecoveredDecisionFetchResponseCandidateV1::dispatch_key)
                        .ok_or(LifecycleIngressIoTargetError::InvalidCertifiedFetch)?;
                    (
                        LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence,
                        None,
                        None,
                        Some(key),
                    )
                }
                PreparedLifecycleIngressIoTarget::Unsupported => {
                    return Err(LifecycleIngressIoTargetError::UnsupportedCarrier);
                }
            };
        let target = LifecycleIngressIoTargetSeal {
            context: self.context,
            ingress_identity: *self.selected_identity(),
            kind,
            certified_serve_request,
            certified_fetch_work_id,
            recovered_decision_fetch_key,
            _linearity: LifecycleIngressIoTargetSealLinearity,
        };
        self.io_target = PreparedLifecycleIngressIoTarget::Unsupported;
        Ok(target)
    }
    /// Restore a one-shot target after the service rejected the atomic capture
    /// before acquiring capacity. The seal must still name this exact selector;
    /// no scalar reconstruction path exists.
    pub(in crate::sumeragi) fn restore_lifecycle_io_target(
        &mut self,
        target: LifecycleIngressIoTargetSeal,
    ) -> Result<(), LifecycleIngressIoTargetSeal> {
        if !matches!(
            self.io_target,
            PreparedLifecycleIngressIoTarget::Unsupported
        ) || target.context != self.context
            || target.ingress_identity != *self.selected_identity()
        {
            return Err(target);
        }
        self.io_target = match target.kind {
            LifecycleIngressIoTargetKind::CertifiedServe => {
                let Some(request) = target.certified_serve_request else {
                    return Err(target);
                };
                if target.certified_fetch_work_id.is_some() {
                    return Err(target);
                }
                if target.recovered_decision_fetch_key.is_some() {
                    return Err(target);
                }
                PreparedLifecycleIngressIoTarget::CertifiedServe { request }
            }
            LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence => {
                if target.certified_serve_request.is_some()
                    || target.certified_fetch_work_id.is_none()
                    || target.recovered_decision_fetch_key.is_some()
                {
                    return Err(target);
                }
                PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence
            }
            LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence => {
                if target.certified_serve_request.is_some()
                    || target.certified_fetch_work_id.is_some()
                    || target.recovered_decision_fetch_key.is_none()
                {
                    return Err(target);
                }
                PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence
            }
        };
        Ok(())
    }
    /// Prove that this selected response names the exact waiting Fetch row and
    /// its installed concrete registry incumbent without changing either.
    pub(super) fn attest_scheduler_fetch_carrier(
        &self,
        coordinator: &LifecycleCoordinator,
        registry: &mut LifecycleWorkRegistryHolder,
    ) -> Result<LifecycleIngressSchedulerFetchSeal, LifecycleIngressSchedulerCarrierError> {
        if !matches!(
            self.io_target,
            PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence
        ) {
            return Err(LifecycleIngressSchedulerCarrierError::UnsupportedCarrier);
        }
        let authority = self
            .selected_certified_fetch_ready_authority()
            .map_err(|_| LifecycleIngressSchedulerCarrierError::InvalidWaitingFetch)?;
        let location = coordinator
            .certified_fetch_current_location(authority)
            .map_err(|_| LifecycleIngressSchedulerCarrierError::InvalidWaitingFetch)?;
        let wait = match coordinator
            .records
            .get(&location.ordinal())
            .map(|record| record.state)
        {
            Some(LifecycleState::Waiting(wait)) => wait,
            Some(
                LifecycleState::Ready | LifecycleState::Claimed(_) | LifecycleState::Terminal(_),
            )
            | None => return Err(LifecycleIngressSchedulerCarrierError::InvalidWaitingFetch),
        };
        let next_generation = certified_fetch_scheduler_generation(wait)
            .ok_or(LifecycleIngressSchedulerCarrierError::InvalidWaitingFetch)?;
        let prepared = self
            .prepare_selected_certified_fetch_completion(registry.registry_mut(), location)
            .map_err(|error| match error {
                CertifiedFetchCompletionPreparationError::ReadyAuthority(_) => {
                    LifecycleIngressSchedulerCarrierError::InvalidWaitingFetch
                }
                CertifiedFetchCompletionPreparationError::Registry(_) => {
                    LifecycleIngressSchedulerCarrierError::InvalidRegistryIncumbent
                }
            })?;
        drop(prepared);
        Ok(LifecycleIngressSchedulerFetchSeal {
            owner: location.owner(),
            ordinal: location.ordinal(),
            key: authority.key,
            slot: location.slot(),
            incumbent_digest: location.incumbent_digest(),
            wake_generation: (wait.source(), next_generation),
            post_submit_wait: super::WaitToken::new(wait.source(), next_generation),
        })
    }
    /// Derive a sealed wake authority only when the queue-selected occurrence
    /// is the unique authenticated winner of its exact response family.
    fn selected_claimed_response_family(
        &self,
    ) -> Result<&PreparedClaimedResponseFamily, CertifiedFetchReadyPublicationError> {
        let selected_identity = self.selected_identity();
        let selected_ordinal = selected_identity.physical_admission_ordinal();
        if selected_ordinal == 0
            || selected_identity.context() != self.context
            || self.queue_witness.identity_for_ordinal(selected_ordinal) != Some(selected_identity)
            || !self.priority_owners.contains(&selected_ordinal)
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidSelectedOccurrence);
        }
        let selected_verdict = self
            .verdicts
            .get(&selected_ordinal)
            .ok_or(CertifiedFetchReadyPublicationError::InvalidSelectedOccurrence)?;
        if !selected_verdict.claimed_response_family {
            return Err(CertifiedFetchReadyPublicationError::SelectedOccurrenceNotClaimedResponse);
        }
        let mut selected_families = self
            .claimed_response_families
            .iter()
            .filter(|(_, prepared)| prepared.ingress_identity == *selected_identity);
        let Some((request_hash, prepared)) = selected_families.next() else {
            return Err(CertifiedFetchReadyPublicationError::SelectedOccurrenceNotClaimedResponse);
        };
        if selected_families.next().is_some()
            || prepared.request_hash() != *request_hash
            || prepared.ingress_identity.physical_admission_ordinal() != selected_ordinal
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidSelectedOccurrence);
        }
        Ok(prepared)
    }
    fn selected_certified_fetch_ready_authority(
        &self,
    ) -> Result<CertifiedFetchReadyAuthority, CertifiedFetchReadyPublicationError> {
        let selected_identity = self.selected_identity();
        let prepared = self.selected_claimed_response_family()?;
        let request_hash = prepared.request_hash();
        let candidate = prepared
            .candidate
            .ordinary()
            .ok_or(CertifiedFetchReadyPublicationError::InvalidCandidateBinding)?;
        let binding = candidate.pending_effect_binding();
        let statement = binding
            .candidate_statement()
            .ok_or(CertifiedFetchReadyPublicationError::InvalidCandidateBinding)?;
        if candidate.context_id() != statement.context_id()
            || candidate.height() != self.context.height()
        {
            return Err(CertifiedFetchReadyPublicationError::ForeignContext);
        }
        if candidate.round() != statement.proposal_round()
            || statement.subject() != Some(candidate.subject())
            || statement.phase().is_none()
            || statement.execution_commitment().is_none()
            || candidate.request_hash() != request_hash
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidCandidateBinding);
        }
        let key = certified_fetch_lifecycle_key(
            self.context,
            statement.round(),
            statement.proposal_round(),
            candidate.subject(),
            statement
                .phase()
                .expect("checked certified Fetch authority phase"),
            statement
                .execution_commitment()
                .expect("checked certified Fetch execution commitment"),
        )
        .ok_or(CertifiedFetchReadyPublicationError::ForeignContext)?;
        if key.phase() != LifecyclePhase::Fetch || key.context() != self.context.id() {
            return Err(CertifiedFetchReadyPublicationError::InvalidCandidateBinding);
        }
        Ok(CertifiedFetchReadyAuthority {
            ingress_identity: *selected_identity,
            request_hash,
            key,
            causal_root: pending_effect_causal_root(binding),
        })
    }
    fn persisted_family(
        &self,
        id: CertifiedFetchBodyPersistenceId,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<&PreparedClaimedResponseFamily, CertifiedFetchBodyPersistenceRetryFailure> {
        let ready = self
            .selected_certified_fetch_ready_authority()
            .map_err(CertifiedFetchBodyPersistenceRetryFailure::Selector)?;
        if ready.ingress_identity != id.ingress_identity
            || self.queue_witness.selected_disposition() != FairV2IngressDequeueDisposition::Admit
        {
            return Err(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity);
        }
        let family = self
            .claimed_response_families
            .get(&ready.request_hash)
            .ok_or(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity)?;
        let (response, responder) = family
            .authenticated_response()
            .ok_or(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity)?;
        if family.ingress_identity != id.ingress_identity
            || family
                .candidate
                .ordinary()
                .map(CertifiedResponsePriorityCandidate::work_id)
                != Some(id.work_id)
            || response != authenticated.response()
            || !family
                .candidate
                .matches_authenticated_response(response, responder)
        {
            return Err(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity);
        }
        Ok(family)
    }
    fn recovered_persisted_family(
        &self,
        completion: &RecoveredDecisionFetchBodyPersistenceCompletionV1,
    ) -> Result<&PreparedClaimedResponseFamily, RecoveredDecisionFetchExactDequeueErrorV1> {
        let id = completion.id;
        if *self.selected_identity() != id.ingress_identity
            || self.queue_witness.selected_disposition() != FairV2IngressDequeueDisposition::Admit
        {
            return Err(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity);
        }
        let family = self
            .claimed_response_families
            .values()
            .find(|family| family.ingress_identity == id.ingress_identity)
            .ok_or(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity)?;
        let Some(candidate) = family.candidate.recovered() else {
            return Err(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity);
        };
        let Some((response, responder)) = family.authenticated_response() else {
            return Err(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity);
        };
        if candidate.dispatch_key() != id.dispatch_key
            || response != completion.authenticated.response()
            || HashOf::new(response) != completion.response_hash()
            || !family
                .candidate
                .matches_authenticated_response(response, responder)
        {
            return Err(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity);
        }
        Ok(family)
    }
    /// Re-probe and pre-lock one recovered response occurrence before LedgerV1 fsync.
    pub(in crate::sumeragi) fn into_locked_recovered_decision_fetch_dequeue<'a>(
        self,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        ingress: &'a FairV2Ingress,
        completion: &RecoveredDecisionFetchBodyPersistenceCompletionV1,
    ) -> Result<
        PreparedRecoveredDecisionFetchExactDequeueV1<'a>,
        RecoveredDecisionFetchExactDequeueErrorV1,
    > {
        let revalidated = {
            let family = self.recovered_persisted_family(completion)?;
            let (response, responder) = family
                .authenticated_response()
                .ok_or(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity)?;
            executor
                .revalidate_recovered_decision_fetch_response_candidate(
                    family
                        .candidate
                        .recovered()
                        .ok_or(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity)?,
                    response,
                    responder,
                )
                .map_err(RecoveredDecisionFetchExactDequeueErrorV1::Executor)?
        };
        if !revalidated {
            return Err(RecoveredDecisionFetchExactDequeueErrorV1::CompletionIdentity);
        }
        let Self {
            context,
            request_fence_active: _,
            queue_witness,
            io_target: _,
            verdicts: _,
            priority_owners: _,
            claimed_response_families,
            selector_debt: _,
        } = self;
        drop(claimed_response_families);
        let locked = queue_witness
            .lock_exact_dequeue_retaining(
                ingress,
                context,
                completion.id.ingress_identity.physical_admission_ordinal(),
            )
            .map_err(|(error, _witness)| RecoveredDecisionFetchExactDequeueErrorV1::Queue(error))?;
        Ok(PreparedRecoveredDecisionFetchExactDequeueV1 { locked })
    }
    fn into_exact_certified_fetch_dequeue(
        self,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        id: CertifiedFetchBodyPersistenceId,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<PreparedCertifiedFetchExactDequeue, CertifiedFetchBodyPersistenceRetryFailure> {
        let revalidated = {
            let family = self.persisted_family(id, authenticated)?;
            let (response, responder) = family
                .authenticated_response()
                .ok_or(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity)?;
            executor
                .revalidate_certified_response_priority_candidate(
                    family
                        .candidate
                        .ordinary()
                        .ok_or(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity)?,
                    response,
                    responder,
                )
                .map_err(CertifiedFetchBodyPersistenceRetryFailure::Executor)?
        };
        if !revalidated {
            return Err(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity);
        }
        let Self {
            context,
            request_fence_active: _,
            queue_witness,
            io_target: _,
            verdicts: _,
            priority_owners: _,
            claimed_response_families,
            selector_debt: _,
        } = self;
        drop(claimed_response_families);
        Ok(PreparedCertifiedFetchExactDequeue {
            context,
            ingress_identity: id.ingress_identity,
            queue_witness,
        })
    }
    /// Mint the sole concrete-registry preflight capability from the selected
    /// authenticated family winner.
    fn selected_certified_fetch_completion_authority(
        &self,
    ) -> Result<CertifiedFetchCompletionAuthority<'_>, CertifiedFetchReadyPublicationError> {
        let ready = self.selected_certified_fetch_ready_authority()?;
        let prepared = self
            .claimed_response_families
            .get(&ready.request_hash)
            .ok_or(CertifiedFetchReadyPublicationError::SelectedOccurrenceNotClaimedResponse)?;
        if prepared.ingress_identity != ready.ingress_identity
            || prepared.ingress_identity != *self.selected_identity()
            || prepared.request_hash() != ready.request_hash
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidSelectedOccurrence);
        }
        let (authenticated_response, authenticated_responder) =
            prepared
                .authenticated_response()
                .ok_or(CertifiedFetchReadyPublicationError::InvalidCandidateBinding)?;
        if !prepared
            .candidate
            .matches_authenticated_response(authenticated_response, authenticated_responder)
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidCandidateBinding);
        }
        Ok(CertifiedFetchCompletionAuthority {
            ready,
            response_hash: prepared.candidate.response_hash(),
            authenticated_responder,
            authenticated_response,
            candidate_pending: prepared
                .candidate
                .ordinary()
                .ok_or(CertifiedFetchReadyPublicationError::InvalidCandidateBinding)?
                .pending_effect_binding(),
        })
    }
    /// Prepare the concrete same-slot conversion while every registry byte is
    /// still untouched and the selected response candidate is available for
    /// exact equality checks.
    ///
    /// This method intentionally neither consumes the selector nor exposes a
    /// queue commit. Phase A uses it before the locked capacity reservation is
    /// planned and submitted. Phase B recaptures a fresh selector/queue cut,
    /// repeats this preflight while the retained family `Arc` is available, and
    /// binds the result to the sealed durability receipt. Only the resulting
    /// receipt-bound registry token may consume the later exact checked-dequeue
    /// result.
    #[allow(dead_code)]
    fn prepare_selected_certified_fetch_completion<'a>(
        &self,
        registry: &'a mut ConcreteLifecycleWorkRegistry,
        location: CertifiedFetchWaitingLocation,
    ) -> Result<PreparedCertifiedFetchCompletion<'a>, CertifiedFetchCompletionPreparationError>
    {
        let authority = self
            .selected_certified_fetch_completion_authority()
            .map_err(CertifiedFetchCompletionPreparationError::ReadyAuthority)?;
        if location.owner().causal_root() != authority.causal_root() {
            return Err(CertifiedFetchCompletionPreparationError::ReadyAuthority(
                CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement,
            ));
        }
        registry
            .prepare_certified_fetch_completion(location, authority)
            .map_err(CertifiedFetchCompletionPreparationError::Registry)
    }
    /// Test-only projection proving the real prepared selector derives one
    /// complete sealed readiness authority without exposing its candidate.
    #[cfg(test)]
    pub(crate) fn certified_fetch_ready_authority_for_test(
        &self,
    ) -> Result<
        (
            LifecycleContext,
            u64,
            LifecycleDigest,
            HashOf<wire::CertifiedBodyRequest>,
            LifecycleKey,
            CausalRoot,
            WaitSource,
        ),
        CertifiedFetchReadyPublicationError,
    > {
        let authority = self.selected_certified_fetch_ready_authority()?;
        Ok((
            authority.ingress_identity.context(),
            authority.ingress_identity.physical_admission_ordinal(),
            authority.ingress_identity.digest(),
            authority.request_hash,
            authority.key,
            authority.causal_root,
            authority.wait_source(),
        ))
    }
    /// Prove that this real prepared selector crosses the sealed
    /// selector-to-registry preflight against an exact installed Fetch and that
    /// dropping the borrow-bound token leaves that incumbent byte-for-byte
    /// present.
    ///
    /// This helper installs only the missing process-local registry fixture. It
    /// consumes an effect and pending binding minted from the production-shaped
    /// body-fetch task; the signed response, QC, responder, and queue identity
    /// all remain those retained by `self`. It neither fabricates nor dequeues a
    /// carrier and exposes no capability constructor.
    #[cfg(test)]
    pub(crate) fn certified_fetch_registry_preflight_for_test(
        &self,
        incumbent_effect: crate::sumeragi::v2::AdapterEffect,
        incumbent_pending: PendingRuntimeEffectBinding,
    ) -> Result<(LifecycleDigest, LifecycleDigest), String> {
        let ready = self
            .selected_certified_fetch_ready_authority()
            .map_err(|error| format!("selected Fetch authority rejected: {error:?}"))?;
        let expected_effect = incumbent_effect.clone();
        let incumbent = ConcreteLifecycleWork::from_exact(incumbent_effect, incumbent_pending)
            .map_err(|(error, _, _)| format!("exact Fetch incumbent rejected: {error:?}"))?;
        if incumbent.causal_root() != ready.causal_root {
            return Err("real Fetch incumbent differs from selector causal authority".to_owned());
        }
        let incumbent_digest = incumbent.digest();
        let ordinal = 1;
        let owner = OwnerId::new(ready.causal_root, ordinal);
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .ok_or_else(|| "exact Fetch registry address was invalid".to_owned())?;
        let location = CertifiedFetchWaitingLocation::new(owner, ordinal, slot, incumbent_digest)
            .ok_or_else(|| "exact Fetch incumbent location was invalid".to_owned())?;
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, incumbent_digest, incumbent)
            .map_err(|(error, _)| format!("exact Fetch registry install rejected: {error:?}"))?;
        let prepared = self
            .prepare_selected_certified_fetch_completion(&mut registry, location)
            .map_err(|error| format!("sealed Fetch registry preflight rejected: {error:?}"))?;
        drop(prepared);
        if !registry.exactly_contains(address, &expected_effect) {
            return Err("dropping sealed Fetch preflight changed its incumbent".to_owned());
        }
        Ok((incumbent_digest, ready.ingress_identity.digest()))
    }
    /// Test-only concrete priority-owner projection for cross-module fixtures.
    #[cfg(test)]
    pub(crate) fn priority_owners_for_test(&self) -> &BTreeSet<u64> {
        &self.priority_owners
    }
    /// Test-only checked selector-debt cardinality.
    #[cfg(test)]
    pub(crate) const fn selector_debt_for_test(&self) -> u64 {
        self.selector_debt()
    }
    /// Test-only family-to-winning-physical-ordinal projection.
    #[cfg(test)]
    pub(crate) fn claimed_family_winners_for_test(
        &self,
    ) -> BTreeMap<HashOf<wire::CertifiedBodyRequest>, u64> {
        self.claimed_response_families
            .iter()
            .map(|(request_hash, prepared)| {
                (
                    *request_hash,
                    prepared.ingress_identity.physical_admission_ordinal(),
                )
            })
            .collect()
    }
    /// Test-only complete verdict-census size.
    #[cfg(test)]
    pub(crate) fn verdict_count_for_test(&self) -> usize {
        self.verdicts.len()
    }
    /// Test-only projection proving the selected target remains embedded in
    /// the complete opaque queue witness and shares its request-fence cut.
    #[cfg(test)]
    pub(crate) fn selected_cut_for_test(
        &self,
    ) -> (LifecycleContext, [u64; 2], u64, u128, bool, bool) {
        let selected_ordinal = self.selected_identity().physical_admission_ordinal();
        (
            self.context(),
            self.selected_positions().components(),
            selected_ordinal,
            self.queue_witness.physical_cut(),
            self.queue_witness.identity_for_ordinal(selected_ordinal)
                == Some(self.selected_identity()),
            self.request_fence_active,
        )
    }
}
fn certified_fetch_scheduler_generation(wait: super::WaitToken) -> Option<u64> {
    wait.observed_generation()
        .checked_add(1)
        .filter(|next| *next != u64::MAX)
}
impl LifecycleCoordinator {
    /// Complete one persisted certified-Fetch response across every exact owner.
    ///
    /// All selector, executor, registry, service, address, and durable-receipt
    /// checks finish before the LedgerV1 publication call. Once that call is
    /// invoked, every error is restart-only: the fresh queue witness and still
    /// indexed I/O completion remain sealed in the returned authority while
    /// the fail-stop output operation closes admission. A successful ledger
    /// cut is followed by the checked dequeue and an assertion-only registry,
    /// coordinator, executor, service, and work-index commit tail.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(crate) fn complete_certified_fetch_body_persistence(
        &mut self,
        registry: &mut LifecycleWorkRegistryHolder,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        services: &mut ProductionV2Services,
        ingress: &FairV2Ingress,
        persisted: PreparedCertifiedFetchBodyPersistenceCompletion,
    ) -> Result<(), CertifiedFetchBodyPersistenceCompletionError> {
        let (completion, work_ack) = persisted.into_parts();
        let CertifiedFetchBodyPersistenceCompletion {
            id,
            authenticated,
            receipt,
        } = completion;
        macro_rules! retry {
            ($failure:expr, $receipt:expr) => {
                return Err(CertifiedFetchBodyPersistenceCompletionError::Retry(
                    CertifiedFetchBodyPersistenceRetryError {
                        failure: $failure,
                        completion: PreparedCertifiedFetchBodyPersistenceCompletion::from_parts(
                            CertifiedFetchBodyPersistenceCompletion {
                                id,
                                authenticated,
                                receipt: $receipt,
                            },
                            work_ack,
                        ),
                    },
                ))
            };
        }
        let selector = match executor.prepare_lifecycle_ingress_selector(
            ingress,
            id.ingress_identity.physical_admission_ordinal(),
        ) {
            Ok(selector) => selector,
            Err(error) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::FreshSelector(error),
                receipt
            ),
        };
        let ready_authority = match selector.selected_certified_fetch_ready_authority() {
            Ok(authority) => authority,
            Err(error) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Selector(error),
                receipt
            ),
        };
        let location = match self.certified_fetch_current_location(ready_authority) {
            Ok(location) => location,
            Err(error) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Selector(error),
                receipt
            ),
        };
        let registry_prepared = match selector
            .prepare_selected_certified_fetch_completion(registry.registry_mut(), location)
        {
            Ok(prepared) => prepared,
            Err(CertifiedFetchCompletionPreparationError::ReadyAuthority(error)) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Selector(error),
                receipt
            ),
            Err(CertifiedFetchCompletionPreparationError::Registry(error)) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Registry(error),
                receipt
            ),
        };
        let durable_registry = match registry_prepared.bind_durable_body_receipt(receipt) {
            Ok(prepared) => prepared,
            Err((error, receipt)) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Registry(error),
                receipt
            ),
        };
        let ready = match self.prepare_certified_fetch_ready_projection(
            ready_authority,
            durable_registry.ready_projection(),
        ) {
            Ok(transition @ PreparedCertifiedFetchReadyTransition::Mutation(_)) => transition,
            Ok(PreparedCertifiedFetchReadyTransition::Stutter(_)) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::CoordinatorStutter,
                durable_registry.abort_before_dequeue()
            ),
            Err(error) => retry!(
                CertifiedFetchBodyPersistenceRetryFailure::Selector(error),
                durable_registry.abort_before_dequeue()
            ),
        };
        let family = match selector.persisted_family(id, &authenticated) {
            Ok(family) => family,
            Err(error) => {
                let receipt = durable_registry.abort_before_dequeue();
                retry!(error, receipt);
            }
        };
        let Some(candidate) = family.candidate.ordinary() else {
            let receipt = durable_registry.abort_before_dequeue();
            retry!(
                CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity,
                receipt
            );
        };
        let executor_prepared = match executor
            .prepare_lifecycle_certified_fetch_completion(candidate, &authenticated)
        {
            Ok(prepared) => prepared,
            Err(error) => {
                let receipt = durable_registry.abort_before_dequeue();
                retry!(
                    CertifiedFetchBodyPersistenceRetryFailure::Executor(error),
                    receipt
                );
            }
        };
        let output_guard = services.lifecycle_output_guard();
        let service_prepared =
            match services.prepare_certified_body_fetch_owner_removal(executor_prepared.task()) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let receipt = durable_registry.abort_before_dequeue();
                    retry!(
                        CertifiedFetchBodyPersistenceRetryFailure::Service(error),
                        receipt
                    );
                }
            };
        let selected_response_matches = {
            let family = match selector.persisted_family(id, &authenticated) {
                Ok(family) => family,
                Err(error) => {
                    let receipt = durable_registry.abort_before_dequeue();
                    retry!(error, receipt);
                }
            };
            durable_registry.matches_selected_response(
                family.ingress_identity,
                family.inbound.as_ref(),
                selector.queue_witness.selected_disposition(),
            )
        };
        if !selected_response_matches {
            let receipt = durable_registry.abort_before_dequeue();
            retry!(
                CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity,
                receipt
            );
        }
        let exact_dequeue =
            match selector.into_exact_certified_fetch_dequeue(executor, id, &authenticated) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let receipt = durable_registry.abort_before_dequeue();
                    retry!(error, receipt);
                }
            };
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            let receipt = durable_registry.abort_before_dequeue();
            retry!(
                CertifiedFetchBodyPersistenceRetryFailure::OutputClosed,
                receipt
            );
        };
        if let PreparedCertifiedFetchReadyTransition::Mutation(ready_mutation) = &ready
            && let Err(error) = ready_mutation.persist_exact_staged_successor()
        {
            let receipt = durable_registry.abort_before_dequeue();
            return Err(
                CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                    CertifiedFetchBodyPersistenceRestartError {
                        failure: CertifiedFetchBodyPersistenceRestartFailure::Ledger(
                            error.to_string(),
                        ),
                        completion: PreparedCertifiedFetchBodyPersistenceCompletion::from_parts(
                            CertifiedFetchBodyPersistenceCompletion {
                                id,
                                authenticated,
                                receipt,
                            },
                            work_ack,
                        ),
                        exact_dequeue,
                    },
                ),
            );
        }
        let dequeued = match exact_dequeue.commit(ingress) {
            Ok(dequeued) => dequeued,
            Err((error, exact_dequeue)) => {
                let receipt = durable_registry.abort_before_dequeue();
                return Err(
                    CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                        CertifiedFetchBodyPersistenceRestartError {
                            failure: CertifiedFetchBodyPersistenceRestartFailure::Queue(error),
                            completion: PreparedCertifiedFetchBodyPersistenceCompletion::from_parts(
                                CertifiedFetchBodyPersistenceCompletion {
                                    id,
                                    authenticated,
                                    receipt,
                                },
                                work_ack,
                            ),
                            exact_dequeue,
                        },
                    ),
                );
            }
        };
        durable_registry.commit_after_exact_dequeue(dequeued);
        match ready {
            PreparedCertifiedFetchReadyTransition::Mutation(ready) => ready.commit(),
            PreparedCertifiedFetchReadyTransition::Stutter(_) => {}
        }
        executor.commit_lifecycle_certified_fetch_completion(executor_prepared, &authenticated);
        let _disposition = service_prepared.commit(operation.permit());
        work_ack.commit();
        operation.complete();
        Ok(())
    }
    /// Exercise the pure logical Ready reducer in coordinator unit tests.
    #[cfg(test)]
    fn publish_certified_fetch_ready_authority(
        &mut self,
        authority: CertifiedFetchReadyAuthority,
    ) -> Result<CertifiedFetchReadyPublication, CertifiedFetchReadyPublicationError> {
        let projection = super::replay_authority::durable_certified_fetch_projection_fixture(
            self.active_context,
            authority.causal_root,
            u8::try_from(authority.key.round().view()).expect("fixture Fetch view fits u8"),
        );
        match self.prepare_certified_fetch_ready_projection(authority, &projection)? {
            PreparedCertifiedFetchReadyTransition::Mutation(prepared) => {
                prepared.commit();
                Ok(CertifiedFetchReadyPublication::Published)
            }
            PreparedCertifiedFetchReadyTransition::Stutter(publication) => Ok(publication),
        }
    }
    /// Resolve one exact Fetch address without deriving a replacement digest.
    fn certified_fetch_current_location(
        &self,
        authority: CertifiedFetchReadyAuthority,
    ) -> Result<CertifiedFetchWaitingLocation, CertifiedFetchReadyPublicationError> {
        if self.fault.is_some() {
            return Err(CertifiedFetchReadyPublicationError::CoordinatorFaulted);
        }
        if authority.ingress_identity.context() != self.active_context
            || authority.ingress_identity.physical_admission_ordinal() == 0
            || authority.key.context() != self.active_context.id()
            || authority.key.round().height() != self.active_context.height()
            || authority.key.phase() != LifecyclePhase::Fetch
        {
            return Err(CertifiedFetchReadyPublicationError::ForeignContext);
        }
        let ordinal = self
            .key_index
            .get(&authority.key)
            .copied()
            .ok_or(CertifiedFetchReadyPublicationError::MissingLifecycleKey)?;
        let record = self
            .records
            .get(&ordinal)
            .ok_or(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)?;
        if record.ordinal != ordinal
            || record.key != authority.key
            || record.owner.causal_root() != authority.causal_root
            || self.owner_index.get(&authority.causal_root) != Some(&record.owner)
            || record.work_class != LifecycleWorkClass::Fetch
            || self.ready_index.contains(&ordinal) != matches!(record.state, LifecycleState::Ready)
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        }
        match record.state {
            LifecycleState::Waiting(wait) => {
                if wait.source() != authority.wait_source()
                    || self.observed_generation.get(&wait.source())
                        != Some(&wait.observed_generation())
                    || self.records.iter().any(|(candidate_ordinal, candidate)| {
                        *candidate_ordinal != ordinal
                            && matches!(
                                candidate.state,
                                LifecycleState::Waiting(candidate_wait)
                                    if candidate_wait.source() == wait.source()
                            )
                    })
                {
                    return Err(CertifiedFetchReadyPublicationError::WrongWaitGeneration);
                }
            }
            LifecycleState::Ready => {}
            LifecycleState::Claimed(_) => {
                return Err(CertifiedFetchReadyPublicationError::ClaimedRecord);
            }
            LifecycleState::Terminal(_) => {
                return Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement);
            }
        }
        let (slot, incumbent_digest) = self
            .exact_fetch_physical_slot(record)
            .ok_or(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)?;
        CertifiedFetchWaitingLocation::new(record.owner, ordinal, slot, incumbent_digest)
            .ok_or(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
    }
    fn prepare_certified_fetch_ready_projection(
        &mut self,
        authority: CertifiedFetchReadyAuthority,
        projection: &super::replay_authority::DurableCertifiedFetchReplayProjectionV1,
    ) -> Result<PreparedCertifiedFetchReadyTransition<'_>, CertifiedFetchReadyPublicationError>
    {
        if self.fault.is_some() {
            return Err(CertifiedFetchReadyPublicationError::CoordinatorFaulted);
        }
        if authority.ingress_identity.context() != self.active_context
            || authority.ingress_identity.physical_admission_ordinal() == 0
            || authority.key.context() != self.active_context.id()
            || authority.key.round().height() != self.active_context.height()
            || authority
                .key
                .proposal_round()
                .is_none_or(|round| round.height() != self.active_context.height())
            || authority.key.phase() != LifecyclePhase::Fetch
        {
            return Err(CertifiedFetchReadyPublicationError::ForeignContext);
        }
        let ordinal = self
            .key_index
            .get(&authority.key)
            .copied()
            .ok_or(CertifiedFetchReadyPublicationError::MissingLifecycleKey)?;
        let Some(record) = self.records.get(&ordinal) else {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        };
        if record.ordinal != ordinal
            || record.key != authority.key
            || self
                .records
                .values()
                .filter(|candidate| candidate.key == authority.key)
                .count()
                != 1
            || self
                .records
                .values()
                .filter(|candidate| candidate.ordinal == ordinal)
                .count()
                != 1
            || self
                .key_index
                .values()
                .filter(|candidate_ordinal| **candidate_ordinal == ordinal)
                .count()
                != 1
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        }
        if record.owner.causal_root() != authority.causal_root {
            return Err(CertifiedFetchReadyPublicationError::ForeignCausalRoot);
        }
        if self.owner_index.get(&authority.causal_root) != Some(&record.owner)
            || self
                .owner_index
                .values()
                .filter(|candidate_owner| **candidate_owner == record.owner)
                .count()
                != 1
        {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        }
        if self.records.values().any(|candidate| {
            candidate.owner.causal_root() == authority.causal_root
                && candidate.owner != record.owner
        }) {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        }
        if record.work_class != LifecycleWorkClass::Fetch
            || record.key.phase() != LifecyclePhase::Fetch
        {
            return Err(CertifiedFetchReadyPublicationError::WrongWorkClass);
        }
        let target_is_indexed_ready = self.ready_index.contains(&ordinal);
        if target_is_indexed_ready != matches!(record.state, LifecycleState::Ready) {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        }
        let owner = record.owner;
        let wait_source = authority.wait_source();
        let wait_token = match record.state {
            LifecycleState::Ready => {
                let Some((_, installed_digest)) = self.exact_fetch_physical_slot(record) else {
                    return Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement);
                };
                if installed_digest != projection.completion_digest()
                    || !self.durable_records.get(&ordinal).is_some_and(|metadata| {
                        projection.exactly_matches_durable_record(
                            self.active_context,
                            record.key,
                            record.owner.causal_root(),
                            metadata.payload,
                            metadata.reconstruction_source,
                            &metadata.replay_authority,
                        )
                    })
                {
                    return Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement);
                }
                return Ok(PreparedCertifiedFetchReadyTransition::Stutter(
                    CertifiedFetchReadyPublication::StutterReady,
                ));
            }
            LifecycleState::Terminal(outcome) => {
                if !self.exact_terminal_tombstone(ordinal, record, outcome) {
                    return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
                }
                return Ok(PreparedCertifiedFetchReadyTransition::Stutter(
                    CertifiedFetchReadyPublication::StutterTerminal,
                ));
            }
            LifecycleState::Claimed(_) => {
                return Err(CertifiedFetchReadyPublicationError::ClaimedRecord);
            }
            LifecycleState::Waiting(wait) if wait.source() != wait_source => {
                return Err(CertifiedFetchReadyPublicationError::WrongWaitSource);
            }
            LifecycleState::Waiting(wait) => wait,
        };
        if self.observed_generation.get(&wait_token.source())
            != Some(&wait_token.observed_generation())
        {
            return Err(CertifiedFetchReadyPublicationError::WrongWaitGeneration);
        }
        if self.records.iter().any(|(candidate_ordinal, candidate)| {
            *candidate_ordinal != ordinal
                && matches!(
                    candidate.state,
                    LifecycleState::Waiting(wait) if wait.source() == wait_token.source()
                )
        }) {
            return Err(CertifiedFetchReadyPublicationError::AmbiguousWaitSource);
        }
        let next_generation = wait_token
            .observed_generation()
            .checked_add(1)
            .ok_or(CertifiedFetchReadyPublicationError::WrongWaitGeneration)?;
        let Some((slot, incumbent_digest)) = self.exact_fetch_physical_slot(record) else {
            return Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement);
        };
        let replacement_digest = projection.completion_digest();
        if replacement_digest == incumbent_digest {
            return Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement);
        }
        let mut next = self.stage_durable_transaction();
        next.publish_ready(ReadyEvent::new(
            ordinal,
            owner,
            wait_token,
            Some(super::PhysicalReplacement::new(
                slot,
                super::PhysicalSlot::new(slot, replacement_digest),
            )),
        ));
        if next.fault.is_some() {
            return Err(CertifiedFetchReadyPublicationError::CoordinatorFaulted);
        }
        let Some(metadata) = next.durable_records.get_mut(&ordinal) else {
            return Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex);
        };
        if !projection.rebind_waiting_fetch_metadata(self.active_context, record.key, metadata) {
            return Err(CertifiedFetchReadyPublicationError::InvalidCandidateBinding);
        }
        debug_assert_eq!(
            next.records.get(&ordinal).map(|record| record.state),
            Some(LifecycleState::Ready)
        );
        debug_assert_eq!(
            next.observed_generation.get(&wait_token.source()),
            Some(&next_generation)
        );
        debug_assert_eq!(
            next.records
                .get(&ordinal)
                .and_then(|record| record.physical_slots.get(&slot)),
            Some(&replacement_digest)
        );
        let location = CertifiedFetchWaitingLocation::new(owner, ordinal, slot, incumbent_digest)
            .ok_or(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)?;
        Ok(PreparedCertifiedFetchReadyTransition::Mutation(
            PreparedCertifiedFetchReadyMutation {
                target: self,
                next,
                location,
            },
        ))
    }
    fn exact_terminal_tombstone(
        &self,
        ordinal: u128,
        record: &super::LifecycleRecord,
        outcome: super::TerminalOutcome,
    ) -> bool {
        self.active_lease
            .as_ref()
            .is_none_or(|lease| lease.ordinal != ordinal)
            && !self.ready_index.contains(&ordinal)
            && self.exact_fetch_physical_slot(record).is_some()
            && self.durable_records.get(&ordinal).is_some_and(|metadata| {
                metadata
                    .payload
                    .matches_terminal(record.work_class, Some(outcome))
            })
            && self
                .capacity_used
                .get(&record.work_class.capacity_class())
                .copied()
                == Some(
                    self.records
                        .values()
                        .filter(|candidate| {
                            candidate.work_class.capacity_class()
                                == record.work_class.capacity_class()
                                && !matches!(candidate.state, LifecycleState::Terminal(_))
                        })
                        .count(),
                )
    }
    fn exact_fetch_physical_slot(
        &self,
        record: &super::LifecycleRecord,
    ) -> Option<(super::PhysicalSlotId, LifecycleDigest)> {
        if record.work_class != LifecycleWorkClass::Fetch
            || record.physical_slots.len() != 1
            || self.episode_authority.universe_for(record.key).as_ref()
                != Some(&record.episode.universe)
            || !self.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            )
            || !record
                .episode
                .consumed_slots
                .is_subset(&record.episode.slot_universe)
            || !record
                .physical_slots
                .keys()
                .all(|slot| record.episode.slot_universe.contains(slot))
        {
            return None;
        }
        let (&slot, &digest) = record.physical_slots.first_key_value()?;
        record
            .episode
            .consumed_slots
            .contains(&slot)
            .then_some((slot, digest))
    }
}
impl<R: crate::sumeragi::v2_effects::EffectRuntime> V2EffectExecutor<R> {
    /// Consume one exact recovered response family into its dedicated body-store command.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(
        &self,
        mut prepared: PreparedLifecycleIngressSelector,
    ) -> Result<
        RecoveredDecisionFetchBodyPersistenceTaskV1,
        RecoveredDecisionFetchBodyPersistencePreparationErrorV1,
    > {
        let selected_identity = *prepared.selected_identity();
        let (request_hash, revalidated) = {
            let family = match prepared.selected_claimed_response_family() {
                Ok(family) => family,
                Err(failure) => {
                    return Err(RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
                        _failure:
                            RecoveredDecisionFetchBodyPersistencePreparationFailureV1::Selector(
                                failure,
                            ),
                        prepared,
                    });
                }
            };
            let Some(candidate) = family.candidate.recovered() else {
                return Err(RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
                    _failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            };
            let Some((response, responder)) = family.authenticated_response() else {
                return Err(RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
                    _failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            };
            (
                family.request_hash(),
                self.revalidate_recovered_decision_fetch_response_candidate(
                    candidate, response, responder,
                ),
            )
        };
        match revalidated {
            Ok(true) => {}
            Ok(false) => {
                return Err(RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
                    _failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            }
            Err(failure) => {
                return Err(RecoveredDecisionFetchBodyPersistencePreparationErrorV1 {
                    _failure: RecoveredDecisionFetchBodyPersistencePreparationFailureV1::Executor(
                        failure,
                    ),
                    prepared,
                });
            }
        }
        let family = prepared
            .claimed_response_families
            .remove(&request_hash)
            .expect("revalidated recovered response family remains present");
        let PreparedClaimedResponseFamily {
            ingress_identity,
            inbound,
            candidate,
        } = family;
        let PreparedCertifiedResponseCandidate::Recovered(candidate) = candidate else {
            unreachable!("revalidated recovered candidate preserves its family");
        };
        assert_eq!(ingress_identity, selected_identity);
        let dispatch_key = candidate.dispatch_key();
        let claim_preflight = candidate.claim_preflight();
        let authenticated = candidate.into_authenticated_response();
        drop(inbound);
        let PreparedLifecycleIngressSelector {
            context: _,
            request_fence_active: _,
            queue_witness,
            io_target: _,
            verdicts: _,
            priority_owners: _,
            claimed_response_families,
            selector_debt: _,
        } = prepared;
        drop(queue_witness);
        drop(claimed_response_families);
        Ok(RecoveredDecisionFetchBodyPersistenceTaskV1 {
            id: RecoveredDecisionFetchBodyPersistenceIdV1 {
                ingress_identity,
                dispatch_key,
            },
            claim_preflight,
            authenticated,
        })
    }
    /// Consume one exact selected family into a bounded body-store command.
    ///
    /// The final equality re-probe runs while the immutable family carrier and
    /// queue witness are still retained. Success then moves the candidate's
    /// unique authenticated-response token into the command and drops the
    /// entire stale selector, including every inbound `Arc` and queue witness.
    /// Failure returns the byte-for-byte preparation and mutates no executor,
    /// queue, tracker, registry, coordinator, or service state.
    pub(in crate::sumeragi) fn prepare_certified_fetch_body_persistence(
        &self,
        mut prepared: PreparedLifecycleIngressSelector,
    ) -> Result<CertifiedFetchBodyPersistenceTask, CertifiedFetchBodyPersistencePreparationError>
    {
        let ready = match prepared.selected_certified_fetch_ready_authority() {
            Ok(ready) => ready,
            Err(failure) => {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(failure),
                    prepared,
                });
            }
        };
        let revalidated = {
            let Some(family) = prepared.claimed_response_families.get(&ready.request_hash) else {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(
                        CertifiedFetchReadyPublicationError::SelectedOccurrenceNotClaimedResponse,
                    ),
                    prepared,
                });
            };
            let Some((response, responder)) = family.authenticated_response() else {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            };
            let Some(candidate) = family.candidate.ordinary() else {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            };
            self.revalidate_certified_response_priority_candidate(candidate, response, responder)
        };
        match revalidated {
            Ok(true) => {}
            Ok(false) => {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(
                        CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                    ),
                    prepared,
                });
            }
            Err(failure) => {
                return Err(CertifiedFetchBodyPersistencePreparationError {
                    failure: CertifiedFetchBodyPersistencePreparationFailure::Executor(failure),
                    prepared,
                });
            }
        }
        let family = prepared
            .claimed_response_families
            .remove(&ready.request_hash)
            .expect("revalidated selected response family remains present");
        assert_eq!(family.ingress_identity, ready.ingress_identity);
        let PreparedClaimedResponseFamily {
            ingress_identity,
            inbound,
            candidate,
        } = family;
        let PreparedCertifiedResponseCandidate::Ordinary(candidate) = candidate else {
            return Err(CertifiedFetchBodyPersistencePreparationError {
                failure: CertifiedFetchBodyPersistencePreparationFailure::Selector(
                    CertifiedFetchReadyPublicationError::InvalidCandidateBinding,
                ),
                prepared,
            });
        };
        let work_id = candidate.work_id();
        let authenticated = candidate.into_authenticated_response();
        drop(inbound);
        let PreparedLifecycleIngressSelector {
            context: _,
            request_fence_active: _,
            queue_witness,
            io_target: _,
            verdicts: _,
            priority_owners: _,
            claimed_response_families,
            selector_debt: _,
        } = prepared;
        drop(queue_witness);
        drop(claimed_response_families);
        Ok(CertifiedFetchBodyPersistenceTask {
            id: CertifiedFetchBodyPersistenceId {
                ingress_identity,
                work_id,
            },
            authenticated,
        })
    }
    /// Prepare one opaque complete selector census from an exact queue target.
    ///
    /// This is the sole crate-visible mint. It returns borrow-free census state,
    /// never rank authority; Phase A or Phase B must consume and revalidate it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn prepare_lifecycle_ingress_selector(
        &self,
        ingress: &FairV2Ingress,
        target_physical_ordinal: u64,
    ) -> Result<PreparedLifecycleIngressSelector, LifecycleIngressSelectorError> {
        let cut = ingress
            .capture_lifecycle_queue_cut(target_physical_ordinal)
            .map_err(|_| LifecycleIngressSelectorError::QueueCutCapture)?;
        self.capture_lifecycle_ingress_selector(cut)
    }
    /// Select the next fair authenticated recovered Decision-Fetch response.
    ///
    /// The queue runs the same strict-then-dependency source/lane selection as
    /// ordinary checked dequeue under its service lock. The executor supplies
    /// the ordinary head-drain predicate and then authenticates the complete
    /// frozen census. An ordinary, obsolete, or foreign-context winner returns
    /// `None` for pass-through; no later recovered response may leapfrog it.
    /// Neither selection nor classification claims, dequeues, or publishes
    /// worker capacity.
    // TODO: Consume this queue-owned selector only from the unified lifecycle
    // Ingress-turn driver when the atomic runner cutover replaces raw dequeue.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn prepare_next_recovered_decision_fetch_ingress_selector(
        &self,
        ingress: &FairV2Ingress,
    ) -> Result<Option<PreparedLifecycleIngressSelector>, LifecycleIngressSelectorError> {
        let terminal_subject = self
            .lifecycle_terminal_subject()
            .map_err(|error| LifecycleIngressSelectorError::ExecutorState(Box::new(error)))?;
        let Some(cut) = ingress
            .capture_next_lifecycle_queue_cut(|occurrence| {
                v2_ingress_head_can_drain(occurrence.inbound(), self, terminal_subject)
            })
            .map_err(|_| LifecycleIngressSelectorError::QueueCutCapture)?
        else {
            return Ok(None);
        };
        let prepared = self.capture_lifecycle_ingress_selector(cut)?;
        if prepared.queue_witness.selected_disposition() != FairV2IngressDequeueDisposition::Admit
            || !matches!(
                prepared.io_target,
                PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence
            )
        {
            return Ok(None);
        }
        if prepared
            .selected_claimed_response_family()
            .ok()
            .and_then(|family| family.candidate.recovered())
            .is_none()
        {
            return Err(LifecycleIngressSelectorError::CandidateRevalidationDrift {
                ordinal: prepared.selected_identity().physical_admission_ordinal(),
            });
        }
        Ok(Some(prepared))
    }
    /// Decide whether an already selected exact cut is the recovered Fetch owner.
    ///
    /// Only the selected response's exact signed-request family is
    /// authenticated. In particular, the lowest physical occurrence of that
    /// family wins even when fair source rotation selected a later
    /// byte-identical duplicate; an unrelated later malformed family cannot
    /// poison an ordinary selected head. This method is read-only and leaves
    /// the cut's queue service guard with the caller.
    pub(super) fn selected_cut_is_recovered_decision_fetch(
        &self,
        cut: &FairIngressQueueCut<'_>,
    ) -> Result<bool, LifecycleIngressSelectorError> {
        self.validate_lifecycle_ingress_selector_authority()
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: None,
                error: Box::new(error),
            })?;
        let context = lifecycle_context_from_wire(self.context());
        if cut.selected_identity().context() != context {
            return Err(LifecycleIngressSelectorError::ForeignContext);
        }
        let selected_ordinal = cut.selected_identity().physical_admission_ordinal();
        let selected_request_hash = cut
            .selector_occurrences()
            .find(|occurrence| occurrence.physical_admission_ordinal() == selected_ordinal)
            .and_then(|occurrence| match occurrence.inbound().message() {
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ..
                }) => Some(response.request_hash),
                _ => None,
            });
        let Some(selected_request_hash) = selected_request_hash else {
            return Ok(false);
        };
        let mut response_candidates = BTreeMap::new();
        for occurrence in cut.selector_occurrences() {
            if occurrence.queue_gate() == FairV2IngressQueueGateVerdict::Blocked {
                continue;
            }
            let inbound = occurrence.inbound();
            let BlockMessage::V2(message) = inbound.message() else {
                continue;
            };
            let drainable = occurrence.is_obsolete()
                || message.validate_version().is_err()
                || inbound.ingress_ownership().is_some_and(|ownership| {
                    self.can_admit_network_message_with_ingress_ownership(message, ownership)
                });
            if !drainable || message.validate_version().is_err() {
                continue;
            }
            let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
            else {
                continue;
            };
            if response.request_hash != selected_request_hash {
                continue;
            }
            let Some(responder) = inbound.sender() else {
                continue;
            };
            let candidate = match self.probe_certified_response_priority(response, responder) {
                Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(_)) => continue,
                Ok(CertifiedResponsePriorityProbe::PreflightRequired(candidate)) => {
                    PreparedCertifiedResponseCandidate::Ordinary(candidate)
                }
                Ok(CertifiedResponsePriorityProbe::RecoveredPreflightRequired(candidate)) => {
                    PreparedCertifiedResponseCandidate::Recovered(candidate)
                }
                Err(error) if response_error_is_remote_nonpriority(&error) => continue,
                Err(error) => {
                    return Err(LifecycleIngressSelectorError::ExecutorAuthority {
                        ordinal: Some(occurrence.physical_admission_ordinal()),
                        error: Box::new(error),
                    });
                }
            };
            if response_candidates
                .insert(occurrence.physical_admission_ordinal(), candidate)
                .is_some()
            {
                return Err(LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                    ordinal: occurrence.physical_admission_ordinal(),
                });
            }
        }

        let family_winners = lowest_physical_ordinal_per_family(
            response_candidates
                .iter()
                .map(|(ordinal, candidate)| (candidate.request_hash(), *ordinal)),
        )?;
        let selected_family = family_winners
            .values()
            .any(|ordinal| *ordinal == selected_ordinal);
        let mut selected_recovered = false;
        for ordinal in family_winners.into_values() {
            let candidate = response_candidates
                .remove(&ordinal)
                .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal })?;
            let occurrence = cut
                .selector_occurrences()
                .find(|occurrence| occurrence.physical_admission_ordinal() == ordinal)
                .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal })?;
            let inbound = occurrence.inbound();
            let BlockMessage::V2(message) = inbound.message() else {
                return Err(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal });
            };
            let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
            else {
                return Err(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal });
            };
            let responder = inbound
                .sender()
                .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal })?;
            let exact = match &candidate {
                PreparedCertifiedResponseCandidate::Ordinary(candidate) => self
                    .revalidate_certified_response_priority_candidate(
                        candidate, response, responder,
                    ),
                PreparedCertifiedResponseCandidate::Recovered(candidate) => self
                    .revalidate_recovered_decision_fetch_response_candidate(
                        candidate, response, responder,
                    ),
            }
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: Some(ordinal),
                error: Box::new(error),
            })?;
            if !exact {
                return Err(LifecycleIngressSelectorError::CandidateRevalidationDrift { ordinal });
            }
            if ordinal == selected_ordinal {
                selected_recovered =
                    matches!(candidate, PreparedCertifiedResponseCandidate::Recovered(_));
            }
        }
        self.validate_lifecycle_ingress_selector_authority()
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: None,
                error: Box::new(error),
            })?;
        if !cut.pre_cut_is_intact() {
            return Err(LifecycleIngressSelectorError::QueueCutChanged);
        }
        Ok(selected_family && selected_recovered)
    }

    /// Convert the selected response family's cut into Phase-A authority.
    pub(super) fn prepare_recovered_decision_fetch_from_selected_cut(
        &self,
        cut: FairIngressQueueCut<'_>,
    ) -> Result<PreparedLifecycleIngressSelector, LifecycleIngressSelectorError> {
        let selected_ordinal = cut.selected_identity().physical_admission_ordinal();
        let selected_request_hash = cut
            .selector_occurrences()
            .find(|occurrence| occurrence.physical_admission_ordinal() == selected_ordinal)
            .and_then(|occurrence| match occurrence.inbound().message() {
                BlockMessage::V2(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                    ..
                }) => Some(response.request_hash),
                _ => None,
            })
            .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                ordinal: selected_ordinal,
            })?;
        let prepared = self.capture_lifecycle_ingress_selector_for_response_family(
            cut,
            Some(selected_request_hash),
        )?;
        if prepared.queue_witness.selected_disposition() != FairV2IngressDequeueDisposition::Admit
            || !matches!(
                prepared.io_target,
                PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence
            )
            || prepared
                .selected_claimed_response_family()
                .ok()
                .and_then(|family| family.candidate.recovered())
                .is_none()
        {
            return Err(LifecycleIngressSelectorError::CandidateRevalidationDrift {
                ordinal: prepared.selected_identity().physical_admission_ordinal(),
            });
        }
        Ok(prepared)
    }
    /// Classify every exact pre-cut fair-ingress occurrence without mutation.
    ///
    /// The queue's service guard remains held while queue state is released for
    /// authentication and body reconstruction. Producer appends may proceed;
    /// the final read-only queue recheck rejects any pre-cut mutation. Returning
    /// the borrow-free preparation releases that guard without removing a row.
    /// Classification never claims a response or publishes selector debt as
    /// scheduler authority; only the consuming persistence/completion surfaces
    /// may transfer ownership.
    pub(super) fn capture_lifecycle_ingress_selector(
        &self,
        cut: FairIngressQueueCut<'_>,
    ) -> Result<PreparedLifecycleIngressSelector, LifecycleIngressSelectorError> {
        self.capture_lifecycle_ingress_selector_for_response_family(cut, None)
    }

    fn capture_lifecycle_ingress_selector_for_response_family(
        &self,
        cut: FairIngressQueueCut<'_>,
        selected_response_family: Option<HashOf<wire::CertifiedBodyRequest>>,
    ) -> Result<PreparedLifecycleIngressSelector, LifecycleIngressSelectorError> {
        self.validate_lifecycle_ingress_selector_authority()
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: None,
                error: Box::new(error),
            })?;
        let context = lifecycle_context_from_wire(self.context());
        if cut.selected_identity().context() != context {
            return Err(LifecycleIngressSelectorError::ForeignContext);
        }
        let request_fence_active =
            self.validated_certified_request_presence()
                .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                    ordinal: None,
                    error: Box::new(error),
                })?;
        let mut occurrence_identities = BTreeMap::new();
        for occurrence in cut.selector_occurrences() {
            let ordinal = occurrence.physical_admission_ordinal();
            let identity = cut
                .identity_for_ordinal(ordinal)
                .copied()
                .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal })?;
            if identity.context() != context
                || identity.physical_admission_ordinal() != ordinal
                || occurrence_identities.insert(ordinal, identity).is_some()
            {
                return Err(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal });
            }
        }
        let expected_ordinals = occurrence_identities
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let selected_ordinal = cut.selected_identity().physical_admission_ordinal();
        let mut io_target = PreparedLifecycleIngressIoTarget::Unsupported;
        let mut verdicts = BTreeMap::new();
        let mut response_candidates = BTreeMap::new();
        for occurrence in cut.selector_occurrences() {
            let ordinal = occurrence.physical_admission_ordinal();
            let mut verdict = LifecycleIngressOccurrenceVerdict::NOT_PRIORITY;
            if occurrence.queue_gate() != FairV2IngressQueueGateVerdict::Blocked {
                let inbound = occurrence.inbound();
                if let BlockMessage::V2(message) = inbound.message() {
                    let physical_completion = matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::PayloadChunk(_)
                            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    );
                    let certified_response = matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    );
                    let selected_response_family_matches =
                        selected_response_family.is_none_or(|selected_request_hash| {
                            matches!(
                                &message.payload,
                                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                                    if response.request_hash == selected_request_hash
                            )
                        });
                    let drainable = selected_response_family_matches
                        && (occurrence.is_obsolete()
                            || message.validate_version().is_err()
                            || self.can_admit_network_message_with_ingress_ownership(
                                message,
                                inbound.ingress_ownership().ok_or(
                                    LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                                        ordinal,
                                    },
                                )?,
                            ));
                    if ordinal == selected_ordinal && drainable {
                        io_target = match &message.payload {
                            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                                let request_hash = HashOf::new(request);
                                let mut digest = [0_u8; 32];
                                digest.copy_from_slice(request_hash.as_ref());
                                PreparedLifecycleIngressIoTarget::CertifiedServe {
                                    request: LifecycleDigest::new(digest),
                                }
                            }
                            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
                                PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence
                            }
                            _ => PreparedLifecycleIngressIoTarget::Unsupported,
                        };
                    }
                    let request_fenced_completion = drainable
                        && selected_response_family_matches
                        && request_fence_active
                        && lifecycle_ingress_resource_is_untrusted(
                            occurrence.source_class(),
                            certified_response,
                        )
                        && occurrence.class() == FairV2IngressClass::TransportCompletion
                        && physical_completion;
                    if drainable
                        && selected_response_family_matches
                        && message.validate_version().is_ok()
                        && let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) =
                            &message.payload
                        && let Some(responder) = inbound.sender()
                    {
                        match self.probe_certified_response_priority(response, responder) {
                            Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(_)) => {
                                if request_fenced_completion {
                                    verdict = verdict.with_authority(
                                        LifecycleIngressPriorityAuthority::RequestFencedCompletion,
                                    );
                                }
                            }
                            Ok(CertifiedResponsePriorityProbe::PreflightRequired(candidate)) => {
                                if request_fenced_completion {
                                    verdict = verdict.with_authority(
                                        LifecycleIngressPriorityAuthority::RequestFencedCompletion,
                                    );
                                }
                                let ingress_identity = occurrence_identities[&ordinal];
                                if !candidate.matches_authenticated_response(response, responder)
                                    || response_candidates
                                        .insert(
                                            ordinal,
                                            PreparedClaimedResponseFamily {
                                                ingress_identity,
                                                inbound: occurrence.clone_inbound(),
                                                candidate:
                                                    PreparedCertifiedResponseCandidate::Ordinary(
                                                        candidate,
                                                    ),
                                            },
                                        )
                                        .is_some()
                                {
                                    return Err(
                                        LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                                            ordinal,
                                        },
                                    );
                                }
                            }
                            Ok(CertifiedResponsePriorityProbe::RecoveredPreflightRequired(
                                candidate,
                            )) => {
                                if request_fenced_completion {
                                    verdict = verdict.with_authority(
                                        LifecycleIngressPriorityAuthority::RequestFencedCompletion,
                                    );
                                }
                                let ingress_identity = occurrence_identities[&ordinal];
                                if !candidate.matches_authenticated_response(response, responder)
                                    || response_candidates
                                        .insert(
                                            ordinal,
                                            PreparedClaimedResponseFamily {
                                                ingress_identity,
                                                inbound: occurrence.clone_inbound(),
                                                candidate:
                                                    PreparedCertifiedResponseCandidate::Recovered(
                                                        candidate,
                                                    ),
                                            },
                                        )
                                        .is_some()
                                {
                                    return Err(
                                        LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                                            ordinal,
                                        },
                                    );
                                }
                            }
                            Err(error) if response_error_is_remote_nonpriority(&error) => {
                                if request_fenced_completion {
                                    verdict = verdict.with_authority(
                                        LifecycleIngressPriorityAuthority::RequestFencedCompletion,
                                    );
                                }
                            }
                            Err(error) => {
                                return Err(LifecycleIngressSelectorError::ExecutorAuthority {
                                    ordinal: Some(ordinal),
                                    error: Box::new(error),
                                });
                            }
                        }
                    } else if request_fenced_completion {
                        verdict = verdict.with_authority(
                            LifecycleIngressPriorityAuthority::RequestFencedCompletion,
                        );
                    }
                }
            }
            if verdicts.insert(ordinal, verdict).is_some() {
                return Err(LifecycleIngressSelectorError::InvalidCensus);
            }
        }
        let family_winners = lowest_physical_ordinal_per_family(
            response_candidates
                .iter()
                .map(|(ordinal, prepared)| (prepared.request_hash(), *ordinal)),
        )?;
        let mut claimed_response_families = BTreeMap::new();
        for (request_hash, ordinal) in family_winners {
            let prepared = response_candidates
                .remove(&ordinal)
                .ok_or(LifecycleIngressSelectorError::InvalidOccurrenceIdentity { ordinal })?;
            if prepared.request_hash() != request_hash
                || prepared.ingress_identity != occurrence_identities[&ordinal]
                || claimed_response_families
                    .insert(request_hash, prepared)
                    .is_some()
            {
                return Err(LifecycleIngressSelectorError::InvalidCensus);
            }
            let verdict = verdicts
                .get_mut(&ordinal)
                .ok_or(LifecycleIngressSelectorError::InvalidCensus)?;
            *verdict =
                verdict.with_authority(LifecycleIngressPriorityAuthority::ClaimedResponseFamily);
        }
        for prepared in claimed_response_families.values() {
            let (response, authenticated_responder) = prepared.authenticated_response().ok_or(
                LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                    ordinal: prepared.ingress_identity.physical_admission_ordinal(),
                },
            )?;
            let revalidated = match &prepared.candidate {
                PreparedCertifiedResponseCandidate::Ordinary(candidate) => self
                    .revalidate_certified_response_priority_candidate(
                        candidate,
                        response,
                        authenticated_responder,
                    ),
                PreparedCertifiedResponseCandidate::Recovered(candidate) => self
                    .revalidate_recovered_decision_fetch_response_candidate(
                        candidate,
                        response,
                        authenticated_responder,
                    ),
            }
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: Some(prepared.ingress_identity.physical_admission_ordinal()),
                error: Box::new(error),
            })?;
            if !revalidated {
                return Err(LifecycleIngressSelectorError::CandidateRevalidationDrift {
                    ordinal: prepared.ingress_identity.physical_admission_ordinal(),
                });
            }
        }
        if matches!(
            io_target,
            PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence
        ) && let Some(selected_family) = claimed_response_families
            .values()
            .find(|family| family.ingress_identity.physical_admission_ordinal() == selected_ordinal)
        {
            io_target = match &selected_family.candidate {
                PreparedCertifiedResponseCandidate::Ordinary(_) => {
                    PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence
                }
                PreparedCertifiedResponseCandidate::Recovered(_) => {
                    PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence
                }
            };
        }
        let (priority_owners, selector_debt) =
            validate_selector_census(&expected_ordinals, &verdicts)
                .map_err(|_| LifecycleIngressSelectorError::InvalidCensus)?;
        self.validate_lifecycle_ingress_selector_authority()
            .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                ordinal: None,
                error: Box::new(error),
            })?;
        let revalidated_presence =
            self.validated_certified_request_presence()
                .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                    ordinal: None,
                    error: Box::new(error),
                })?;
        if revalidated_presence != request_fence_active || !cut.pre_cut_is_intact() {
            return Err(LifecycleIngressSelectorError::QueueCutChanged);
        }
        let queue_witness = cut.into_prepared_witness();
        if !queue_witness.is_internally_exact() {
            return Err(LifecycleIngressSelectorError::InvalidCensus);
        }
        Ok(PreparedLifecycleIngressSelector {
            context,
            request_fence_active,
            queue_witness,
            io_target,
            verdicts,
            priority_owners,
            claimed_response_families,
            selector_debt,
        })
    }
}
/// Map receiver-local delivery ownership into the formal ingress resource lane.
///
/// Certified responses are route-neutral in the formal scheduler and always
/// occupy its aggregate untrusted source even though the production envelope
/// necessarily arrived through an authenticated hop. Other physical carriers
/// retain their receiver-local source class.
const fn lifecycle_ingress_resource_is_untrusted(
    source_class: FairV2IngressSourceClass,
    certified_response: bool,
) -> bool {
    certified_response || matches!(source_class, FairV2IngressSourceClass::Anonymous)
}
fn lifecycle_context_from_wire(context: &wire::HeightContext) -> LifecycleContext {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(context.id().0.as_ref());
    LifecycleContext::new(LifecycleDigest::new(digest), context.height)
}
fn response_error_is_remote_nonpriority(error: &EffectTransportError) -> bool {
    matches!(
        error,
        EffectTransportError::Authentication(
            V2TransportError::Wire(_)
                | V2TransportError::OuterIdentityMismatch { .. }
                | V2TransportError::InvalidSignature { .. }
                | V2TransportError::InvalidProposalBody(_)
        ) | EffectTransportError::BodyMismatch(_)
    )
}
fn lowest_physical_ordinal_per_family<K>(
    occurrences: impl IntoIterator<Item = (K, u64)>,
) -> Result<BTreeMap<K, u64>, LifecycleIngressSelectorError>
where
    K: Ord,
{
    let mut lowest: BTreeMap<K, u64> = BTreeMap::new();
    for (family, ordinal) in occurrences {
        if ordinal == 0 {
            return Err(LifecycleIngressSelectorError::InvalidCensus);
        }
        lowest
            .entry(family)
            .and_modify(|current| *current = (*current).min(ordinal))
            .or_insert(ordinal);
    }
    Ok(lowest)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectorCensusError {
    KeySetMismatch,
    CardinalityOverflow,
}
fn validate_selector_census(
    expected_ordinals: &BTreeSet<u64>,
    verdicts: &BTreeMap<u64, LifecycleIngressOccurrenceVerdict>,
) -> Result<(BTreeSet<u64>, u64), SelectorCensusError> {
    if verdicts.len() != expected_ordinals.len()
        || verdicts
            .keys()
            .any(|ordinal| !expected_ordinals.contains(ordinal))
    {
        return Err(SelectorCensusError::KeySetMismatch);
    }
    let priority_owners = verdicts
        .iter()
        .filter_map(|(ordinal, verdict)| verdict.is_priority().then_some(*ordinal))
        .collect::<BTreeSet<_>>();
    let selector_debt = u64::try_from(priority_owners.len())
        .map_err(|_| SelectorCensusError::CardinalityOverflow)?;
    Ok((priority_owners, selector_debt))
}
#[cfg(test)]
mod tests {
    use super::super::schema::{
        AdmissionDecision, AdmissionRequest, CandidateAdmission, CapacityClass, CapacityGeometry,
        CoordinatorFault, DurablePayloadReference, InitialLifecycleState, LeaseId, LifecycleStage,
        LifecycleStageKind, OwnerId, PhysicalGeometry, PhysicalSlot, PhysicalSlotId,
        PredecessorScope, SchedulerInputs, SchedulerReadyInputs, TerminalOutcome, TurnOutcome,
        TurnPlan, WaitToken,
    };
    use super::*;
    use iroha_crypto::Hash;
    fn digest(seed: u8) -> LifecycleDigest {
        LifecycleDigest::new([seed; 32])
    }
    fn context(seed: u8) -> LifecycleContext {
        LifecycleContext::new(digest(seed), 7)
    }
    fn request_hash(seed: u8) -> HashOf<wire::CertifiedBodyRequest> {
        HashOf::from_untyped_unchecked(Hash::new([seed]))
    }
    fn fetch_key(context: LifecycleContext, seed: u8) -> LifecycleKey {
        super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::FetchBody,
            seed,
        )
        .key
    }
    fn capacities(limit: usize) -> CapacityGeometry {
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, limit)))
    }
    fn waiting_fetch(
        context: LifecycleContext,
        key: LifecycleKey,
        causal_root: CausalRoot,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        generation: u64,
    ) -> (LifecycleCoordinator, CertifiedFetchReadyAuthority, u128) {
        let source = certified_fetch_wait_source(request_hash);
        let wait = WaitToken::new(source, generation);
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::FetchBody,
            u8::try_from(key.round().view()).expect("fixture Fetch view fits u8"),
        );
        assert_eq!(replay.key, key);
        let candidate = CandidateAdmission::new(
            key,
            causal_root,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            InitialLifecycleState::Ready,
            causal_root.digest(),
            DurablePayloadReference::None,
            replay.authority,
            PhysicalGeometry::new([PhysicalSlot::new(slot, digest(0xA1))], [slot]),
            None,
        );
        let mut coordinator = LifecycleCoordinator::new(context, 0, capacities(8));
        let AdmissionDecision::Admitted { owner, ordinal, .. } =
            coordinator.admit(AdmissionRequest::Candidate(candidate))
        else {
            panic!("sealed certified-Fetch fixture must admit")
        };
        assert_eq!(owner.causal_root(), causal_root);
        coordinator.ready_index.remove(&ordinal);
        coordinator
            .records
            .get_mut(&ordinal)
            .expect("admitted Fetch row")
            .state = LifecycleState::Waiting(wait);
        coordinator.observed_generation.insert(source, generation);
        let authority = CertifiedFetchReadyAuthority {
            ingress_identity: PendingFairIngressIdentity::for_test(context, digest(0xA2), 11),
            request_hash,
            key,
            causal_root,
        };
        (coordinator, authority, ordinal)
    }
    fn assert_rejection_preserved(before: &LifecycleCoordinator, after: &LifecycleCoordinator) {
        assert_eq!(after.fault, before.fault);
        assert_eq!(after.high_water, before.high_water);
        assert_eq!(after.records, before.records);
        assert_eq!(after.key_index, before.key_index);
        assert_eq!(after.owner_index, before.owner_index);
        assert_eq!(after.ready_index, before.ready_index);
        assert_eq!(after.admission_waits, before.admission_waits);
        assert_eq!(after.active_lease, before.active_lease);
        assert_eq!(after.next_lease, before.next_lease);
        assert_eq!(after.durable_records, before.durable_records);
        assert_eq!(after.capacity_geometry, before.capacity_geometry);
        assert_eq!(after.capacity_used, before.capacity_used);
        assert_eq!(after.capacity_generation, before.capacity_generation);
        assert_eq!(after.observed_generation, before.observed_generation);
        assert_eq!(after.producer_debts, before.producer_debts);
    }
    #[test]
    fn selector_census_requires_exact_keys_and_counts_concrete_occurrences() {
        let expected = BTreeSet::from([1, 2, 3]);
        let zero = BTreeMap::from([
            (1, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
            (2, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
            (3, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
        ]);
        assert_eq!(
            validate_selector_census(&expected, &zero),
            Ok((BTreeSet::new(), 0))
        );
        let priority = BTreeMap::from([
            (
                1,
                LifecycleIngressOccurrenceVerdict::NOT_PRIORITY
                    .with_authority(LifecycleIngressPriorityAuthority::RequestFencedCompletion)
                    .with_authority(LifecycleIngressPriorityAuthority::ClaimedResponseFamily),
            ),
            (2, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
            (
                3,
                LifecycleIngressOccurrenceVerdict::NOT_PRIORITY
                    .with_authority(LifecycleIngressPriorityAuthority::ClaimedResponseFamily),
            ),
        ]);
        assert_eq!(
            validate_selector_census(&expected, &priority),
            Ok((BTreeSet::from([1, 3]), 2))
        );
        let missing = BTreeMap::from([
            (1, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
            (2, LifecycleIngressOccurrenceVerdict::NOT_PRIORITY),
        ]);
        assert_eq!(
            validate_selector_census(&expected, &missing),
            Err(SelectorCensusError::KeySetMismatch)
        );
    }
    #[test]
    fn response_family_winner_is_lowest_physical_ordinal_not_iteration_order() {
        let first = lowest_physical_ordinal_per_family([
            ("request-a", 9),
            ("request-b", 4),
            ("request-a", 2),
            ("request-b", 7),
        ])
        .expect("non-zero physical ordinals form an exact family census");
        let reversed = lowest_physical_ordinal_per_family([
            ("request-b", 7),
            ("request-a", 2),
            ("request-b", 4),
            ("request-a", 9),
        ])
        .expect("selection is independent of source iteration");
        assert_eq!(first, BTreeMap::from([("request-a", 2), ("request-b", 4)]));
        assert_eq!(reversed, first);
        assert!(matches!(
            lowest_physical_ordinal_per_family([("request-a", 0)]),
            Err(LifecycleIngressSelectorError::InvalidCensus)
        ));
    }
    #[test]
    fn exact_certified_fetch_wake_reuses_owner_ordinal_and_record() {
        let context = context(1);
        let key = fetch_key(context, 3);
        let root = CausalRoot::new(digest(4));
        let (mut coordinator, authority, ordinal) =
            waiting_fetch(context, key, root, request_hash(5), 7);
        let owner = coordinator.records[&ordinal].owner;
        let record_count = coordinator.records.len();
        let high_water = coordinator.high_water;
        let capacity_used = coordinator.capacity_used.clone();
        let slot = *coordinator.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("one exact Fetch slot")
            .0;
        let incumbent_digest = coordinator.records[&ordinal].physical_slots[&slot];
        let projection = super::super::replay_authority::durable_certified_fetch_projection_fixture(
            context, root, 3,
        );
        let slot_universe = coordinator.records[&ordinal].episode.slot_universe.clone();
        let consumed_slots = coordinator.records[&ordinal].episode.consumed_slots.clone();
        assert_ne!(incumbent_digest, authority.ingress_identity.digest());
        let before = coordinator.clone();
        let PreparedCertifiedFetchReadyTransition::Mutation(prepared) = coordinator
            .prepare_certified_fetch_ready_projection(authority, &projection)
            .expect("exact response prepares one logical replacement")
        else {
            panic!("waiting Fetch preparation cannot stutter")
        };
        assert_eq!(prepared.location.owner(), owner);
        assert_eq!(prepared.location.ordinal(), ordinal);
        assert_eq!(prepared.location.slot(), slot);
        assert_eq!(prepared.location.incumbent_digest(), incumbent_digest);
        assert_rejection_preserved(&before, prepared.target_for_test());
        assert_eq!(prepared.target_for_test().capacity_used, capacity_used);
        prepared.commit();
        assert_eq!(coordinator.records.len(), record_count);
        assert_eq!(coordinator.high_water, high_water);
        assert_eq!(coordinator.key_index.get(&key), Some(&ordinal));
        assert_eq!(coordinator.records[&ordinal].owner, owner);
        assert_eq!(coordinator.records[&ordinal].state, LifecycleState::Ready);
        assert_eq!(coordinator.capacity_used, capacity_used);
        assert_eq!(
            coordinator.records[&ordinal].episode.slot_universe,
            slot_universe
        );
        assert_eq!(
            coordinator.records[&ordinal].episode.consumed_slots,
            consumed_slots
        );
        assert_eq!(coordinator.records[&ordinal].physical_slots.len(), 1);
        assert_eq!(
            coordinator.records[&ordinal].physical_slots[&slot],
            projection.completion_digest()
        );
        assert_eq!(
            coordinator
                .observed_generation
                .get(&authority.wait_source()),
            Some(&8)
        );
        assert_eq!(coordinator.fault, None);
    }
    #[test]
    fn duplicate_certified_fetch_wake_stutters_without_generation_change() {
        let context = context(2);
        let key = fetch_key(context, 4);
        let (mut coordinator, authority, ordinal) =
            waiting_fetch(context, key, CausalRoot::new(digest(5)), request_hash(6), 0);
        let (&slot, _) = coordinator.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("one exact Fetch slot");
        let incumbent_digest = coordinator.records[&ordinal].physical_slots[&slot];
        assert_eq!(
            coordinator.publish_certified_fetch_ready_authority(authority),
            Ok(CertifiedFetchReadyPublication::Published)
        );
        let generation = coordinator.observed_generation[&authority.wait_source()];
        let high_water = coordinator.high_water;
        let record_count = coordinator.records.len();
        assert_eq!(
            coordinator.publish_certified_fetch_ready_authority(authority),
            Ok(CertifiedFetchReadyPublication::StutterReady)
        );
        assert_eq!(coordinator.records[&ordinal].state, LifecycleState::Ready);
        assert_eq!(
            coordinator.observed_generation[&authority.wait_source()],
            generation
        );
        assert_eq!(coordinator.high_water, high_water);
        assert_eq!(coordinator.records.len(), record_count);
        let exact_ready = coordinator.clone();
        let mut stale_carrier = exact_ready.clone();
        stale_carrier
            .records
            .get_mut(&ordinal)
            .expect("exact ready Fetch row")
            .physical_slots
            .insert(slot, incumbent_digest);
        let before = stale_carrier.clone();
        assert_eq!(
            stale_carrier.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &stale_carrier);
        let mut damaged_geometry = exact_ready;
        damaged_geometry
            .records
            .get_mut(&ordinal)
            .expect("exact ready Fetch row")
            .episode
            .consumed_slots
            .remove(&slot);
        let before = damaged_geometry.clone();
        assert_eq!(
            damaged_geometry.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &damaged_geometry);
    }
    #[test]
    fn invalid_physical_replacement_rejects_without_mutation() {
        let context = context(0x21);
        let key = fetch_key(context, 6);
        let (coordinator, authority, ordinal) =
            waiting_fetch(context, key, CausalRoot::new(digest(7)), request_hash(8), 1);
        let (&slot, &_incumbent_digest) = coordinator.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("one exact Fetch slot");
        let mut trial = coordinator.clone();
        let projection = super::super::replay_authority::durable_certified_fetch_projection_fixture(
            context,
            authority.causal_root,
            6,
        );
        trial
            .records
            .get_mut(&ordinal)
            .expect("one exact Fetch row")
            .physical_slots
            .insert(slot, projection.completion_digest());
        let before = trial.clone();
        assert_eq!(
            trial.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &trial);
        let mut missing_universe = coordinator.clone();
        missing_universe
            .records
            .get_mut(&ordinal)
            .expect("exact Fetch row")
            .episode
            .slot_universe
            .remove(&slot);
        let before = missing_universe.clone();
        assert_eq!(
            missing_universe.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &missing_universe);
        let mut missing_consumed = coordinator.clone();
        missing_consumed
            .records
            .get_mut(&ordinal)
            .expect("exact Fetch row")
            .episode
            .consumed_slots
            .remove(&slot);
        let before = missing_consumed.clone();
        assert_eq!(
            missing_consumed.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &missing_consumed);
        let mut foreign_capacity_geometry = coordinator.clone();
        foreign_capacity_geometry
            .records
            .get_mut(&ordinal)
            .expect("exact Fetch row")
            .episode
            .slot_universe
            .insert(PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0));
        let before = foreign_capacity_geometry.clone();
        assert_eq!(
            foreign_capacity_geometry.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &foreign_capacity_geometry);
        let mut multiple_slots = coordinator;
        let extra_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 1);
        multiple_slots
            .records
            .get_mut(&ordinal)
            .expect("exact Fetch row")
            .physical_slots
            .insert(extra_slot, digest(0x22));
        let before = multiple_slots.clone();
        assert_eq!(
            multiple_slots.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&before, &multiple_slots);
    }
    #[test]
    fn foreign_context_key_and_causal_root_fail_without_coordinator_mutation() {
        let context = context(3);
        let key = fetch_key(context, 5);
        let root = CausalRoot::new(digest(6));
        let (coordinator, authority, _) = waiting_fetch(context, key, root, request_hash(7), 2);
        let mut foreign_context = authority;
        foreign_context.ingress_identity = PendingFairIngressIdentity::for_test(
            LifecycleContext::new(digest(0xF0), 7),
            authority.ingress_identity.digest(),
            authority.ingress_identity.physical_admission_ordinal(),
        );
        let mut trial = coordinator.clone();
        assert_eq!(
            trial.publish_certified_fetch_ready_authority(foreign_context),
            Err(CertifiedFetchReadyPublicationError::ForeignContext)
        );
        assert_rejection_preserved(&coordinator, &trial);
        let mut foreign_key = authority;
        foreign_key.key = fetch_key(context, 0xF1);
        let mut trial = coordinator.clone();
        assert_eq!(
            trial.publish_certified_fetch_ready_authority(foreign_key),
            Err(CertifiedFetchReadyPublicationError::MissingLifecycleKey)
        );
        assert_rejection_preserved(&coordinator, &trial);
        let mut foreign_root = authority;
        foreign_root.causal_root = CausalRoot::new(digest(0xF2));
        let mut trial = coordinator.clone();
        assert_eq!(
            trial.publish_certified_fetch_ready_authority(foreign_root),
            Err(CertifiedFetchReadyPublicationError::ForeignCausalRoot)
        );
        assert_rejection_preserved(&coordinator, &trial);
        let mut faulted = coordinator.clone();
        faulted.fault = Some(CoordinatorFault::InvalidSchedulerInputs);
        let before = faulted.clone();
        assert_eq!(
            faulted.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::CoordinatorFaulted)
        );
        assert_rejection_preserved(&before, &faulted);
    }
    #[test]
    fn damaged_ready_key_and_owner_indexes_reject_without_normalization() {
        let context = context(0x31);
        let key = fetch_key(context, 8);
        let root = CausalRoot::new(digest(9));
        let (coordinator, authority, ordinal) =
            waiting_fetch(context, key, root, request_hash(10), 4);
        let mut spurious_ready = coordinator.clone();
        spurious_ready.ready_index.insert(ordinal);
        let before = spurious_ready.clone();
        assert_eq!(
            spurious_ready.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &spurious_ready);
        let mut reverse_key_alias = coordinator.clone();
        reverse_key_alias
            .key_index
            .insert(fetch_key(context, 0x32), ordinal);
        let before = reverse_key_alias.clone();
        assert_eq!(
            reverse_key_alias.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &reverse_key_alias);
        let mut reverse_owner_alias = coordinator.clone();
        reverse_owner_alias.owner_index.insert(
            CausalRoot::new(digest(0x34)),
            reverse_owner_alias.records[&ordinal].owner,
        );
        let before = reverse_owner_alias.clone();
        assert_eq!(
            reverse_owner_alias.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &reverse_owner_alias);
        let mut internal_ordinal_alias = coordinator.clone();
        let mut aliased_record = internal_ordinal_alias.records[&ordinal].clone();
        aliased_record.key = fetch_key(context, 0x35);
        internal_ordinal_alias
            .records
            .insert(ordinal + 1, aliased_record);
        let before = internal_ordinal_alias.clone();
        assert_eq!(
            internal_ordinal_alias.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &internal_ordinal_alias);
        let mut inconsistent_owner = coordinator.clone();
        let mut foreign_record = inconsistent_owner.records[&ordinal].clone();
        let foreign_ordinal = ordinal + 1;
        foreign_record.ordinal = foreign_ordinal;
        foreign_record.key = fetch_key(context, 0x33);
        foreign_record.owner = OwnerId::new(root, foreign_ordinal);
        inconsistent_owner
            .key_index
            .insert(foreign_record.key, foreign_ordinal);
        inconsistent_owner
            .records
            .insert(foreign_ordinal, foreign_record);
        let before = inconsistent_owner.clone();
        assert_eq!(
            inconsistent_owner.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &inconsistent_owner);
        let mut missing_ready = coordinator;
        assert_eq!(
            missing_ready.publish_certified_fetch_ready_authority(authority),
            Ok(CertifiedFetchReadyPublication::Published)
        );
        missing_ready.ready_index.remove(&ordinal);
        let before = missing_ready.clone();
        assert_eq!(
            missing_ready.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &missing_ready);
    }
    #[test]
    fn wrong_certified_fetch_wait_source_or_generation_fails_closed() {
        let context = context(4);
        let key = fetch_key(context, 6);
        let (coordinator, authority, ordinal) =
            waiting_fetch(context, key, CausalRoot::new(digest(7)), request_hash(8), 3);
        let mut wrong_source = coordinator.clone();
        let other_wait = WaitToken::new(certified_fetch_wait_source(request_hash(0xF3)), 3);
        wrong_source
            .records
            .get_mut(&ordinal)
            .expect("exact row")
            .state = LifecycleState::Waiting(other_wait);
        let before = wrong_source.clone();
        assert_eq!(
            wrong_source.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::WrongWaitSource)
        );
        assert_rejection_preserved(&before, &wrong_source);
        let mut wrong_generation = coordinator;
        wrong_generation
            .records
            .get_mut(&ordinal)
            .expect("exact row")
            .state = LifecycleState::Waiting(WaitToken::new(authority.wait_source(), 4));
        let before = wrong_generation.clone();
        assert_eq!(
            wrong_generation.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::WrongWaitGeneration)
        );
        assert_rejection_preserved(&before, &wrong_generation);
    }
    #[test]
    fn scheduler_fetch_generation_stays_below_the_terminal_wait_value() {
        let source = WaitSource::External(digest(0xF4));
        assert_eq!(
            certified_fetch_scheduler_generation(WaitToken::new(source, 7)),
            Some(8)
        );
        assert_eq!(
            certified_fetch_scheduler_generation(WaitToken::new(source, u64::MAX - 2)),
            Some(u64::MAX - 1)
        );
        assert_eq!(
            certified_fetch_scheduler_generation(WaitToken::new(source, u64::MAX - 1)),
            None
        );
        assert_eq!(
            certified_fetch_scheduler_generation(WaitToken::new(source, u64::MAX)),
            None
        );
    }
    #[test]
    fn ambiguous_certified_fetch_source_and_generation_overflow_reject_unchanged() {
        let context = context(0x41);
        let key = fetch_key(context, 9);
        let (mut coordinator, authority, ordinal) = waiting_fetch(
            context,
            key,
            CausalRoot::new(digest(10)),
            request_hash(11),
            6,
        );
        let other_key = fetch_key(context, 0x42);
        let other_root = CausalRoot::new(digest(0x43));
        let other_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 1);
        let replay = super::super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::FetchBody,
            0x42,
        );
        assert_eq!(replay.key, other_key);
        let other = CandidateAdmission::new(
            other_key,
            other_root,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            InitialLifecycleState::Waiting(WaitToken::new(authority.wait_source(), 6)),
            other_root.digest(),
            DurablePayloadReference::None,
            replay.authority,
            PhysicalGeometry::new([PhysicalSlot::new(other_slot, digest(0x44))], [other_slot]),
            None,
        );
        assert!(matches!(
            coordinator.admit(AdmissionRequest::Candidate(other)),
            AdmissionDecision::Admitted { .. }
        ));
        let before = coordinator.clone();
        assert_eq!(
            coordinator.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::AmbiguousWaitSource)
        );
        assert_rejection_preserved(&before, &coordinator);
        let mut overflow = before;
        overflow.records.get_mut(&ordinal).expect("exact row").state =
            LifecycleState::Waiting(WaitToken::new(authority.wait_source(), u64::MAX));
        for record in overflow.records.values_mut() {
            if record.ordinal != ordinal {
                record.state = LifecycleState::Terminal(TerminalOutcome::Cancelled);
            }
        }
        overflow
            .observed_generation
            .insert(authority.wait_source(), u64::MAX);
        let before = overflow.clone();
        assert_eq!(
            overflow.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::WrongWaitGeneration)
        );
        assert_rejection_preserved(&before, &overflow);
    }
    #[test]
    fn claimed_rejects_but_terminal_stutters_without_advancing_generation() {
        let context = context(5);
        let key = fetch_key(context, 7);
        let (coordinator, authority, ordinal) =
            waiting_fetch(context, key, CausalRoot::new(digest(8)), request_hash(9), 5);
        let mut claimed = coordinator.clone();
        claimed.records.get_mut(&ordinal).expect("exact row").state =
            LifecycleState::Claimed(LeaseId(99));
        let before = claimed.clone();
        assert_eq!(
            claimed.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::ClaimedRecord)
        );
        assert_rejection_preserved(&before, &claimed);
        let mut damaged_terminal = coordinator.clone();
        damaged_terminal
            .records
            .get_mut(&ordinal)
            .expect("exact row")
            .state = LifecycleState::Terminal(TerminalOutcome::Cancelled);
        let before = damaged_terminal.clone();
        assert_eq!(
            damaged_terminal.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &damaged_terminal);
        let mut terminal = coordinator;
        assert_eq!(
            terminal.publish_certified_fetch_ready_authority(authority),
            Ok(CertifiedFetchReadyPublication::Published)
        );
        let record = &terminal.records[&ordinal];
        let inputs = SchedulerInputs::new(
            [],
            [(ordinal, SchedulerReadyInputs::new(record, None, [0; 6]))],
        )
        .expect("one exact ready Fetch scheduler row");
        let TurnPlan::Execute(lease) = terminal.plan_turn(inputs) else {
            panic!("exact ready Fetch must execute")
        };
        terminal.settle_turn(lease, TurnOutcome::Terminal(TerminalOutcome::Cancelled));
        assert_eq!(terminal.fault, None);
        let generation = terminal.observed_generation[&authority.wait_source()];
        let high_water = terminal.high_water;
        let record_count = terminal.records.len();
        let mut damaged_geometry = terminal.clone();
        let slot = *damaged_geometry.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("terminal Fetch retains one exact slot")
            .0;
        damaged_geometry
            .records
            .get_mut(&ordinal)
            .expect("terminal Fetch row")
            .episode
            .consumed_slots
            .remove(&slot);
        let before = damaged_geometry.clone();
        assert_eq!(
            damaged_geometry.publish_certified_fetch_ready_authority(authority),
            Err(CertifiedFetchReadyPublicationError::InvalidCoordinatorIndex)
        );
        assert_rejection_preserved(&before, &damaged_geometry);
        assert_eq!(
            terminal.publish_certified_fetch_ready_authority(authority),
            Ok(CertifiedFetchReadyPublication::StutterTerminal)
        );
        assert_eq!(
            terminal.observed_generation[&authority.wait_source()],
            generation
        );
        assert_eq!(terminal.high_water, high_water);
        assert_eq!(terminal.records.len(), record_count);
        assert_eq!(
            terminal.records[&ordinal].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
    }
    #[test]
    fn certified_fetch_completion_source_keeps_the_durable_cut_ordered() {
        let source = include_str!("v2_lifecycle_selector.rs");
        let transaction = source
            .split("pub(crate) fn complete_certified_fetch_body_persistence(")
            .nth(1)
            .expect("one consuming certified-Fetch completion transaction")
            .split("fn publish_certified_fetch_ready_authority(")
            .next()
            .expect("test-only reducer helper follows the transaction");
        let ordered = [
            "prepare_lifecycle_ingress_selector(",
            "certified_fetch_current_location(",
            "prepare_selected_certified_fetch_completion(",
            "bind_durable_body_receipt(receipt)",
            "prepare_certified_fetch_ready_projection(",
            "prepare_lifecycle_certified_fetch_completion(",
            "prepare_certified_body_fetch_owner_removal(",
            "matches_selected_response(",
            "into_exact_certified_fetch_dequeue(",
            "begin_fail_stop_operation()",
            "persist_exact_staged_successor()",
            "exact_dequeue.commit(ingress)",
            "commit_after_exact_dequeue(dequeued)",
            "ready.commit()",
            "commit_lifecycle_certified_fetch_completion",
            "service_prepared.commit(operation.permit())",
            "work_ack.commit()",
            "operation.complete()",
        ];
        let mut cursor = 0;
        for required in ordered {
            let relative = transaction[cursor..]
                .find(required)
                .unwrap_or_else(|| panic!("completion transaction omitted {required}"));
            cursor += relative + required.len();
        }
        assert!(!transaction.contains("runtime_lifecycle_ordinal"));
        assert!(!transaction.contains("BodyAvailableReservation"));
    }
    #[test]
    fn certified_response_maps_to_formal_untrusted_resource_source() {
        assert!(lifecycle_ingress_resource_is_untrusted(
            FairV2IngressSourceClass::Validator,
            true,
        ));
        assert!(lifecycle_ingress_resource_is_untrusted(
            FairV2IngressSourceClass::Authenticated,
            true,
        ));
        assert!(lifecycle_ingress_resource_is_untrusted(
            FairV2IngressSourceClass::Anonymous,
            false,
        ));
        assert!(!lifecycle_ingress_resource_is_untrusted(
            FairV2IngressSourceClass::Validator,
            false,
        ));
        assert!(!lifecycle_ingress_resource_is_untrusted(
            FairV2IngressSourceClass::Authenticated,
            false,
        ));
    }
}
