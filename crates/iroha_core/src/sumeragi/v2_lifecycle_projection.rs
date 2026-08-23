//! Sealed projection from exact runtime-bound adapter effects into lifecycle admission.
use super::replay_authority::{
    CertifiedFetchReplayEvidenceV1, CertifiedServeReplayEvidencePairV1,
    CertifiedServeTerminalReplayAuthorityPairV1, RecoveredStandaloneValidateSourceV1,
};
use super::schema::{
    AdmissionRequest, CandidateAdmission, CausalRoot, DurableBodyFrameReference,
    DurablePayloadReference, DurableServeNegativeOutcome, InitialLifecycleState, LifecycleContext,
    LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRound, LifecycleStage,
    LifecycleStageKind, LifecycleWorkClass, OwnerId, PhysicalGeometry, PhysicalSlot,
    PhysicalSlotId, PredecessorScope, TerminalOutcome, WaitSource, producer_turn_key_for_serve,
};
use super::work_registry::{
    AttemptedProducerTurnV1, CertifiedServeRegistryBatchPublicationError,
    CertifiedServeTerminalRegistryPublicationError, PreparedCertifiedServeRegistryBatchV1,
    PreparedCertifiedServeTerminalRegistryTransitionV1,
    PreparedProducerTurnTerminalRegistryTransitionV1, ProducerTurnTerminalRegistryPublicationError,
};
#[cfg(any(not(test), feature = "bls"))]
use crate::sumeragi::v2_body_store::DurableCertifiedServeBodyReadbackV1;
#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::{
    CertifiedServePayloadStoreError, CertifiedServePayloadStoreV1,
};
use crate::sumeragi::{
    v2::{AdapterEffect, SignRequest, VerifiedHeightContext},
    v2_body_store::{
        AuthenticatedGenesisBodyStoreFrameV1, BodyValidationRejectionIdentity, DurableBodyReceipt,
        DurableBodyValidationOutcome, V2BodyStore, V2BodyStoreError,
    },
    v2_certified_serve_payload_store::{
        AuthenticatedRecoveredCertifiedServePayload,
        AuthenticatedRecoveredCertifiedServePayloadState, CertifiedServePayloadNegativeOutcome,
        CertifiedServePayloadRetentionError, CertifiedServeTerminalPersistenceError,
        DurableCertifiedServeAdmissionPublication, DurableCertifiedServeAdmissionReceipt,
        DurableCertifiedServeAdmissionStateV1, DurableCertifiedServeCompletedReceipt,
        DurableCertifiedServeNegativeReceipt,
    },
    v2_core::EquivocationKind,
    v2_runtime::{PendingRuntimeEffectBinding, RuntimeCandidateSemanticStatement},
    v2_transport::AuthenticatedCertifiedBodyRequest,
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::Encode;
use thiserror::Error;
const BLOCK_SUBJECT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:block-subject:v1";
const EXECUTION_COMMITMENT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:execution-commitment:v1";
const TIMEOUT_CERTIFICATE_ENVELOPE_SUBJECT_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:timeout-certificate-envelope-subject:v1";
const EQUIVOCATION_SUBJECT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:equivocation-subject:v1";
const CERTIFIED_SERVE_KEY_SUBJECT_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:certified-serve-key-subject:v1";
const CERTIFIED_FETCH_WAIT_SOURCE_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:certified-fetch-wait-source:v1";
const DURABLE_VALIDATION_WAIT_SOURCE_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:durable-validation-wait-source:v1";
const REDUCER_FENCE_WAIT_SOURCE_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:reducer-fence:v1";
#[cfg(test)]
const PRODUCER_TURN_PHYSICAL_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:lifecycle:producer-turn-physical:v1";
/// Fail-closed reason why an exact adapter effect could not become lifecycle admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterEffectAdmissionError {
    /// The runtime sidecar did not exactly bind the supplied concrete effect.
    UnboundEffect,
    /// The verified height context and coordinator episode disagree.
    ForeignContext,
    /// A bound effect carried internally inconsistent semantic coordinates.
    InvalidCarrier,
    /// Store or Validate lost the inherited route-neutral body statement.
    MissingInheritedStatement,
    /// Broadcast carried a transport-only auxiliary payload.
    UnsupportedBroadcastPayload,
}
/// Authority-free projection of one exact runtime-bound adapter effect.
///
/// This value contains only deterministic lifecycle coordinates and physical
/// geometry. It cannot become an admission until a closed replay-evidence
/// carrier constructs the final [`CandidateAdmission`].
pub(super) struct AuthorityFreeAdmissionProjection {
    /// Exact logical key derived from the verified height and adapter effect.
    pub(super) key: LifecycleKey,
    /// Runtime causal owner retained by the exact pending binding.
    pub(super) causal_root: CausalRoot,
    /// Logical work class derived from the effect shape.
    pub(super) work_class: LifecycleWorkClass,
    /// Fixed V1 stage and predecessor scope.
    pub(super) stage: LifecycleStage,
    /// Initial state for a freshly reconstructed logical candidate.
    pub(super) initial_state: InitialLifecycleState,
    /// Restart-stable digest of the runtime causal owner.
    pub(super) reconstruction_source: LifecycleDigest,
    /// Exact physical effect slot reconstructed from the pending binding.
    pub(super) physical_geometry: PhysicalGeometry,
}
/// Fail-closed reason why durable Certified-Serve work could not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CertifiedServeAdmissionError {
    /// The verified height context and coordinator episode disagree.
    ForeignContext,
    /// The sealed authenticated request lost its exact structural identity.
    InvalidRequest,
    /// The post-fsync receipt names another request or certificate.
    ReceiptMismatch,
}
/// Fail-closed reason why a ledger body-frame reference could not be rebound.
#[derive(Debug, Error)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(super) enum DurableBodyFrameRecoveryError {
    /// Opening or enumerating the canonical body store failed.
    #[error(transparent)]
    BodyStore(#[from] V2BodyStoreError),
    /// No exact body-store frame matched all retained ledger coordinates.
    #[error("durable lifecycle body frame is absent from the opened body store")]
    Missing,
    /// More than one catalog row projected to the same supposedly exact frame.
    #[error("durable lifecycle body frame is ambiguous in the opened body store")]
    Ambiguous,
}
/// Opaque body-store recovery authority for one exact LedgerV1 frame reference.
///
/// The receipt and manifest have no parts API. A future registry reconstruction
/// transaction must consume this seal together with the row's proposal/QC or
/// validation authority; body bytes alone never authorize consensus work.
#[derive(Debug)]
#[must_use = "authenticated body-frame recovery must be joined to replay authority"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct AuthenticatedDurableBodyFrameRecovery {
    reference: DurableBodyFrameReference,
    manifest: wire::PayloadManifest,
    receipt: DurableBodyReceipt,
    genesis_authority: Option<AuthenticatedGenesisBodyStoreFrameV1>,
}
impl AuthenticatedDurableBodyFrameRecovery {
    /// Consume this exact catalog seal only through its frame-bound Certified
    /// Fetch replay family.
    pub(super) fn into_certified_fetch_body(
        self,
        evidence: &CertifiedFetchReplayEvidenceV1,
    ) -> Option<DurableBodyReceipt> {
        evidence
            .exactly_matches_recovered_body_frame(&self.reference, &self.manifest, &self.receipt)
            .then_some(self.receipt)
    }
    /// Consume this catalog seal only through an exact recovered standalone Validate family.
    pub(super) fn into_standalone_validate_body(
        self,
        evidence: &RecoveredStandaloneValidateSourceV1,
    ) -> Option<DurableBodyReceipt> {
        let exact_genesis_authority = self
            .genesis_authority
            .as_ref()
            .is_some_and(|proof| proof.exactly_matches(&self.receipt));
        (evidence.exactly_matches_recovered_body_frame(
            &self.reference,
            &self.manifest,
            &self.receipt,
        ) && (!evidence.requires_genesis_authority_body_store() || exact_genesis_authority))
            .then_some(self.receipt)
    }
}
/// Failure at the payload-first/ledger-second Certified-Serve admission
/// boundary.
#[derive(Debug, Error)]
#[cfg(test)]
pub(crate) enum CertifiedServeAdmissionBoundaryError {
    /// The exact payload could not be durably published or safely rolled back.
    #[error(transparent)]
    PayloadStore(#[from] CertifiedServePayloadStoreError),
    /// The authenticated request and its durable receipt did not project into
    /// the active lifecycle episode.
    #[error("authenticated Certified-Serve projection failed: {0:?}")]
    Projection(CertifiedServeAdmissionError),
}
/// Stable failure class for the receipt-free Certified-Serve terminal owner
/// transaction. The enclosing result distinguishes safe prepublication input
/// rejection from restart-required owner invariant or durability failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CertifiedServeTerminalSettlementFailureV1 {
    /// The coordinator, active lease, or attached LedgerV1 was not exact.
    Coordinator,
    /// The signed request did not name the active Serve storage family.
    RequestAuthority,
    /// Completion was attempted after the exact body-store owner left this owner.
    BodyStoreUnavailable,
    /// Terminal payload persistence did not return an exact durable receipt.
    PayloadStore,
    /// The post-fsync receipt could not close the terminal replay family.
    TerminalAuthority,
    /// The complete current or prospective concrete census was not exact.
    Registry,
    /// Exact LedgerV1 successor publication failed.
    Ledger,
}
/// Fail-closed class for replaying an already-completed Certified-Serve response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum CertifiedServeTerminalReplayFailureV1 {
    /// The sealed replay, authenticated request, or terminal LedgerV1 row diverged.
    Authorization,
    /// Launch did not retain the exact worker-owned body-store instance seal.
    BodyStoreUnavailable,
    /// The worker readback or reconstructed response diverged from the tombstone.
    PayloadStore,
}
/// Stable failure class for the claimed ProducerTurn terminal transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProducerTurnTerminalSettlementFailureV1 {
    /// The coordinator, claim, or attached LedgerV1 was not exact.
    Coordinator,
    /// The complete current or prospective concrete census was not exact.
    Registry,
    /// Exact LedgerV1 successor publication failed.
    Ledger,
}
/// Opaque fail-stop result retaining the claim and any prepared carrier
/// transition. The logical claim and incumbent carrier remain owned by the
/// faulted lifecycle owner for restart reconciliation.
#[must_use = "ProducerTurn settlement failure requires process restart"]
#[derive(Debug)]
pub(crate) struct ProducerTurnTerminalSettlementErrorV1 {
    failure: ProducerTurnTerminalSettlementFailureV1,
    _attempted: AttemptedProducerTurnV1,
    _transition: Option<PreparedProducerTurnTerminalRegistryTransitionV1>,
}
impl ProducerTurnTerminalSettlementErrorV1 {
    /// Return the stable fail-stop class.
    pub(crate) const fn failure(&self) -> ProducerTurnTerminalSettlementFailureV1 {
        self.failure
    }
}
#[derive(Clone, Copy, Debug)]
enum DurableCertifiedServeTerminalPublicationV1 {
    Completed(DurableCertifiedServeCompletedReceipt),
    Negative(DurableCertifiedServeNegativeReceipt),
}
/// Opaque fail-stop result retaining every move-only authority still needed by
/// startup reconciliation. The live owner remains faulted and continues to own
/// its exact terminal payload store.
#[must_use = "terminal settlement failure requires process restart"]
#[derive(Debug)]
pub(crate) struct CertifiedServeTerminalSettlementRestartV1 {
    failure: CertifiedServeTerminalSettlementFailureV1,
    _lease: super::TurnLease,
    _publication: Option<DurableCertifiedServeTerminalPublicationV1>,
    _transition: Option<PreparedCertifiedServeTerminalRegistryTransitionV1>,
}
impl CertifiedServeTerminalSettlementRestartV1 {
    /// Return the stable fail-stop class without releasing retained authority.
    pub(crate) const fn failure(&self) -> CertifiedServeTerminalSettlementFailureV1 {
        self.failure
    }
}
/// Ownership-preserving terminal settlement failure. Prepublication failures
/// return the unchanged active lease; restart-required failures retain all
/// post-fsync authority opaquely.
#[must_use = "the Certified-Serve terminal settlement failure must be handled"]
#[derive(Debug)]
pub(crate) struct CertifiedServeTerminalSettlementErrorV1 {
    kind: CertifiedServeTerminalSettlementErrorKindV1,
}
#[allow(variant_size_differences)]
#[derive(Debug)]
enum CertifiedServeTerminalSettlementErrorKindV1 {
    Prepublication {
        failure: CertifiedServeTerminalSettlementFailureV1,
        lease: super::TurnLease,
    },
    RestartRequired(CertifiedServeTerminalSettlementRestartV1),
}
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "first-release worker settlement does not yet consume the retained terminal diagnostic accessors"
    )
)]
impl CertifiedServeTerminalSettlementErrorV1 {
    /// Return the stable failure class.
    pub(crate) const fn failure(&self) -> CertifiedServeTerminalSettlementFailureV1 {
        match &self.kind {
            CertifiedServeTerminalSettlementErrorKindV1::Prepublication { failure, .. } => *failure,
            CertifiedServeTerminalSettlementErrorKindV1::RestartRequired(restart) => {
                restart.failure()
            }
        }
    }
    /// Whether terminal storage may already be durable and startup is required.
    pub(crate) const fn restart_required(&self) -> bool {
        matches!(
            &self.kind,
            CertifiedServeTerminalSettlementErrorKindV1::RestartRequired(_)
        )
    }
    /// Recover the unchanged active lease only before any terminal publication.
    pub(crate) fn into_lease(self) -> Result<super::TurnLease, Self> {
        match self.kind {
            CertifiedServeTerminalSettlementErrorKindV1::Prepublication { lease, .. } => Ok(lease),
            kind @ CertifiedServeTerminalSettlementErrorKindV1::RestartRequired(_) => {
                Err(Self { kind })
            }
        }
    }
    fn prepublication(
        failure: CertifiedServeTerminalSettlementFailureV1,
        lease: super::TurnLease,
    ) -> Self {
        Self {
            kind: CertifiedServeTerminalSettlementErrorKindV1::Prepublication { failure, lease },
        }
    }
    fn requiring_restart(restart: CertifiedServeTerminalSettlementRestartV1) -> Self {
        Self {
            kind: CertifiedServeTerminalSettlementErrorKindV1::RestartRequired(restart),
        }
    }
}
/// Stable classification for the sealed fresh Certified-Serve transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CertifiedServeConcreteAdmissionFailureV1 {
    /// The selector target names another context, command family, or request.
    SelectorAuthority,
    /// Payload retention failed before a post-fsync receipt existed.
    PayloadStore,
    /// The authenticated request could not form the closed replay family.
    Projection,
    /// The live coordinator or its attached LedgerV1 was not exact.
    Coordinator,
    /// The prospective coordinator/registry census was not exact.
    Registry,
    /// LedgerV1 publication was invoked and its result requires restart.
    Ledger,
    /// The exact pre-ledger Pending abort did not complete durably.
    PendingAbort,
}
/// Opaque ownership-preserving result of one selector-bound payload-first admission.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the selector-owned Certified-Serve target must be handled"]
pub(crate) struct CertifiedServeConcreteAdmissionV1 {
    kind: CertifiedServeConcreteAdmissionKindV1,
}
/// Move-only authority to replay one already-completed Certified-Serve response.
///
/// This value is minted only after the payload tombstone and its exact
/// LedgerV1 Serve/Producer pair have been cross-checked. It carries no raw
/// constructor, so the worker can use it only for the same request, response,
/// and lifecycle ordinal.
#[must_use = "a completed Certified-Serve replay must be dispatched or fail closed"]
pub(in crate::sumeragi) struct CertifiedServeTerminalReplayAuthorizationV1 {
    key: LifecycleKey,
    ordinal: u128,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
}
impl CertifiedServeTerminalReplayAuthorizationV1 {
    /// Return the already-terminal Serve ordinal retained by this authority.
    pub(in crate::sumeragi) const fn ordinal(&self) -> u128 {
        self.ordinal
    }
    /// Check that a worker dispatch still carries the exact authenticated request.
    pub(in crate::sumeragi) fn authorizes_request(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        authenticated.request_hash() == self.request_hash
            && HashOf::new(authenticated.request()) == self.request_hash
    }
    fn authorizes_response(&self, response: &wire::CertifiedBodyResponse) -> bool {
        response.request_hash == self.request_hash && HashOf::new(response) == self.response_hash
    }
}
#[allow(variant_size_differences)]
enum CertifiedServeConcreteAdmissionKindV1 {
    Published {
        decision: super::AdmissionDecision,
        target: super::LifecycleIngressIoTargetSeal,
        terminal_replay: Option<CertifiedServeTerminalReplayAuthorizationV1>,
    },
    Retryable {
        failure: CertifiedServeConcreteAdmissionFailureV1,
        decision: Option<super::AdmissionDecision>,
        target: super::LifecycleIngressIoTargetSeal,
    },
    RestartRequired {
        failure: CertifiedServeConcreteAdmissionFailureV1,
        _target: super::LifecycleIngressIoTargetSeal,
        _publication: Option<DurableCertifiedServeAdmissionPublication>,
        _replay: Option<CertifiedServeReplayEvidencePairV1>,
        _batch: Option<PreparedCertifiedServeRegistryBatchV1>,
    },
}
/// Safe consuming continuation after a published result or proven pre-ledger rollback.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "the selector-owned Certified-Serve continuation must be handled"]
pub(crate) struct CertifiedServeConcreteAdmissionContinuationV1 {
    decision: Option<super::AdmissionDecision>,
    failure: Option<CertifiedServeConcreteAdmissionFailureV1>,
    target: super::LifecycleIngressIoTargetSeal,
    terminal_replay: Option<CertifiedServeTerminalReplayAuthorizationV1>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl CertifiedServeConcreteAdmissionV1 {
    /// Return the durable admission decision when one was safely published or
    /// conclusively declined before LedgerV1.
    pub(crate) const fn decision(&self) -> Option<super::AdmissionDecision> {
        match &self.kind {
            CertifiedServeConcreteAdmissionKindV1::Published { decision, .. } => Some(*decision),
            CertifiedServeConcreteAdmissionKindV1::Retryable { decision, .. } => *decision,
            CertifiedServeConcreteAdmissionKindV1::RestartRequired { .. } => None,
        }
    }
    /// Return whether the process must restart before touching this owner.
    pub(crate) const fn restart_required(&self) -> bool {
        matches!(
            &self.kind,
            CertifiedServeConcreteAdmissionKindV1::RestartRequired { .. }
        )
    }
    /// Return the stable failure class, if this was not a publication success.
    pub(crate) const fn failure(&self) -> Option<CertifiedServeConcreteAdmissionFailureV1> {
        match &self.kind {
            CertifiedServeConcreteAdmissionKindV1::Published { .. } => None,
            CertifiedServeConcreteAdmissionKindV1::Retryable { failure, .. }
            | CertifiedServeConcreteAdmissionKindV1::RestartRequired { failure, .. } => {
                Some(*failure)
            }
        }
    }
    /// Extract a safe continuation only when no fail-stop authority is retained.
    pub(crate) fn into_safe_continuation(
        self,
    ) -> Result<CertifiedServeConcreteAdmissionContinuationV1, Self> {
        match self.kind {
            CertifiedServeConcreteAdmissionKindV1::Published {
                decision,
                target,
                terminal_replay,
            } => Ok(CertifiedServeConcreteAdmissionContinuationV1 {
                decision: Some(decision),
                failure: None,
                target,
                terminal_replay,
            }),
            CertifiedServeConcreteAdmissionKindV1::Retryable {
                failure,
                decision,
                target,
            } => Ok(CertifiedServeConcreteAdmissionContinuationV1 {
                decision,
                failure: Some(failure),
                target,
                terminal_replay: None,
            }),
            kind @ CertifiedServeConcreteAdmissionKindV1::RestartRequired { .. } => {
                Err(Self { kind })
            }
        }
    }
    fn published(
        decision: super::AdmissionDecision,
        target: super::LifecycleIngressIoTargetSeal,
    ) -> Self {
        Self {
            kind: CertifiedServeConcreteAdmissionKindV1::Published {
                decision,
                target,
                terminal_replay: None,
            },
        }
    }
    fn published_terminal_replay(
        decision: super::AdmissionDecision,
        target: super::LifecycleIngressIoTargetSeal,
        terminal_replay: CertifiedServeTerminalReplayAuthorizationV1,
    ) -> Self {
        Self {
            kind: CertifiedServeConcreteAdmissionKindV1::Published {
                decision,
                target,
                terminal_replay: Some(terminal_replay),
            },
        }
    }
    fn retryable(
        failure: CertifiedServeConcreteAdmissionFailureV1,
        decision: Option<super::AdmissionDecision>,
        target: super::LifecycleIngressIoTargetSeal,
    ) -> Self {
        Self {
            kind: CertifiedServeConcreteAdmissionKindV1::Retryable {
                failure,
                decision,
                target,
            },
        }
    }
    fn requiring_restart(
        failure: CertifiedServeConcreteAdmissionFailureV1,
        target: super::LifecycleIngressIoTargetSeal,
        publication: Option<DurableCertifiedServeAdmissionPublication>,
        replay: Option<CertifiedServeReplayEvidencePairV1>,
        batch: Option<PreparedCertifiedServeRegistryBatchV1>,
    ) -> Self {
        Self {
            kind: CertifiedServeConcreteAdmissionKindV1::RestartRequired {
                failure,
                _target: target,
                _publication: publication,
                _replay: replay,
                _batch: batch,
            },
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl CertifiedServeConcreteAdmissionContinuationV1 {
    /// Return the safe logical decision, if projection failed before one existed.
    pub(crate) const fn decision(&self) -> Option<super::AdmissionDecision> {
        self.decision
    }
    /// Return the safe pre-ledger failure class, if any.
    pub(crate) const fn failure(&self) -> Option<CertifiedServeConcreteAdmissionFailureV1> {
        self.failure
    }
    /// Recover the unchanged selector target only from this safe continuation.
    pub(crate) fn into_target(self) -> super::LifecycleIngressIoTargetSeal {
        self.target
    }
    /// Recover the selector target together with any sealed completed-response replay.
    pub(crate) fn into_target_and_terminal_replay(
        self,
    ) -> (
        super::LifecycleIngressIoTargetSeal,
        Option<CertifiedServeTerminalReplayAuthorizationV1>,
    ) {
        (self.target, self.terminal_replay)
    }
}
fn certified_serve_terminal_replay_decision(
    coordinator: &super::LifecycleCoordinator,
    verified: &VerifiedHeightContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    publication: &DurableCertifiedServeAdmissionPublication,
) -> Option<(
    super::AdmissionDecision,
    Option<CertifiedServeTerminalReplayAuthorizationV1>,
)> {
    if publication.is_pending()
        || !publication.exactly_matches_authenticated_request(authenticated)
        || coordinator.active_context() != lifecycle_context(verified.context())
    {
        return None;
    }
    let request = authenticated.request();
    request.validate(verified.context()).ok()?;
    if authenticated.request_hash() != HashOf::new(request)
        || request.certificate.round.context_id != request.round.context_id
        || request.certificate.round.height != request.round.height
        || request.certificate.proposal_round != request.round
        || request.certificate.subject != request.subject
    {
        return None;
    }
    let request_digest = digest_from_bytes(authenticated.request_hash().as_ref());
    let certificate_digest = digest_from_bytes(HashOf::new(&request.certificate).as_ref());
    let (payload, outcome, completed_response_hash) = match publication.state() {
        DurableCertifiedServeAdmissionStateV1::Pending => return None,
        DurableCertifiedServeAdmissionStateV1::Completed(response_hash) => {
            let response = digest_from_bytes(response_hash.as_ref());
            (
                DurablePayloadReference::CertifiedServeCompleted {
                    request: request_digest,
                    certificate: certificate_digest,
                    response,
                },
                TerminalOutcome::Completed(Some(response)),
                Some(response_hash),
            )
        }
        DurableCertifiedServeAdmissionStateV1::Negative(outcome) => {
            let outcome = match outcome {
                CertifiedServePayloadNegativeOutcome::Cancelled => {
                    DurableServeNegativeOutcome::Cancelled
                }
                CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                    DurableServeNegativeOutcome::Rejected(code)
                }
                CertifiedServePayloadNegativeOutcome::Failed(code) => {
                    DurableServeNegativeOutcome::Failed(code)
                }
            };
            (
                DurablePayloadReference::CertifiedServeNegative {
                    request: request_digest,
                    certificate: certificate_digest,
                    outcome,
                },
                outcome.terminal(),
                None,
            )
        }
    };
    let key = lifecycle_key(
        verified.context(),
        request.certificate.round,
        Some(request.round),
        Some(certified_serve_key_subject(
            request.subject,
            authenticated.request_hash(),
        )),
        LifecyclePhase::Serve,
        Some(execution_commitment(
            request.certificate.execution_commitment,
        )),
    );
    let serve_ordinal = coordinator.key_index.get(&key).copied()?;
    let producer_ordinal = serve_ordinal.checked_add(1)?;
    let serve = coordinator.records.get(&serve_ordinal)?;
    let serve_metadata = coordinator.durable_records.get(&serve_ordinal)?;
    let producer = coordinator.records.get(&producer_ordinal)?;
    let producer_metadata = coordinator.durable_records.get(&producer_ordinal)?;
    let owner = serve.owner;
    let producer_debt_is_exact = if matches!(producer.state, super::LifecycleState::Terminal(_)) {
        !coordinator.producer_debts.contains_key(&serve_ordinal)
    } else {
        coordinator.producer_debts.get(&serve_ordinal) == Some(&producer_ordinal)
    };
    if serve.key != key
        || serve.ordinal != serve_ordinal
        || serve.owner.first_admission_ordinal() != serve_ordinal
        || serve.owner.causal_root().digest() != request_digest
        || serve.work_class != LifecycleWorkClass::CertifiedServe
        || serve.stage.kind() != LifecycleStageKind::CertifiedServe
        || serve.state != super::LifecycleState::Terminal(outcome)
        || serve_metadata.reconstruction_source != request_digest
        || serve_metadata.payload != payload
        || !serve_metadata.replay_authority.structurally_matches_record(
            coordinator.active_context(),
            serve.key,
            serve.work_class,
            serve.stage,
            serve_metadata.payload,
        )
        || !serve_metadata
            .replay_authority
            .exactly_matches_certified_serve_publication(authenticated, publication.receipt())
        || producer.key != producer_turn_key_for_serve(key)?
        || producer.ordinal != producer_ordinal
        || producer.owner != owner
        || producer.work_class != LifecycleWorkClass::ProducerTurn
        || producer.stage.kind() != LifecycleStageKind::ProducerTurn
        || producer_metadata.reconstruction_source != request_digest
        || producer_metadata.payload != DurablePayloadReference::None
        || !producer_metadata
            .replay_authority
            .structurally_matches_record(
                coordinator.active_context(),
                producer.key,
                producer.work_class,
                producer.stage,
                producer_metadata.payload,
            )
        || !producer_metadata
            .replay_authority
            .exactly_matches_certified_serve_publication(authenticated, publication.receipt())
        || !serve_metadata
            .replay_authority
            .same_persisted_family(&producer_metadata.replay_authority)
        || !producer_debt_is_exact
        || (outcome == TerminalOutcome::Cancelled
            && producer.state != super::LifecycleState::Terminal(TerminalOutcome::Cancelled))
    {
        return None;
    }
    let decision = match outcome {
        TerminalOutcome::Completed(Some(_)) => {
            super::AdmissionDecision::ReplayTerminal { owner, outcome }
        }
        TerminalOutcome::Cancelled | TerminalOutcome::Rejected(_) | TerminalOutcome::Failed(_) => {
            super::AdmissionDecision::StutterTerminal { owner }
        }
        TerminalOutcome::Advanced | TerminalOutcome::Completed(None) => return None,
    };
    let terminal_replay =
        completed_response_hash.map(
            |response_hash| CertifiedServeTerminalReplayAuthorizationV1 {
                key,
                ordinal: serve_ordinal,
                request_hash: authenticated.request_hash(),
                response_hash,
            },
        );
    Some((decision, terminal_replay))
}
impl super::ProductionLifecycleOwnerV1 {
    /// Persist one selected Certified-Serve payload, then atomically publish its
    /// adjacent LedgerV1 rows and two exact concrete carriers.
    ///
    /// This boundary accepts only the selector's opaque target and the
    /// authenticated signed request. It accepts no route, queue witness,
    /// candidate, effect, pending binding, ordinal, digest, or replay bytes.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn admit_selected_certified_serve(
        &mut self,
        target: super::LifecycleIngressIoTargetSeal,
        local_signer: &KeyPair,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> CertifiedServeConcreteAdmissionV1 {
        if self.coordinator.fault().is_some() || self.coordinator.ledger_store.is_none() {
            return CertifiedServeConcreteAdmissionV1::requiring_restart(
                CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                target,
                None,
                None,
                None,
            );
        }
        if target.context() != self.coordinator.active_context()
            || target.kind() != super::LifecycleIngressIoTargetKind::CertifiedServe
            || !target.matches_certified_serve_request(authenticated.request_hash())
        {
            return CertifiedServeConcreteAdmissionV1::retryable(
                CertifiedServeConcreteAdmissionFailureV1::SelectorAuthority,
                None,
                target,
            );
        }
        if {
            let registry = self.registry.registry_mut();
            !registry.exactly_covers_all_live_work(&self.verified, &self.coordinator)
        } {
            return CertifiedServeConcreteAdmissionV1::retryable(
                CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                None,
                target,
            );
        }
        let publication = match self
            .payload_store
            .retain_for_admission_with_verified_retention(
                &self.verified,
                local_signer,
                authenticated,
            ) {
            Ok(publication) => publication,
            Err(CertifiedServePayloadRetentionError::Unchanged(_error)) => {
                return CertifiedServeConcreteAdmissionV1::retryable(
                    CertifiedServeConcreteAdmissionFailureV1::PayloadStore,
                    None,
                    target,
                );
            }
            Err(CertifiedServePayloadRetentionError::PublicationAmbiguous(_error)) => {
                self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return CertifiedServeConcreteAdmissionV1::requiring_restart(
                    CertifiedServeConcreteAdmissionFailureV1::PayloadStore,
                    target,
                    None,
                    None,
                    None,
                );
            }
        };
        if !publication.is_pending() {
            return match certified_serve_terminal_replay_decision(
                &self.coordinator,
                &self.verified,
                authenticated,
                &publication,
            ) {
                Some((decision, Some(terminal_replay))) => {
                    CertifiedServeConcreteAdmissionV1::published_terminal_replay(
                        decision,
                        target,
                        terminal_replay,
                    )
                }
                Some((decision, None)) => {
                    CertifiedServeConcreteAdmissionV1::published(decision, target)
                }
                None => self.certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                    None,
                    None,
                    None,
                ),
            };
        }
        let prepared = match prepare_certified_serve_admission(
            self.coordinator.active_context(),
            &self.verified,
            authenticated,
            publication.receipt(),
        ) {
            Ok(prepared) => prepared,
            Err(_) => {
                return self.certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Projection,
                    None,
                    None,
                    None,
                );
            }
        };
        let (candidate, replay) = prepared.into_candidate_and_replay();
        let serve_key = candidate.key;
        let mut staged = self.coordinator.stage_durable_transaction();
        let (decision, ordinal_reservation) =
            staged.reduce_admit_with_durable_ordinals(AdmissionRequest::Candidate(candidate));
        match decision {
            super::AdmissionDecision::Admitted {
                producer_turn_ordinal: Some(_),
                ..
            } => {}
            super::AdmissionDecision::Retry { .. } => {
                return CertifiedServeConcreteAdmissionV1::published(decision, target);
            }
            super::AdmissionDecision::WaitForCapacity(_)
            | super::AdmissionDecision::Rejected(_)
            | super::AdmissionDecision::NonCandidate
            | super::AdmissionDecision::FailClosed(_) => {
                return self.certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                    Some(decision),
                    Some(replay),
                    None,
                );
            }
            _ => {
                return self.certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                    None,
                    Some(replay),
                    None,
                );
            }
        }
        let Some(ordinal_reservation) = ordinal_reservation else {
            return self.certified_serve_preledger_failure(
                target,
                publication,
                CertifiedServeConcreteAdmissionFailureV1::Coordinator,
                None,
                Some(replay),
                None,
            );
        };
        let batch = match PreparedCertifiedServeRegistryBatchV1::from_fresh_admitted_pair(
            &staged, serve_key, replay,
        ) {
            Ok(batch) => batch,
            Err(replay) => {
                return self.certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Registry,
                    None,
                    Some(replay),
                    None,
                );
            }
        };
        let publication_result = self
            .registry
            .registry_mut()
            .install_certified_serve_fresh_batch_before_publication(
                batch,
                &self.verified,
                &self.coordinator,
                &staged,
                || {
                    self.coordinator
                        .persist_exact_staged_successor_with_ordinal_reservation(
                            &staged,
                            &ordinal_reservation,
                        )
                },
            );
        match publication_result {
            Ok(()) => {
                self.coordinator = staged;
                CertifiedServeConcreteAdmissionV1::published(decision, target)
            }
            Err(CertifiedServeRegistryBatchPublicationError::Preflight(batch)) => self
                .certified_serve_preledger_failure(
                    target,
                    publication,
                    CertifiedServeConcreteAdmissionFailureV1::Registry,
                    None,
                    None,
                    Some(batch),
                ),
            Err(CertifiedServeRegistryBatchPublicationError::Publication(_, batch)) => {
                self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
                CertifiedServeConcreteAdmissionV1::requiring_restart(
                    CertifiedServeConcreteAdmissionFailureV1::Ledger,
                    target,
                    Some(publication),
                    None,
                    Some(batch),
                )
            }
        }
    }
    fn certified_serve_preledger_failure(
        &mut self,
        target: super::LifecycleIngressIoTargetSeal,
        publication: DurableCertifiedServeAdmissionPublication,
        failure: CertifiedServeConcreteAdmissionFailureV1,
        decision: Option<super::AdmissionDecision>,
        replay: Option<CertifiedServeReplayEvidencePairV1>,
        batch: Option<PreparedCertifiedServeRegistryBatchV1>,
    ) -> CertifiedServeConcreteAdmissionV1 {
        if publication.can_abort_fresh_pending()
            && self
                .payload_store
                .rollback_pending(publication.receipt())
                .is_ok()
        {
            return CertifiedServeConcreteAdmissionV1::retryable(failure, decision, target);
        }
        self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
        CertifiedServeConcreteAdmissionV1::requiring_restart(
            if publication.can_abort_fresh_pending() {
                CertifiedServeConcreteAdmissionFailureV1::PendingAbort
            } else {
                failure
            },
            target,
            Some(publication),
            replay,
            batch,
        )
    }
}
impl super::ProductionLifecycleOwnerV1 {
    /// Persist and publish one exact completed Certified-Serve terminal.
    ///
    /// The terminal receipt is created inside this owner from its retained
    /// payload store and its exact retained body-store instance. No receipt,
    /// payload id, candidate, ordinal, digest, or replay parts enter this API.
    #[cfg(any(not(test), feature = "bls"))]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "first-release owner-to-worker terminal completion handoff is not wired yet"
        )
    )]
    pub(in crate::sumeragi) fn settle_certified_serve_completed(
        &mut self,
        lease: super::TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        durable_body: &DurableBodyReceipt,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServeTerminalSettlementErrorV1> {
        self.preflight_certified_serve_terminal(&lease, authenticated)?;
        // TODO: After the owner-to-worker launch consumes `body_store`, replace
        // this unlaunched-owner completion seam with one worker-authenticated
        // completion capability bound to the retained store-instance seal.
        let Some(body_store) = self.body_store.as_ref() else {
            return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable,
                lease,
            ));
        };
        let receipt = match self.payload_store.persist_completed_with_exact_body(
            authenticated,
            durable_body,
            body_store,
            response,
        ) {
            Ok(receipt) => receipt,
            Err(CertifiedServeTerminalPersistenceError::InputRejected(_)) => {
                return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                    CertifiedServeTerminalSettlementFailureV1::PayloadStore,
                    lease,
                ));
            }
            Err(
                CertifiedServeTerminalPersistenceError::StoreInvariant(_)
                | CertifiedServeTerminalPersistenceError::PublicationAmbiguous(_),
            ) => {
                return Err(self.certified_serve_terminal_restart(
                    CertifiedServeTerminalSettlementFailureV1::PayloadStore,
                    lease,
                    None,
                    None,
                ));
            }
        };
        self.publish_certified_serve_terminal(
            lease,
            authenticated,
            DurableCertifiedServeTerminalPublicationV1::Completed(receipt),
        )
    }
    /// Persist and publish a completed Certified-Serve returned by the launched
    /// I/O worker.
    ///
    /// Launch leaves this owner a comparison-only body-store identity while
    /// moving the exact store into the worker. The move-only readback must have
    /// been minted by that same store instance and is consumed before the
    /// terminal LedgerV1 transition is published.
    #[cfg(any(not(test), feature = "bls"))]
    pub(in crate::sumeragi) fn settle_certified_serve_worker_completed(
        &mut self,
        lease: super::TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        body_readback: DurableCertifiedServeBodyReadbackV1,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServeTerminalSettlementErrorV1> {
        self.preflight_certified_serve_terminal(&lease, authenticated)?;
        if self.body_store.is_some() {
            return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable,
                lease,
            ));
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable,
                lease,
            ));
        };
        let receipt = match self.payload_store.persist_completed_with_worker_readback(
            authenticated,
            body_readback,
            body_store_identity,
            response,
        ) {
            Ok(receipt) => receipt,
            Err(CertifiedServeTerminalPersistenceError::InputRejected(_)) => {
                return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                    CertifiedServeTerminalSettlementFailureV1::PayloadStore,
                    lease,
                ));
            }
            Err(
                CertifiedServeTerminalPersistenceError::StoreInvariant(_)
                | CertifiedServeTerminalPersistenceError::PublicationAmbiguous(_),
            ) => {
                return Err(self.certified_serve_terminal_restart(
                    CertifiedServeTerminalSettlementFailureV1::PayloadStore,
                    lease,
                    None,
                    None,
                ));
            }
        };
        self.publish_certified_serve_terminal(
            lease,
            authenticated,
            DurableCertifiedServeTerminalPublicationV1::Completed(receipt),
        )
    }
    /// Verify and consume one worker reconstruction of an already-completed
    /// Certified-Serve before its exact response is delivered again.
    ///
    /// This path never creates or advances a lifecycle row. The move-only
    /// authorization was minted from the existing terminal LedgerV1 pair, and
    /// the worker readback must reproduce the payload-store tombstone exactly.
    #[cfg(any(not(test), feature = "bls"))]
    pub(in crate::sumeragi) fn verify_certified_serve_terminal_replay(
        &mut self,
        authorization: CertifiedServeTerminalReplayAuthorizationV1,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        body_readback: DurableCertifiedServeBodyReadbackV1,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServeTerminalReplayFailureV1> {
        if !authorization.authorizes_request(authenticated)
            || !authorization.authorizes_response(response)
            || self.coordinator.active_context() != lifecycle_context(self.verified.context())
        {
            return Err(CertifiedServeTerminalReplayFailureV1::Authorization);
        }
        let request = authenticated.request();
        if request.validate(self.verified.context()).is_err() {
            return Err(CertifiedServeTerminalReplayFailureV1::Authorization);
        }
        let request_digest = digest_from_bytes(authenticated.request_hash().as_ref());
        let certificate_digest = digest_from_bytes(HashOf::new(&request.certificate).as_ref());
        let response_digest = digest_from_bytes(authorization.response_hash.as_ref());
        let expected_payload = DurablePayloadReference::CertifiedServeCompleted {
            request: request_digest,
            certificate: certificate_digest,
            response: response_digest,
        };
        let expected_key = lifecycle_key(
            self.verified.context(),
            request.certificate.round,
            Some(request.round),
            Some(certified_serve_key_subject(
                request.subject,
                authenticated.request_hash(),
            )),
            LifecyclePhase::Serve,
            Some(execution_commitment(
                request.certificate.execution_commitment,
            )),
        );
        let (Some(record), Some(metadata)) = (
            self.coordinator.records.get(&authorization.ordinal),
            self.coordinator.durable_records.get(&authorization.ordinal),
        ) else {
            return Err(CertifiedServeTerminalReplayFailureV1::Authorization);
        };
        if authorization.key != expected_key
            || record.key != authorization.key
            || record.ordinal != authorization.ordinal
            || record.owner.first_admission_ordinal() != authorization.ordinal
            || record.owner.causal_root().digest() != request_digest
            || record.work_class != LifecycleWorkClass::CertifiedServe
            || record.stage.kind() != LifecycleStageKind::CertifiedServe
            || record.state
                != super::LifecycleState::Terminal(TerminalOutcome::Completed(Some(
                    response_digest,
                )))
            || metadata.reconstruction_source != request_digest
            || metadata.payload != expected_payload
            || !metadata.replay_authority.structurally_matches_record(
                self.coordinator.active_context(),
                record.key,
                record.work_class,
                record.stage,
                metadata.payload,
            )
        {
            return Err(CertifiedServeTerminalReplayFailureV1::Authorization);
        }
        if self.body_store.is_some() {
            return Err(CertifiedServeTerminalReplayFailureV1::BodyStoreUnavailable);
        }
        let Some(body_store_identity) = self.body_store_identity.as_ref() else {
            return Err(CertifiedServeTerminalReplayFailureV1::BodyStoreUnavailable);
        };
        let receipt = self
            .payload_store
            .persist_completed_with_worker_readback(
                authenticated,
                body_readback,
                body_store_identity,
                response,
            )
            .map_err(|_| CertifiedServeTerminalReplayFailureV1::PayloadStore)?;
        if receipt.response_hash() != authorization.response_hash {
            return Err(CertifiedServeTerminalReplayFailureV1::PayloadStore);
        }
        Ok(())
    }
    /// Persist and publish one exact typed negative Certified-Serve terminal.
    ///
    /// The retained payload store derives the opaque request id from the
    /// authenticated request. No caller-supplied id or terminal receipt is
    /// accepted.
    pub(in crate::sumeragi) fn settle_certified_serve_negative(
        &mut self,
        lease: super::TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        outcome: CertifiedServePayloadNegativeOutcome,
    ) -> Result<(), CertifiedServeTerminalSettlementErrorV1> {
        self.preflight_certified_serve_terminal(&lease, authenticated)?;
        let receipt = match self
            .payload_store
            .persist_negative_for_authenticated_request(authenticated, outcome)
        {
            Ok(receipt) => receipt,
            Err(
                CertifiedServeTerminalPersistenceError::InputRejected(_)
                | CertifiedServeTerminalPersistenceError::StoreInvariant(_)
                | CertifiedServeTerminalPersistenceError::PublicationAmbiguous(_),
            ) => {
                return Err(self.certified_serve_terminal_restart(
                    CertifiedServeTerminalSettlementFailureV1::PayloadStore,
                    lease,
                    None,
                    None,
                ));
            }
        };
        self.publish_certified_serve_terminal(
            lease,
            authenticated,
            DurableCertifiedServeTerminalPublicationV1::Negative(receipt),
        )
    }
    fn preflight_certified_serve_terminal(
        &mut self,
        lease: &super::TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<(), CertifiedServeTerminalSettlementErrorV1> {
        if self.coordinator.fault.is_some() || self.coordinator.ledger_store.is_none() {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
                None,
                None,
            ));
        }
        if self.coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class != LifecycleWorkClass::CertifiedServe
        {
            return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
            ));
        }
        if !self
            .coordinator
            .records
            .get(&lease.ordinal)
            .is_some_and(|record| {
                record.owner == lease.owner
                    && record.state == super::LifecycleState::Claimed(lease.id)
            })
        {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
                None,
                None,
            ));
        }
        let Some(producer_ordinal) = self.coordinator.producer_debts.get(&lease.ordinal).copied()
        else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
                None,
                None,
            ));
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            self.coordinator.records.get(&lease.ordinal),
            self.coordinator.durable_records.get(&lease.ordinal),
            self.coordinator.records.get(&producer_ordinal),
            self.coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
                None,
                None,
            ));
        };
        if serve.ordinal.checked_add(1) != Some(producer.ordinal)
            || serve.owner != producer.owner
            || serve.work_class != LifecycleWorkClass::CertifiedServe
            || producer.work_class != LifecycleWorkClass::ProducerTurn
            || !super::schema::serve_and_producer_keys_match(serve.key, producer.key)
            || !serve_metadata
                .replay_authority
                .same_persisted_family(&producer_metadata.replay_authority)
        {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Coordinator,
                lease.clone(),
                None,
                None,
            ));
        }
        if !self
            .registry
            .registry_mut()
            .preflight_certified_serve_terminal_owner_state(&self.coordinator, lease)
        {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Registry,
                lease.clone(),
                None,
                None,
            ));
        }
        let request_is_exact = serve_metadata
            .replay_authority
            .exactly_matches_certified_serve_request(authenticated)
            && producer_metadata
                .replay_authority
                .exactly_matches_certified_serve_request(authenticated);
        if !request_is_exact {
            return Err(CertifiedServeTerminalSettlementErrorV1::prepublication(
                CertifiedServeTerminalSettlementFailureV1::RequestAuthority,
                lease.clone(),
            ));
        }
        if !self
            .registry
            .registry_mut()
            .preflight_certified_serve_terminal_settlement(&self.coordinator, lease, authenticated)
        {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Registry,
                lease.clone(),
                None,
                None,
            ));
        }
        Ok(())
    }
    fn publish_certified_serve_terminal(
        &mut self,
        lease: super::TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        publication: DurableCertifiedServeTerminalPublicationV1,
    ) -> Result<(), CertifiedServeTerminalSettlementErrorV1> {
        let Some(producer_ordinal) = self.coordinator.producer_debts.get(&lease.ordinal).copied()
        else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::TerminalAuthority,
                lease,
                Some(publication),
                None,
            ));
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            self.coordinator.records.get(&lease.ordinal),
            self.coordinator.durable_records.get(&lease.ordinal),
            self.coordinator.records.get(&producer_ordinal),
            self.coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::TerminalAuthority,
                lease,
                Some(publication),
                None,
            ));
        };
        let terminal = match publication {
            DurableCertifiedServeTerminalPublicationV1::Completed(receipt) => {
                CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                    self.coordinator.active_context,
                    serve,
                    serve_metadata,
                    producer,
                    producer_metadata,
                    receipt,
                )
            }
            DurableCertifiedServeTerminalPublicationV1::Negative(receipt) => {
                CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
                    self.coordinator.active_context,
                    serve,
                    serve_metadata,
                    producer,
                    producer_metadata,
                    receipt,
                )
            }
        };
        let Some(terminal) = terminal else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::TerminalAuthority,
                lease,
                Some(publication),
                None,
            ));
        };
        let transition = self
            .registry
            .registry_mut()
            .prepare_certified_serve_terminal_transition(
                &self.coordinator,
                &lease,
                authenticated,
                &terminal,
            );
        let Some(transition) = transition else {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::Registry,
                lease,
                Some(publication),
                None,
            ));
        };
        let outcome = terminal.terminal_outcome();
        let mut staged = self.coordinator.stage_durable_transaction();
        staged.reduce_settle_turn(
            lease.clone(),
            super::TurnOutcome::Terminal(outcome),
            Some(terminal),
        );
        if staged.fault.is_some() {
            return Err(self.certified_serve_terminal_restart(
                CertifiedServeTerminalSettlementFailureV1::TerminalAuthority,
                lease,
                Some(publication),
                Some(transition),
            ));
        }
        let publication_result = self
            .registry
            .registry_mut()
            .publish_certified_serve_terminal_transition(
                transition,
                &self.coordinator,
                &staged,
                &lease,
                || self.coordinator.persist_exact_staged_successor(&staged),
            );
        match publication_result {
            Ok(()) => {
                self.coordinator = staged;
                Ok(())
            }
            Err(CertifiedServeTerminalRegistryPublicationError::Preflight(transition)) => Err(self
                .certified_serve_terminal_restart(
                    CertifiedServeTerminalSettlementFailureV1::Registry,
                    lease,
                    Some(publication),
                    Some(transition),
                )),
            Err(CertifiedServeTerminalRegistryPublicationError::Publication(_, transition)) => {
                Err(self.certified_serve_terminal_restart(
                    CertifiedServeTerminalSettlementFailureV1::Ledger,
                    lease,
                    Some(publication),
                    Some(transition),
                ))
            }
        }
    }
    fn certified_serve_terminal_restart(
        &mut self,
        failure: CertifiedServeTerminalSettlementFailureV1,
        lease: super::TurnLease,
        publication: Option<DurableCertifiedServeTerminalPublicationV1>,
        transition: Option<PreparedCertifiedServeTerminalRegistryTransitionV1>,
    ) -> CertifiedServeTerminalSettlementErrorV1 {
        if self.coordinator.fault.is_none() {
            self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
        }
        CertifiedServeTerminalSettlementErrorV1::requiring_restart(
            CertifiedServeTerminalSettlementRestartV1 {
                failure,
                _lease: lease,
                _publication: publication,
                _transition: transition,
            },
        )
    }
}
impl super::ProductionLifecycleOwnerV1 {
    /// Persist and publish the exact `Advanced` terminal for one attempted
    /// ProducerTurn. The move-only claim is the only accepted authority; no
    /// ordinal, lease, digest, or legacy worker episode enters this boundary.
    pub(in crate::sumeragi) fn settle_producer_turn_advanced(
        &mut self,
        attempted: AttemptedProducerTurnV1,
    ) -> Result<(), ProducerTurnTerminalSettlementErrorV1> {
        if self.coordinator.fault.is_some() || self.coordinator.ledger_store.is_none() {
            return Err(self.producer_turn_terminal_restart(
                ProducerTurnTerminalSettlementFailureV1::Coordinator,
                attempted,
                None,
            ));
        }
        let Some(transition) = self
            .registry
            .registry()
            .prepare_producer_turn_terminal_transition(
                &self.verified,
                &self.coordinator,
                &attempted,
            )
        else {
            return Err(self.producer_turn_terminal_restart(
                ProducerTurnTerminalSettlementFailureV1::Registry,
                attempted,
                None,
            ));
        };
        let mut staged = self.coordinator.stage_durable_transaction();
        staged.reduce_settle_turn(
            attempted.claimed().lease().clone(),
            super::TurnOutcome::Advanced,
            None,
        );
        if staged.fault.is_some() {
            return Err(self.producer_turn_terminal_restart(
                ProducerTurnTerminalSettlementFailureV1::Coordinator,
                attempted,
                Some(transition),
            ));
        }
        let publication = self
            .registry
            .registry_mut()
            .publish_producer_turn_terminal_transition(
                transition,
                &self.verified,
                &self.coordinator,
                &staged,
                &attempted,
                || self.coordinator.persist_exact_staged_successor(&staged),
            );
        match publication {
            Ok(()) => {
                self.coordinator = staged;
                Ok(())
            }
            Err(ProducerTurnTerminalRegistryPublicationError::Preflight(transition)) => Err(self
                .producer_turn_terminal_restart(
                    ProducerTurnTerminalSettlementFailureV1::Registry,
                    attempted,
                    Some(transition),
                )),
            Err(ProducerTurnTerminalRegistryPublicationError::Publication(_, transition)) => {
                Err(self.producer_turn_terminal_restart(
                    ProducerTurnTerminalSettlementFailureV1::Ledger,
                    attempted,
                    Some(transition),
                ))
            }
        }
    }

