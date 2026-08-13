//! Sealed projection from exact runtime-bound adapter effects into lifecycle admission.

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::Encode;
use thiserror::Error;

use super::replay_authority::{
    CertifiedFetchReplayEvidenceV1, CertifiedServeReplayEvidencePairV1,
    CertifiedServeTerminalReplayAuthorityPairV1, exact_direct_signed_admission_authority,
};
use super::schema::{
    AdmissionRequest, CandidateAdmission, CausalRoot, DurableBodyFrameReference,
    DurablePayloadReference, DurableServeNegativeOutcome, InitialLifecycleState, LifecycleContext,
    LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRound, LifecycleStage,
    LifecycleStageKind, LifecycleWorkClass, OwnerId, PhysicalGeometry, PhysicalSlot,
    PhysicalSlotId, PredecessorScope, TerminalOutcome, WaitSource, producer_turn_key_for_serve,
};
use super::work_registry::{
    CertifiedServeRegistryBatchPublicationError, CertifiedServeTerminalRegistryPublicationError,
    PreparedCertifiedServeRegistryBatchV1, PreparedCertifiedServeTerminalRegistryTransitionV1,
};
use crate::sumeragi::{
    v2::{AdapterEffect, SignRequest, VerifiedHeightContext},
    v2_body_store::{
        BodyValidationRejectionIdentity, DurableBodyReceipt, DurableBodyValidationOutcome,
        V2BodyStore, V2BodyStoreError,
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

#[cfg(test)]
use crate::sumeragi::v2_certified_serve_payload_store::{
    CertifiedServePayloadStoreError, CertifiedServePayloadStoreV1,
};

const BLOCK_SUBJECT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:block-subject:v1";
const EXECUTION_COMMITMENT_DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:execution-commitment:v1";
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
    /// No exact sealed replay wrapper exists at this raw admission boundary.
    UnsupportedReplayAuthority,
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

#[allow(variant_size_differences)]
enum CertifiedServeConcreteAdmissionKindV1 {
    Published {
        decision: super::AdmissionDecision,
        target: super::LifecycleIngressIoTargetSeal,
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
            CertifiedServeConcreteAdmissionKindV1::Published { decision, target } => {
                Ok(CertifiedServeConcreteAdmissionContinuationV1 {
                    decision: Some(decision),
                    failure: None,
                    target,
                })
            }
            CertifiedServeConcreteAdmissionKindV1::Retryable {
                failure,
                decision,
                target,
            } => Ok(CertifiedServeConcreteAdmissionContinuationV1 {
                decision,
                failure: Some(failure),
                target,
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
            kind: CertifiedServeConcreteAdmissionKindV1::Published { decision, target },
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
}

fn certified_serve_terminal_replay_decision(
    coordinator: &super::LifecycleCoordinator,
    verified: &VerifiedHeightContext,
    authenticated: &AuthenticatedCertifiedBodyRequest,
    publication: &DurableCertifiedServeAdmissionPublication,
) -> Option<super::AdmissionDecision> {
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
    let (payload, outcome) = match publication.state() {
        DurableCertifiedServeAdmissionStateV1::Pending => return None,
        DurableCertifiedServeAdmissionStateV1::Completed(response) => {
            let response = digest_from_bytes(response.as_ref());
            (
                DurablePayloadReference::CertifiedServeCompleted {
                    request: request_digest,
                    certificate: certificate_digest,
                    response,
                },
                TerminalOutcome::Completed(Some(response)),
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
    Some(match outcome {
        TerminalOutcome::Completed(Some(_)) => {
            super::AdmissionDecision::ReplayTerminal { owner, outcome }
        }
        TerminalOutcome::Cancelled | TerminalOutcome::Rejected(_) | TerminalOutcome::Failed(_) => {
            super::AdmissionDecision::StutterTerminal { owner }
        }
        TerminalOutcome::Advanced | TerminalOutcome::Completed(None) => return None,
    })
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
            !registry.exactly_covers_recovered_ready_work(&self.coordinator)
                && !registry
                    .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
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
                Some(decision) => CertifiedServeConcreteAdmissionV1::published(decision, target),
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
        let decision = staged.reduce_admit(AdmissionRequest::Candidate(candidate));
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
                &self.coordinator,
                &staged,
                || self.coordinator.persist_exact_staged_successor(&staged),
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
    #[cfg_attr(not(test), allow(dead_code))]
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

    /// Persist and publish one exact typed negative Certified-Serve terminal.
    ///
    /// The retained payload store derives the opaque request id from the
    /// authenticated request. No caller-supplied id or terminal receipt is
    /// accepted.
    #[cfg_attr(not(test), allow(dead_code))]
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
        if !publication.is_pending() && !self.key_index.contains_key(&candidate.key) {
            self.fault = Some(super::CoordinatorFault::DurabilityFailure);
            return Err(CertifiedServeAdmissionBoundaryError::PayloadStore(
                CertifiedServePayloadStoreError::OrphanTerminalPayload,
            ));
        }
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

pub(super) fn admission_request(
    active_context: LifecycleContext,
    verified: &VerifiedHeightContext,
    effect: &AdapterEffect,
    binding: &PendingRuntimeEffectBinding,
) -> Result<AdmissionRequest, AdapterEffectAdmissionError> {
    let projected = authority_free_admission_projection(active_context, verified, effect, binding)?;
    let replay_authority = exact_direct_signed_admission_authority(effect, binding)
        .ok_or(AdapterEffectAdmissionError::UnsupportedReplayAuthority)?;
    let candidate = CandidateAdmission::new(
        projected.key,
        projected.causal_root,
        projected.work_class,
        projected.stage,
        projected.initial_state,
        projected.reconstruction_source,
        DurablePayloadReference::None,
        replay_authority,
        projected.physical_geometry,
        None,
    );
    Ok(AdmissionRequest::Candidate(candidate))
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
                    highest.map(|qc| block_subject(qc.subject)),
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
    authenticate_durable_body_frame_catalog(
        active_context,
        expected,
        store.recovery_catalog()?.into_values(),
    )
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
pub(super) fn reducer_fence_wait_source(context: LifecycleContext) -> WaitSource {
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
        let validated =
            crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::validated_for_test(
                crate::sumeragi::v2_body_store::ValidatedBodyReceipt::for_test(durable.clone()),
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
    use std::{collections::BTreeSet, num::NonZeroU64};

    use iroha_crypto::{Algorithm, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock},
        peer::PeerId,
    };
    use tempfile::TempDir;

    use super::super::{
        AdmissionDecision, AdmissionRejection, AuthenticatedLifecycleRecoveryCut,
        LifecycleCoordinator, LifecycleState, RetryAction, RolloverSnapshot, SchedulerInputs,
        SchedulerReadyInputs, TurnPlan, WaitSource,
        schema::{CapacityClass, CapacityGeometry},
    };
    use super::*;
    use crate::sumeragi::{
        v2::AdapterEquivocationEvidence,
        v2_body_store::V2BodyStore,
        v2_certified_serve_payload_store::{
            AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadNegativeOutcome,
            CertifiedServePayloadStoreV1,
        },
        v2_chunks::encode_payload,
        v2_core::{EventTag, Generation},
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
        v2_transport::authenticate_certified_body_request,
    };

    struct Fixture {
        verified: VerifiedHeightContext,
        keys: Vec<KeyPair>,
        context: wire::HeightContext,
        round: wire::ConsensusRound,
        tag: EventTag,
        body: Vec<u8>,
        encoded_chunks: Vec<Vec<u8>>,
        subject: wire::BlockSubject,
        manifest: wire::PayloadManifest,
        proposal: wire::Proposal,
        prepare_vote: wire::Vote,
        commit_vote: wire::Vote,
        prepare_qc: wire::QuorumCertificate,
        commit_qc: wire::QuorumCertificate,
        timeout_vote: wire::TimeoutVote,
        timeout_certificate: wire::TimeoutCertificate,
    }

    type ExpectedProjection = (
        AdapterEffect,
        LifecycleWorkClass,
        LifecyclePhase,
        LifecycleStageKind,
    );

    impl Fixture {
        #[allow(clippy::too_many_lines)]
        fn new() -> Self {
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic lifecycle-projection BLS key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture BLS proof of possession")
                })
                .collect::<Vec<_>>();
            let roster = keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                network_id: crate::sumeragi::synthetic_network_id(
                    "sumeragi-v2-lifecycle-projection-test",
                ),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 1,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"lifecycle projection nexus context"),
                execution_policy_hash: Hash::new(b"lifecycle projection execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 512 * 1024,
                    max_chunk_count: 1024,
                },
                leader_seed: [0xA7; 32],
            };
            let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
                .expect("verified lifecycle-projection context");
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            };
            let tag = EventTag::new(context.height, round.view, Generation::new(1));
            let body = vec![0x41; 4];
            let subject = block_subject_for_body(&body, 0x41);
            let encoded_chunks =
                wire::encode_payload_chunks(context.da_layout, &body).expect("encode fixture body");
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(body.len()).expect("small fixture body"),
                &encoded_chunks,
            )
            .expect("derive fixture manifest");
            let proposal = wire::Proposal {
                round,
                proposer: context.leader(round.view),
                subject,
                manifest: manifest.clone(),
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![0x41],
            };
            let commitment = execution_commitment_for(0x41);
            let prepare_vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signer: 0,
                signature: vec![0x42],
            };
            let commit_vote = wire::Vote {
                phase: wire::GlobalPhase::Commit,
                signature: vec![0x43],
                ..prepare_vote.clone()
            };
            let prepare_qc = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x44],
            };
            let commit_qc = wire::QuorumCertificate {
                phase: wire::GlobalPhase::Commit,
                aggregate_signature: vec![0x45],
                ..prepare_qc.clone()
            };
            let timeout_vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: None,
                signer: 0,
                signature: vec![0x46],
            };
            let timeout_certificate = wire::TimeoutCertificate {
                round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0x47],
                }],
            };
            Self {
                verified,
                keys,
                context,
                round,
                tag,
                body,
                encoded_chunks,
                subject,
                manifest,
                proposal,
                prepare_vote,
                commit_vote,
                prepare_qc,
                commit_qc,
                timeout_vote,
                timeout_certificate,
            }
        }

        fn coordinator(&self) -> LifecycleCoordinator {
            LifecycleCoordinator::new(
                lifecycle_context(&self.context),
                0,
                CapacityGeometry::new(CapacityClass::ALL.map(|class| (class, 64))),
            )
        }

        fn authenticated_serve_request(
            &self,
            requester_index: usize,
        ) -> AuthenticatedCertifiedBodyRequest {
            self.authenticated_serve_request_for(self.round, self.subject, requester_index)
        }

        fn authenticated_serve_request_for(
            &self,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
            requester_index: usize,
        ) -> AuthenticatedCertifiedBodyRequest {
            let execution_commitment = execution_commitment_for(0x81);
            let signers = vec![0, 1, 2];
            let preimage = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment,
                signer: 0,
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = signers
                .iter()
                .map(|signer| {
                    Signature::new(
                        self.keys[usize::try_from(*signer).expect("small fixture signer")]
                            .private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate fixture PrepareQC");
            let mut request = wire::CertifiedBodyRequest {
                round,
                subject,
                certificate: wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signers,
                    aggregate_signature,
                },
                requester: PeerId::new(self.keys[requester_index].public_key().clone()),
                signature: Vec::new(),
            };
            request.signature = Signature::new(
                self.keys[requester_index].private_key(),
                &request.signature_preimage(),
            )
            .payload()
            .to_vec();
            let requester = request.requester.clone();
            authenticate_certified_body_request(
                &self.context,
                request,
                &requester,
                |context, certificate| {
                    wire::finality::verify_quorum_certificate_with_validator_pops(
                        context,
                        certificate,
                        self.verified.proofs_of_possession(),
                    )
                    .map_err(|error| error.to_string())
                },
            )
            .expect("authenticate exact fixture CertifiedBodyRequest")
        }

        fn canonical_body_and_manifest(&self) -> (Vec<u8>, wire::PayloadManifest) {
            let leader = self.context.leader(self.round.view);
            let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
            let header = BlockHeader::new(
                NonZeroU64::new(self.round.height).expect("non-zero fixture height"),
                None,
                None,
                None,
                1_000,
                self.round.view,
            );
            let signature =
                SignatureOf::try_from_hash(self.keys[leader_index].private_key(), header.hash())
                    .expect("sign fixture block header");
            let block = SignedBlock::presigned(
                BlockSignature::new(u64::from(leader), signature),
                header,
                Vec::new(),
            );
            let body = block.encode_wire().expect("canonical SignedBlockWire");
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block.hash(),
                payload_hash: Hash::new(&body),
            };
            let manifest = encode_payload(&self.context, self.round, subject, &body)
                .expect("encode canonical fixture payload")
                .manifest()
                .clone();
            (body, manifest)
        }

        fn proposal_for(&self, marker: u8, signature: u8) -> wire::Proposal {
            let body = vec![marker; 4];
            let subject = block_subject_for_body(&body, marker);
            let encoded_chunks = wire::encode_payload_chunks(self.context.da_layout, &body)
                .expect("encode conflicting fixture body");
            let manifest = wire::PayloadManifest::derive(
                &self.context,
                self.round,
                subject,
                u64::try_from(body.len()).expect("small fixture body"),
                &encoded_chunks,
            )
            .expect("derive conflicting fixture manifest");
            wire::Proposal {
                round: self.round,
                proposer: self.context.leader(self.round.view),
                subject,
                manifest,
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![signature],
            }
        }
    }

    fn block_subject_for_body(body: &[u8], marker: u8) -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xB1])),
            payload_hash: Hash::new(body),
        }
    }

    fn execution_commitment_for(marker: u8) -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 1]),
            Hash::new([marker, 2]),
            Hash::new([marker, 3]),
            1,
            Hash::new([marker, 4]),
        )
    }

    fn bound_ownership(
        effect: &AdapterEffect,
        owner_tag: EventTag,
        ordinal: u128,
    ) -> RuntimeEffectOwnership {
        bind_adapter_effect_batch_ownership(
            core::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(owner_tag, ordinal)],
        )
        .expect("bind exact lifecycle-projection effect")
        .pop()
        .expect("one bound ownership")
    }

    fn candidate(
        fixture: &Fixture,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> CandidateAdmission {
        let pending = ownership
            .pending_adapter_effect_binding(effect)
            .expect("mint ordinal-free pending lifecycle binding");
        let request = admission_request(
            lifecycle_context(&fixture.context),
            &fixture.verified,
            effect,
            &pending,
        )
        .expect("project exact bound adapter effect");
        let AdmissionRequest::Candidate(candidate) = request else {
            panic!("all adapter effects project to lifecycle candidates")
        };
        candidate
    }

    fn assert_candidate_shape(
        candidate: &CandidateAdmission,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        work_class: LifecycleWorkClass,
        phase: LifecyclePhase,
        stage_kind: LifecycleStageKind,
    ) {
        assert_eq!(candidate.work_class, work_class);
        assert_eq!(candidate.key.phase(), phase);
        assert_eq!(candidate.stage.kind(), stage_kind);
        assert_eq!(
            candidate.stage.predecessor_scope(),
            PredecessorScope::Independent
        );
        assert_eq!(candidate.initial_state, InitialLifecycleState::Ready);
        assert_eq!(candidate.payload, DurablePayloadReference::None);
        assert!(candidate.producer_turn.is_none());
        assert_eq!(
            candidate.causal_root.digest(),
            candidate.reconstruction_source
        );
        assert_eq!(candidate.physical_geometry.initial.len(), 1);
        assert_eq!(candidate.physical_geometry.replenishment_slots.len(), 1);
        let slot = candidate.physical_geometry.initial[0];
        assert_eq!(
            slot.id().capacity_class(),
            Some(work_class.capacity_class())
        );
        assert_eq!(slot.id().index(), 0);
        assert!(
            candidate
                .physical_geometry
                .replenishment_slots
                .contains(&slot.id())
        );
        let authority = ownership
            .pending_adapter_effect_binding(effect)
            .expect("the tested effect remains exactly bound");
        assert_eq!(
            slot.digest(),
            digest_from_hash(authority.exact_effect_identity())
        );
    }

    fn vote_conflict(fixture: &Fixture) -> (wire::Vote, wire::Vote) {
        let first = fixture.prepare_vote.clone();
        let second = wire::Vote {
            subject: fixture.proposal_for(0x52, 0x53).subject,
            execution_commitment: execution_commitment_for(0x52),
            signature: vec![0x53],
            ..first.clone()
        };
        (first, second)
    }

    fn authenticated_payload_cut(
        fixture: &Fixture,
        payload_root: &std::path::Path,
        body_store: &V2BodyStore,
        local_signer: &KeyPair,
    ) -> (
        CertifiedServePayloadStoreV1,
        AuthenticatedCertifiedServePayloadRecoveryCut,
    ) {
        let (store, recovery) = CertifiedServePayloadStoreV1::open(payload_root, &fixture.context)
            .expect("reopen exact Certified-Serve payload store");
        let authenticated = recovery
            .authenticate(&fixture.verified, local_signer, body_store)
            .expect("authenticate exact Certified-Serve recovery cut");
        (store, authenticated)
    }

    fn lifecycle_recovery_cut(
        fixture: &Fixture,
        ledger_root: &std::path::Path,
        payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> AuthenticatedLifecycleRecoveryCut {
        let (_store, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
            ledger_root,
            lifecycle_context(&fixture.context),
        )
        .expect("decode the exact lifecycle ledger authenticated by the fixture cut");
        AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(ledger, [], [], payloads)
            .expect("assemble sealed lifecycle recovery cut")
    }

    fn execute_ready_turn(coordinator: &mut LifecycleCoordinator) -> super::super::TurnLease {
        let ready = coordinator.ready_index.iter().map(|ordinal| {
            let record = &coordinator.records[ordinal];
            (*ordinal, SchedulerReadyInputs::new(record, None, [0; 6]))
        });
        let TurnPlan::Execute(lease) = coordinator.plan_turn(
            SchedulerInputs::new([], ready).expect("Serve ready rows have unique ordinals"),
        ) else {
            panic!("one ready Certified-Serve record must execute")
        };
        lease
    }

    fn reduce_completed_serve_for_test(
        coordinator: &mut LifecycleCoordinator,
        lease: super::super::TurnLease,
        receipt: DurableCertifiedServeCompletedReceipt,
    ) -> bool {
        let Some(producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal).copied() else {
            return false;
        };
        let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
            coordinator.active_context,
            &coordinator.records[&lease.ordinal],
            &coordinator.durable_records[&lease.ordinal],
            &coordinator.records[&producer_ordinal],
            &coordinator.durable_records[&producer_ordinal],
            receipt,
        );
        let Some(terminal) = terminal else {
            return false;
        };
        coordinator.settle_turn_with_durable_serve_terminal(lease, terminal);
        coordinator.fault().is_none()
    }

    fn reduce_negative_serve_for_test(
        coordinator: &mut LifecycleCoordinator,
        lease: super::super::TurnLease,
        receipt: DurableCertifiedServeNegativeReceipt,
    ) -> bool {
        let Some(producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal).copied() else {
            return false;
        };
        let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
            coordinator.active_context,
            &coordinator.records[&lease.ordinal],
            &coordinator.durable_records[&lease.ordinal],
            &coordinator.records[&producer_ordinal],
            &coordinator.durable_records[&producer_ordinal],
            receipt,
        );
        let Some(terminal) = terminal else {
            return false;
        };
        coordinator.settle_turn_with_durable_serve_terminal(lease, terminal);
        coordinator.fault().is_none()
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pending_certified_serve_admits_one_ready_serve_and_adjacent_dormant_producer() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let receipt = payload_store
            .persist_pending(&request)
            .expect("persist signed request before admission");
        let mut coordinator = fixture.coordinator();

        let decision = coordinator
            .admit_certified_serve(&fixture.verified, &request, receipt)
            .expect("project exact durable request");
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal,
            ..
        } = decision
        else {
            panic!("fresh Certified-Serve request must be admitted")
        };
        assert_eq!(ordinal, 1);
        assert_eq!(producer_turn_ordinal, Some(2));
        assert_eq!(coordinator.records.len(), 2);
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, receipt),
            Ok(AdmissionDecision::Retry {
                ordinal: 1,
                action: RetryAction::ReenqueueIncumbent,
                ..
            })
        ));
        assert_eq!(coordinator.records.len(), 2, "exact retry remains 1 + 1");

        let serve = &coordinator.records[&1];
        let producer = &coordinator.records[&2];
        assert_eq!(serve.work_class, LifecycleWorkClass::CertifiedServe);
        assert_eq!(serve.stage.kind(), LifecycleStageKind::CertifiedServe);
        assert_eq!(
            serve.stage.predecessor_scope(),
            PredecessorScope::ReadyOrdinalPrefix
        );
        assert_eq!(serve.state, LifecycleState::Ready);
        assert_eq!(producer.work_class, LifecycleWorkClass::ProducerTurn);
        assert_eq!(producer.stage.kind(), LifecycleStageKind::ProducerTurn);
        assert_eq!(
            producer.stage.predecessor_scope(),
            PredecessorScope::ProducerHandoffBarrier
        );
        assert!(matches!(
            producer.state,
            LifecycleState::Waiting(wait)
                if wait.source() == WaitSource::ProducerTurn(ordinal)
        ));
        assert_eq!(producer.ordinal, ordinal + 1);
        assert_eq!(producer.owner, serve.owner);

        let expected_request = digest_from_bytes(request.request_hash().as_ref());
        let expected_certificate =
            digest_from_bytes(HashOf::new(&request.request().certificate).as_ref());
        assert_eq!(serve.owner.causal_root().digest(), expected_request);
        assert_eq!(
            coordinator.durable_records[&1].reconstruction_source,
            expected_request
        );
        assert_eq!(
            coordinator.durable_records[&2].reconstruction_source,
            expected_request
        );
        assert_eq!(
            coordinator.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServePending {
                request: expected_request,
                certificate: expected_certificate,
            }
        );
        assert_eq!(
            serve.key.context(),
            lifecycle_context(&fixture.context).id()
        );
        assert_eq!(
            serve.key.round(),
            LifecycleRound::new(fixture.round.height, fixture.round.view)
        );
        assert_eq!(
            serve.key.proposal_round(),
            Some(LifecycleRound::new(
                fixture.round.height,
                fixture.round.view,
            ))
        );
        assert_eq!(
            serve.key.subject(),
            Some(certified_serve_key_subject(
                request.request().subject,
                request.request_hash(),
            ))
        );
        assert_ne!(
            serve.key.subject(),
            Some(block_subject(request.request().subject)),
            "Serve key subject is request-bound rather than a raw block subject"
        );
        assert_eq!(serve.key.phase(), LifecyclePhase::Serve);
        assert_eq!(
            serve.key.execution_commitment(),
            Some(execution_commitment(
                request.request().certificate.execution_commitment,
            ))
        );
        assert_eq!(producer.key.phase(), LifecyclePhase::ProducerTurn);
        assert_eq!(producer.key.subject(), serve.key.subject());
        assert_eq!(producer.key.context(), serve.key.context());
        assert_eq!(producer.key.round(), serve.key.round());
        assert_eq!(producer.key.proposal_round(), serve.key.proposal_round());
        assert_eq!(
            producer.key.execution_commitment(),
            serve.key.execution_commitment()
        );
        assert_eq!(
            serve.physical_slots.values().copied().collect::<Vec<_>>(),
            vec![digest_from_hash(&receipt.payload_hash())]
        );
        assert_eq!(
            serve.physical_slots.keys().copied().collect::<Vec<_>>(),
            vec![PhysicalSlotId::for_capacity(CapacityClass::Serve, 0)]
        );
        assert_eq!(
            producer
                .physical_slots
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![domain_digest(
                PRODUCER_TURN_PHYSICAL_DOMAIN,
                request.request_hash().as_ref(),
            )]
        );
        assert_eq!(
            producer.physical_slots.keys().copied().collect::<Vec<_>>(),
            vec![PhysicalSlotId::for_capacity(CapacityClass::Producer, 0)]
        );
    }

    #[test]
    fn capacity_wait_retains_one_bounded_payload_publication() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let first = fixture.authenticated_serve_request(2);
        let waiting = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(&fixture.context),
            0,
            CapacityGeometry::new([
                (CapacityClass::Consensus, 64),
                (CapacityClass::Effect, 64),
                (CapacityClass::Serve, 1),
                (CapacityClass::Producer, 2),
            ]),
        );

        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &first,
            ),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &waiting,
            ),
            Ok(AdmissionDecision::WaitForCapacity(_))
        ));
        drop(payload_store);

        let (mut payload_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("reopen payload store with one bounded wait");
        assert_eq!(recovery.len(), 2);
        assert!(
            recovery
                .iter()
                .any(|payload| payload.id().request_hash() == first.request_hash())
        );
        assert!(
            recovery
                .iter()
                .any(|payload| payload.id().request_hash() == waiting.request_hash())
        );
        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &waiting,
            ),
            Ok(AdmissionDecision::WaitForCapacity(_))
        ));
        drop(payload_store);
        let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("reopen after unchanged-generation retry");
        assert_eq!(
            recovery.len(),
            2,
            "retries reuse the single payload owned by the admission wait"
        );
    }

    #[test]
    fn conclusive_admission_rejection_rolls_back_the_pending_payload() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(&fixture.context),
            0,
            CapacityGeometry::new([
                (CapacityClass::Consensus, 64),
                (CapacityClass::Effect, 64),
                (CapacityClass::Serve, 0),
                (CapacityClass::Producer, 1),
            ]),
        );

        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &request,
            ),
            Ok(AdmissionDecision::Rejected(
                AdmissionRejection::InvalidEpisodeUniverse
            ))
        ));
        drop(payload_store);
        let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("reopen after conclusive rejection");
        assert!(
            recovery.is_empty(),
            "a rejected request cannot consume durable payload capacity"
        );
    }

    #[test]
    fn certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let other = fixture.authenticated_serve_request(2);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist admitted request");
        let other_pending = payload_store
            .persist_pending(&other)
            .expect("persist foreign request");
        let foreign_terminal = payload_store
            .persist_negative(
                other_pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(17),
            )
            .expect("persist foreign negative result");
        let mut coordinator = fixture.coordinator();
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let pending_serve_replay = coordinator.durable_records[&1].replay_authority.clone();
        let pending_producer_replay = coordinator.durable_records[&2].replay_authority.clone();
        let lease = execute_ready_turn(&mut coordinator);

        assert!(!reduce_negative_serve_for_test(
            &mut coordinator,
            lease.clone(),
            foreign_terminal,
        ));
        assert_eq!(coordinator.active_lease, Some(lease.clone()));
        let terminal = payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(19),
            )
            .expect("persist exact negative result");
        assert!(reduce_negative_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));

        assert_eq!(
            coordinator.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Rejected(19))
        );
        assert_eq!(coordinator.records[&2].state, LifecycleState::Ready);
        assert!(matches!(
            coordinator.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServeNegative {
                outcome: DurableServeNegativeOutcome::Rejected(19),
                ..
            }
        ));
        assert!(
            !pending_serve_replay
                .same_persisted_family(&coordinator.durable_records[&1].replay_authority)
        );
        assert!(
            !pending_producer_replay
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        assert!(
            coordinator.durable_records[&2]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &request,
            ),
            Ok(AdmissionDecision::StutterTerminal { .. })
        ));
    }

    #[test]
    fn certified_serve_terminal_family_mismatch_fails_without_state_mutation() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist admitted request");
        let mut coordinator = fixture.coordinator();
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let lease = execute_ready_turn(&mut coordinator);
        let terminal = payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Failed(31),
            )
            .expect("persist exact negative result");
        let foreign_producer_replay = coordinator.durable_records[&2]
            .replay_authority
            .with_certified_serve_frame_hash_for_test(Hash::new(
                b"foreign pending ProducerTurn payload frame",
            ))
            .expect("ProducerTurn retains a Certified-Serve storage source");
        coordinator
            .durable_records
            .get_mut(&2)
            .expect("admission retained ProducerTurn metadata")
            .replay_authority = foreign_producer_replay;
        let records = coordinator.records.clone();
        let durable_records = coordinator.durable_records.clone();
        let ready_index = coordinator.ready_index.clone();
        let producer_debts = coordinator.producer_debts.clone();
        let capacity_used = coordinator.capacity_used.clone();
        let active_lease = coordinator.active_lease.clone();

        assert!(!reduce_negative_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));
        assert_eq!(coordinator.records, records);
        assert_eq!(coordinator.durable_records, durable_records);
        assert_eq!(coordinator.ready_index, ready_index);
        assert_eq!(coordinator.producer_debts, producer_debts);
        assert_eq!(coordinator.capacity_used, capacity_used);
        assert_eq!(coordinator.active_lease, active_lease);
        assert_eq!(coordinator.fault(), None);
    }

    #[test]
    fn cancelled_certified_serve_tombstone_replays_with_its_terminal_producer_pair() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist admitted request");
        let mut coordinator = fixture.coordinator();
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let lease = execute_ready_turn(&mut coordinator);
        let terminal = payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            )
            .expect("persist cancellation before ledger settlement");
        assert!(reduce_negative_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));
        assert_eq!(
            coordinator.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
        assert_eq!(
            coordinator.records[&2].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
        assert!(!coordinator.producer_debts.contains_key(&1));
        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &request,
            ),
            Ok(AdmissionDecision::StutterTerminal { .. })
        ));
    }

    #[test]
    fn certified_serve_completion_settles_from_the_post_fsync_response_receipt() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist admitted request");
        let mut coordinator = fixture.coordinator();
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let pending_serve_replay = coordinator.durable_records[&1].replay_authority.clone();
        let pending_producer_replay = coordinator.durable_records[&2].replay_authority.clone();
        let lease = execute_ready_turn(&mut coordinator);
        let responder = 0;
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.keys[usize::try_from(responder).expect("small responder")].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        let terminal = payload_store
            .persist_completed(&request, &response)
            .expect("persist exact response metadata");
        assert!(reduce_completed_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));

        let response = digest_from_bytes(HashOf::new(&response).as_ref());
        assert_eq!(
            coordinator.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Completed(Some(response)))
        );
        assert_eq!(coordinator.records[&2].state, LifecycleState::Ready);
        assert!(matches!(
            coordinator.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServeCompleted {
                response: retained,
                ..
            } if retained == response
        ));
        assert!(
            !pending_serve_replay
                .same_persisted_family(&coordinator.durable_records[&1].replay_authority)
        );
        assert!(
            !pending_producer_replay
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        assert!(
            coordinator.durable_records[&2]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        assert!(matches!(
            coordinator.persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &request,
            ),
            Ok(AdmissionDecision::ReplayTerminal {
                outcome: TerminalOutcome::Completed(Some(retained)),
                ..
            }) if retained == response
        ));
    }

    #[test]
    fn certified_serve_rejects_a_receipt_for_another_signed_request() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let first = fixture.authenticated_serve_request(2);
        let second = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        payload_store
            .persist_pending(&first)
            .expect("persist first request");
        let second_receipt = payload_store
            .persist_pending(&second)
            .expect("persist second request");
        let mut coordinator = fixture.coordinator();

        assert_eq!(
            coordinator.admit_certified_serve(&fixture.verified, &first, second_receipt),
            Err(CertifiedServeAdmissionError::ReceiptMismatch)
        );
        assert!(coordinator.records.is_empty());
    }

    #[test]
    fn two_signed_requests_for_one_body_have_distinct_serve_lifecycles() {
        let temporary = TempDir::new().expect("temporary directory");
        let fixture = Fixture::new();
        let first = fixture.authenticated_serve_request(2);
        let second = fixture.authenticated_serve_request(3);
        assert_eq!(first.request().subject, second.request().subject);
        assert_ne!(first.request_hash(), second.request_hash());
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
                .expect("open payload store");
        let first_receipt = payload_store
            .persist_pending(&first)
            .expect("persist first request");
        let second_receipt = payload_store
            .persist_pending(&second)
            .expect("persist second request");
        let mut coordinator = fixture.coordinator();

        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &first, first_receipt),
            Ok(AdmissionDecision::Admitted {
                ordinal: 1,
                producer_turn_ordinal: Some(2),
                ..
            })
        ));
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &second, second_receipt),
            Ok(AdmissionDecision::Admitted {
                ordinal: 3,
                producer_turn_ordinal: Some(4),
                ..
            })
        ));
        assert_ne!(coordinator.records[&1].key, coordinator.records[&3].key);
        assert_ne!(
            coordinator.records[&1].key.subject(),
            coordinator.records[&3].key.subject()
        );
    }

    #[test]
    fn durable_rollover_removes_the_exact_capacity_wait_payload() {
        let temporary = TempDir::new().expect("temporary directory");
        let retired_ledger_root = temporary.path().join("retired-ledger");
        let successor_ledger_root = temporary.path().join("successor-ledger");
        let payload_root = temporary.path().join("payloads");
        let fixture = Fixture::new();
        let first = fixture.authenticated_serve_request(2);
        let waiting = fixture.authenticated_serve_request(3);
        let geometry = CapacityGeometry::new([
            (CapacityClass::Consensus, 4),
            (CapacityClass::Effect, 4),
            (CapacityClass::Serve, 1),
            (CapacityClass::Producer, 1),
        ]);
        let mut coordinator =
            LifecycleCoordinator::new(lifecycle_context(&fixture.context), 0, geometry.clone());
        coordinator
            .attach_empty_test_ledger(&retired_ledger_root)
            .expect("attach retired lifecycle ledger");
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open retired payload store");
        let first_receipt = payload_store
            .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &first)
            .expect("persist first request");
        assert!(matches!(
            coordinator
                .persist_and_admit_certified_serve(
                    &mut payload_store,
                    &fixture.verified,
                    &fixture.keys[0],
                    &first,
                )
                .expect("admit first Serve"),
            AdmissionDecision::Admitted {
                ordinal: 1,
                producer_turn_ordinal: Some(2),
                ..
            }
        ));
        assert!(matches!(
            coordinator
                .persist_and_admit_certified_serve(
                    &mut payload_store,
                    &fixture.verified,
                    &fixture.keys[0],
                    &waiting,
                )
                .expect("retain one exact capacity fence"),
            AdmissionDecision::WaitForCapacity(_)
        ));
        let waiting_key = *coordinator
            .admission_waits
            .keys()
            .next()
            .expect("capacity wait remains coordinator-owned");
        assert!(
            coordinator.admission_waits[&waiting_key]
                .serve_payload_receipt
                .is_some(),
            "the sealed admission boundary retains its own rollback receipt"
        );
        let live_cut = payload_store
            .authenticate_current_for_lifecycle_retirement(
                super::ProductionLifecycleServeRetirementAuthenticationPermitV1::for_test(),
                &fixture.verified,
                &fixture.keys[0],
            )
            .expect("authenticate admitted and wait-owned live Serve payloads");
        let live_ledger = super::ledger::LifecycleLedgerV1::from_coordinator(&coordinator)
            .expect("project the exact live finalization ledger");
        let retained = super::open::authenticate_live_finalization_serve_census(
            &fixture.verified,
            &live_ledger,
            &coordinator,
            &live_cut,
        )
        .expect("join the exact ledger and admission-wait payload census");
        assert_eq!(retained, BTreeSet::from([first_receipt.id()]));
        let exact_wait_receipt = coordinator.admission_waits[&waiting_key]
            .serve_payload_receipt
            .expect("capacity wait owns its exact payload receipt");
        coordinator
            .admission_waits
            .get_mut(&waiting_key)
            .expect("capacity wait remains installed")
            .serve_payload_receipt = Some(
            exact_wait_receipt
                .with_request_hash_for_test(HashOf::from_untyped_unchecked(Hash::new([0xE7; 32]))),
        );
        assert!(
            super::open::authenticate_live_finalization_serve_census(
                &fixture.verified,
                &live_ledger,
                &coordinator,
                &live_cut,
            )
            .is_err(),
            "a drifted wait receipt must not authenticate an unrelated pending payload"
        );
        coordinator
            .admission_waits
            .get_mut(&waiting_key)
            .expect("capacity wait remains installed")
            .serve_payload_receipt = Some(exact_wait_receipt);
        let cancellation = payload_store
            .persist_negative(
                first_receipt.id(),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            )
            .expect("persist exact admitted-Serve cancellation");
        let successor = LifecycleContext::new(LifecycleDigest::new([0xDD; 32]), 2);
        let successor_authority = super::super::authority::test_authority(
            successor,
            (2_u8..=5).map(|byte| LifecycleDigest::new([byte; 32])),
            0,
            geometry,
        )
        .expect("construct successor test authority");

        coordinator.rollover_with_payload_store(
            RolloverSnapshot {
                retired_context: lifecycle_context(&fixture.context),
                successor_context: successor,
                successor_predecessor: lifecycle_context(&fixture.context).id(),
                successor_authority,
                successor_ledger_root: Some(successor_ledger_root),
                serve_cancellations: vec![cancellation],
                retained_high_water: 2,
                retire_ordinals: BTreeSet::from([1, 2]),
                retire_admission_keys: BTreeSet::from([waiting_key]),
            },
            &mut payload_store,
        );

        assert_eq!(coordinator.fault(), None);
        assert_eq!(coordinator.active_context(), successor);
        drop(payload_store);
        let (_, recovered) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("reopen retired payload store");
        assert_eq!(recovered.len(), 1);
        assert!(recovered.get(first_receipt.id()).is_some());
    }

    #[test]
    fn durable_open_prunes_authenticated_pending_store_only_orphans() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let admitted_request = fixture.authenticated_serve_request(2);
        let orphan_request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let admitted_receipt = payload_store
            .persist_pending(&admitted_request)
            .expect("persist ledger-backed request");
        payload_store
            .persist_pending(&orphan_request)
            .expect("persist payload-only crash tail");
        drop(payload_store);

        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(
                &fixture.verified,
                &admitted_request,
                admitted_receipt,
            ),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        drop(coordinator);

        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (mut payload_store, payloads) =
            authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
        assert_eq!(payloads.len(), 2);
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            cut,
        )
        .expect("ledger-backed request resolves while store-only orphan is pruned");

        assert_eq!(restarted.high_water, 2);
        assert_eq!(restarted.records.len(), 2);
        assert_eq!(restarted.records[&1].state, LifecycleState::Ready);
        assert!(matches!(
            restarted.records[&2].state,
            LifecycleState::Waiting(wait) if wait.source() == WaitSource::ProducerTurn(1)
        ));
        let orphan_subject = certified_serve_key_subject(
            orphan_request.request().subject,
            orphan_request.request_hash(),
        );
        assert!(
            restarted
                .records
                .values()
                .all(|record| record.key.subject() != Some(orphan_subject))
        );
        drop(restarted);
        drop(payload_store);
        let (_, pruned) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("reopen pruned payload store");
        assert_eq!(pruned.len(), 1, "store-only crash tail is removed durably");
    }

    #[test]
    fn durable_open_rejects_a_terminal_store_only_payload() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist pending orphan");
        payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Failed(7),
            )
            .expect("persist impossible terminal orphan");
        drop(payload_store);

        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (mut payload_store, payloads) =
            authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let authority = fixture.coordinator().episode_authority;
        assert!(
            LifecycleCoordinator::open_with_authority(
                authority,
                &ledger_root,
                &mut payload_store,
                cut,
            )
            .is_err(),
            "terminal payloads cannot exist without a ledger admission"
        );
        drop(payload_store);
        let (_, recovered) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("failed open preserves terminal evidence");
        assert_eq!(recovered.len(), 1);
    }

    #[test]
    fn durable_open_rejects_a_recovery_cut_from_another_same_context_store() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let first_root = temporary.path().join("first-payloads");
        let second_root = temporary.path().join("second-payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let first = fixture.authenticated_serve_request(2);
        let second = fixture.authenticated_serve_request(3);
        let (mut first_store, _) =
            CertifiedServePayloadStoreV1::open(&first_root, &fixture.context)
                .expect("open first payload store");
        first_store
            .persist_pending(&first)
            .expect("persist first-store payload");
        drop(first_store);
        let (mut second_store, _) =
            CertifiedServePayloadStoreV1::open(&second_root, &fixture.context)
                .expect("open second payload store");
        second_store
            .persist_pending(&second)
            .expect("persist second-store payload");
        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (first_store, payloads) =
            authenticated_payload_cut(&fixture, &first_root, &body_store, &fixture.keys[0]);
        drop(first_store);
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let authority = fixture.coordinator().episode_authority;

        assert!(
            LifecycleCoordinator::open_with_authority(
                authority,
                &ledger_root,
                &mut second_store,
                cut,
            )
            .is_err(),
            "same-context stores cannot exchange authenticated recovery cuts"
        );
    }

    #[test]
    fn durable_open_rejects_a_ledger_serve_missing_from_authenticated_storage() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let admitted_payload_root = temporary.path().join("admitted-payloads");
        let empty_payload_root = temporary.path().join("empty-payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&admitted_payload_root, &fixture.context)
                .expect("open admitted payload store");
        let receipt = payload_store
            .persist_pending(&request)
            .expect("persist admitted request");
        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, receipt),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        drop(coordinator);

        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (mut payload_store, payloads) =
            authenticated_payload_cut(&fixture, &empty_payload_root, &body_store, &fixture.keys[0]);
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        assert!(
            LifecycleCoordinator::open_with_authority(
                authority,
                &ledger_root,
                &mut payload_store,
                cut,
            )
            .is_err()
        );
        let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(&fixture.context),
        )
        .expect("failed open leaves ledger readable");
        assert_eq!(ledger.high_water(), 2);
        assert_eq!(ledger.records()[0].terminal(), Some(None));
    }

    #[test]
    fn durable_open_applies_typed_negative_payload_store_ahead_cut() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist pending request");
        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(19),
            )
            .expect("persist typed negative store-ahead cut");
        drop(payload_store);
        drop(coordinator);

        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (mut payload_store, payloads) =
            authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            cut,
        )
        .expect("typed negative store-ahead cut settles atomically");

        assert_eq!(
            restarted.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Rejected(19))
        );
        assert_eq!(restarted.records[&2].state, LifecycleState::Ready);
        assert!(matches!(
            restarted.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServeNegative {
                outcome: DurableServeNegativeOutcome::Rejected(19),
                ..
            }
        ));
        let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(&fixture.context),
        )
        .expect("reload reconciled negative ledger");
        assert_eq!(
            ledger.records()[0].terminal(),
            Some(Some(TerminalOutcome::Rejected(19)))
        );
    }

    #[test]
    fn durable_open_applies_completed_payload_store_ahead_cut() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let (body, manifest) = fixture.canonical_body_and_manifest();
        let request = fixture.authenticated_serve_request_for(manifest.round, manifest.subject, 3);
        let mut body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        body_store
            .store(manifest.clone(), body.clone())
            .expect("persist canonical response body");
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist pending request");
        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let responder = 0;
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest,
            body,
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.keys[usize::try_from(responder).expect("small responder")].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        payload_store
            .persist_completed(&request, &response)
            .expect("persist completed response metadata");
        drop(payload_store);
        drop(coordinator);

        let (mut payload_store, payloads) = authenticated_payload_cut(
            &fixture,
            &payload_root,
            &body_store,
            &fixture.keys[usize::try_from(responder).expect("small responder")],
        );
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            cut,
        )
        .expect("completed store-ahead cut settles atomically");
        let response_digest = digest_from_bytes(HashOf::new(&response).as_ref());

        assert_eq!(
            restarted.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
        );
        assert_eq!(restarted.records[&2].state, LifecycleState::Ready);
        assert!(matches!(
            restarted.durable_records[&1].payload,
            DurablePayloadReference::CertifiedServeCompleted {
                response,
                ..
            } if response == response_digest
        ));
        let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(&fixture.context),
        )
        .expect("reload reconciled completion ledger");
        assert_eq!(
            ledger.records()[0].terminal(),
            Some(Some(TerminalOutcome::Completed(Some(response_digest))))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn settled_negative_frame_persists_and_reopens_with_the_exact_replay_pair() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let request = fixture.authenticated_serve_request(3);
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist pending request");
        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let lease = execute_ready_turn(&mut coordinator);
        let terminal = payload_store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(41),
            )
            .expect("persist exact terminal frame");
        assert!(reduce_negative_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        drop(coordinator);
        drop(payload_store);

        let body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        let (mut payload_store, payloads) =
            authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
        let recovered = payloads
            .get(pending.id())
            .expect("authenticated cut retains terminal request");
        let projection =
            recovered_certified_serve_projection(lifecycle_context(&fixture.context), recovered)
                .expect("project exact terminal recovery frame");
        let (candidate, payload, outcome, replay) = projection.into_parts();
        assert_eq!(candidate.payload, payload);
        assert_eq!(outcome, Some(TerminalOutcome::Rejected(41)));
        assert!(replay.as_ref().is_some_and(|replay| {
            replay.exactly_matches_recovered_candidate(
                lifecycle_context(&fixture.context),
                &candidate,
            )
        }));
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            cut,
        )
        .expect("steady terminal negative frame reopens exactly");
        assert_eq!(
            restarted.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Rejected(41))
        );
        assert!(
            restarted.durable_records[&1]
                .replay_authority
                .same_persisted_family(&restarted.durable_records[&2].replay_authority)
        );
        assert!(
            restarted.durable_records[&2]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn settled_completed_frame_persists_and_reopens_with_the_exact_replay_pair() {
        let temporary = TempDir::new().expect("temporary directory");
        let ledger_root = temporary.path().join("ledger");
        let payload_root = temporary.path().join("payloads");
        let body_root = temporary.path().join("bodies");
        let fixture = Fixture::new();
        let (body, manifest) = fixture.canonical_body_and_manifest();
        let request = fixture.authenticated_serve_request_for(manifest.round, manifest.subject, 3);
        let mut body_store =
            V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
        body_store
            .store(manifest.clone(), body.clone())
            .expect("persist canonical response body");
        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
                .expect("open payload store");
        let pending = payload_store
            .persist_pending(&request)
            .expect("persist pending request");
        let mut coordinator = fixture.coordinator();
        let authority = coordinator.episode_authority.clone();
        coordinator
            .attach_empty_test_ledger(&ledger_root)
            .expect("attach empty durable ledger");
        assert!(matches!(
            coordinator.admit_certified_serve(&fixture.verified, &request, pending),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        let lease = execute_ready_turn(&mut coordinator);
        let responder = 0;
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest,
            body,
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.keys[usize::try_from(responder).expect("small responder")].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        let terminal = payload_store
            .persist_completed(&request, &response)
            .expect("persist exact completed frame");
        assert!(reduce_completed_serve_for_test(
            &mut coordinator,
            lease,
            terminal,
        ));
        assert!(
            coordinator.durable_records[&1]
                .replay_authority
                .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
        );
        assert!(
            coordinator.durable_records[&2]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
        drop(coordinator);
        drop(payload_store);

        let (mut payload_store, payloads) = authenticated_payload_cut(
            &fixture,
            &payload_root,
            &body_store,
            &fixture.keys[usize::try_from(responder).expect("small responder")],
        );
        let recovered = payloads
            .get(pending.id())
            .expect("authenticated cut retains completed request");
        let projection =
            recovered_certified_serve_projection(lifecycle_context(&fixture.context), recovered)
                .expect("project exact completed recovery frame");
        let (candidate, payload, outcome, replay) = projection.into_parts();
        let response_digest = digest_from_bytes(HashOf::new(&response).as_ref());
        assert_eq!(candidate.payload, payload);
        assert_eq!(
            outcome,
            Some(TerminalOutcome::Completed(Some(response_digest)))
        );
        assert!(replay.as_ref().is_some_and(|replay| {
            replay.exactly_matches_recovered_candidate(
                lifecycle_context(&fixture.context),
                &candidate,
            )
        }));
        let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            cut,
        )
        .expect("steady completed frame reopens exactly");
        assert_eq!(
            restarted.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
        );
        assert!(
            restarted.durable_records[&1]
                .replay_authority
                .same_persisted_family(&restarted.durable_records[&2].replay_authority)
        );
        assert!(
            restarted.durable_records[&1]
                .replay_authority
                .certified_serve_frame_hash_is(terminal.payload_hash())
        );
    }

    #[allow(clippy::too_many_lines)]
    fn accepted_effects(fixture: &Fixture) -> Vec<ExpectedProjection> {
        let mut unsigned_proposal = fixture.proposal.clone();
        unsigned_proposal.signature.clear();
        let mut unsigned_prepare_vote = fixture.prepare_vote.clone();
        unsigned_prepare_vote.signature.clear();
        let mut unsigned_commit_vote = fixture.commit_vote.clone();
        unsigned_commit_vote.signature.clear();
        let mut unsigned_timeout_vote = fixture.timeout_vote.clone();
        unsigned_timeout_vote.signature.clear();

        let certified_sources = fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let entered_tag = EventTag::new(
            fixture.context.height,
            fixture.round.view + 1,
            Generation::new(0),
        );
        let proposal_conflict = AdapterEquivocationEvidence::proposal_for_test(
            fixture.proposal.clone(),
            fixture.proposal_for(0x51, 0x51),
        );
        let (first_vote, second_vote) = vote_conflict(fixture);
        let vote_conflict = AdapterEquivocationEvidence::vote_for_test(first_vote, second_vote);
        let timeout_conflict = AdapterEquivocationEvidence::timeout_vote_for_test(
            fixture.timeout_vote.clone(),
            wire::TimeoutVote {
                highest_prepare_qc: Some(fixture.prepare_qc.clone()),
                signature: vec![0x54],
                ..fixture.timeout_vote.clone()
            },
        );

        vec![
            (
                AdapterEffect::Sign {
                    tag: fixture.tag,
                    request: SignRequest::Proposal(unsigned_proposal),
                },
                LifecycleWorkClass::SignProposal,
                LifecyclePhase::Proposal,
                LifecycleStageKind::SignProposal,
            ),
            (
                AdapterEffect::Sign {
                    tag: fixture.tag,
                    request: SignRequest::Vote(unsigned_prepare_vote),
                },
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Prepare,
                LifecycleStageKind::SignPrepareVote,
            ),
            (
                AdapterEffect::Sign {
                    tag: fixture.tag,
                    request: SignRequest::Vote(unsigned_commit_vote),
                },
                LifecycleWorkClass::SignVote,
                LifecyclePhase::Commit,
                LifecycleStageKind::SignCommitVote,
            ),
            (
                AdapterEffect::Sign {
                    tag: fixture.tag,
                    request: SignRequest::TimeoutVote(unsigned_timeout_vote),
                },
                LifecycleWorkClass::SignTimeout,
                LifecyclePhase::Timeout,
                LifecycleStageKind::SignTimeoutVote,
            ),
            (
                AdapterEffect::FetchBody {
                    tag: fixture.tag,
                    round: fixture.round,
                    subject: fixture.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                },
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                LifecycleStageKind::FetchBody,
            ),
            (
                AdapterEffect::FetchBody {
                    tag: fixture.tag,
                    round: fixture.round,
                    subject: fixture.subject,
                    manifest: None,
                    certified_sources,
                    certificate: Some(fixture.prepare_qc.clone()),
                },
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                LifecycleStageKind::FetchBody,
            ),
            (
                AdapterEffect::Apply {
                    tag: fixture.tag,
                    subject: fixture.subject,
                    certificate: fixture.commit_qc.clone(),
                },
                LifecycleWorkClass::Apply,
                LifecyclePhase::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(fixture.proposal.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastProposal,
                LifecycleStageKind::BroadcastProposal,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastPrepareVote,
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(fixture.commit_vote.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastCommitVote,
                LifecycleStageKind::BroadcastCommitVote,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastPrepareQc,
                LifecycleStageKind::BroadcastPrepareQc,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.commit_qc.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastCommitQc,
                LifecycleStageKind::BroadcastCommitQc,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(fixture.timeout_vote.clone()),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastTimeoutVote,
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
            (
                AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutCertificate(
                        fixture.timeout_certificate.clone(),
                    ),
                )),
                LifecycleWorkClass::Broadcast,
                LifecyclePhase::BroadcastTc,
                LifecycleStageKind::BroadcastTc,
            ),
            (
                AdapterEffect::EnterView {
                    tag: entered_tag,
                    certificate: fixture.timeout_certificate.clone(),
                    protected_lock: Some(fixture.prepare_qc.clone()),
                },
                LifecycleWorkClass::EnterView,
                LifecyclePhase::EnterView,
                LifecycleStageKind::EnterView,
            ),
            (
                AdapterEffect::ReportEquivocation {
                    evidence: proposal_conflict,
                },
                LifecycleWorkClass::EquivocationReport,
                LifecyclePhase::DiagnosticProposalEquivocation,
                LifecycleStageKind::ReportProposalEquivocation,
            ),
            (
                AdapterEffect::ReportEquivocation {
                    evidence: vote_conflict,
                },
                LifecycleWorkClass::EquivocationReport,
                LifecyclePhase::DiagnosticVoteEquivocation,
                LifecycleStageKind::ReportVoteEquivocation,
            ),
            (
                AdapterEffect::ReportEquivocation {
                    evidence: timeout_conflict,
                },
                LifecycleWorkClass::EquivocationReport,
                LifecyclePhase::DiagnosticTimeoutEquivocation,
                LifecycleStageKind::ReportTimeoutEquivocation,
            ),
            (
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.subject,
                    certificate: fixture.prepare_qc.clone(),
                },
                LifecycleWorkClass::InvalidBodyReport,
                LifecyclePhase::DiagnosticInvalidBody,
                LifecycleStageKind::ReportInvalidBody,
            ),
        ]
    }

    #[test]
    fn every_adapter_effect_class_and_specialized_phase_projects_ready_one_slot_work() {
        let fixture = Fixture::new();
        let cases = accepted_effects(&fixture);
        assert_eq!(cases.len(), 19);
        for (ordinal, (effect, work_class, phase, stage_kind)) in (1_u128..).zip(cases) {
            let ownership = bound_ownership(&effect, fixture.tag, ordinal);
            let projected = candidate(&fixture, &effect, &ownership);
            assert_candidate_shape(
                &projected, &effect, &ownership, work_class, phase, stage_kind,
            );
            if phase == LifecyclePhase::Timeout {
                assert_eq!(projected.key.proposal_round(), None);
                assert_eq!(projected.key.subject(), None);
                assert_eq!(projected.key.execution_commitment(), None);
            }
            let mut coordinator = fixture.coordinator();
            let decision = coordinator
                .admit_bound_adapter_effect(&fixture.verified, &effect, &ownership)
                .expect("authenticated effect projection succeeds");
            if matches!(
                work_class,
                LifecycleWorkClass::Store
                    | LifecycleWorkClass::Validate
                    | LifecycleWorkClass::Apply
            ) {
                assert_eq!(
                    decision,
                    AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata),
                    "raw body-stage projection cannot bypass receipt-bound staging"
                );
            } else {
                assert!(matches!(
                    decision,
                    AdmissionDecision::Admitted { ordinal: 1, .. }
                ));
            }
        }
    }

    #[test]
    fn certified_store_and_validate_inherit_authority_but_require_receipt_bound_staging() {
        let fixture = Fixture::new();
        let fetch = AdapterEffect::FetchBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
            manifest: None,
            certified_sources: fixture
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
            certificate: Some(fixture.prepare_qc.clone()),
        };
        let fetch_owner = bound_ownership(&fetch, fixture.tag, 20);
        let store = AdapterEffect::StoreBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
        };
        let store_owner = fetch_owner
            .rebind_as_inherited_adapter_effect(&store)
            .expect("Fetch authorizes exact Store successor");
        let validate = AdapterEffect::ValidateBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
        };
        let validate_owner = store_owner
            .rebind_as_inherited_adapter_effect(&validate)
            .expect("Store authorizes exact Validate successor");

        let store_candidate = candidate(&fixture, &store, &store_owner);
        let validate_candidate = candidate(&fixture, &validate, &validate_owner);
        assert_candidate_shape(
            &store_candidate,
            &store,
            &store_owner,
            LifecycleWorkClass::Store,
            LifecyclePhase::Store,
            LifecycleStageKind::StoreBody,
        );
        assert_candidate_shape(
            &validate_candidate,
            &validate,
            &validate_owner,
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            LifecycleStageKind::ValidateBody,
        );
        let expected_commitment = Some(execution_commitment(
            fixture.prepare_qc.execution_commitment,
        ));
        assert_eq!(
            store_candidate.key.execution_commitment(),
            expected_commitment
        );
        assert_eq!(
            validate_candidate.key.execution_commitment(),
            expected_commitment
        );
        assert_eq!(store_candidate.causal_root, validate_candidate.causal_root);

        let mut coordinator = fixture.coordinator();
        assert_eq!(
            coordinator
                .admit_bound_adapter_effect(&fixture.verified, &store, &store_owner)
                .expect("raw Store projection is structurally authenticated"),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata),
            "Fetch-to-Store admission requires the receipt-bound body transition"
        );
        assert_eq!(
            coordinator
                .admit_bound_adapter_effect(&fixture.verified, &validate, &validate_owner)
                .expect("raw Validate projection is structurally authenticated"),
            AdmissionDecision::Rejected(AdmissionRejection::InvalidDurableMetadata),
            "Store-to-Validate admission requires the receipt-bound body transition"
        );
    }

    #[test]
    fn recovery_cut_consumes_exact_terminal_validate_body_outcome() {
        let temporary = TempDir::new().expect("temporary lifecycle recovery roots");
        let fixture = Fixture::new();
        let fetch = AdapterEffect::FetchBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
            manifest: None,
            certified_sources: fixture
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
            certificate: Some(fixture.prepare_qc.clone()),
        };
        let fetch_owner = bound_ownership(&fetch, fixture.tag, 20);
        let store = AdapterEffect::StoreBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
        };
        let store_owner = fetch_owner
            .rebind_as_inherited_adapter_effect(&store)
            .expect("Fetch authorizes exact Store successor");
        let validate = AdapterEffect::ValidateBody {
            tag: fixture.tag,
            round: fixture.round,
            subject: fixture.subject,
        };
        let validate_owner = store_owner
            .rebind_as_inherited_adapter_effect(&validate)
            .expect("Store authorizes exact Validate successor");
        let mut validate_candidate = candidate(&fixture, &validate, &validate_owner);
        let durable = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.round,
            fixture.subject,
            HashOf::new(&fixture.manifest),
        );
        validate_candidate.payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(lifecycle_context(&fixture.context), &durable)
                .expect("validated body belongs to the fixture lifecycle context"),
        );
        let outcome =
            crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::validated_for_test(
                crate::sumeragi::v2_body_store::ValidatedBodyReceipt::for_test_with_commitment(
                    durable,
                    fixture.prepare_qc.execution_commitment,
                ),
            );
        let body_store = V2BodyStore::open(temporary.path().join("body"), fixture.context.clone())
            .expect("open exact-context body store");
        let (mut payload_store, payloads) = authenticated_payload_cut(
            &fixture,
            &temporary.path().join("payload"),
            &body_store,
            &fixture.keys[0],
        );
        let owner = OwnerId::new(validate_candidate.causal_root, 1);
        let record = super::super::ledger::LifecycleLedgerRecordV1::new(
            validate_candidate.key,
            owner,
            1,
            validate_candidate.work_class,
            validate_candidate.stage,
            Some(TerminalOutcome::Advanced),
            validate_candidate.reconstruction_source,
            validate_candidate.payload,
            validate_candidate.replay_authority.clone(),
            super::super::schema::DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate ledger row");
        let ledger = super::super::ledger::LifecycleLedgerV1::new(
            lifecycle_context(&fixture.context),
            1,
            vec![record],
            std::collections::BTreeMap::new(),
        )
        .expect("construct exact no-child terminal ledger");
        let recovery = AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
            ledger.clone(),
            [],
            [(validate_candidate.clone(), outcome)],
            payloads,
        )
        .expect("exact body outcome seals the terminal Validate recovery identity");
        let ledger_root = temporary.path().join("ledger");
        let (ledger_store, empty) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(&fixture.context),
        )
        .expect("open empty lifecycle ledger");
        assert!(empty.records().is_empty());
        ledger_store
            .persist(&ledger)
            .expect("persist exact no-child terminal ledger");
        drop(ledger_store);
        let authority = fixture.coordinator().episode_authority;
        let reopened = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut payload_store,
            recovery,
        )
        .expect("open terminal Validate with exact no-child recovery proof");
        assert_eq!(
            reopened.records[&1].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            reopened.durable_records[&1].continuation,
            super::super::schema::DurableContinuation::AdvancedNoSuccessor
        );

        let rejected =
            crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::rejected_for_test(
                crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
                    fixture.context.id(),
                    fixture.round,
                    fixture.subject,
                    HashOf::new(&fixture.manifest),
                ),
            );
        let (_payload_store, payloads) = authenticated_payload_cut(
            &fixture,
            &temporary.path().join("payload-foreign"),
            &body_store,
            &fixture.keys[0],
        );
        assert!(
            AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
                ledger,
                [validate_candidate.clone()],
                [(validate_candidate, rejected)],
                payloads,
            )
            .is_none(),
            "one semantic key cannot be both live recovery work and a no-child tombstone proof"
        );
    }

    #[test]
    fn coordinator_method_enforces_zero_to_one_retry_and_foreign_owner_rejection() {
        let fixture = Fixture::new();
        let mut unsigned = fixture.proposal.clone();
        unsigned.signature.clear();
        let effect = AdapterEffect::Sign {
            tag: fixture.tag,
            request: SignRequest::Proposal(unsigned),
        };
        let exact = bound_ownership(&effect, fixture.tag, 30);
        let foreign_tag = EventTag::new(
            fixture.tag.height(),
            fixture.tag.view(),
            Generation::new(fixture.tag.generation().get() + 1),
        );
        let foreign = bound_ownership(&effect, foreign_tag, 31);
        let mut coordinator = fixture.coordinator();

        assert!(matches!(
            coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &exact),
            Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
        ));
        assert_eq!(coordinator.records.len(), 1, "0 -> 1 owner admission");
        assert!(matches!(
            coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &exact),
            Ok(AdmissionDecision::Retry {
                ordinal: 1,
                action: RetryAction::StutterLiveSigner,
                ..
            })
        ));
        assert_eq!(coordinator.records.len(), 1, "same-owner retry is 1 -> 1");
        assert_eq!(
            coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &foreign),
            Ok(AdmissionDecision::Rejected(
                AdmissionRejection::ForeignOwner
            ))
        );
        assert_eq!(coordinator.records.len(), 1);
    }

    #[test]
    fn unbound_and_foreign_context_effects_fail_before_admission() {
        let fixture = Fixture::new();
        let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
        ));
        let unbound = RuntimeEffectOwnership::fresh_for_test(fixture.tag, 40);
        let mut coordinator = fixture.coordinator();
        assert_eq!(
            coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &unbound),
            Err(AdapterEffectAdmissionError::UnboundEffect)
        );
        assert!(coordinator.records.is_empty());

        let ownership = bound_ownership(&effect, fixture.tag, 41);
        let foreign_context = LifecycleContext::new(LifecycleDigest::new([0xFF; 32]), 1);
        let mut foreign = LifecycleCoordinator::new(
            foreign_context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.map(|class| (class, 64))),
        );
        assert_eq!(
            foreign.admit_bound_adapter_effect(&fixture.verified, &effect, &ownership),
            Err(AdapterEffectAdmissionError::ForeignContext)
        );
        assert!(foreign.records.is_empty());
    }

    #[test]
    fn broadcast_vote_and_qc_have_collision_free_specialized_keys() {
        let fixture = Fixture::new();
        let vote = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
        ));
        let qc = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
        ));
        let vote_owner = bound_ownership(&vote, fixture.tag, 50);
        let qc_owner = bound_ownership(&qc, fixture.tag, 51);
        let vote_candidate = candidate(&fixture, &vote, &vote_owner);
        let qc_candidate = candidate(&fixture, &qc, &qc_owner);

        assert_eq!(vote_candidate.key.subject(), qc_candidate.key.subject());
        assert_eq!(
            vote_candidate.key.execution_commitment(),
            qc_candidate.key.execution_commitment()
        );
        assert_eq!(
            vote_candidate.key.phase(),
            LifecyclePhase::BroadcastPrepareVote
        );
        assert_eq!(qc_candidate.key.phase(), LifecyclePhase::BroadcastPrepareQc);
        assert_ne!(vote_candidate.key, qc_candidate.key);
        assert_ne!(
            vote_candidate.physical_geometry.initial[0].digest(),
            qc_candidate.physical_geometry.initial[0].digest()
        );
    }

    #[test]
    fn all_eight_auxiliary_broadcast_payloads_are_explicitly_rejected() {
        let fixture = Fixture::new();
        let certified_request = wire::CertifiedBodyRequest {
            round: fixture.round,
            subject: fixture.subject,
            certificate: fixture.prepare_qc.clone(),
            requester: fixture.context.roster[0].validator.clone(),
            signature: vec![0x61],
        };
        let commit_request = wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            network_id: fixture.context.network_id,
            context_id: fixture.context.id(),
            height: fixture.context.height,
            requester: fixture.context.roster[0].validator.clone(),
            signature: vec![0x62],
        };
        let payloads = vec![
            wire::ConsensusMessageV2Payload::PayloadManifest(fixture.manifest.clone()),
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::new(&fixture.manifest),
                index: 0,
                bytes: fixture.encoded_chunks[0].clone(),
                sender: 0,
                signature: vec![0x63],
            }),
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(certified_request.clone()),
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
                request_hash: HashOf::new(&certified_request),
                manifest: fixture.manifest.clone(),
                body: fixture.body.clone(),
                responder: 0,
                signature: vec![0x64],
            }),
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(commit_request.clone()),
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                wire::CommitCertificateResponse {
                    request_hash: HashOf::new(&commit_request),
                    certificate: fixture.commit_qc.clone(),
                    responder: fixture.context.roster[0].validator.clone(),
                    signature: vec![0x65],
                },
            ),
            wire::ConsensusMessageV2Payload::VrfCommit(wire::VrfCommit {
                epoch: fixture.context.epoch,
                commitment: [0x66; 32],
                signer: 0,
                bls_sig: vec![0x66],
            }),
            wire::ConsensusMessageV2Payload::VrfReveal(wire::VrfReveal {
                epoch: fixture.context.epoch,
                reveal: [0x67; 32],
                signer: 0,
                vrf_proof: vec![0x67],
                bls_sig: vec![0x67],
            }),
        ];
        assert_eq!(payloads.len(), 8);

        for (ordinal, payload) in (60_u128..).zip(payloads) {
            let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(payload));
            let ownership = bound_ownership(&effect, fixture.tag, ordinal);
            let mut coordinator = fixture.coordinator();
            assert_eq!(
                coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &ownership),
                Err(AdapterEffectAdmissionError::UnsupportedBroadcastPayload)
            );
            assert!(coordinator.records.is_empty());
        }
    }

    #[test]
    fn enter_view_key_and_physical_identity_retain_protected_commitment() {
        let fixture = Fixture::new();
        let entered_tag = EventTag::new(
            fixture.context.height,
            fixture.round.view + 1,
            Generation::new(0),
        );
        let second_lock = wire::QuorumCertificate {
            execution_commitment: execution_commitment_for(0x71),
            aggregate_signature: vec![0x71],
            ..fixture.prepare_qc.clone()
        };
        let first = AdapterEffect::EnterView {
            tag: entered_tag,
            certificate: fixture.timeout_certificate.clone(),
            protected_lock: Some(fixture.prepare_qc.clone()),
        };
        let second = AdapterEffect::EnterView {
            tag: entered_tag,
            certificate: fixture.timeout_certificate.clone(),
            protected_lock: Some(second_lock),
        };
        let first_owner = bound_ownership(&first, fixture.tag, 70);
        let second_owner = bound_ownership(&second, fixture.tag, 71);
        let first_candidate = candidate(&fixture, &first, &first_owner);
        let second_candidate = candidate(&fixture, &second, &second_owner);

        assert_eq!(
            first_candidate.key.subject(),
            second_candidate.key.subject()
        );
        assert_ne!(
            first_candidate.key.execution_commitment(),
            second_candidate.key.execution_commitment()
        );
        assert_ne!(first_candidate.key, second_candidate.key);
        assert_ne!(
            first_candidate.physical_geometry.initial[0].digest(),
            second_candidate.physical_geometry.initial[0].digest()
        );
    }

    #[test]
    fn diagnostic_logical_identity_normalizes_order_and_signatures_but_physical_does_not() {
        let fixture = Fixture::new();
        let (first, second) = vote_conflict(&fixture);
        let mut resigned = first.clone();
        resigned.signature = vec![0x7F];
        let forward = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(first.clone(), second.clone()),
        };
        let reversed = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(second.clone(), first.clone()),
        };
        let re_signed = AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(resigned, second),
        };
        let forward_owner = bound_ownership(&forward, fixture.tag, 80);
        let reversed_owner = bound_ownership(&reversed, fixture.tag, 81);
        let re_signed_owner = bound_ownership(&re_signed, fixture.tag, 82);
        let forward_candidate = candidate(&fixture, &forward, &forward_owner);
        let reversed_candidate = candidate(&fixture, &reversed, &reversed_owner);
        let re_signed_candidate = candidate(&fixture, &re_signed, &re_signed_owner);

        assert_eq!(forward_candidate.key, reversed_candidate.key);
        assert_eq!(forward_candidate.key, re_signed_candidate.key);
        let forward_digest = forward_candidate.physical_geometry.initial[0].digest();
        let reversed_digest = reversed_candidate.physical_geometry.initial[0].digest();
        let re_signed_digest = re_signed_candidate.physical_geometry.initial[0].digest();
        assert_ne!(forward_digest, reversed_digest);
        assert_ne!(forward_digest, re_signed_digest);
        assert_ne!(reversed_digest, re_signed_digest);
    }

    #[test]
    fn bound_but_drifted_carriers_fail_closed_without_records() {
        let fixture = Fixture::new();
        let mut signed_proposal = fixture.proposal.clone();
        signed_proposal.signature = vec![0x91];
        let pre_signed = AdapterEffect::Sign {
            tag: fixture.tag,
            request: SignRequest::Proposal(signed_proposal),
        };
        let invalid_body = AdapterEffect::ReportInvalidCertifiedBody {
            subject: fixture.proposal_for(0x92, 0x92).subject,
            certificate: fixture.prepare_qc.clone(),
        };
        let foreign_protocol = AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            protocol_version: wire::PROTOCOL_VERSION + 1,
            payload: wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
        });

        for (ordinal, effect) in (90_u128..).zip([pre_signed, invalid_body, foreign_protocol]) {
            let ownership = bound_ownership(&effect, fixture.tag, ordinal);
            let mut coordinator = fixture.coordinator();
            assert_eq!(
                coordinator.admit_bound_adapter_effect(&fixture.verified, &effect, &ownership),
                Err(AdapterEffectAdmissionError::InvalidCarrier)
            );
            assert!(coordinator.records.is_empty());
        }
    }
}