    fn producer_turn_terminal_restart(
        &mut self,
        failure: ProducerTurnTerminalSettlementFailureV1,
        attempted: AttemptedProducerTurnV1,
        transition: Option<PreparedProducerTurnTerminalRegistryTransitionV1>,
    ) -> ProducerTurnTerminalSettlementErrorV1 {
        if self.coordinator.fault.is_none() {
            self.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure);
        }
        ProducerTurnTerminalSettlementErrorV1 {
            failure,
            _attempted: attempted,
            _transition: transition,
        }
    }
}
impl super::LifecycleCoordinator {
    /// Durably retain and atomically admit one authenticated Certified-Serve
    /// request.
    ///
    /// Payload publication necessarily precedes ledger publication so restart
    /// can always resolve a durable Serve row. Capacity-fenced requests retain
    /// one payload owned by their bounded admission-wait entry; conclusive
    /// rejections synchronously remove the exact pending payload again. A ledger
    /// durability failure retains the payload as an authenticated crash tail
    /// for startup reconciliation and latches the coordinator fault.
    #[cfg(test)]
    pub(crate) fn persist_and_admit_certified_serve(
        &mut self,
        payload_store: &mut CertifiedServePayloadStoreV1,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<super::AdmissionDecision, CertifiedServeAdmissionBoundaryError> {
        if self.active_context() != lifecycle_context(verified.context()) {
            return Err(CertifiedServeAdmissionBoundaryError::Projection(
                CertifiedServeAdmissionError::ForeignContext,
            ));
        }
        if let Some(fault) = self.fault() {
            return Ok(super::AdmissionDecision::FailClosed(fault));
        }
        let publication = match payload_store.retain_for_admission_with_verified_retention(
            verified,
            local_signer,
            authenticated,
        ) {
            Ok(publication) => publication,
            Err(CertifiedServePayloadRetentionError::Unchanged(error)) => {
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(error));
            }
            Err(CertifiedServePayloadRetentionError::PublicationAmbiguous(error)) => {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(error));
            }
        };
        let receipt = publication.receipt();
        if !publication.is_pending() {
            if !publication.exactly_matches_authenticated_request(authenticated) {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(
                    CertifiedServePayloadStoreError::TerminalConflict,
                ));
            }
            let outcome = match publication.state() {
                DurableCertifiedServeAdmissionStateV1::Pending => unreachable!(
                    "non-pending Certified-Serve publication cannot retain Pending state"
                ),
                DurableCertifiedServeAdmissionStateV1::Completed(response) => {
                    TerminalOutcome::Completed(Some(digest_from_bytes(response.as_ref())))
                }
                DurableCertifiedServeAdmissionStateV1::Negative(
                    CertifiedServePayloadNegativeOutcome::Cancelled,
                ) => TerminalOutcome::Cancelled,
                DurableCertifiedServeAdmissionStateV1::Negative(
                    CertifiedServePayloadNegativeOutcome::Rejected(code),
                ) => TerminalOutcome::Rejected(code),
                DurableCertifiedServeAdmissionStateV1::Negative(
                    CertifiedServePayloadNegativeOutcome::Failed(code),
                ) => TerminalOutcome::Failed(code),
            };
            let terminal = self.records.iter().find_map(|(ordinal, record)| {
                let metadata = self.durable_records.get(ordinal)?;
                (record.work_class == LifecycleWorkClass::CertifiedServe
                    && record.state == super::LifecycleState::Terminal(outcome)
                    && metadata
                        .payload
                        .matches_terminal(record.work_class, Some(outcome))
                    && metadata
                        .replay_authority
                        .exactly_matches_certified_serve_publication(authenticated, receipt))
                .then_some(record.owner)
            });
            let Some(owner) = terminal else {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(
                    CertifiedServePayloadStoreError::OrphanTerminalPayload,
                ));
            };
            return Ok(match outcome {
                TerminalOutcome::Completed(Some(_)) => {
                    super::AdmissionDecision::ReplayTerminal { owner, outcome }
                }
                TerminalOutcome::Advanced | TerminalOutcome::Completed(None) => unreachable!(
                    "Certified-Serve terminal publication cannot encode an unbound completion"
                ),
                TerminalOutcome::Cancelled
                | TerminalOutcome::Rejected(_)
                | TerminalOutcome::Failed(_) => super::AdmissionDecision::StutterTerminal { owner },
            });
        }
        let request = match certified_serve_admission_request(
            self.active_context(),
            verified,
            authenticated,
            receipt,
        ) {
            Ok(request) => request,
            Err(error) => {
                if !publication.is_pending() {
                    self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                    return Err(CertifiedServeAdmissionBoundaryError::Projection(error));
                }
                if let Err(rollback) = payload_store.rollback_pending(receipt) {
                    self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                    return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(rollback));
                }
                return Err(CertifiedServeAdmissionBoundaryError::Projection(error));
            }
        };
        let AdmissionRequest::Candidate(candidate) = &request else {
            unreachable!("Certified-Serve projection always yields one candidate")
        };
        let candidate_key = candidate.key;
        let decision = self.admit(request);
        if matches!(decision, super::AdmissionDecision::WaitForCapacity(_)) {
            let attached = self
                .admission_waits
                .get_mut(&candidate_key)
                .is_some_and(|waiting| match waiting.serve_payload_receipt {
                    Some(existing) => existing == receipt,
                    None => {
                        waiting.serve_payload_receipt = Some(receipt);
                        true
                    }
                });
            if !attached {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Ok(super::AdmissionDecision::FailClosed(
                    super::CoordinatorFault::DurabilityFailure,
                ));
            }
        }
        if matches!(
            decision,
            super::AdmissionDecision::Rejected(_) | super::AdmissionDecision::NonCandidate
        ) {
            if !publication.is_pending() {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(
                    CertifiedServePayloadStoreError::TerminalConflict,
                ));
            }
            if let Err(error) = payload_store.rollback_pending(receipt) {
                self.fault = Some(super::CoordinatorFault::DurabilityFailure);
                return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(error));
            }
        }
        Ok(decision)
    }
}
/// Complete recovery projection for one authenticated Certified-Serve payload.
///
/// A Pending frame projects a Pending candidate; a terminal frame projects
/// the exact terminal payload and replay authority. The optional terminal cut
/// additionally describes a payload store which may be ahead of the ledger.
pub(super) struct RecoveredCertifiedServeProjection {
    candidate: CandidateAdmission,
    replay: CertifiedServeReplayEvidencePairV1,
    resolved_payload: DurablePayloadReference,
    terminal_outcome: Option<TerminalOutcome>,
    terminal_replay: Option<CertifiedServeTerminalReplayAuthorityPairV1>,
}
impl RecoveredCertifiedServeProjection {
    /// Consume the sealed projection into coordinator- and registry-owned
    /// recovery parts.
    pub(super) fn into_registry_parts(
        self,
    ) -> (
        CandidateAdmission,
        DurablePayloadReference,
        Option<TerminalOutcome>,
        Option<CertifiedServeTerminalReplayAuthorityPairV1>,
        CertifiedServeReplayEvidencePairV1,
    ) {
        (
            self.candidate,
            self.resolved_payload,
            self.terminal_outcome,
            self.terminal_replay,
            self.replay,
        )
    }
    /// Consume a test projection without exporting its runtime-only replay pair.
    #[cfg(test)]
    pub(super) fn into_parts(
        self,
    ) -> (
        CandidateAdmission,
        DurablePayloadReference,
        Option<TerminalOutcome>,
        Option<CertifiedServeTerminalReplayAuthorityPairV1>,
    ) {
        (
            self.candidate,
            self.resolved_payload,
            self.terminal_outcome,
            self.terminal_replay,
        )
    }
}
/// One exact post-fsync Certified-Serve candidate kept inseparable from the
/// common replay family required by both adjacent concrete carriers.
#[must_use = "the prepared Certified-Serve admission still owns its replay family"]
pub(super) struct PreparedCertifiedServeAdmissionV1 {
    candidate: CandidateAdmission,
    replay: CertifiedServeReplayEvidencePairV1,
}
impl PreparedCertifiedServeAdmissionV1 {
    /// Consume the closed projection at the coordinator/registry transaction.
    pub(super) fn into_candidate_and_replay(
        self,
    ) -> (CandidateAdmission, CertifiedServeReplayEvidencePairV1) {
        (self.candidate, self.replay)
    }
}
#[derive(Clone, Copy)]
struct ProjectedShape {
    key: LifecycleKey,
    work_class: LifecycleWorkClass,
    stage_kind: LifecycleStageKind,
}
/// Project one authenticated, durably retained request into an atomic
/// Certified-Serve/ProducerTurn admission.
///
/// # Errors
///
/// Returns an error when the verified episode is foreign, the signed request
/// is structurally inconsistent, or the post-fsync receipt names different
/// request or certificate bytes.
#[cfg(test)]
pub(super) fn certified_serve_admission_request(
    active_context: LifecycleContext,
    verified: &VerifiedHeightContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    receipt: DurableCertifiedServeAdmissionReceipt,
) -> Result<AdmissionRequest, CertifiedServeAdmissionError> {
    prepare_certified_serve_admission(active_context, verified, authenticated, receipt).map(
        |prepared| {
            let (candidate, _replay) = prepared.into_candidate_and_replay();
            AdmissionRequest::Candidate(candidate)
        },
    )
}
/// Close one authenticated, post-fsync request over the candidate and the
/// opaque replay family that must enter both concrete registry rows.
pub(super) fn prepare_certified_serve_admission(
    active_context: LifecycleContext,
    verified: &VerifiedHeightContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    receipt: DurableCertifiedServeAdmissionReceipt,
) -> Result<PreparedCertifiedServeAdmissionV1, CertifiedServeAdmissionError> {
    let context = verified.context();
    if lifecycle_context(context) != active_context {
        return Err(CertifiedServeAdmissionError::ForeignContext);
    }
    authenticated
        .request()
        .validate(context)
        .map_err(|_| CertifiedServeAdmissionError::InvalidRequest)?;
    let request_hash = authenticated.request_hash();
    if receipt.id().request_hash() != request_hash
        || receipt.certificate_hash() != HashOf::new(&authenticated.request().certificate)
    {
        return Err(CertifiedServeAdmissionError::ReceiptMismatch);
    }
    let replay = CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
        active_context,
        authenticated,
        receipt,
    )
    .ok_or(CertifiedServeAdmissionError::ReceiptMismatch)?;
    let candidate = certified_serve_candidate(active_context, authenticated, &replay)?;
    Ok(PreparedCertifiedServeAdmissionV1 { candidate, replay })
}
/// Reconstruct one authenticated payload-store record into its exact
/// admission candidate, resolved durable payload, and optional terminal cut.
///
/// # Errors
///
/// Returns an error when the authenticated record no longer projects into the
/// active lifecycle context or its request/certificate coordinates drifted.
pub(super) fn recovered_certified_serve_projection(
    active_context: LifecycleContext,
    recovered: &AuthenticatedRecoveredCertifiedServePayload,
) -> Result<RecoveredCertifiedServeProjection, CertifiedServeAdmissionError> {
    let authenticated = recovered.request();
    let replay =
        CertifiedServeReplayEvidencePairV1::from_authenticated_recovery(active_context, recovered)
            .ok_or(CertifiedServeAdmissionError::ReceiptMismatch)?;
    let mut candidate = certified_serve_candidate(active_context, authenticated, &replay)?;
    let request = digest_from_bytes(authenticated.request_hash().as_ref());
    let certificate = digest_from_bytes(recovered.certificate_hash().as_ref());
    let (resolved_payload, terminal_outcome) = match recovered.state() {
        AuthenticatedRecoveredCertifiedServePayloadState::Pending => (
            DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            },
            None,
        ),
        AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) => {
            let response = digest_from_bytes(completed.response_hash().as_ref());
            (
                DurablePayloadReference::CertifiedServeCompleted {
                    request,
                    certificate,
                    response,
                },
                Some(TerminalOutcome::Completed(Some(response))),
            )
        }
        AuthenticatedRecoveredCertifiedServePayloadState::Negative(outcome) => {
            let (outcome, terminal) = match outcome {
                CertifiedServePayloadNegativeOutcome::Cancelled => (
                    DurableServeNegativeOutcome::Cancelled,
                    TerminalOutcome::Cancelled,
                ),
                CertifiedServePayloadNegativeOutcome::Rejected(code) => (
                    DurableServeNegativeOutcome::Rejected(*code),
                    TerminalOutcome::Rejected(*code),
                ),
                CertifiedServePayloadNegativeOutcome::Failed(code) => (
                    DurableServeNegativeOutcome::Failed(*code),
                    TerminalOutcome::Failed(*code),
                ),
            };
            (
                DurablePayloadReference::CertifiedServeNegative {
                    request,
                    certificate,
                    outcome,
                },
                Some(terminal),
            )
        }
    };
    let terminal_replay = terminal_outcome
        .map(|outcome| {
            CertifiedServeTerminalReplayAuthorityPairV1::from_authenticated_recovery(
                active_context,
                recovered,
                &candidate,
                resolved_payload,
                outcome,
            )
            .ok_or(CertifiedServeAdmissionError::ReceiptMismatch)
        })
        .transpose()?;
    if terminal_replay
        .as_ref()
        .is_some_and(|replay| !replay.bind_recovered_candidate(active_context, &mut candidate))
    {
        return Err(CertifiedServeAdmissionError::ReceiptMismatch);
    }
    Ok(RecoveredCertifiedServeProjection {
        candidate,
        replay,
        resolved_payload,
        terminal_outcome,
        terminal_replay,
    })
}
fn certified_serve_candidate(
    active_context: LifecycleContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    replay: &CertifiedServeReplayEvidencePairV1,
) -> Result<CandidateAdmission, CertifiedServeAdmissionError> {
    let request = authenticated.request();
    let request_hash = authenticated.request_hash();
    let certificate = &request.certificate;
    if request_hash != HashOf::new(request)
        || digest_from_bytes(request.round.context_id.0.as_ref()) != active_context.id()
        || request.round.height != active_context.height()
        || certificate.round.context_id != request.round.context_id
        || certificate.round.height != request.round.height
        || certificate.proposal_round != request.round
        || certificate.subject != request.subject
    {
        return Err(CertifiedServeAdmissionError::InvalidRequest);
    }
    replay
        .admission_candidate(active_context)
        .ok_or(CertifiedServeAdmissionError::InvalidRequest)
}
/// Project exact runtime coordinates without attaching replay authority.
///
/// The returned value is inert. Closed replay evidence must perform the final
/// candidate construction; this function accepts no decoded authority and has
/// no format-default or fallback path.
pub(super) fn authority_free_admission_projection(
    active_context: LifecycleContext,
    verified: &VerifiedHeightContext,
    effect: &AdapterEffect,
    binding: &PendingRuntimeEffectBinding,
) -> Result<AuthorityFreeAdmissionProjection, AdapterEffectAdmissionError> {
    if !binding.exactly_binds_adapter_effect(effect) {
        return Err(AdapterEffectAdmissionError::UnboundEffect);
    }
    let context = verified.context();
    if lifecycle_context(context) != active_context {
        return Err(AdapterEffectAdmissionError::ForeignContext);
    }
    let shape = project_shape(context, effect, binding.candidate_statement())?;
    if shape.key.context() != active_context.id()
        || shape.key.round().height() != active_context.height()
        || shape
            .key
            .proposal_round()
            .is_some_and(|round| round.height() != active_context.height())
    {
        return Err(AdapterEffectAdmissionError::ForeignContext);
    }
    let causal_root = pending_effect_causal_root(binding);
    let causal_digest = causal_root.digest();
    let physical_digest = digest_from_hash(binding.exact_effect_identity());
    let slot_id = PhysicalSlotId::for_capacity(shape.work_class.capacity_class(), 0);
    Ok(AuthorityFreeAdmissionProjection {
        key: shape.key,
        causal_root,
        work_class: shape.work_class,
        stage: LifecycleStage::new(shape.stage_kind, PredecessorScope::Independent),
        initial_state: InitialLifecycleState::Ready,
        reconstruction_source: causal_digest,
        physical_geometry: PhysicalGeometry::new(
            [PhysicalSlot::new(slot_id, physical_digest)],
            [slot_id],
        ),
    })
}
fn project_shape(
    context: &wire::HeightContext,
    effect: &AdapterEffect,
    inherited: Option<RuntimeCandidateSemanticStatement>,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    match effect {
        AdapterEffect::Sign { tag, request } => project_sign(context, *tag, request),
        AdapterEffect::Broadcast(message) => project_broadcast(context, message),
        AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest,
            certified_sources,
            certificate,
        } => project_fetch(
            context,
            *tag,
            *round,
            *subject,
            manifest.as_ref(),
            certified_sources,
            certificate.as_ref(),
        ),
        AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } => project_inherited_body_stage(
            context,
            *tag,
            *round,
            *subject,
            inherited,
            LifecyclePhase::Store,
            LifecycleWorkClass::Store,
            LifecycleStageKind::StoreBody,
        ),
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => project_inherited_body_stage(
            context,
            *tag,
            *round,
            *subject,
            inherited,
            LifecyclePhase::Validate,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
        ),
        AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } => project_apply(context, *tag, *subject, certificate),
        AdapterEffect::EnterView {
            tag,
            certificate,
            protected_lock,
        } => project_enter_view(context, *tag, certificate, protected_lock.as_ref()),
        AdapterEffect::ReportEquivocation { evidence } => {
            evidence
                .validate_structure(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            let round = evidence.round();
            validate_round(context, round)?;
            let (phase, stage_kind) = match evidence.kind() {
                EquivocationKind::Proposal => (
                    LifecyclePhase::DiagnosticProposalEquivocation,
                    LifecycleStageKind::ReportProposalEquivocation,
                ),
                EquivocationKind::Vote => (
                    LifecyclePhase::DiagnosticVoteEquivocation,
                    LifecycleStageKind::ReportVoteEquivocation,
                ),
                EquivocationKind::Timeout => (
                    LifecyclePhase::DiagnosticTimeoutEquivocation,
                    LifecycleStageKind::ReportTimeoutEquivocation,
                ),
            };
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    round,
                    None,
                    Some(equivocation_subject(evidence)),
                    phase,
                    None,
                ),
                work_class: LifecycleWorkClass::EquivocationReport,
                stage_kind,
            })
        }
        AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } => {
            certificate
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            if certificate.phase != wire::GlobalPhase::Prepare || certificate.subject != *subject {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    certificate.round,
                    Some(certificate.proposal_round),
                    Some(block_subject(*subject)),
                    LifecyclePhase::DiagnosticInvalidBody,
                    Some(execution_commitment(certificate.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::InvalidBodyReport,
                stage_kind: LifecycleStageKind::ReportInvalidBody,
            })
        }
    }
}
fn project_sign(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    request: &SignRequest,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    match request {
        SignRequest::Proposal(proposal) => {
            if !proposal.signature.is_empty() {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            let mut structural = proposal.clone();
            structural.signature.push(1);
            structural
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            validate_tag_for_round(context, tag, proposal.round)?;
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    proposal.round,
                    Some(proposal.round),
                    Some(block_subject(proposal.subject)),
                    LifecyclePhase::Proposal,
                    None,
                ),
                work_class: LifecycleWorkClass::SignProposal,
                stage_kind: LifecycleStageKind::SignProposal,
            })
        }
        SignRequest::Vote(vote) => {
            if !vote.signature.is_empty() {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            let mut structural = vote.clone();
            structural.signature.push(1);
            structural
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            validate_tag_for_round(context, tag, vote.round)?;
            let (phase, stage_kind) = match vote.phase {
                wire::GlobalPhase::Prepare => {
                    (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
                }
                wire::GlobalPhase::Commit => {
                    (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
                }
            };
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    vote.round,
                    Some(vote.proposal_round),
                    Some(block_subject(vote.subject)),
                    phase,
                    Some(execution_commitment(vote.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::SignVote,
                stage_kind,
            })
        }
        SignRequest::TimeoutVote(vote) => {
            if !vote.signature.is_empty() {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            let mut structural = vote.clone();
            structural.signature.push(1);
            structural
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            validate_tag_for_round(context, tag, vote.round)?;
            let highest = vote.highest_prepare_qc.as_ref();
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    vote.round,
                    highest.map(|qc| qc.proposal_round),
                    highest.map(|qc| block_subject(qc.subject)),
                    LifecyclePhase::Timeout,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::SignTimeout,
                stage_kind: LifecycleStageKind::SignTimeoutVote,
            })
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn project_fetch(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest: Option<&wire::PayloadManifest>,
    certified_sources: &[iroha_data_model::peer::PeerId],
    certificate: Option<&wire::QuorumCertificate>,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    validate_tag_for_round(context, tag, round)?;
    if let Some(manifest) = manifest {
        manifest
            .validate(context)
            .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
        if manifest.round != round || manifest.subject != subject {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
    }
    let (key_round, proposal_round, authority) = match certificate {
        None => {
            if manifest.is_none() || !certified_sources.is_empty() {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            (round, round, None)
        }
        Some(certificate) => {
            certificate
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            if certificate.proposal_round != round
                || certificate.subject != subject
                || !certified_sources
                    .iter()
                    .eq(context.roster.iter().map(|entry| &entry.validator))
            {
                return Err(AdapterEffectAdmissionError::InvalidCarrier);
            }
            (
                certificate.round,
                certificate.proposal_round,
                Some((certificate.phase, certificate.execution_commitment)),
            )
        }
    };
    let key = match authority {
        None => lifecycle_key(
            context,
            key_round,
            Some(proposal_round),
            Some(block_subject(subject)),
            LifecyclePhase::Fetch,
            None,
        ),
        Some((phase, commitment)) => certified_fetch_lifecycle_key(
            lifecycle_context(context),
            key_round,
            proposal_round,
            subject,
            phase,
            commitment,
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
    };
    Ok(ProjectedShape {
        key,
        work_class: LifecycleWorkClass::Fetch,
        stage_kind: LifecycleStageKind::FetchBody,
    })
}
#[allow(clippy::too_many_arguments)]
fn project_inherited_body_stage(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    effect_round: wire::ConsensusRound,
    effect_subject: wire::BlockSubject,
    inherited: Option<RuntimeCandidateSemanticStatement>,
    phase: LifecyclePhase,
    work_class: LifecycleWorkClass,
    stage_kind: LifecycleStageKind,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    let inherited = inherited.ok_or(AdapterEffectAdmissionError::MissingInheritedStatement)?;
    validate_round(context, inherited.round())?;
    validate_round(context, inherited.proposal_round())?;
    if inherited.context_id() != context.id()
        || inherited.proposal_round() != effect_round
        || inherited.subject() != Some(effect_subject)
        || inherited.phase().is_some() != inherited.execution_commitment().is_some()
        || !matches!(
            inherited.phase(),
            None | Some(wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)
        )
    {
        return Err(AdapterEffectAdmissionError::InvalidCarrier);
    }
    validate_tag_for_round(context, tag, inherited.round())?;
    Ok(ProjectedShape {
        key: lifecycle_key(
            context,
            inherited.round(),
            Some(inherited.proposal_round()),
            Some(block_subject(effect_subject)),
            phase,
            inherited.execution_commitment().map(execution_commitment),
        ),
        work_class,
        stage_kind,
    })
}
fn project_apply(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    subject: wire::BlockSubject,
    certificate: &wire::QuorumCertificate,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    certificate
        .validate(context)
        .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
    if certificate.phase != wire::GlobalPhase::Commit || certificate.subject != subject {
        return Err(AdapterEffectAdmissionError::InvalidCarrier);
    }
    validate_tag_for_round(context, tag, certificate.round)?;
    Ok(ProjectedShape {
        key: lifecycle_key(
            context,
            certificate.round,
            Some(certificate.proposal_round),
            Some(block_subject(subject)),
            LifecyclePhase::Apply,
            Some(execution_commitment(certificate.execution_commitment)),
        ),
        work_class: LifecycleWorkClass::Apply,
        stage_kind: LifecycleStageKind::ApplyDecision,
    })
}
fn project_broadcast(
    context: &wire::HeightContext,
    message: &wire::ConsensusMessageV2,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    message
        .validate_version()
        .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
    match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            proposal
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    proposal.round,
                    Some(proposal.round),
                    Some(block_subject(proposal.subject)),
                    LifecyclePhase::BroadcastProposal,
                    None,
                ),
                work_class: LifecycleWorkClass::Broadcast,
                stage_kind: LifecycleStageKind::BroadcastProposal,
            })
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            vote.validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            let (phase, stage_kind) = match vote.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareVote,
                    LifecycleStageKind::BroadcastPrepareVote,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitVote,
                    LifecycleStageKind::BroadcastCommitVote,
                ),
            };
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    vote.round,
                    Some(vote.proposal_round),
                    Some(block_subject(vote.subject)),
                    phase,
                    Some(execution_commitment(vote.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::Broadcast,
                stage_kind,
            })
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            certificate
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            let (phase, stage_kind) = match certificate.phase {
                wire::GlobalPhase::Prepare => (
                    LifecyclePhase::BroadcastPrepareQc,
                    LifecycleStageKind::BroadcastPrepareQc,
                ),
                wire::GlobalPhase::Commit => (
                    LifecyclePhase::BroadcastCommitQc,
                    LifecycleStageKind::BroadcastCommitQc,
                ),
            };
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    certificate.round,
                    Some(certificate.proposal_round),
                    Some(block_subject(certificate.subject)),
                    phase,
                    Some(execution_commitment(certificate.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::Broadcast,
                stage_kind,
            })
        }
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            vote.validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            let highest = vote.highest_prepare_qc.as_ref();
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    vote.round,
                    highest.map(|qc| qc.proposal_round),
                    highest.map(|qc| block_subject(qc.subject)),
                    LifecyclePhase::BroadcastTimeoutVote,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::Broadcast,
                stage_kind: LifecycleStageKind::BroadcastTimeoutVote,
            })
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            certificate
                .validate(context)
                .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
            let highest = certificate.highest_prepare_qc();
            Ok(ProjectedShape {
                key: lifecycle_key(
                    context,
                    certificate.round,
                    highest.map(|qc| qc.proposal_round),
                    Some(timeout_certificate_envelope_subject(certificate)),
                    LifecyclePhase::BroadcastTc,
                    highest.map(|qc| execution_commitment(qc.execution_commitment)),
                ),
                work_class: LifecycleWorkClass::Broadcast,
                stage_kind: LifecycleStageKind::BroadcastTc,
            })
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => {
            Err(AdapterEffectAdmissionError::UnsupportedBroadcastPayload)
        }
    }
}
fn project_enter_view(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    certificate: &wire::TimeoutCertificate,
    protected_lock: Option<&wire::QuorumCertificate>,
) -> Result<ProjectedShape, AdapterEffectAdmissionError> {
    certificate
        .validate(context)
        .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
    if tag.height() != context.height
        || certificate.round.context_id != context.id()
        || certificate.round.height != context.height
        || certificate.round.view.checked_add(1) != Some(tag.view())
    {
        return Err(AdapterEffectAdmissionError::InvalidCarrier);
    }
    if let Some(protected) = protected_lock {
        protected
            .validate(context)
            .map_err(|_| AdapterEffectAdmissionError::InvalidCarrier)?;
        if protected.phase != wire::GlobalPhase::Prepare
            || protected.proposal_round.context_id != context.id()
            || protected.proposal_round.height != context.height
            || protected.proposal_round.view >= tag.view()
        {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
    }
    if let Some(highest) = certificate.highest_prepare_qc() {
        let Some(protected) = protected_lock else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        if protected.round.view < highest.round.view
            || (protected.round.view == highest.round.view
                && (protected.round != highest.round
                    || protected.proposal_round != highest.proposal_round
                    || protected.phase != highest.phase
                    || protected.subject != highest.subject
                    || protected.execution_commitment != highest.execution_commitment))
        {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
    }
    let execution_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    };
    Ok(ProjectedShape {
        key: lifecycle_key(
            context,
            execution_round,
            protected_lock.map(|lock| lock.proposal_round),
            protected_lock.map(|lock| block_subject(lock.subject)),
            LifecyclePhase::EnterView,
            protected_lock.map(|lock| execution_commitment(lock.execution_commitment)),
        ),
        work_class: LifecycleWorkClass::EnterView,
        stage_kind: LifecycleStageKind::EnterView,
    })
}
fn validate_tag_for_round(
    context: &wire::HeightContext,
    tag: crate::sumeragi::v2_core::EventTag,
    round: wire::ConsensusRound,
) -> Result<(), AdapterEffectAdmissionError> {
    validate_round(context, round)?;
    if tag.height() != context.height || tag.view() < round.view {
        return Err(AdapterEffectAdmissionError::InvalidCarrier);
    }
    Ok(())
}
fn validate_round(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
) -> Result<(), AdapterEffectAdmissionError> {
    if round.context_id != context.id() || round.height != context.height {
        return Err(AdapterEffectAdmissionError::ForeignContext);
    }
    Ok(())
}
pub(in crate::sumeragi) fn lifecycle_context(context: &wire::HeightContext) -> LifecycleContext {
    LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height)
}
/// Project one non-forgeable body-store receipt into its LedgerV1 frame reference.
///
/// The returned value binds only the exact fsynced body bytes. Restart must
/// still join the proposal/QC or validation authority appropriate to the row.
pub(super) fn durable_body_frame_reference(
    active_context: LifecycleContext,
    receipt: &DurableBodyReceipt,
) -> Option<DurableBodyFrameReference> {
    let context = digest_from_bytes(receipt.context_id().0.as_ref());
    let round = receipt.round();
    if context != active_context.id() || round.height != active_context.height() {
        return None;
    }
    Some(DurableBodyFrameReference::new(
        context,
        LifecycleRound::new(round.height, round.view),
        block_subject(receipt.subject()),
        digest_from_bytes(receipt.manifest_hash().as_ref()),
        digest_from_hash(&receipt.frame_hash()),
    ))
}
/// Rebind one exact LedgerV1 body-frame reference from an opened body store.
///
/// This performs a complete catalog census rather than inverting the lifecycle
/// subject digest. Exactly one manifest/receipt pair must reproduce the five
/// retained coordinates. The returned seal remains insufficient without the
/// row-specific signed proposal, QC, WAL, or validation authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn authenticate_durable_body_frame_recovery(
    active_context: LifecycleContext,
    store: &V2BodyStore,
    expected: DurableBodyFrameReference,
) -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError> {
    let mut recovered = authenticate_durable_body_frame_catalog(
        active_context,
        expected,
        store.recovery_catalog()?.into_values(),
    )?;
    recovered.genesis_authority = store.authenticate_genesis_authority_frame(&recovered.receipt);
    Ok(recovered)
}
fn authenticate_durable_body_frame_catalog(
    active_context: LifecycleContext,
    expected: DurableBodyFrameReference,
    catalog: impl IntoIterator<Item = (wire::PayloadManifest, DurableBodyReceipt)>,
) -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError> {
    let mut exact = catalog.into_iter().filter(|(manifest, receipt)| {
        manifest.round == receipt.round()
            && manifest.subject == receipt.subject()
            && HashOf::new(manifest) == receipt.manifest_hash()
            && durable_body_frame_reference(active_context, receipt) == Some(expected)
    });
    let Some((manifest, receipt)) = exact.next() else {
        return Err(DurableBodyFrameRecoveryError::Missing);
    };
    if exact.next().is_some() {
        return Err(DurableBodyFrameRecoveryError::Ambiguous);
    }
    Ok(AuthenticatedDurableBodyFrameRecovery {
        reference: expected,
        manifest,
        receipt,
        genesis_authority: None,
    })
}
fn lifecycle_key(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    proposal_round: Option<wire::ConsensusRound>,
    subject: Option<LifecycleDigest>,
    phase: LifecyclePhase,
    execution_commitment: Option<LifecycleDigest>,
) -> LifecycleKey {
    LifecycleKey::new(
        digest_from_bytes(context.id().0.as_ref()),
        LifecycleRound::new(round.height, round.view),
        proposal_round.map(|round| LifecycleRound::new(round.height, round.view)),
        subject,
        phase,
        execution_commitment,
    )
}
/// Derive the lifecycle-domain digest for one exact block subject.
pub(super) fn block_subject(subject: wire::BlockSubject) -> LifecycleDigest {
    domain_digest(BLOCK_SUBJECT_DOMAIN, &subject.encode())
}
/// Derive the lifecycle-domain digest for one exact execution commitment.
pub(super) fn execution_commitment(commitment: wire::ExecutionCommitment) -> LifecycleDigest {
    domain_digest(EXECUTION_COMMITMENT_DOMAIN, &commitment.encode())
}
/// Derive the lifecycle subject for one complete authenticated timeout-certificate envelope.
///
/// A same-round certificate may legitimately acquire a different quorum
/// aggregation while retaining the same highest PrepareQC. Those envelopes
/// require distinct durable output rows so the newer certificate still enters
/// service I/O; exact byte retries retain the same key and stutter.
pub(super) fn timeout_certificate_envelope_subject(
    certificate: &wire::TimeoutCertificate,
) -> LifecycleDigest {
    domain_digest(
        TIMEOUT_CERTIFICATE_ENVELOPE_SUBJECT_DOMAIN,
        &certificate.encode(),
    )
}
/// Derive the logical key shared by certified-Fetch admission and its
/// authenticated late response. Ordinary, uncertified Fetch work is excluded:
/// this helper requires explicit certificate phase and commitment authority.
pub(super) fn certified_fetch_lifecycle_key(
    active_context: LifecycleContext,
    round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    authority_phase: wire::GlobalPhase,
    commitment: wire::ExecutionCommitment,
) -> Option<LifecycleKey> {
    let round_context = digest_from_bytes(round.context_id.0.as_ref());
    let proposal_context = digest_from_bytes(proposal_round.context_id.0.as_ref());
    if round_context != active_context.id()
        || proposal_context != active_context.id()
        || round.height != active_context.height()
        || proposal_round.height != active_context.height()
    {
        return None;
    }
    match authority_phase {
        wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit => {}
    }
    Some(LifecycleKey::new(
        active_context.id(),
        LifecycleRound::new(round.height, round.view),
        Some(LifecycleRound::new(
            proposal_round.height,
            proposal_round.view,
        )),
        Some(block_subject(subject)),
        LifecyclePhase::Fetch,
        Some(execution_commitment(commitment)),
    ))
}
/// Derive the unique external generation source for one exact signed
/// certified-body request. Future Fetch settlement and response wake
/// publication must share this function.
pub(super) fn certified_fetch_wait_source(
    request_hash: HashOf<wire::CertifiedBodyRequest>,
) -> WaitSource {
    WaitSource::External(domain_digest(
        CERTIFIED_FETCH_WAIT_SOURCE_DOMAIN,
        request_hash.as_ref(),
    ))
}
/// Derive the unique external generation source for one exact closed durable
/// Validate carrier.
///
/// This raw projection remains sealed inside the lifecycle module. Its sole
/// caller is the borrow-bound concrete-registry preflight, which supplies the
/// coordinator-minted address, revalidated pending-binding coordinates, exact
/// durable frame hash, independently transferred expected manifest hash, and
/// the immutable lifecycle key/stage accepted before async detachment.
/// Including the inherited statement prevents an in-flight Prepare/Commit
/// authority refinement from sharing wake authority with the old carrier.
#[allow(clippy::too_many_arguments)]
pub(super) fn durable_validation_wait_source(
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    incumbent_digest: LifecycleDigest,
    causal_lifecycle_key: &Hash,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    durable_frame_hash: &Hash,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
) -> WaitSource {
    let mut encoded = Vec::new();
    append_field(&mut encoded, owner.causal_root().digest().as_bytes());
    append_field(&mut encoded, &owner.first_admission_ordinal().to_le_bytes());
    append_field(&mut encoded, &ordinal.to_le_bytes());
    append_field(&mut encoded, &slot.0.to_le_bytes());
    append_field(&mut encoded, &slot.1.to_le_bytes());
    append_field(&mut encoded, incumbent_digest.as_bytes());
    append_field(&mut encoded, causal_lifecycle_key.as_ref());
    append_field(&mut encoded, durable_frame_hash.as_ref());
    append_field(&mut encoded, expected_manifest_hash.as_ref());
    append_field(&mut encoded, lifecycle_key.context().as_bytes());
    append_field(&mut encoded, &lifecycle_key.round().height().to_le_bytes());
    append_field(&mut encoded, &lifecycle_key.round().view().to_le_bytes());
    match lifecycle_key.proposal_round() {
        None => encoded.push(0),
        Some(round) => {
            encoded.push(1);
            append_field(&mut encoded, &round.height().to_le_bytes());
            append_field(&mut encoded, &round.view().to_le_bytes());
        }
    }
    match lifecycle_key.subject() {
        None => encoded.push(0),
        Some(subject) => {
            encoded.push(1);
            append_field(&mut encoded, subject.as_bytes());
        }
    }
    encoded.push(
        u8::try_from(
            LifecyclePhase::ALL
                .iter()
                .position(|phase| *phase == lifecycle_key.phase())
                .expect("closed lifecycle phase is present in its canonical inventory"),
        )
        .expect("closed lifecycle phase inventory fits u8"),
    );
    match lifecycle_key.execution_commitment() {
        None => encoded.push(0),
        Some(commitment) => {
            encoded.push(1);
            append_field(&mut encoded, commitment.as_bytes());
        }
    }
    encoded.push(
        u8::try_from(
            LifecycleStageKind::ALL
                .iter()
                .position(|kind| *kind == lifecycle_stage.kind())
                .expect("closed lifecycle stage is present in its canonical inventory"),
        )
        .expect("closed lifecycle stage inventory fits u8"),
    );
    encoded.push(match lifecycle_stage.predecessor_scope() {
        PredecessorScope::Independent => 0,
        PredecessorScope::ReadyOrdinalPrefix => 1,
        PredecessorScope::ProducerHandoffBarrier => 2,
    });
    match candidate_statement {
        None => encoded.push(0),
        Some(statement) => {
            encoded.push(1);
            append_field(&mut encoded, &statement.context_id().encode());
            append_field(&mut encoded, &statement.round().encode());
            append_field(&mut encoded, &statement.proposal_round().encode());
            append_field(&mut encoded, &statement.subject().encode());
            append_field(&mut encoded, &statement.phase().encode());
            append_field(&mut encoded, &statement.execution_commitment().encode());
        }
    }
    WaitSource::External(domain_digest(
        DURABLE_VALIDATION_WAIT_SOURCE_DOMAIN,
        &encoded,
    ))
}
/// Derive the one context-scoped external generation source which wakes direct
/// completions after the adapter's reducer fence changes.
///
/// The generation itself remains process-local and is sampled from the same
/// borrow-bound adapter token. Including both authenticated context identity
/// and height makes accidental cross-height context reuse fail closed.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) fn reducer_fence_wait_source(context: LifecycleContext) -> WaitSource {
    let encoded = (*context.id().as_bytes(), context.height()).encode();
    WaitSource::External(domain_digest(REDUCER_FENCE_WAIT_SOURCE_DOMAIN, &encoded))
}
/// Recover the coordinator's semantic owner root from one sealed pending
/// runtime-effect binding.
pub(super) fn pending_effect_causal_root(binding: &PendingRuntimeEffectBinding) -> CausalRoot {
    CausalRoot::new(digest_from_hash(binding.causal_lifecycle_key()))
}
/// Authenticate the durable body outcome carried by one terminal Validate parent.
///
/// This projection deliberately proves only the exact body outcome and the
/// immutable parent identity. `AdvancedNoSuccessor` remains the checksummed
/// ledger's record of the historical reducer branch: replaying the same body
/// after later WAL/reducer progress cannot reliably reproduce that old
/// `Inactive` or `NoEffect` classification.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn recovered_validate_no_successor_ledger_identity_is_authenticated(
    context: LifecycleContext,
    key: LifecycleKey,
    causal_root: CausalRoot,
    reconstruction_source: LifecycleDigest,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
    outcome: &DurableBodyValidationOutcome,
) -> bool {
    let durable = outcome.durable_body();
    let expected_payload =
        durable_body_frame_reference(context, durable).map(DurablePayloadReference::BodyFrame);
    let expected_context = digest_from_bytes(durable.context_id().0.as_ref());
    let expected_proposal_round = LifecycleRound::new(durable.round().height, durable.round().view);
    let expected_subject = block_subject(durable.subject());
    let outcome_is_exact = match (
        outcome.validated_receipt(),
        outcome.rejection_identity(),
        outcome.missing_merge_sidecar(),
    ) {
        (Some(receipt), None, None) => {
            receipt.durable() == durable
                && key.execution_commitment().is_none_or(|commitment| {
                    commitment == execution_commitment(receipt.execution_commitment())
                })
        }
        (None, Some(BodyValidationRejectionIdentity::Rejected), None) => true,
        _ => false,
    };
    context.id() == expected_context
        && context.height() == durable.round().height
        && durable.round().context_id == durable.context_id()
        && key.context() == expected_context
        && key.round().height() == context.height()
        && key.proposal_round() == Some(expected_proposal_round)
        && key.subject() == Some(expected_subject)
        && key.phase() == LifecyclePhase::Validate
        && causal_root.digest() == reconstruction_source
        && stage.kind() == LifecycleStageKind::ValidateBody
        && stage.predecessor_scope() == PredecessorScope::Independent
        && Some(payload) == expected_payload
        && outcome_is_exact
}
/// Authenticate a terminal Validate candidate including its transient physical episode.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn recovered_validate_no_successor_is_authenticated(
    context: LifecycleContext,
    candidate: &CandidateAdmission,
    outcome: &DurableBodyValidationOutcome,
) -> bool {
    let canonical_geometry = candidate.physical_geometry.canonicalized();
    let normalized_geometry = candidate.physical_geometry.normalized();
    let geometry_is_exact = matches!(
        (canonical_geometry, normalized_geometry),
        (Ok(canonical), Ok((physical, universe, consumed)))
            if canonical == candidate.physical_geometry
                && physical.len() == 1
                && universe.len() == 1
                && consumed == universe
                && physical.keys().all(|slot| {
                    slot.capacity_class() == Some(LifecycleWorkClass::Validate.capacity_class())
                })
    );
    recovered_validate_no_successor_ledger_identity_is_authenticated(
        context,
        candidate.key,
        candidate.causal_root,
        candidate.reconstruction_source,
        candidate.stage,
        candidate.payload,
        outcome,
    ) && candidate.work_class == LifecycleWorkClass::Validate
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.producer_turn.is_none()
        && geometry_is_exact
}
/// Build the six-field Serve-key subject from the certified block and exact
/// signed request. The request hash is deliberately part of the semantic key:
/// the durable ledger stores one terminal response per record, and responses
/// are valid only for their exact request hash. Two requesters for one body
/// therefore cannot alias one singular cached response lifecycle.
pub(super) fn certified_serve_key_subject(
    subject: wire::BlockSubject,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
) -> LifecycleDigest {
    let mut projection = Vec::new();
    projection.extend_from_slice(CERTIFIED_SERVE_KEY_SUBJECT_DOMAIN);
    append_field(&mut projection, &subject.encode());
    append_field(&mut projection, request_hash.as_ref());
    digest_from_hash(&Hash::new(projection))
}
fn equivocation_subject(
    evidence: &crate::sumeragi::v2::AdapterEquivocationEvidence,
) -> LifecycleDigest {
    let mut projection = Vec::new();
    projection.extend_from_slice(EQUIVOCATION_SUBJECT_DOMAIN);
    projection.push(match evidence.kind() {
        EquivocationKind::Proposal => 1,
        EquivocationKind::Vote => 2,
        EquivocationKind::Timeout => 3,
    });
    projection.extend_from_slice(&evidence.offender_index().to_le_bytes());
    let (first, second) = evidence.canonical_unsigned_statement_pair();
    append_field(&mut projection, &first);
    append_field(&mut projection, &second);
    digest_from_hash(&Hash::new(projection))
}
fn domain_digest(domain: &[u8], encoded: &[u8]) -> LifecycleDigest {
    let mut projection = Vec::with_capacity(domain.len() + 8 + encoded.len());
    projection.extend_from_slice(domain);
    append_field(&mut projection, encoded);
    digest_from_hash(&Hash::new(projection))
}
fn append_field(projection: &mut Vec<u8>, field: &[u8]) {
    projection.extend_from_slice(
        &u64::try_from(field.len())
            .expect("bounded lifecycle projection field fits u64")
            .to_le_bytes(),
    );
    projection.extend_from_slice(field);
}
fn digest_from_hash(hash: &Hash) -> LifecycleDigest {
    digest_from_bytes(hash.as_ref())
}
fn digest_from_bytes(hash: &[u8]) -> LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash);
    LifecycleDigest::new(bytes)
}
#[cfg(test)]
mod wait_source_tests {
    use super::*;
    fn validate_recovery_fixture() -> (
        LifecycleContext,
        CandidateAdmission,
        crate::sumeragi::v2_body_store::DurableBodyReceipt,
    ) {
        let context_hash = Hash::new(b"validate recovery context");
        let context = LifecycleContext::new(digest_from_hash(&context_hash), 7);
        let (replay, durable) = super::super::replay_authority::exact_body_record_fixture(
            context,
            LifecycleStageKind::ValidateBody,
            3,
        );
        let source = LifecycleDigest::new([0x73; 32]);
        let slot = PhysicalSlotId::for_capacity(super::super::schema::CapacityClass::Effect, 0);
        let candidate = CandidateAdmission::new(
            replay.key,
            CausalRoot::new(source),
            replay.work_class,
            replay.stage,
            InitialLifecycleState::Ready,
            source,
            replay.payload,
            replay.authority,
            PhysicalGeometry::new(
                [PhysicalSlot::new(slot, LifecycleDigest::new([0x74; 32]))],
                [slot],
            ),
            None,
        );
        (context, candidate, durable)
    }
    #[test]
    fn reducer_fence_source_is_context_and_height_scoped() {
        let first = LifecycleContext::new(LifecycleDigest::new([0xA1; 32]), 7);
        let other_context = LifecycleContext::new(LifecycleDigest::new([0xA2; 32]), 7);
        let other_height = LifecycleContext::new(first.id(), 8);
        let source = reducer_fence_wait_source(first);
        assert!(matches!(source, WaitSource::External(_)));
        assert_eq!(source, reducer_fence_wait_source(first));
        assert_ne!(source, reducer_fence_wait_source(other_context));
        assert_ne!(source, reducer_fence_wait_source(other_height));
    }
    #[test]
    fn durable_body_frame_projection_binds_every_receipt_coordinate() {
        let (context, candidate, durable) = validate_recovery_fixture();
        let reference = durable_body_frame_reference(context, &durable)
            .expect("exact body receipt projects into its active context");
        assert!(reference.matches_key(candidate.key));
        assert_eq!(reference.context, context.id());
        assert_eq!(reference.round, candidate.key.proposal_round().unwrap());
        assert_eq!(reference.subject, candidate.key.subject().unwrap());
        assert_eq!(
            reference.manifest,
            digest_from_bytes(durable.manifest_hash().as_ref())
        );
        assert_eq!(reference.frame, digest_from_hash(&durable.frame_hash()));
        let foreign_context = LifecycleContext::new(LifecycleDigest::new([0x76; 32]), 7);
        assert_eq!(
            durable_body_frame_reference(foreign_context, &durable),
            None
        );
        let foreign_height = LifecycleContext::new(context.id(), 8);
        assert_eq!(durable_body_frame_reference(foreign_height, &durable), None);
    }
    #[test]
    fn durable_body_frame_recovery_requires_one_exact_catalog_row() {
        let (context, _, template) = validate_recovery_fixture();
        let manifest = wire::PayloadManifest {
            round: template.round(),
            subject: template.subject(),
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 2,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 16,
                max_chunk_count: 16,
            },
            chunk_hashes: vec![Hash::new(b"durable body frame chunk")],
            chunk_root: Hash::new(b"durable body frame root"),
        };
        let receipt = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
            template.context_id(),
            template.round(),
            template.subject(),
            HashOf::new(&manifest),
        );
        let expected = durable_body_frame_reference(context, &receipt)
            .expect("catalog receipt projects into its active context");
        let recovered = authenticate_durable_body_frame_catalog(
            context,
            expected,
            [(manifest.clone(), receipt.clone())],
        )
        .expect("one exact manifest and receipt recover one opaque frame seal");
        assert_eq!(recovered.reference, expected);
        assert_eq!(recovered.manifest, manifest);
        assert_eq!(recovered.receipt, receipt);
        assert!(matches!(
            authenticate_durable_body_frame_catalog(context, expected, []),
            Err(DurableBodyFrameRecoveryError::Missing)
        ));
        assert!(matches!(
            authenticate_durable_body_frame_catalog(
                context,
                expected,
                [
                    (manifest.clone(), receipt.clone()),
                    (manifest.clone(), receipt.clone()),
                ],
            ),
            Err(DurableBodyFrameRecoveryError::Ambiguous)
        ));
        let mut foreign_manifest = manifest;
        foreign_manifest.payload_size_bytes = 2;
        assert!(matches!(
            authenticate_durable_body_frame_catalog(
                context,
                expected,
                [(foreign_manifest, receipt)],
            ),
            Err(DurableBodyFrameRecoveryError::Missing)
        ));
    }
    #[test]
    fn terminal_validate_recovery_binds_exact_body_outcome_and_parent_identity() {
        let (context, candidate, durable) = validate_recovery_fixture();
        let execution_commitment =
            super::super::replay_authority::exact_body_execution_commitment_fixture(context, 3);
        let validated =
            crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::validated_for_test(
                crate::sumeragi::v2_body_store::ValidatedBodyReceipt::for_test_with_commitment(
                    durable.clone(),
                    execution_commitment,
                ),
            );
        assert!(recovered_validate_no_successor_is_authenticated(
            context, &candidate, &validated,
        ));
        let rejected =
            crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::rejected_for_test(
                durable,
            );
        assert!(recovered_validate_no_successor_is_authenticated(
            context, &candidate, &rejected,
        ));
        let mut committed_rejection = candidate.clone();
        committed_rejection.key.execution_commitment = Some(LifecycleDigest::new([0x74; 32]));
        assert!(recovered_validate_no_successor_is_authenticated(
            context,
            &committed_rejection,
            &rejected,
        ));
        let mut foreign = candidate.clone();
        foreign.key.subject = Some(LifecycleDigest::new([0x75; 32]));
        assert!(!recovered_validate_no_successor_is_authenticated(
            context, &foreign, &validated,
        ));
        let mut substituted = candidate.clone();
        let DurablePayloadReference::BodyFrame(mut substituted_frame) = substituted.payload else {
            panic!("Validate recovery fixture must retain its durable body frame");
        };
        substituted_frame.frame = LifecycleDigest::new([0x76; 32]);
        substituted.payload = DurablePayloadReference::BodyFrame(substituted_frame);
        assert!(!recovered_validate_no_successor_is_authenticated(
            context,
            &substituted,
            &validated,
        ));
        let mut malformed = candidate;
        malformed.physical_geometry = PhysicalGeometry::new([], []);
        assert!(!recovered_validate_no_successor_is_authenticated(
            context, &malformed, &rejected,
        ));
    }
}
#[cfg(all(test, feature = "bls"))]
mod tests {
    include!("tests/v2_lifecycle_projection_cases.rs");
}
