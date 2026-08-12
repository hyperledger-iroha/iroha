//! Scheduler-free registry for exact concrete lifecycle work.
//!
//! The logical coordinator retains only authenticated slot digests. This
//! module keeps the corresponding process-local effect values in a separate,
//! deterministic map so planning never makes the coordinator own physical
//! bytes or service handles.

use std::{collections::BTreeMap, fmt, path::Path};

use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    peer::PeerId,
};
use norito::codec::Encode;

#[cfg(test)]
use super::{AdmissionRequest, LeaseId};
use super::{
    AuthenticatedLifecycleRecoveryCut, CandidateAdmission, CapacityClass, InitialLifecycleState,
    LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey, LifecyclePhase,
    LifecycleStage, LifecycleStageKind, LifecycleWorkClass, OwnerId, PhysicalReplacement,
    PhysicalSlot, PhysicalSlotId, PredecessorScope, ReadyEvent, TurnLease, WaitSource, WaitToken,
    authority,
    body_pipeline_transition::{
        SealedInvalidBodyReportProjection, SealedInvalidBodyReportProjectionPermit,
        SealedValidateNoSuccessorProjection, SealedValidateNoSuccessorProjectionPermit,
    },
    ingress_position::PendingFairIngressIdentity,
    open::{LifecycleOpenCommitError, LifecycleOpenError, PreparedLifecycleCoordinatorOpen},
    projection::{self, AdapterEffectAdmissionError, certified_fetch_lifecycle_key},
    replay_authority::{
        AuthenticatedCertifiedFetchReplayOriginV1, CertifiedFetchReplayEvidenceV1,
        CertifiedStoreReplayEvidenceV1, CertifiedValidateReplayEvidenceV1,
        DurableValidateReplayEvidenceV1, RemoteProposalFetchReplayEvidenceV1,
        RemoteProposalStoreReplayEvidenceV1, RemoteProposalStoredReplayEvidenceV1,
        RemoteProposalValidateReplayEvidenceV1, SealedLiveWalPersistedEffectV1,
        SignedBroadcastReplayEvidenceV1, SignedEquivocationReplayEvidenceV1,
    },
    schema::DurablePayloadReference,
    selector::CertifiedFetchCompletionAuthority,
    wal_recovery::{
        AuthenticatedWalVoteLifecycleRepair, DurableAuthenticatedWalVoteLifecycleRepair,
        RecoveredWalVoteLifecycleRepairError,
        authenticate_recovered_wal_vote_lifecycle_from_durable_body,
        authenticate_recovered_wal_vote_lifecycle_from_ledger_parent,
    },
};
#[cfg(test)]
use crate::sumeragi::v2_runtime::bind_adapter_effect_batch_ownership;
use crate::sumeragi::{
    InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterEffect, PreparedInvalidBodyReportAdapterReplay,
        PreparedReadyDurableValidateAdapterPublication, ReadyDurableValidateAdapterPublicationKind,
        RecoveredWalVoteSign, SumeragiV2Adapter, VerifiedHeightContext,
    },
    v2_body_store::{
        BodyValidationError, BodyValidationRejectionIdentity, DurableBodyReceipt,
        DurableBodyValidationOutcome, DurableCertifiedFetchBodyReceipt, RecoveredValidatedBodyCut,
        RecoveredValidatedBodyCutError, V2BodyStore, V2BodyStoreError, ValidatedBodyReceipt,
    },
    v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
    v2_core::EventTag,
    v2_runtime::{
        PendingRuntimeEffectBinding, RuntimeCandidateSemanticStatement, RuntimeEffectOwnership,
        reconstruct_recovered_wal_vote_successor,
    },
};

/// Logical address of one exact concrete-work slot.
///
/// Digest-only indexing is intentionally forbidden: two logical body stages
/// may retain the same physical carrier while inheriting different authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ConcreteWorkAddress {
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
}

/// One-shot proof that an installed body carrier owns candidate projection.
///
/// Only this registry module can construct the permit. Replay evidence may
/// consume it to attach its private canonical authority, so sibling callers
/// cannot project a candidate from separated effect, receipt, and pending
/// parts.
pub(in crate::sumeragi) struct InstalledBodyCandidateProjectionPermit {
    _linearity: InstalledBodyCandidateProjectionLinearity,
}

struct InstalledBodyCandidateProjectionLinearity;

impl Drop for InstalledBodyCandidateProjectionLinearity {
    fn drop(&mut self) {}
}

impl InstalledBodyCandidateProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: InstalledBodyCandidateProjectionLinearity,
        }
    }
}

/// One-shot proof that a move-only successor token owns candidate projection.
///
/// This permit is distinct from installed-carrier projection: the successor is
/// still nested under its exact parent registry borrow and cannot be installed
/// or admitted independently of the future composite transaction.
pub(in crate::sumeragi) struct SealedBodySuccessorProjectionPermit {
    _linearity: SealedBodySuccessorProjectionLinearity,
}

struct SealedBodySuccessorProjectionLinearity;

impl Drop for SealedBodySuccessorProjectionLinearity {
    fn drop(&mut self) {}
}

impl SealedBodySuccessorProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: SealedBodySuccessorProjectionLinearity,
        }
    }
}

impl ConcreteWorkAddress {
    /// Construct an address only after coordinator admission returned its
    /// immutable owner and record ordinal.
    pub(super) const fn new(owner: OwnerId, ordinal: u128, slot: PhysicalSlotId) -> Option<Self> {
        if ordinal == 0
            || owner.first_admission_ordinal() == 0
            || owner.first_admission_ordinal() > ordinal
        {
            return None;
        }
        Some(Self {
            owner,
            ordinal,
            slot,
        })
    }
}

/// Same-address logical replacement coordinates prepared by the coordinator.
///
/// One slot field represents both sides of the replacement, so this value
/// cannot describe an old-slot/new-slot move. Construction also rejects a
/// no-op digest before either the queue or registry may be mutated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CertifiedFetchReplacementLocation {
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    incumbent_digest: LifecycleDigest,
    replacement_digest: LifecycleDigest,
}

impl CertifiedFetchReplacementLocation {
    /// Seal one exact logical replacement at an already admitted address.
    pub(super) fn new(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        incumbent_digest: LifecycleDigest,
        replacement_digest: LifecycleDigest,
    ) -> Option<Self> {
        if ConcreteWorkAddress::new(owner, ordinal, slot).is_none()
            || incumbent_digest == replacement_digest
        {
            return None;
        }
        Some(Self {
            owner,
            ordinal,
            slot,
            incumbent_digest,
            replacement_digest,
        })
    }

    /// Return the immutable logical owner.
    pub(super) const fn owner(self) -> OwnerId {
        self.owner
    }

    /// Return the existing record ordinal.
    pub(super) const fn ordinal(self) -> u128 {
        self.ordinal
    }

    /// Return the one physical slot shared by incumbent and completion.
    pub(super) const fn slot(self) -> PhysicalSlotId {
        self.slot
    }

    /// Return the digest expected on the incumbent `FetchBody`.
    pub(super) const fn incumbent_digest(self) -> LifecycleDigest {
        self.incumbent_digest
    }

    /// Return the queue-identity digest published by the logical replacement.
    pub(super) const fn replacement_digest(self) -> LifecycleDigest {
        self.replacement_digest
    }

    const fn address(self) -> ConcreteWorkAddress {
        ConcreteWorkAddress {
            owner: self.owner,
            ordinal: self.ordinal,
            slot: self.slot,
        }
    }
}

/// Exact response owner returned only after the selected queue CAS succeeds.
///
/// Construction and consumption remain private to this inert module. The
/// future composite transaction must move its checked-dequeue result into this
/// value; accepting a cloned pre-CAS envelope would not prove physical removal.
#[derive(Debug)]
#[must_use = "a dequeued response still owns its exact ingress carrier"]
struct CertifiedFetchDequeuedResponse {
    ingress_identity: PendingFairIngressIdentity,
    inbound: InboundBlockMessage,
}

impl CertifiedFetchDequeuedResponse {
    // TODO: Let only the future output-permitted composite queue transaction
    // call this mint when it exposes its checked-dequeue success token.
    #[allow(dead_code)]
    fn after_exact_dequeue(
        ingress_identity: PendingFairIngressIdentity,
        inbound: InboundBlockMessage,
    ) -> Self {
        Self {
            ingress_identity,
            inbound,
        }
    }
}

/// Closed installed form of an authenticated certified-Fetch completion.
///
/// This payload directly owns the incumbent effect and pending binding moved
/// unchanged from the pending-adapter variant, plus the exact dequeued response
/// carrier and the facts against which it was authenticated.
#[derive(Debug)]
struct CertifiedFetchCompletion {
    address: ConcreteWorkAddress,
    incumbent_effect: AdapterEffect,
    incumbent_pending: PendingRuntimeEffectBinding,
    incumbent_digest: LifecycleDigest,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    durable_receipt: DurableCertifiedFetchBodyReceipt,
    replay_evidence: CertifiedFetchReplayEvidenceV1,
    dequeued: CertifiedFetchDequeuedResponse,
}

impl CertifiedFetchCompletion {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        self.incumbent_pending
            .exactly_binds_adapter_effect(&self.incumbent_effect)
            && matches!(&self.incumbent_effect, AdapterEffect::FetchBody { .. })
            && ConcreteWorkAddress::new(self.address.owner, self.address.ordinal, self.address.slot)
                == Some(self.address)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(
                    self.incumbent_pending.causal_lifecycle_key(),
                ))
            && self.incumbent_digest
                == digest_from_hash(self.incumbent_pending.exact_effect_identity())
            && installed_digest == self.dequeued.ingress_identity.digest()
            && self.dequeued.ingress_identity.physical_admission_ordinal() != 0
            && exact_dequeued_response_matches(
                &self.dequeued,
                &self.incumbent_effect,
                self.request_hash,
                self.response_hash,
                &self.authenticated_responder,
                &self.durable_receipt,
            )
            && dequeued_certified_response(&self.dequeued).is_some_and(|response| {
                self.replay_evidence.exactly_matches_fetch(
                    &self.incumbent_effect,
                    response,
                    &self.durable_receipt,
                )
            })
    }
}

/// Closed durable form of one admitted `StoreBody` effect.
///
/// The expected manifest hash is transferred independently from the
/// authenticated parent response. It deliberately remains distinct from the
/// body-store receipt's own manifest hash so validation proves agreement
/// between the transport family and the durable catalog entry.
#[derive(Debug)]
struct DurableStoreBody {
    address: ConcreteWorkAddress,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: CertifiedStoreReplayEvidenceV1,
}

impl DurableStoreBody {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        let AdapterEffect::StoreBody { round, subject, .. } = &self.effect else {
            return false;
        };
        ConcreteWorkAddress::new(self.address.owner, self.address.ordinal, self.address.slot)
            == Some(self.address)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && installed_digest == digest_from_hash(self.pending.exact_effect_identity())
            && self.durable_receipt.context_id() == round.context_id
            && self.durable_receipt.round() == *round
            && self.durable_receipt.subject() == *subject
            && self.durable_receipt.manifest_hash() == self.expected_manifest_hash
            && self
                .replay_evidence
                .exactly_matches_store(&self.effect, &self.durable_receipt)
    }

    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.replay_evidence.project_installed_store_candidate(
            InstalledBodyCandidateProjectionPermit::new(),
            verified,
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
}

/// Closed durable form of one admitted `ValidateBody` effect.
///
/// The body receipt remains attached to the exact causal lineage that moved
/// through Fetch and Store. The independently transferred manifest hash is a
/// second authority coordinate: it is never reconstructed from the receipt.
#[derive(Debug)]
struct DurableValidateBody {
    address: ConcreteWorkAddress,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: DurableValidateReplayEvidenceV1,
}

impl DurableValidateBody {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        let AdapterEffect::ValidateBody { round, subject, .. } = &self.effect else {
            return false;
        };
        ConcreteWorkAddress::new(self.address.owner, self.address.ordinal, self.address.slot)
            == Some(self.address)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && installed_digest == digest_from_hash(self.pending.exact_effect_identity())
            && self.durable_receipt.context_id() == round.context_id
            && self.durable_receipt.round() == *round
            && self.durable_receipt.subject() == *subject
            && self.durable_receipt.manifest_hash() == self.expected_manifest_hash
            && self.replay_evidence.exactly_matches_validate_pending(
                &self.effect,
                &self.durable_receipt,
                &self.pending,
            )
    }

    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.replay_evidence.project_installed_validate_candidate(
            InstalledBodyCandidateProjectionPermit::new(),
            verified,
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
}

/// Same-address closed result of one completed durable body validation.
///
/// The exact incumbent carrier is moved into this value rather than cloned or
/// reconstructed. Its original digest remains a separate authority coordinate
/// while the installed row uses the outcome-bound replacement digest.
#[derive(Debug)]
struct DurableValidateCompletion {
    address: ConcreteWorkAddress,
    incumbent: DurableValidateBody,
    incumbent_digest: LifecycleDigest,
    outcome: DurableBodyValidationOutcome,
}

impl DurableValidateCompletion {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        self.incumbent.address == self.address
            && self.incumbent.validates(self.incumbent_digest)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(
                    self.incumbent.pending.causal_lifecycle_key(),
                ))
            && self
                .incumbent
                .pending
                .exactly_binds_adapter_effect(&self.incumbent.effect)
            && self.outcome.durable_body() == &self.incumbent.durable_receipt
            && self.incumbent.durable_receipt.manifest_hash()
                == self.incumbent.expected_manifest_hash
            && self.outcome.validated_receipt().is_none_or(|receipt| {
                validate_validated_receipt_authority(&self.incumbent, receipt).is_ok()
            })
            && durable_validate_completion_digest(
                self.incumbent_digest,
                self.incumbent.expected_manifest_hash,
                &self.outcome,
            ) == Some(installed_digest)
            && installed_digest != self.incumbent_digest
    }
}

/// Closed, move-only replay-evidence preflight for one directly signed effect.
///
/// This inert token owns the exact runtime effect and pending binding together
/// with the only replay wrapper valid for its closed class. It has no install,
/// commit, parts, or effect access; a future admission transaction must consume
/// the whole value without weakening this boundary.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "direct signed replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedDirectSignedReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: DirectSignedReplayEvidenceV1,
}

enum DirectSignedReplayEvidenceV1 {
    Broadcast(SignedBroadcastReplayEvidenceV1),
    ReportEquivocation(SignedEquivocationReplayEvidenceV1),
}

enum DirectSignedReplayPreAdmissionFailure {
    UnsupportedEffect,
    InvalidReplayEvidence,
}

/// Opaque ownership-preserving failure from direct signed replay preflight.
pub(super) struct DirectSignedReplayPreAdmissionError {
    _effect: AdapterEffect,
    _pending: PendingRuntimeEffectBinding,
    _failure: DirectSignedReplayPreAdmissionFailure,
}

#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectSignedReplayPreAdmission {
    /// Seal exactly one Broadcast or ReportEquivocation effect and its binding.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_exact(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, DirectSignedReplayPreAdmissionError> {
        let replay_evidence = match &effect {
            AdapterEffect::Broadcast(_) => {
                SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
                    .map(DirectSignedReplayEvidenceV1::Broadcast)
                    .ok_or(DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence)
            }
            AdapterEffect::ReportEquivocation { .. } => {
                SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)
                    .map(DirectSignedReplayEvidenceV1::ReportEquivocation)
                    .ok_or(DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence)
            }
            _ => Err(DirectSignedReplayPreAdmissionFailure::UnsupportedEffect),
        };
        let replay_evidence = match replay_evidence {
            Ok(replay_evidence) => replay_evidence,
            Err(failure) => {
                return Err(DirectSignedReplayPreAdmissionError {
                    _effect: effect,
                    _pending: pending,
                    _failure: failure,
                });
            }
        };
        let sealed = Self {
            effect,
            pending,
            replay_evidence,
        };
        if !sealed.validates() {
            let Self {
                effect,
                pending,
                replay_evidence: _,
            } = sealed;
            return Err(DirectSignedReplayPreAdmissionError {
                _effect: effect,
                _pending: pending,
                _failure: DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence,
            });
        }
        Ok(sealed)
    }

    fn validates(&self) -> bool {
        match (&self.effect, &self.replay_evidence) {
            (AdapterEffect::Broadcast(_), DirectSignedReplayEvidenceV1::Broadcast(evidence)) => {
                evidence.exactly_matches_effect(&self.effect, &self.pending)
            }
            (
                AdapterEffect::ReportEquivocation { .. },
                DirectSignedReplayEvidenceV1::ReportEquivocation(evidence),
            ) => evidence.exactly_matches_effect(&self.effect, &self.pending),
            _ => false,
        }
    }
}

/// Closed authenticated-Proposal replay preflight for one ordinary Fetch.
///
/// The sole constructor consumes the exact runtime ownership which already
/// carries the receiver-authenticated signed Proposal seal. No caller can
/// supply a Proposal, ingress carrier, pending root, or replay source parts.
/// Dropping this token is publication-inert.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "remote Proposal Fetch replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedRemoteProposalFetchReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalFetchReplayEvidenceV1,
}

/// Closed ordinary Store successor of one authenticated Proposal Fetch.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "remote Proposal Store replay evidence still requires its durable body receipt"]
pub(super) struct PreparedRemoteProposalStoreReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalStoreReplayEvidenceV1,
}

/// Closed durable Store replay evidence waiting for its exact Validate owner.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "durable remote Proposal replay evidence still requires its Validate successor"]
pub(super) struct PreparedRemoteProposalStoredReplayPreAdmission {
    store_effect: AdapterEffect,
    store_pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: RemoteProposalStoredReplayEvidenceV1,
}

/// Closed canonical Validate replay evidence from one signed remote Proposal.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "remote Proposal Validate replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedRemoteProposalValidateReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: RemoteProposalValidateReplayEvidenceV1,
}

/// Ownership-preserving failure from the authenticated Proposal Fetch cut.
pub(super) struct RemoteProposalFetchReplayPreAdmissionError {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}

/// Ownership-preserving failure from the fixed Fetch-to-Store projection.
pub(super) struct RemoteProposalStoreReplayPreAdmissionError {
    _fetch: PreparedRemoteProposalFetchReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}

/// Ownership-preserving failure from the exact durable-receipt join.
pub(super) struct RemoteProposalDurableReplayPreAdmissionError {
    _store: PreparedRemoteProposalStoreReplayPreAdmission,
    _durable_receipt: DurableBodyReceipt,
}

/// Ownership-preserving failure from the fixed Store-to-Validate projection.
pub(super) struct RemoteProposalValidateReplayPreAdmissionError {
    _stored: PreparedRemoteProposalStoredReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}

#[allow(dead_code)]
impl PreparedRemoteProposalFetchReplayPreAdmission {
    /// Consume one runtime-owned ordinary Fetch carrying exact signed-Proposal evidence.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_exact_fetch(
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<Self, RemoteProposalFetchReplayPreAdmissionError> {
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Some(replay_evidence) = ownership.exact_remote_proposal_fetch_replay(&effect) else {
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        };
        let sealed = Self {
            effect,
            pending,
            replay_evidence,
        };
        if !sealed.validates() {
            let Self {
                effect,
                pending: _,
                replay_evidence: _,
            } = sealed;
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        }
        Ok(sealed)
    }

    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_fetch_pending(&self.effect, &self.pending)
    }

    /// Recheck an exact retry without replacing the retained signed origin.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let Some(pending) = ownership.pending_adapter_effect_binding(effect) else {
            return false;
        };
        let Some(candidate) = ownership.exact_remote_proposal_fetch_replay(effect) else {
            return false;
        };
        self.validates()
            && self.effect == *effect
            && self.pending == pending
            && self
                .replay_evidence
                .exactly_matches_retry(&candidate, effect)
    }

    /// Consume the Fetch origin only through its exact ordinary Store successor.
    #[allow(clippy::result_large_err)]
    pub(super) fn project_store(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedRemoteProposalStoreReplayPreAdmission,
        RemoteProposalStoreReplayPreAdmissionError,
    > {
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalStoreReplayPreAdmissionError {
                _fetch: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Self {
            effect: fetch_effect,
            pending: fetch_pending,
            replay_evidence,
        } = self;
        match replay_evidence.project_exact_store(&effect, &pending) {
            Ok(replay_evidence) => {
                let store = PreparedRemoteProposalStoreReplayPreAdmission {
                    effect,
                    pending,
                    replay_evidence,
                };
                debug_assert!(store.validates());
                Ok(store)
            }
            Err(replay_evidence) => Err(RemoteProposalStoreReplayPreAdmissionError {
                _fetch: Self {
                    effect: fetch_effect,
                    pending: fetch_pending,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
}

#[allow(dead_code)]
impl PreparedRemoteProposalStoreReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_store_pending(&self.effect, &self.pending)
    }

    /// Recheck an exact Store retry without replacing its Proposal origin.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .pending_adapter_effect_binding(effect)
            .is_some_and(|pending| {
                self.validates()
                    && self.effect == *effect
                    && self.pending == pending
                    && self
                        .replay_evidence
                        .exactly_matches_store_pending(effect, &pending)
            })
    }

    /// Join the exact store-minted BodyFrame without exposing either input.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_durable_body(
        self,
        durable_receipt: DurableBodyReceipt,
    ) -> Result<
        PreparedRemoteProposalStoredReplayPreAdmission,
        RemoteProposalDurableReplayPreAdmissionError,
    > {
        let Self {
            effect,
            pending,
            replay_evidence,
        } = self;
        match replay_evidence.bind_durable_body(&effect, &durable_receipt) {
            Ok(replay_evidence) => {
                let stored = PreparedRemoteProposalStoredReplayPreAdmission {
                    store_effect: effect,
                    store_pending: pending,
                    durable_receipt,
                    replay_evidence,
                };
                debug_assert!(stored.validates());
                Ok(stored)
            }
            Err(replay_evidence) => Err(RemoteProposalDurableReplayPreAdmissionError {
                _store: Self {
                    effect,
                    pending,
                    replay_evidence,
                },
                _durable_receipt: durable_receipt,
            }),
        }
    }
}

#[allow(dead_code)]
impl PreparedRemoteProposalStoredReplayPreAdmission {
    fn validates(&self) -> bool {
        self.store_pending
            .exactly_binds_adapter_effect(&self.store_effect)
            && self
                .replay_evidence
                .exactly_matches_store(&self.store_effect, &self.durable_receipt)
    }

    /// Consume the durable Store family only through its exact Validate successor.
    #[allow(clippy::result_large_err)]
    pub(super) fn project_validate(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedRemoteProposalValidateReplayPreAdmission,
        RemoteProposalValidateReplayPreAdmissionError,
    > {
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Self {
            store_effect,
            store_pending,
            durable_receipt,
            replay_evidence,
        } = self;
        match replay_evidence.project_exact_validate(
            &store_effect,
            &durable_receipt,
            &effect,
            &pending,
        ) {
            Ok(replay_evidence) => {
                let validate = PreparedRemoteProposalValidateReplayPreAdmission {
                    effect,
                    pending,
                    durable_receipt,
                    replay_evidence,
                };
                debug_assert!(validate.validates());
                Ok(validate)
            }
            Err(replay_evidence) => Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: Self {
                    store_effect,
                    store_pending,
                    durable_receipt,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
}

#[allow(dead_code)]
impl PreparedRemoteProposalValidateReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence.exactly_matches_validate_pending(
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }

    /// Recheck an exact Validate retry without replacing the retained family.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .pending_adapter_effect_binding(effect)
            .is_some_and(|pending| {
                self.validates()
                    && self.effect == *effect
                    && self.pending == pending
                    && self.replay_evidence.exactly_matches_validate_pending(
                        effect,
                        &self.durable_receipt,
                        &pending,
                    )
            })
    }

    /// Consume the exact remote-Proposal Validate pre-admission into its
    /// closed durable carrier without accepting a manifest, receipt, pending
    /// binding, or installed digest from the caller.
    ///
    /// The address is structural admission output only. This conversion does
    /// not install it, and failure returns the complete move-only token.
    #[allow(clippy::result_large_err)]
    fn into_durable_validate_carrier(
        self,
        address: ConcreteWorkAddress,
    ) -> Result<(DurableValidateBody, LifecycleDigest), Self> {
        if !self.validates()
            || address.owner.causal_root()
                != super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            || address.slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(self);
        }
        let digest = digest_from_hash(self.pending.exact_effect_identity());
        let expected_manifest_hash = self.durable_receipt.manifest_hash();
        let carrier = DurableValidateBody {
            address,
            effect: self.effect,
            pending: self.pending,
            durable_receipt: self.durable_receipt,
            expected_manifest_hash,
            replay_evidence: DurableValidateReplayEvidenceV1::remote_proposal(self.replay_evidence),
        };
        if !carrier.validates(digest) {
            let DurableValidateBody {
                address: _,
                effect,
                pending,
                durable_receipt,
                expected_manifest_hash: _,
                replay_evidence,
            } = carrier;
            let DurableValidateReplayEvidenceV1::RemoteProposal(replay_evidence) = replay_evidence
            else {
                unreachable!("remote adoption retains its exact replay variant")
            };
            return Err(Self {
                effect,
                pending,
                durable_receipt,
                replay_evidence,
            });
        }
        Ok((carrier, digest))
    }
}

/// Closed concrete form of one fsynced recovered WAL `Sign` successor.
///
/// The complete durable logical repair and detached validated predecessor stay
/// together in this carrier. No effect, pending binding, validation receipt,
/// or durable receipt can be extracted from it; the future typed Sign
/// executor must consume this closed variant as a whole.
struct DurableRecoveredWalSignWork {
    repair: DurableAuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
}

impl fmt::Debug for DurableRecoveredWalSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalSignWork")
            .field("child_ordinal", &self.repair.child_ordinal())
            .field("parent_address", &self.validation.address)
            .finish_non_exhaustive()
    }
}

impl DurableRecoveredWalSignWork {
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        let repair = self.repair.repair();
        let Ok((physical, universe, consumed)) = repair.child().physical_geometry.normalized()
        else {
            return false;
        };
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        detached_recovered_validation_is_exact(repair, &self.validation)
            && repair.child().initial_state == InitialLifecycleState::Ready
            && repair.child().work_class == LifecycleWorkClass::SignVote
            && physical.len() == 1
            && universe.len() == 1
            && consumed == universe
            && physical.get(&effect_slot) == Some(&installed_digest)
    }

    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.validates_digest(installed_digest)
            && address.owner == self.validation.address.owner
            && address.owner.causal_root() == self.repair.repair().child().causal_root
            && address.ordinal == self.repair.child_ordinal()
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
    }

    fn validates_in_store(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && store.revalidates_durable_authenticated_wal_vote_repair(&self.repair)
    }
}

/// Whether one concrete registry row is still an executable adapter effect or
/// a closed durable carrier awaiting its future typed consumer. Keeping the
/// move-only carriers inline avoids adding another heap-allocation fail-stop
/// cut between physical evidence and exact-address ownership.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Debug)]
enum ConcreteLifecycleWorkKind {
    PendingAdapter {
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    },
    CertifiedFetchCompletion(CertifiedFetchCompletion),
    DurableStoreBody(DurableStoreBody),
    DurableValidateBody(DurableValidateBody),
    DurableValidateCompletion(DurableValidateCompletion),
    DurableRecoveredWalSign(DurableRecoveredWalSignWork),
}

/// One move-only concrete effect paired with its sealed pending authority.
#[derive(Debug)]
#[must_use = "dropping concrete lifecycle work abandons its exact physical owner"]
pub(super) struct ConcreteLifecycleWork {
    digest: LifecycleDigest,
    kind: ConcreteLifecycleWorkKind,
}

impl ConcreteLifecycleWork {
    /// Consume one exact effect and ordinal-free binding into registry work.
    pub(super) fn from_exact(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        if !pending.exactly_binds_adapter_effect(&effect) {
            return Err((RegistryError::UnboundEffect, effect, pending));
        }
        let digest = digest_from_hash(pending.exact_effect_identity());
        Ok(Self {
            digest,
            kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
        })
    }

    /// Revalidate the sealed binding and its derived physical digest.
    pub(super) fn validate_exact(&self) -> bool {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter { effect, pending } => {
                pending.exactly_binds_adapter_effect(effect)
                    && self.digest == digest_from_hash(pending.exact_effect_identity())
            }
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                completion.validates(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableStoreBody(store) => store.validates(self.digest),
            ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                validate.validates(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                completion.validates(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.validates_digest(self.digest)
            }
        }
    }

    fn validates_at(&self, address: ConcreteWorkAddress) -> bool {
        self.validate_exact()
            && match &self.kind {
                ConcreteLifecycleWorkKind::PendingAdapter { .. } => true,
                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                    completion.address == address
                }
                ConcreteLifecycleWorkKind::DurableStoreBody(store) => store.address == address,
                ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                    validate.address == address
                }
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                    completion.address == address
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                    sign.validates_at(address, self.digest)
                }
            }
    }

    /// Derive the coordinator causal root from the sealed pending key.
    pub(super) fn causal_root(&self) -> super::CausalRoot {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter { pending, .. } => {
                super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
            }
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                completion.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableStoreBody(store) => store.address.owner.causal_root(),
            ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                validate.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                completion.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.repair.repair().child().causal_root
            }
        }
    }

    /// Return the exact physical effect digest installed with this work.
    pub(super) const fn digest(&self) -> LifecycleDigest {
        self.digest
    }

    /// Recover one still-pending adapter pair after a failed or deferred transaction.
    /// A closed lifecycle carrier requires its future typed consumer and fails stop here.
    pub(super) fn into_pair(self) -> (AdapterEffect, PendingRuntimeEffectBinding) {
        let ConcreteLifecycleWorkKind::PendingAdapter { effect, pending } = self.kind else {
            panic!("closed lifecycle work requires its future typed consumer")
        };
        (effect, pending)
    }

    /// Borrow the exact adapter identity without separating it from authority.
    /// For a completion this is its retained, non-executable incumbent Fetch.
    pub(super) const fn effect(&self) -> &AdapterEffect {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter { effect, .. } => effect,
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                &completion.incumbent_effect
            }
            ConcreteLifecycleWorkKind::DurableStoreBody(store) => &store.effect,
            ConcreteLifecycleWorkKind::DurableValidateBody(validate) => &validate.effect,
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                &completion.incumbent.effect
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.repair.installed_child_effect()
            }
        }
    }

    const fn is_pending_adapter(&self) -> bool {
        matches!(&self.kind, ConcreteLifecycleWorkKind::PendingAdapter { .. })
    }

    const fn pending_adapter_pair(&self) -> Option<(&AdapterEffect, &PendingRuntimeEffectBinding)> {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter { effect, pending } => {
                Some((effect, pending))
            }
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_) => None,
            ConcreteLifecycleWorkKind::DurableStoreBody(_) => None,
            ConcreteLifecycleWorkKind::DurableValidateBody(_) => None,
            ConcreteLifecycleWorkKind::DurableValidateCompletion(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_) => None,
        }
    }
}

/// Closed pre-mutation failure inventory for certified-Fetch conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CertifiedFetchCompletionError {
    /// The logical owner, ordinal, slot, or old/new digest pair is malformed.
    InvalidLocation,
    /// No incumbent exists at the exact logical address.
    MissingIncumbent,
    /// The incumbent lost its sealed effect/binding integrity.
    CorruptIncumbent,
    /// The incumbent is not one still-pending exact `FetchBody` effect.
    WrongIncumbentShape,
    /// The logical owner does not name the incumbent pending causal root.
    ForeignCausalOwner,
    /// The coordinator's incumbent digest differs from the concrete Fetch.
    IncumbentDigestMismatch,
    /// The logical replacement digest is not the selected queue identity.
    ReplacementDigestMismatch,
    /// The selected queue identity is zero or belongs to another wire context.
    InvalidQueueIdentity,
    /// The selected response candidate does not carry the incumbent binding.
    CandidateBindingMismatch,
    /// The selected authenticated response family does not match the Fetch.
    ResponseFamilyMismatch,
    /// Canonical replay evidence could not bind the sealed Fetch origin.
    InvalidReplayEvidence,
    /// The durable receipt does not bind the exact response and incumbent Fetch.
    DurableReceiptMismatch,
    /// The checked-dequeue result did not return the prepared exact occurrence;
    /// after a real queue CAS the caller must fail stop and never retry it.
    DequeuedResponseMismatch,
}

/// Closed failure inventory for preparing one scheduled certified-Fetch execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CertifiedFetchExecutionError {
    /// The lease is not exactly one independent `FetchBody` effect slot.
    InvalidLeaseShape,
    /// Exact address, causal owner, or installed-digest validation failed.
    Registry(RegistryError),
    /// The exact lease address still contains an executable adapter effect.
    WrongWorkKind,
    /// The installed completion is not one exact certified-Fetch response.
    InvalidCompletionShape,
    /// The supplied reducer effect is not the exact `StoreBody` successor.
    InvalidStoreSuccessor,
}

/// Closed failure inventory for preparing one scheduled durable Store execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableStoreExecutionError {
    /// The lease is not exactly one independent `StoreBody` effect slot.
    InvalidLeaseShape,
    /// Exact address, causal owner, installed digest, or carrier validation failed.
    Registry(RegistryError),
    /// The exact lease address does not contain a closed durable Store carrier.
    WrongWorkKind,
    /// The installed Store carrier lost its exact durable or runtime binding.
    InvalidStoreShape,
    /// The verified height context could not authenticate the bound Store projection.
    Projection(AdapterEffectAdmissionError),
    /// The authenticated projection differs from the lease or physical geometry.
    InvalidProjection,
    /// The supplied reducer effect is not the exact `ValidateBody` successor.
    InvalidValidateSuccessor,
}

/// Closed failure inventory for preparing one scheduled durable Validate execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableValidateExecutionError {
    /// The lease is not exactly one independent `ValidateBody` effect slot.
    InvalidLeaseShape,
    /// Exact address, causal owner, installed digest, or carrier validation failed.
    Registry(RegistryError),
    /// The exact lease address does not contain a closed durable Validate carrier.
    WrongWorkKind,
    /// The installed Validate carrier lost its exact durable or runtime binding.
    InvalidValidateShape,
    /// The verified height context could not authenticate the bound Validate projection.
    Projection(AdapterEffectAdmissionError),
    /// The authenticated projection differs from the lease or physical geometry.
    InvalidProjection,
    /// The validation receipt belongs to another durable body or is malformed.
    InvalidValidationReceipt,
    /// Existing Prepare/Commit authority disagrees with deterministic validation.
    ConflictingValidationCommitment,
    /// The completion digest would not replace the incumbent physical identity.
    InvalidValidationCompletionDigest,
}

/// Closed failure inventory for preparing one Ready Validate completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyDurableValidateExecutionError {
    /// The lease is not exactly one independent Ready `ValidateBody` slot.
    InvalidLeaseShape,
    /// Exact address, causal owner, or installed replacement digest failed.
    Registry(RegistryError),
    /// The exact address does not contain a closed Validate completion.
    WrongWorkKind,
    /// The retained carrier, outcome, or replacement authority is malformed.
    InvalidCompletionShape,
    /// The verified height context rejected the retained Validate projection.
    Projection(AdapterEffectAdmissionError),
    /// The authenticated projection differs from the lease or physical geometry.
    InvalidProjection,
}

/// Closed registry-side failure while converting one executed Validate wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableValidateCompletionConversionError {
    /// Exact row reattachment rejected the executed request or incumbent.
    Execution(DurableValidateExecutionError),
    /// The dispatch's sealed wake token no longer names its exact request.
    InvalidWakeAuthority,
    /// The body-store outcome was not one closed executable or deferred form.
    InvalidOutcome,
    /// The outcome-bound replacement digest was absent, unchanged, or inconsistent.
    InvalidReplacementDigest,
}

/// Closed precommit failure from volatile Validate completion publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableValidateCompletionPublicationError {
    /// Registry reattachment or same-address conversion failed exactly.
    Registry(DurableValidateCompletionConversionError),
    /// The coordinator had already latched an unrelated fail-closed condition.
    CoordinatorFaulted,
    /// The exact row was no longer the dispatch's unique Waiting authority.
    InvalidWaitingState,
    /// Pure Ready publication changed more than the exact row and generation.
    InvalidStagedTransition,
}

/// Borrow-bound execution authority for one closed certified-Fetch completion.
///
/// Preparation mutates nothing. The exclusive registry borrow keeps the exact
/// completion address stable while a direct adapter transition is previewed;
/// dropping the token therefore leaves the registry byte-for-byte unchanged.
///
/// The installed completion owns a store-minted durable response receipt, and
/// preparation rechecks that receipt before exposing this execution borrow.
#[must_use = "a prepared certified-Fetch execution still owns its registry cut"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedCertifiedFetchExecution<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
}

/// Borrow-bound execution authority for one closed durable Store carrier.
///
/// Preparation and drop mutate nothing. The exclusive registry borrow keeps
/// the exact Store address stable while the direct `BodyStored` adapter step
/// is previewed and its ordinal-free Validate successor is sealed.
#[must_use = "a prepared durable Store execution still owns its registry cut"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableStoreExecution<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
}

/// Borrow-bound execution authority for one closed durable Validate carrier.
///
/// Preparation and drop mutate nothing. The exclusive registry borrow keeps
/// the exact Validate address stable while a future validation service and
/// reducer seam inspect its durable body authority.
#[must_use = "a prepared durable Validate execution still owns its registry cut"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableValidateExecution<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
}

/// Closed executable outcome retained by a Ready Validate completion.
///
/// This is deliberately only a discriminator. The fixed adapter join consumes
/// a private-field authority derived from the exact carrier, so neither variant
/// exposes a constructor, coordinates, receipt, or diagnostic payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadyDurableValidateOutcomeKind {
    /// Deterministic validation produced a durable execution commitment.
    Validated,
    /// Deterministic validation produced the one canonical rejection identity.
    Rejected,
}

/// Closed failure while classifying one exact Ready Validate carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyValidateCarrierError {
    /// The exact physical address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The address contains a concrete carrier from another lifecycle stage.
    WrongWorkKind,
    /// The closed Validate carrier or completion outcome is malformed.
    InvalidCarrier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadyValidateCarrierKind {
    ExecuteBody,
    ValidatedCompletion,
    RejectedCompletion,
}

/// Opaque registry proof for one exact Ready Validate physical carrier.
///
/// Construction stays inside this module. The coordinator schema may only
/// bind the seal to the same logical address and installed digest; it cannot
/// choose or rewrite the closed carrier classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReadyValidateCarrierSeal {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    kind: ReadyValidateCarrierKind,
    payload: DurablePayloadReference,
}

impl ReadyValidateCarrierSeal {
    /// Return whether this seal names the exact coordinator-owned slot.
    pub(super) fn matches(
        self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        self.address.owner == owner
            && self.address.ordinal == ordinal
            && self.address.slot == slot
            && self.digest == digest
    }

    /// Return whether the sealed carrier may emit one Consensus report.
    pub(super) const fn requires_consensus_capacity(self) -> bool {
        matches!(self.kind, ReadyValidateCarrierKind::RejectedCompletion)
    }

    /// Return whether the carrier's receipt reproduces the ledger body frame.
    pub(super) fn matches_durable_payload(
        self,
        key: LifecycleKey,
        payload: DurablePayloadReference,
    ) -> bool {
        self.payload == payload
            && super::body_pipeline_transition::durable_validate_payload_is_exact(key, payload)
    }
}

/// Borrow-bound execution authority for one Ready Validate completion.
///
/// Preparation and drop mutate nothing. The exclusive registry borrow keeps
/// the exact completion carrier stable while the adapter previews its direct
/// validation transition. Fields and receipt authority remain sealed.
#[must_use = "a Ready Validate completion still owns its registry execution cut"]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct PreparedReadyDurableValidateExecution<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    outcome_kind: ReadyDurableValidateOutcomeKind,
    lease: TurnLease,
}

/// Detached, move-only registry authority for the exact recovered Validate
/// parent of one WAL-ahead phase vote.
///
/// Construction consumes only a fully prepared Ready/validated completion.
/// The concrete carrier remains private and is restored at its exact address
/// if this cut is dropped before the recovered vote is joined.
#[must_use = "a recovered WAL Validate cut must be joined or restored"]
pub(crate) struct RecoveredWalValidateRegistryCut<'registry> {
    registry: Option<&'registry mut ConcreteLifecycleWorkRegistry>,
    address: ConcreteWorkAddress,
    work: Option<ConcreteLifecycleWork>,
}

/// Opaque exact LedgerV1 store/frame retained by recovered-parent startup.
///
/// Neither the store nor decoded records can be extracted. The later fsync
/// transaction must consume this value together with the authenticated parent
/// repair, preserving the exact opened snapshot across the crash splice.
#[allow(dead_code)]
#[must_use = "an opened recovered WAL ledger must remain sealed through persistence"]
pub(crate) struct OpenedRecoveredWalValidateLedger {
    store: super::ledger::LifecycleLedgerStoreV1,
    opened: super::ledger::LifecycleLedgerV1,
}

/// Opaque failure from storage-authenticated recovered-parent reconstruction.
///
/// Every variant owns the WAL or successor authority still in flight, the
/// exact opened ledger when one exists, and the detached body marker until it
/// has transferred into a sealed validation outcome. Dropping any pre-join
/// failure restores that marker to the same body-store instance.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered-parent reconstruction still owns startup authority"]
pub(crate) struct RecoveredWalParentFactoryError<'body> {
    failure: RecoveredWalParentFactoryFailure<'body>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalParentFactoryFailure<'body> {
    LedgerOpen {
        _error: super::ledger::LifecycleLedgerError,
        _recovered: RecoveredWalVoteSign,
    },
    BodyMarker {
        _error: RecoveredValidatedBodyCutError,
        _ledger: OpenedRecoveredWalValidateLedger,
        _recovered: RecoveredWalVoteSign,
    },
    LedgerParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _recovered: RecoveredWalVoteSign,
    },
    RuntimeParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _recovered: RecoveredWalVoteSign,
    },
    Lifecycle {
        _ledger: OpenedRecoveredWalValidateLedger,
        _body: RecoveredValidatedBodyCut<'body>,
        _error: RecoveredWalVoteLifecycleRepairError,
    },
    RegistryParent {
        _ledger: OpenedRecoveredWalValidateLedger,
        _repair: AuthenticatedWalVoteLifecycleRepair,
        _body: RecoveredValidatedBodyCut<'body>,
    },
}

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalParentFactoryError<'_> {
    /// Return a stable diagnostic without exposing any retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalParentFactoryFailure::LedgerOpen { .. } => {
                "recovered WAL lifecycle ledger could not be opened"
            }
            RecoveredWalParentFactoryFailure::BodyMarker { .. } => {
                "recovered WAL vote has no exact revalidated body marker"
            }
            RecoveredWalParentFactoryFailure::LedgerParent { .. } => {
                "recovered WAL vote has no exact durable Validate parent"
            }
            RecoveredWalParentFactoryFailure::RuntimeParent { .. } => {
                "durable Validate parent could not reconstruct its runtime binding"
            }
            RecoveredWalParentFactoryFailure::Lifecycle { .. } => {
                "recovered Validate-to-Sign lifecycle projection failed"
            }
            RecoveredWalParentFactoryFailure::RegistryParent { .. } => {
                "recovered Validate parent conflicts with concrete registry state"
            }
        }
    }
}

/// Exclusive reservation for the detached Validate address and its projected
/// Sign successor address.
///
/// The parent address is vacant from the registry detach onward. The child
/// address is filled only by the pure LedgerV1 staging preflight and must also
/// be vacant before fsync. Retaining the exclusive registry borrow prevents a
/// concurrent concrete admission from invalidating either check.
struct RecoveredWalValidateRegistryReservation<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    parent_address: ConcreteWorkAddress,
    child: Option<(ConcreteWorkAddress, LifecycleDigest)>,
}

struct DetachedRecoveredValidateCompletion {
    address: ConcreteWorkAddress,
    installed_digest: LifecycleDigest,
    incumbent_address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: DetachedValidateReplayEvidenceV1,
    outcome: DurableBodyValidationOutcome,
}

/// Exact provenance retained while a Validate completion is detached.
///
/// A live authenticated carrier moves its certified or remote-Proposal replay
/// family into this cut.
/// Cold WAL recovery instead consumes the separately authenticated body-store
/// marker after LedgerV1 and store equality have already been re-established;
/// it cannot manufacture the absent transport origin because this tranche
/// intentionally adds no replay field to LedgerV1.
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum DetachedValidateReplayEvidenceV1 {
    Retained(DurableValidateReplayEvidenceV1),
    RecoveredBodyMarker(DurableBodyReceipt),
}

impl DetachedValidateReplayEvidenceV1 {
    fn exactly_matches_durable_body(&self, receipt: &DurableBodyReceipt) -> bool {
        match self {
            Self::Retained(evidence) => evidence.exactly_matches_durable_body(receipt),
            Self::RecoveredBodyMarker(recovered) => recovered == receipt,
        }
    }
}

/// Exact validated parent authority retained beside its authenticated WAL
/// lifecycle repair.
///
/// The validated-body outcome, durable receipt, original registry address,
/// both installed digests, and exclusive vacant-address reservation stay
/// opaque. The fsync/install composite must consume this value as a whole; it
/// cannot persist the logical repair while discarding the storage-authenticated
/// validation result. Any later logical or ledger failure is fail-stop/restart,
/// not an ordinary rollback which can restore the already-consumed binding.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a recovered validated WAL repair has not completed startup"]
pub(crate) struct AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    repair: AuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl AuthenticatedRecoveredWalValidateLifecycleRepair<'_> {
    /// Revalidate the retained concrete pair and its exact durable validation.
    pub(crate) fn concrete_pair_and_validation_are_exact(&self) -> bool {
        recovered_validate_authority_is_exact(&self.repair, &self.validation, &self.reservation)
    }
}

fn recovered_validate_authority_is_exact(
    repair: &AuthenticatedWalVoteLifecycleRepair,
    validation: &DetachedRecoveredValidateCompletion,
    reservation: &RecoveredWalValidateRegistryReservation<'_>,
) -> bool {
    detached_recovered_validation_is_exact(repair, validation)
        && reservation.parent_address == validation.address
        && !reservation
            .registry
            .entries
            .contains_key(&validation.address)
}

fn detached_recovered_validation_is_exact(
    repair: &AuthenticatedWalVoteLifecycleRepair,
    validation: &DetachedRecoveredValidateCompletion,
) -> bool {
    let Some(validated) = validation.outcome.validated_receipt() else {
        return false;
    };
    let Ok((physical, universe, consumed)) = repair.parent().physical_geometry.normalized() else {
        return false;
    };
    ConcreteWorkAddress::new(
        validation.address.owner,
        validation.address.ordinal,
        validation.address.slot,
    ) == Some(validation.address)
        && validation.address == validation.incumbent_address
        && validation.address.owner.causal_root() == repair.parent().causal_root
        && physical.len() == 1
        && universe.len() == 1
        && consumed == universe
        && physical.get(&validation.address.slot) == Some(&validation.incumbent_digest)
        && &validation.durable_receipt == validated.durable()
        && validation.expected_manifest_hash == validated.durable().manifest_hash()
        && validation
            .replay_evidence
            .exactly_matches_durable_body(&validation.durable_receipt)
        && validation.installed_digest != validation.incumbent_digest
        && durable_validate_completion_digest(
            validation.incumbent_digest,
            validation.expected_manifest_hash,
            &validation.outcome,
        ) == Some(validation.installed_digest)
        && repair.concrete_pair_matches_validation(validated)
}

// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_BEGIN
/// Post-fsync authority for one exact recovered Validate-to-Sign splice.
///
/// This move-only token retains the exclusive registry reservation, the full
/// detached validation completion, and the frame-bound durable logical repair.
/// It exposes no parts or receipt extraction. The next startup tranche must
/// consume it directly when installing the projected Sign child.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "a durable recovered WAL repair still reserves its concrete handoff"]
pub(crate) struct DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    repair: DurableAuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
}

// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_BEGIN
/// Exclusive post-install view of one exact recovered WAL Sign child.
///
/// The complete durable authority lives in the closed registry row. This
/// token retains the registry's exclusive borrow so no caller can replace,
/// take, or execute the child before the future startup transaction commits
/// its remaining coordinator and adapter publications.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an installed recovered WAL Sign child still seals startup"]
pub(crate) struct InstalledRecoveredWalSignRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    parent_address: ConcreteWorkAddress,
    child_address: ConcreteWorkAddress,
    child_digest: LifecycleDigest,
}

/// Opaque fail-stop error from post-fsync recovered Sign installation.
///
/// Every variant owns the complete uninstalled durable repair and exclusive
/// registry reservation. It exposes diagnostics only, so a failed store/frame
/// check cannot leak raw effect, pending, receipt, or retry authority.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered Sign installation still owns startup authority"]
pub(crate) struct RecoveredWalSignInstallError<'registry> {
    failure: RecoveredWalSignInstallFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalSignInstallFailure<'registry> {
    InvalidPreflight {
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    #[cfg(test)]
    StoreOpen {
        _error: super::ledger::LifecycleLedgerError,
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
}

impl RecoveredWalSignInstallError<'_> {
    /// Return a stable diagnostic without releasing retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalSignInstallFailure::InvalidPreflight { .. } => {
                "fsynced recovered WAL Sign child failed exact registry preflight"
            }
            #[cfg(test)]
            RecoveredWalSignInstallFailure::StoreOpen { .. } => {
                "recovered WAL ledger store could not be reopened for Sign installation"
            }
        }
    }
}

/// Opaque fail-stop error from the recovered Validate LedgerV1 fsync splice.
///
/// Every variant owns either the complete pre-fsync authority or the complete
/// post-fsync authority. Even a preflight failure is restart-only: callers
/// cannot recover the consumed effect, pending binding, validation receipt, or
/// registry borrow and cannot present the failure as an ordinary rollback.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered WAL persistence still owns its registry reservation"]
pub(crate) struct RecoveredWalValidateLedgerPersistError<'registry> {
    failure: RecoveredWalValidateLedgerPersistFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalValidateLedgerPersistFailure<'registry> {
    InvalidAuthority {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    ParentLedgerMismatch {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    Stage {
        _error: super::ledger::LifecycleLedgerError,
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    InvalidChildAddress {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    OccupiedReservation {
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    Persist {
        _error: super::ledger::LifecycleLedgerError,
        _authority: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
    PostFsync {
        _authority: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    },
}

impl RecoveredWalValidateLedgerPersistError<'_> {
    /// Return a stable diagnostic without releasing any retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalValidateLedgerPersistFailure::InvalidAuthority { .. } => {
                "recovered WAL validation authority is inconsistent"
            }
            RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch { .. } => {
                "recovered Validate address does not bind the exact opened ledger parent"
            }
            RecoveredWalValidateLedgerPersistFailure::Stage { .. } => {
                "recovered WAL ledger repair could not be staged"
            }
            RecoveredWalValidateLedgerPersistFailure::InvalidChildAddress { .. } => {
                "recovered WAL Sign child has no exact concrete address"
            }
            RecoveredWalValidateLedgerPersistFailure::OccupiedReservation { .. } => {
                "recovered WAL parent or Sign child registry address is occupied"
            }
            RecoveredWalValidateLedgerPersistFailure::Persist { .. } => {
                "recovered WAL ledger fsync did not complete authoritatively"
            }
            RecoveredWalValidateLedgerPersistFailure::PostFsync { .. } => {
                "fsynced recovered WAL repair failed its sealed postcondition"
            }
        }
    }
}

impl RecoveredWalValidateRegistryReservation<'_> {
    fn bind_child_if_vacant(
        &mut self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> bool {
        if self.registry.entries.contains_key(&self.parent_address)
            || self.registry.entries.contains_key(&address)
        {
            return false;
        }
        match self.child {
            Some(bound) => bound == (address, digest),
            None => {
                self.child = Some((address, digest));
                true
            }
        }
    }

    fn exact_vacant_pair(&self, validation: &DetachedRecoveredValidateCompletion) -> bool {
        let Some((child, _digest)) = self.child else {
            return false;
        };
        self.parent_address == validation.address
            && child != self.parent_address
            && !self.registry.entries.contains_key(&self.parent_address)
            && !self.registry.entries.contains_key(&child)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry> AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    /// Match immutable parent identity only; the immediately following typed
    /// ledger stage is the sole authority for accepting either the live parent
    /// or the exact already-repaired parent/child stutter.
    fn ledger_parent_core_identity_is_exact(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> bool {
        let candidate = self.repair.parent();
        if ledger.context().id() != candidate.key.context()
            || ledger.context().height() != candidate.key.round().height()
        {
            return false;
        }
        let mut matching = ledger
            .records()
            .iter()
            .filter(|record| record.key() == Some(candidate.key));
        let Some(parent) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        parent.owner() == self.validation.address.owner
            && parent.ordinal() == self.validation.address.ordinal
            && parent.work_class() == Some(candidate.work_class)
            && parent.stage() == Some(candidate.stage)
            && parent.reconstruction_source() == candidate.reconstruction_source
            && parent.durable_payload() == Some(candidate.payload)
    }

    fn projected_child_address(
        &self,
        child_ordinal: u128,
    ) -> Option<(ConcreteWorkAddress, LifecycleDigest)> {
        let (physical, universe, consumed) =
            self.repair.child().physical_geometry.normalized().ok()?;
        if physical.len() != 1 || universe.len() != 1 || consumed != universe {
            return None;
        }
        let (&slot, &digest) = physical.first_key_value()?;
        let address = ConcreteWorkAddress::new(self.validation.address.owner, child_ordinal, slot)?;
        (address != self.validation.address).then_some((address, digest))
    }

    /// Stage against the exact opened ledger, reserve the projected child, and
    /// fsync the complete replacement without exposing inner authority.
    ///
    /// The store re-loads and compares `opened` immediately before staging, so
    /// a stale snapshot fails closed. Any returned error retains this exclusive
    /// registry borrow and is fail-stop/restart, regardless of whether bytes
    /// reached disk before the failure was observed.
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_in_opened_ledger(
        mut self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        opened: &super::ledger::LifecycleLedgerV1,
    ) -> Result<
        (
            super::ledger::LifecycleLedgerV1,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
            bool,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        if !self.concrete_pair_and_validation_are_exact() {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        if !self.ledger_parent_core_identity_is_exact(opened) {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        let (expected, child_ordinal, expected_changed) =
            match opened.stage_authenticated_wal_vote_repair(&self.repair) {
                Ok(staged) => staged,
                Err(error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                            _error: error,
                            _authority: self,
                        },
                    });
                }
            };
        let Some((child_address, child_digest)) = self.projected_child_address(child_ordinal)
        else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidChildAddress {
                    _authority: self,
                },
            });
        };
        if !self
            .reservation
            .bind_child_if_vacant(child_address, child_digest)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::OccupiedReservation {
                    _authority: self,
                },
            });
        }

        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let (persisted, repair, changed) =
            match store.persist_authenticated_wal_vote_repair(opened, repair) {
                Ok(persisted) => persisted,
                Err((error, repair)) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                            _error: error,
                            _authority: AuthenticatedRecoveredWalValidateLifecycleRepair {
                                repair,
                                validation,
                                reservation,
                            },
                        },
                    });
                }
            };
        let durable = DurableAuthenticatedRecoveredWalValidateLifecycleRepair {
            repair,
            validation,
            reservation,
        };
        if persisted != expected
            || changed != expected_changed
            || durable.repair.child_ordinal() != child_ordinal
            || !durable.post_fsync_authority_is_exact(store)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                    _authority: durable,
                },
            });
        }
        Ok((persisted, durable, changed))
    }
}

#[cfg(test)]
impl<'registry> AuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    fn parent_ledger_for_test(
        &self,
        owner: OwnerId,
        ordinal: u128,
    ) -> Result<super::ledger::LifecycleLedgerV1, super::ledger::LifecycleLedgerError> {
        let parent = self.repair.parent();
        super::ledger::LifecycleLedgerV1::new(
            LifecycleContext::new(parent.key.context(), parent.key.round().height()),
            ordinal,
            vec![super::ledger::LifecycleLedgerRecordV1::new(
                parent.key,
                owner,
                ordinal,
                parent.work_class,
                parent.stage,
                None,
                parent.reconstruction_source,
                parent.payload,
                parent.replay_authority.clone(),
                super::schema::DurableContinuation::None,
            )?],
            BTreeMap::new(),
        )
    }

    /// Verify that a row with the right semantic projection but the wrong
    /// durable ordinal, owner identity, or row inventory cannot pass the outer
    /// address-to-ledger binding.
    pub(crate) fn rejects_wrong_ledger_parent_bindings_for_test(&self) -> bool {
        let parent = self.repair.parent();
        let address = self.validation.address;
        let Some(other_ordinal) = address.ordinal.checked_add(1) else {
            return false;
        };
        let Ok(exact) = self.parent_ledger_for_test(address.owner, address.ordinal) else {
            return false;
        };
        let child = self.repair.child();
        let first_ordinal = address.owner.first_admission_ordinal();
        let Ok(preceding_child) = super::ledger::LifecycleLedgerRecordV1::new(
            child.key,
            address.owner,
            first_ordinal,
            child.work_class,
            child.stage,
            None,
            child.reconstruction_source,
            child.payload,
            child.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        ) else {
            return false;
        };
        let Ok(displaced_parent) = super::ledger::LifecycleLedgerRecordV1::new(
            parent.key,
            address.owner,
            other_ordinal,
            parent.work_class,
            parent.stage,
            None,
            parent.reconstruction_source,
            parent.payload,
            parent.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        ) else {
            return false;
        };
        let Ok(wrong_ordinal) = super::ledger::LifecycleLedgerV1::new(
            exact.context(),
            other_ordinal,
            vec![preceding_child, displaced_parent],
            BTreeMap::new(),
        ) else {
            return false;
        };
        let wrong_owner = OwnerId::new(parent.causal_root, other_ordinal);
        let Ok(wrong_owner) = self.parent_ledger_for_test(wrong_owner, other_ordinal) else {
            return false;
        };
        let wrong_row = super::ledger::LifecycleLedgerV1::empty(exact.context());
        self.ledger_parent_core_identity_is_exact(&exact)
            && !self.ledger_parent_core_identity_is_exact(&wrong_ordinal)
            && !self.ledger_parent_core_identity_is_exact(&wrong_owner)
            && !self.ledger_parent_core_identity_is_exact(&wrong_row)
    }

    /// Prove structurally valid replay-origin substitutions fail on both rows.
    pub(crate) fn rejects_foreign_replay_authorities_for_test(&self) -> bool {
        let address = self.validation.address;
        let Ok(seed) = self.parent_ledger_for_test(address.owner, address.ordinal) else {
            return false;
        };
        let Ok((repaired, child_ordinal, changed)) =
            seed.stage_authenticated_wal_vote_repair(&self.repair)
        else {
            return false;
        };
        let Ok((physical, _, _)) = self.repair.child().physical_geometry.normalized() else {
            return false;
        };
        let Some((&child_slot, _)) = physical.first_key_value() else {
            return false;
        };
        let Some(child_address) =
            ConcreteWorkAddress::new(address.owner, child_ordinal, child_slot)
        else {
            return false;
        };
        let projection = AuthenticatedRecoveredWalSignProjection {
            parent: self.repair.parent().clone(),
            child: self.repair.child().clone(),
            parent_address: address,
            child_address,
        };
        let context = repaired.context();
        let Some(foreign_parent) = repaired.with_foreign_replay_authority_for_test(address.ordinal)
        else {
            return false;
        };
        let Some(foreign_child) = repaired.with_foreign_replay_authority_for_test(child_ordinal)
        else {
            return false;
        };
        changed
            && projection.repaired_pair_is_exact(context, repaired.records())
            && !projection.repaired_pair_is_exact(context, foreign_parent.records())
            && !projection.repaired_pair_is_exact(context, foreign_child.records())
    }

    /// Consume the complete outer authority through one real fsync, reopen the
    /// frame, and prove that the authenticated repeat stutters exactly.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    pub(crate) fn persist_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            super::ledger::WalVoteLedgerRepairTestSummary,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let seed = match self.parent_ledger_for_test(
            self.validation.address.owner,
            self.validation.address.ordinal,
        ) {
            Ok(seed) => seed,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        if !opened.records().is_empty() || opened.high_water() != 0 {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        if let Err(error) = store.persist(&seed) {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                    _error: error,
                    _authority: self,
                },
            });
        }
        let (repaired, durable, first_changed) = self.persist_in_opened_ledger(&store, &seed)?;
        let (reopened_store, reopened) =
            match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
                Ok(reopened) => reopened,
                Err(_error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                            _authority: durable,
                        },
                    });
                }
            };
        let reopened_exact =
            reopened == repaired && durable.post_fsync_authority_is_exact(&reopened_store);
        let (repeated, child_ordinal, repeat_changed) =
            match durable.stage_repeat_for_test(&reopened) {
                Ok(repeated) => repeated,
                Err(_error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                            _authority: durable,
                        },
                    });
                }
            };
        if repeated != repaired || child_ordinal != durable.repair.child_ordinal() {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::PostFsync {
                    _authority: durable,
                },
            });
        }
        let parent_ordinal = durable.validation.address.ordinal;
        let parent = repeated
            .records()
            .iter()
            .find(|record| record.ordinal() == parent_ordinal);
        let child = repeated
            .records()
            .iter()
            .find(|record| record.ordinal() == child_ordinal);
        let repair = durable.repair.repair();
        let edge = repair.edge();
        let parent_advanced = parent.is_some_and(|record| {
            record.key() == Some(repair.parent().key)
                && record.owner() == durable.validation.address.owner
                && record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
                && record.continuation()
                    == Some(super::schema::DurableContinuation::successor(
                        edge,
                        child_ordinal,
                    ))
        });
        let child_live = child.is_some_and(|record| {
            let candidate = repair.child();
            candidate.initial_state == InitialLifecycleState::Ready
                && record.key() == Some(candidate.key)
                && record.owner() == durable.validation.address.owner
                && record.work_class() == Some(candidate.work_class)
                && record.stage() == Some(candidate.stage)
                && record.reconstruction_source() == candidate.reconstruction_source
                && record.durable_payload() == Some(candidate.payload)
                && record.terminal() == Some(None)
                && record.continuation() == Some(super::schema::DurableContinuation::None)
        });
        let summary = super::ledger::WalVoteLedgerRepairTestSummary::new(
            child_ordinal,
            edge,
            first_changed,
            repeat_changed,
            parent_advanced,
            child_live,
            repeated.high_water(),
            durable.repair.ledger_frame_hash() != LifecycleDigest::new([0_u8; 32]),
            reopened_exact,
        );
        Ok((summary, durable))
    }

    /// Exercise the outer stale-snapshot guard without releasing its sealed
    /// error authority. The exact parent snapshot is intentionally not written
    /// to the opened store before the consuming persistence call.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_stale_snapshot_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let seed = match self.parent_ledger_for_test(
            self.validation.address.owner,
            self.validation.address.ordinal,
        ) {
            Ok(seed) => seed,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        if !opened.records().is_empty() || opened.high_water() != 0 {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::ParentLedgerMismatch {
                    _authority: self,
                },
            });
        }
        self.persist_in_opened_ledger(&store, &seed)
            .map(|(_ledger, durable, _changed)| durable)
    }

    /// Reopen an existing ledger frame and consume this fresh startup
    /// authority through the same idempotent fsync seam.
    ///
    /// This models a crash after ledger publication but before registry
    /// installation. Only the exact already-repaired pair may return
    /// `changed == false`; the production staging function rejects every
    /// third parent/child shape.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_reopened_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        (
            bool,
            DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        ),
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        let context = LifecycleContext::new(
            self.repair.parent().key.context(),
            self.repair.parent().key.round().height(),
        );
        let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok(opened) => opened,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Persist {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        self.persist_in_opened_ledger(&store, &opened)
            .map(|(_ledger, durable, changed)| (changed, durable))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry> DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry> {
    fn post_fsync_authority_is_exact(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        let repair = self.repair.repair();
        let Some((child, child_digest)) = self.reservation.child else {
            return false;
        };
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        recovered_validate_authority_is_exact(repair, &self.validation, &self.reservation)
            && self.reservation.exact_vacant_pair(&self.validation)
            && child.owner == self.validation.address.owner
            && child.owner.causal_root() == repair.child().causal_root
            && child.ordinal == self.repair.child_ordinal()
            && child.slot == effect_slot
            && self
                .reservation
                .registry
                .entries
                .keys()
                .all(|address| address.owner != child.owner)
            && repair
                .child()
                .physical_geometry
                .normalized()
                .ok()
                .is_some_and(|(physical, universe, consumed)| {
                    physical.len() == 1
                        && universe.len() == 1
                        && consumed == universe
                        && physical.get(&child.slot) == Some(&child_digest)
                })
            && store.revalidates_durable_authenticated_wal_vote_repair(&self.repair)
    }

    /// Consume the complete post-fsync authority into one exact closed Sign
    /// registry row.
    ///
    /// The current store frame, idempotent repaired-pair shape, parent/child
    /// vacancies, empty causal owner, receipt ordinal, sole Effect slot, and
    /// child digest are all checked before the single insertion. An error
    /// therefore retains the complete uninstalled authority. After insertion
    /// no fallible operation runs; the returned opaque cut keeps the registry
    /// exclusively borrowed and revalidates the exact row without exposing it.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_sign(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        if !self.post_fsync_authority_is_exact(store) {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::InvalidPreflight { _authority: self },
            });
        }
        let (child_address, child_digest) = self
            .reservation
            .child
            .expect("exact post-fsync authority reserves one Sign child");
        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        let work = ConcreteLifecycleWork {
            digest: child_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalSign(DurableRecoveredWalSignWork {
                repair,
                validation,
            }),
        };
        debug_assert!(work.validates_at(child_address));
        let std::collections::btree_map::Entry::Vacant(entry) =
            registry.entries.entry(child_address)
        else {
            unreachable!("exclusive preflight proved the recovered Sign address vacant")
        };
        entry.insert(work);
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address,
            child_digest,
        })
    }

    #[cfg(test)]
    fn stage_repeat_for_test(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> Result<(super::ledger::LifecycleLedgerV1, u128, bool), super::ledger::LifecycleLedgerError>
    {
        ledger.stage_authenticated_wal_vote_repair(self.repair.repair())
    }

    /// Reopen the focused store and revalidate the frame-bound receipt plus
    /// both still-vacant registry reservations without exposing either one.
    #[cfg(test)]
    pub(crate) fn remains_exact_for_test(&self, root: &std::path::Path) -> bool {
        let repair = self.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, ledger)| {
                self.post_fsync_authority_is_exact(&store)
                    && self.stage_repeat_for_test(&ledger).ok().is_some_and(
                        |(repeated, ordinal, changed)| {
                            repeated == ledger && ordinal == self.repair.child_ordinal() && !changed
                        },
                    )
            })
    }

    /// Reopen the supplied ledger root and consume this durable authority into
    /// its exact recovered Sign row.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn install_for_test(
        self,
        root: &std::path::Path,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let repair = self.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        let store = match super::ledger::LifecycleLedgerStoreV1::open(root, context) {
            Ok((store, _opened)) => store,
            Err(error) => {
                return Err(RecoveredWalSignInstallError {
                    failure: RecoveredWalSignInstallFailure::StoreOpen {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        self.install_recovered_sign(&store)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl InstalledRecoveredWalSignRegistryCut<'_> {
    fn installed_entry_is_exact(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        if self.parent_address == self.child_address
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
        {
            return false;
        }
        self.registry
            .entries
            .get(&self.child_address)
            .is_some_and(|work| {
                work.digest == self.child_digest
                    && work.validates_at(self.child_address)
                    && matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                            if sign.validates_in_store(
                                self.child_address,
                                self.child_digest,
                                store,
                            )
                    )
            })
    }

    /// Reopen the receipt's height-local store and prove the installed parent,
    /// child, owner-count, ordinal, sole Effect slot, digest, and frame binding.
    #[cfg(test)]
    pub(crate) fn exact_installed_shape_for_test(&self, root: &std::path::Path) -> bool {
        let Some(work) = self.registry.entries.get(&self.child_address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) = &work.kind else {
            return false;
        };
        let repair = sign.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, _opened)| self.installed_entry_is_exact(&store))
    }
}

#[cfg(test)]
impl RecoveredWalSignInstallError<'_> {
    /// Prove that this opaque error still owns the complete exact authority and
    /// both registry vacancies when checked against the original store.
    pub(crate) fn retains_exact_vacancies_for_test(&self, root: &std::path::Path) -> bool {
        let authority = match &self.failure {
            RecoveredWalSignInstallFailure::InvalidPreflight {
                _authority: authority,
            }
            | RecoveredWalSignInstallFailure::StoreOpen {
                _authority: authority,
                ..
            } => authority,
        };
        let repair = authority.repair.repair();
        let context = LifecycleContext::new(
            repair.parent().key.context(),
            repair.parent().key.round().height(),
        );
        super::ledger::LifecycleLedgerStoreV1::open(root, context)
            .ok()
            .is_some_and(|(store, _opened)| authority.post_fsync_authority_is_exact(&store))
    }
}
// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_END
// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_END

// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_BEGIN
/// Opaque logical projection minted only from one exact installed Sign row.
///
/// Its fields are private, it has no constructor or parts API, and it carries
/// no effect, pending binding, body receipt, or ledger receipt. The durable
/// open module may query or splice it, but callers cannot supply substitute
/// parent/child candidates to the authenticated recovery cut.
pub(super) struct AuthenticatedRecoveredWalSignProjection {
    parent: CandidateAdmission,
    child: CandidateAdmission,
    parent_address: ConcreteWorkAddress,
    child_address: ConcreteWorkAddress,
}

impl AuthenticatedRecoveredWalSignProjection {
    /// Return whether both sealed candidates belong to one exact context.
    pub(super) fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        let Ok((physical, universe, consumed)) = self.child.physical_geometry.normalized() else {
            return false;
        };
        context.id() == self.parent.key.context()
            && context.height() == self.parent.key.round().height()
            && context.id() == self.child.key.context()
            && context.height() == self.child.key.round().height()
            && self.parent.work_class == LifecycleWorkClass::Validate
            && self.child.work_class == LifecycleWorkClass::SignVote
            && self.parent.causal_root == self.child.causal_root
            && self.parent_address.owner.causal_root() == self.parent.causal_root
            && self.child_address.owner == self.parent_address.owner
            && self.child_address.owner.causal_root() == self.child.causal_root
            && self.parent_address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            && self.parent_address.ordinal < self.child_address.ordinal
            && self.child_address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            && physical.len() == 1
            && universe.len() == 1
            && consumed == universe
            && physical.contains_key(&self.child_address.slot)
    }

    /// Return the sealed recovered Validate semantic key.
    pub(super) const fn parent_key(&self) -> LifecycleKey {
        self.parent.key
    }

    /// Return the sealed recovered Sign semantic key.
    pub(super) const fn child_key(&self) -> LifecycleKey {
        self.child.key
    }

    fn continuation_edge(&self) -> Option<super::schema::DurableContinuationEdge> {
        match (self.child.key.phase(), self.child.stage.kind()) {
            (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                Some(super::schema::DurableContinuationEdge::ValidateToSignPrepare)
            }
            (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                Some(super::schema::DurableContinuationEdge::ValidateToSignCommit)
            }
            _ => None,
        }
    }

    fn repaired_child_record_is_exact(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        self.belongs_to_context(context)
            && record.key() == Some(self.child.key)
            && record.owner() == self.child_address.owner
            && record.ordinal() == self.child_address.ordinal
            && record.work_class() == Some(self.child.work_class)
            && record.stage() == Some(self.child.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.child.reconstruction_source
            && record.durable_payload() == Some(self.child.payload)
            && record.replay_matches_candidate(&self.child)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && self.child.initial_state == InitialLifecycleState::Ready
            && self.child.producer_turn.is_none()
    }

    /// Prove that one repaired LedgerV1 frame retains both exact sides and its
    /// typed Validate→Sign edge at the installed concrete addresses.
    pub(super) fn repaired_pair_is_exact(
        &self,
        context: LifecycleContext,
        records: &[super::ledger::LifecycleLedgerRecordV1],
    ) -> bool {
        let Some(edge) = self.continuation_edge() else {
            return false;
        };
        let Some(parent) = records
            .iter()
            .find(|record| record.ordinal() == self.parent_address.ordinal)
        else {
            return false;
        };
        let Some(child) = records
            .iter()
            .find(|record| record.ordinal() == self.child_address.ordinal)
        else {
            return false;
        };
        self.repaired_child_record_is_exact(context, child)
            && parent.key() == Some(self.parent.key)
            && parent.owner() == self.parent_address.owner
            && parent.ordinal() == self.parent_address.ordinal
            && parent.work_class() == Some(self.parent.work_class)
            && parent.stage() == Some(self.parent.stage)
            && parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && parent.reconstruction_source() == self.parent.reconstruction_source
            && parent.durable_payload() == Some(self.parent.payload)
            && parent.replay_matches_candidate(&self.parent)
            && parent.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    edge,
                    self.child_address.ordinal,
                ))
            && self.parent.initial_state == InitialLifecycleState::Ready
            && self.parent.producer_turn.is_none()
    }

    /// Install the exact opaque Sign child only when one live repaired ledger
    /// row retains its complete installed address and logical identity.
    ///
    /// This is the production reconstruction surface for the post-fsync crash
    /// cut. It accepts no caller-supplied candidate and mutates the destination
    /// only after every durable field has matched.
    pub(super) fn insert_repaired_child_from_record(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !self.repaired_child_record_is_exact(context, record)
            || candidates.contains_key(&self.parent.key)
            || candidates.contains_key(&self.child.key)
        {
            return false;
        }
        candidates.insert(self.child.key, self.child.clone());
        true
    }

    /// Atomically replace the exact parent or stutter on the exact child.
    pub(super) fn splice_candidates(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        match (
            candidates.get(&self.parent.key),
            candidates.get(&self.child.key),
        ) {
            (Some(parent), None) if parent == &self.parent => {
                let removed = candidates
                    .remove(&self.parent.key)
                    .expect("exact recovered Validate parent was preflighted");
                debug_assert_eq!(&removed, &self.parent);
                let displaced = candidates.insert(self.child.key, self.child.clone());
                debug_assert!(displaced.is_none());
                true
            }
            // A fresh startup after the ledger fsync reconstructs the already
            // live Sign child and must stutter at this logical splice.
            (None, Some(child)) if child == &self.child => true,
            // Any foreign value occupying either semantic key, both exact
            // sides at once, or neither side fails before mutation.
            _ => false,
        }
    }

    /// Prove the parent is absent and the exact child is retained.
    pub(super) fn owns_spliced_candidates(
        &self,
        candidates: &BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        !candidates.contains_key(&self.parent.key)
            && candidates.get(&self.child.key) == Some(&self.child)
    }

    /// Build one closed repaired-pair fixture without exposing either raw
    /// candidate to sibling lifecycle tests.
    #[cfg(all(test, feature = "bls"))]
    pub(super) fn repaired_ledger_fixture_for_test(
        context: LifecycleContext,
        marker: u8,
    ) -> Option<(Self, super::ledger::LifecycleLedgerV1)> {
        let root = super::CausalRoot::new(LifecycleDigest::new([marker.wrapping_add(3); 32]));
        let parent_replay = super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::ValidateBody,
            marker,
        );
        let child_replay = super::replay_authority::exact_record_fixture(
            context,
            LifecycleStageKind::SignPrepareVote,
            marker,
        );
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let parent = CandidateAdmission::new(
            parent_replay.key,
            root,
            LifecycleWorkClass::Validate,
            LifecycleStage::new(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            ),
            InitialLifecycleState::Ready,
            root.digest(),
            parent_replay.payload,
            parent_replay.authority,
            super::PhysicalGeometry::new(
                [PhysicalSlot::new(
                    effect_slot,
                    LifecycleDigest::new([marker.wrapping_add(4); 32]),
                )],
                [effect_slot],
            ),
            None,
        );
        let child = CandidateAdmission::new(
            child_replay.key,
            root,
            LifecycleWorkClass::SignVote,
            LifecycleStage::new(
                LifecycleStageKind::SignPrepareVote,
                PredecessorScope::Independent,
            ),
            InitialLifecycleState::Ready,
            root.digest(),
            DurablePayloadReference::None,
            child_replay.authority,
            super::PhysicalGeometry::new(
                [PhysicalSlot::new(
                    effect_slot,
                    LifecycleDigest::new([marker.wrapping_add(5); 32]),
                )],
                [effect_slot],
            ),
            None,
        );
        let owner = OwnerId::new(root, 1);
        let parent_address = ConcreteWorkAddress::new(owner, 1, effect_slot)?;
        let child_address = ConcreteWorkAddress::new(owner, 2, effect_slot)?;
        let parent_record = super::ledger::LifecycleLedgerRecordV1::new(
            parent.key,
            owner,
            parent_address.ordinal,
            parent.work_class,
            parent.stage,
            Some(super::TerminalOutcome::Advanced),
            parent.reconstruction_source,
            parent.payload,
            parent.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::ValidateToSignPrepare,
                child_address.ordinal,
            ),
        )
        .ok()?;
        let child_record = super::ledger::LifecycleLedgerRecordV1::new(
            child.key,
            owner,
            child_address.ordinal,
            child.work_class,
            child.stage,
            None,
            child.reconstruction_source,
            child.payload,
            child.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
        .ok()?;
        let ledger = super::ledger::LifecycleLedgerV1::new(
            context,
            child_address.ordinal,
            vec![parent_record, child_record],
            BTreeMap::new(),
        )
        .ok()?;
        Some((
            Self {
                parent,
                child,
                parent_address,
                child_address,
            },
            ledger,
        ))
    }

    /// Seed only the opaque projection's parent in a focused recovery fixture.
    #[cfg(test)]
    pub(super) fn seed_parent_candidate_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates
            .insert(self.parent.key, self.parent.clone())
            .is_none()
    }

    /// Seed only the opaque projection's child in a focused recovery fixture.
    #[cfg(test)]
    pub(super) fn seed_child_candidate_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        candidates
            .insert(self.child.key, self.child.clone())
            .is_none()
    }

    /// Seed both exact sides to prove the production splice rejects ambiguity.
    #[cfg(test)]
    pub(super) fn seed_both_candidates_for_test(
        &self,
        candidates: &mut BTreeMap<LifecycleKey, CandidateAdmission>,
    ) -> bool {
        if !candidates.is_empty() {
            return false;
        }
        candidates.insert(self.parent.key, self.parent.clone());
        candidates.insert(self.child.key, self.child.clone());
        true
    }
}

/// Sealed coordinator-open result for one installed recovered Sign.
///
/// The registry remains exclusively borrowed and the authenticated recovery
/// cut stays beside the opened coordinator. No ordinary coordinator, concrete
/// row, candidate, or receipt extraction surface exists.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "opened recovered WAL Sign startup has not published adapter status"]
pub(crate) struct OpenedRecoveredWalSignLifecycleCut<'registry> {
    installed: InstalledRecoveredWalSignRegistryCut<'registry>,
    recovery: AuthenticatedLifecycleRecoveryCut,
    coordinator: LifecycleCoordinator,
}

/// Opaque fail-stop coordinator-open error retaining every volatile input.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "failed recovered WAL coordinator open still owns all startup authority"]
pub(crate) struct RecoveredWalSignLifecycleOpenError<'registry> {
    failure: RecoveredWalSignLifecycleOpenFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalSignLifecycleOpenFailure<'registry> {
    InvalidAuthority {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    InvalidRegistry {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    InvalidRecovery {
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    Prepare {
        _error: LifecycleOpenError,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    PreCommitMismatch {
        _prepared: PreparedLifecycleCoordinatorOpen,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    Commit {
        _error: LifecycleOpenCommitError,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
    PostCommitMismatch {
        _coordinator: LifecycleCoordinator,
        _installed: InstalledRecoveredWalSignRegistryCut<'registry>,
        _recovery: AuthenticatedLifecycleRecoveryCut,
    },
}

impl RecoveredWalSignLifecycleOpenError<'_> {
    /// Stable diagnostic which exposes none of the retained recovery parts.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalSignLifecycleOpenFailure::InvalidAuthority { .. } => {
                "verified height cannot derive recovered lifecycle authority"
            }
            RecoveredWalSignLifecycleOpenFailure::InvalidRegistry { .. } => {
                "installed recovered Sign registry seal is inconsistent"
            }
            RecoveredWalSignLifecycleOpenFailure::InvalidRecovery { .. } => {
                "authenticated recovery lacks the exact recovered WAL handoff"
            }
            RecoveredWalSignLifecycleOpenFailure::Prepare { .. } => {
                "repaired lifecycle ledger could not prepare an exact coordinator open"
            }
            RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch { .. } => {
                "prepared coordinator disagrees with the installed recovered Sign"
            }
            RecoveredWalSignLifecycleOpenFailure::Commit { .. } => {
                "exact recovered coordinator stores could not be published"
            }
            RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch { .. } => {
                "published coordinator disagrees with the installed recovered Sign"
            }
        }
    }
}

impl InstalledRecoveredWalSignRegistryCut<'_> {
    fn structurally_exact_sign(&self) -> Option<&DurableRecoveredWalSignWork> {
        if self.parent_address == self.child_address
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.child_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) = &work.kind else {
            return None;
        };
        (work.digest == self.child_digest
            && work.validates_at(self.child_address)
            && sign.validates_at(self.child_address, self.child_digest))
        .then_some(sign)
    }

    fn authenticated_projection(&self) -> Option<AuthenticatedRecoveredWalSignProjection> {
        let sign = self.structurally_exact_sign()?;
        let repair = sign.repair.repair();
        Some(AuthenticatedRecoveredWalSignProjection {
            parent: repair.parent().clone(),
            child: repair.child().clone(),
            parent_address: self.parent_address,
            child_address: self.child_address,
        })
    }

    fn coordinator_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let candidate = &projection.child;
        let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
            return false;
        };
        let Some(record) = coordinator.records.get(&self.child_address.ordinal) else {
            return false;
        };
        let Some(durable) = coordinator.durable_records.get(&self.child_address.ordinal) else {
            return false;
        };
        coordinator.fault.is_none()
            && coordinator.active_context.id() == candidate.key.context()
            && coordinator.active_context.height() == candidate.key.round().height()
            && coordinator.high_water >= self.child_address.ordinal
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.producer_turn.is_none()
            && record.key == candidate.key
            && record.owner == self.child_address.owner
            && record.owner.causal_root() == candidate.causal_root
            && record.ordinal == self.child_address.ordinal
            && record.work_class == LifecycleWorkClass::SignVote
            && record.stage == candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && durable.matches_admission(candidate)
            && coordinator.key_index.get(&candidate.key) == Some(&self.child_address.ordinal)
            && coordinator.owner_index.get(&candidate.causal_root) == Some(&record.owner)
            && coordinator
                .ready_index
                .contains(&self.child_address.ordinal)
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(prepared.store())
            && self.coordinator_is_exact(prepared.coordinator(), projection)
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(store)
            && self.coordinator_is_exact(coordinator, projection)
    }
}

impl<'registry> InstalledRecoveredWalSignRegistryCut<'registry> {
    #[allow(clippy::result_large_err)]
    fn open_with_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(projection) = self.authenticated_projection() else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRegistry {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        if !recovery.splice_recovered_wal_sign(&projection)
            || !recovery.owns_recovered_wal_sign(&projection)
        {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidRecovery {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let prepared = match LifecycleCoordinator::prepare_with_authority_borrowed(
            authority,
            ledger_root,
            payload_store,
            &recovery,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Prepare {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.prepared_join_is_exact(&prepared, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch {
                    _prepared: prepared,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        let coordinator = match prepared.commit(payload_store, &recovery) {
            Ok(coordinator) => coordinator,
            Err(error) => {
                return Err(RecoveredWalSignLifecycleOpenError {
                    failure: RecoveredWalSignLifecycleOpenFailure::Commit {
                        _error: error,
                        _installed: self,
                        _recovery: recovery,
                    },
                });
            }
        };
        if !self.opened_join_is_exact(&coordinator, &recovery, &projection) {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch {
                    _coordinator: coordinator,
                    _installed: self,
                    _recovery: recovery,
                },
            });
        }
        Ok(OpenedRecoveredWalSignLifecycleCut {
            installed: self,
            recovery,
            coordinator,
        })
    }

    /// Prepare, exact-join, and durably publish the recovered coordinator from
    /// production verified/configured authority without releasing the registry
    /// borrow.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_coordinator_from_verified(
        self,
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(authority) =
            authority::production_authority(verified, config, reply_route_source_capacity)
        else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidAuthority {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        self.open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    /// Open with the minimal exact test authority while retaining all seals.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_coordinator_for_test(
        self,
        verified: &VerifiedHeightContext,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        OpenedRecoveredWalSignLifecycleCut<'registry>,
        RecoveredWalSignLifecycleOpenError<'registry>,
    > {
        let Some(authority) = authority::recovered_wal_test_authority(verified) else {
            return Err(RecoveredWalSignLifecycleOpenError {
                failure: RecoveredWalSignLifecycleOpenFailure::InvalidAuthority {
                    _installed: self,
                    _recovery: recovery,
                },
            });
        };
        self.open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    /// Corrupt only the opaque installed-token digest for a focused negative
    /// test. The closed registry row and its complete durable authority remain
    /// present and exclusively borrowed.
    #[cfg(test)]
    pub(crate) fn corrupt_registry_seal_for_test(&mut self) {
        self.child_digest = LifecycleDigest::new([0xFF; 32]);
    }

    /// Seed the exact opaque recovered Validate parent for a focused fixture.
    #[cfg(test)]
    pub(crate) fn seed_parent_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection()
            .is_some_and(|projection| recovery.seed_recovered_wal_parent_for_test(&projection))
    }

    /// Seed the exact opaque recovered Sign child for a re-entry fixture.
    #[cfg(test)]
    pub(crate) fn seed_child_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection()
            .is_some_and(|projection| recovery.seed_recovered_wal_child_for_test(&projection))
    }

    /// Seed both opaque projection sides for an ambiguous-recovery negative.
    #[cfg(test)]
    pub(crate) fn seed_both_recovery_for_test(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.authenticated_projection().is_some_and(|projection| {
            recovery.seed_both_recovered_wal_candidates_for_test(&projection)
        })
    }
}

#[cfg(test)]
impl OpenedRecoveredWalSignLifecycleCut<'_> {
    /// Revalidate the complete installed/recovery/coordinator/store join.
    pub(crate) fn exact_join_for_test(&self) -> bool {
        let Some(projection) = self.installed.authenticated_projection() else {
            return false;
        };
        self.installed
            .opened_join_is_exact(&self.coordinator, &self.recovery, &projection)
    }
}

#[cfg(test)]
impl RecoveredWalSignLifecycleOpenError<'_> {
    fn installed(&self) -> &InstalledRecoveredWalSignRegistryCut<'_> {
        match &self.failure {
            RecoveredWalSignLifecycleOpenFailure::InvalidAuthority { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::InvalidRegistry { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::InvalidRecovery { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::Prepare { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::PreCommitMismatch { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::Commit { _installed, .. }
            | RecoveredWalSignLifecycleOpenFailure::PostCommitMismatch { _installed, .. } => {
                _installed
            }
        }
    }

    /// Prove the error retains one exact installed row against the ledger.
    pub(crate) fn retains_exact_installed_for_test(&self, ledger_root: &Path) -> bool {
        self.installed().exact_installed_shape_for_test(ledger_root)
    }

    /// Prove the error still exclusively owns a closed recovered Sign row.
    pub(crate) fn retains_closed_registry_row_for_test(&self) -> bool {
        let installed = self.installed();
        installed
            .registry
            .entries
            .get(&installed.child_address)
            .is_some_and(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
                )
            })
    }
}
// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_END

impl DetachedRecoveredValidateCompletion {
    fn restore(
        self,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> ConcreteLifecycleWork {
        let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) = self.replay_evidence
        else {
            panic!("a cold recovered body marker cannot reconstruct live certified Fetch origin")
        };
        ConcreteLifecycleWork {
            digest: self.installed_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(DurableValidateCompletion {
                address: self.address,
                incumbent: DurableValidateBody {
                    address: self.incumbent_address,
                    effect,
                    pending,
                    durable_receipt: self.durable_receipt,
                    expected_manifest_hash: self.expected_manifest_hash,
                    replay_evidence,
                },
                incumbent_digest: self.incumbent_digest,
                outcome: self.outcome,
            }),
        }
    }
}

/// Ownership-preserving failure from the fixed recovered-WAL parent join.
///
/// No adapter effect, pending binding, recovered vote, or registry entry is
/// exposed. Before projection, dropping this value restores the exact detached
/// carrier; a lifecycle-authentication failure retains all linear authority
/// and requires restart rather than falling back to ordinary execution.
#[must_use = "failed recovered WAL validation still owns its sealed authority"]
pub(crate) struct RecoveredWalValidateRegistryJoinError<'registry> {
    failure: RecoveredWalValidateRegistryJoinFailure<'registry>,
}

#[allow(clippy::large_enum_variant, variant_size_differences)]
enum RecoveredWalValidateRegistryJoinFailure<'registry> {
    InvalidCarrier {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _recovered: RecoveredWalVoteSign,
    },
    Projection {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _recovered: RecoveredWalVoteSign,
    },
    Lifecycle {
        _cut: RecoveredWalValidateRegistryCut<'registry>,
        _error: RecoveredWalVoteLifecycleRepairError,
        _completion: DetachedRecoveredValidateCompletion,
    },
}

impl RecoveredWalValidateRegistryJoinError<'_> {
    /// Return a stable diagnostic without exposing retained authority.
    pub(crate) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalValidateRegistryJoinFailure::InvalidCarrier { .. } => {
                "recovered Validate registry carrier is invalid"
            }
            RecoveredWalValidateRegistryJoinFailure::Projection { .. } => {
                "recovered vote does not project from the exact Validate registry carrier"
            }
            RecoveredWalValidateRegistryJoinFailure::Lifecycle { _error, .. } => _error.reason(),
        }
    }
}

impl Drop for RecoveredWalValidateRegistryCut<'_> {
    fn drop(&mut self) {
        let Some(work) = self.work.take() else {
            return;
        };
        let Some(registry) = self.registry.as_deref_mut() else {
            debug_assert!(
                false,
                "detached recovered Validate lost its registry borrow"
            );
            return;
        };
        match registry.entries.entry(self.address) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(work);
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                debug_assert!(false, "detached recovered Validate address was replaced");
            }
        }
    }
}

include!("v2_lifecycle_work_registry_validate_recovery.rs");

// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DetachedDurableValidateExecution {
    /// Execute the exact detached request through the scheduler-free body-store
    /// validation boundary.
    ///
    /// The request is consumed once. A storage failure returns it intact for a
    /// typed recovery decision; a successful storage call seals the request and
    /// closed outcome together in one move-only token.
    #[allow(clippy::result_large_err)]
    fn execute<F, E>(
        self,
        body_store: &mut V2BodyStore,
        validator: F,
    ) -> Result<
        ExecutedDurableValidateExecution,
        (V2BodyStoreError, DetachedDurableValidateExecution),
    >
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        let outcome = match body_store.execute_durable_validation(
            self.durable_receipt.clone(),
            self.expected_manifest_hash,
            validator,
        ) {
            Ok(outcome) => outcome,
            Err(error) => return Err((error, self)),
        };
        if outcome.durable_body() != &self.durable_receipt {
            return Err((V2BodyStoreError::ReceiptMismatch, self));
        }
        Ok(ExecutedDurableValidateExecution {
            request: self,
            outcome,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl ExecutedDurableValidateExecution {
    /// Borrow the body-store-minted closed result without separating it from
    /// the detached registry authority.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        &self.outcome
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDurableValidateCompletion<'_> {
    /// Return the exact reducer coordinates retained across detached execution.
    pub(super) const fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        (
            self.executed.request.tag,
            self.executed.request.round,
            self.executed.request.subject,
        )
    }

    /// Borrow the closed body-store outcome retained under the exact registry
    /// reattachment.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        &self.executed.outcome
    }
}
// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_END

// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DurableValidateDispatch {
    /// Execute the exact request after its claimed lifecycle row became an
    /// external wait.
    ///
    /// This is the sole externally visible execution path. A body-store error
    /// reconstructs and returns the complete dispatch, including its exact
    /// wake authority, so retry cannot mint a second request or wait token.
    #[allow(clippy::result_large_err)]
    pub(super) fn execute<F, E>(
        self,
        body_store: &mut V2BodyStore,
        validator: F,
    ) -> Result<ExecutedDurableValidateDispatch, (V2BodyStoreError, Self)>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        let Self { request, wake } = self;
        match request.execute(body_store, validator) {
            Ok(executed) => Ok(ExecutedDurableValidateDispatch { executed, wake }),
            Err((error, request)) => Err((error, Self { request, wake })),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl ExecutedDurableValidateDispatch {
    /// Borrow the closed result without separating it from wake authority.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        self.executed.outcome()
    }

    #[cfg(test)]
    const fn wait_token_for_test(&self) -> WaitToken {
        self.wake.wait_token
    }
}

#[cfg(test)]
impl DurableValidateDispatch {
    const fn wait_token_for_test(&self) -> WaitToken {
        self.wake.wait_token
    }
}
// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_END

// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DurableValidateCompletionAuthority {
    /// Exact immutable owner of the waiting record.
    pub(super) const fn owner(self) -> OwnerId {
        self.address.owner
    }

    /// Existing lifecycle ordinal; completion never allocates another one.
    pub(super) const fn ordinal(self) -> u128 {
        self.address.ordinal
    }

    /// Equal-address physical slot retained across publication.
    pub(super) const fn slot(self) -> PhysicalSlotId {
        self.address.slot
    }

    /// Digest of the original closed Validate carrier.
    pub(super) const fn incumbent_digest(self) -> LifecycleDigest {
        self.incumbent_digest
    }

    /// Outcome-bound digest installed only for executable outcomes.
    pub(super) const fn replacement_digest(self) -> Option<LifecycleDigest> {
        self.replacement_digest
    }

    /// Exact wait token retained from the claimed-side dispatch cut.
    pub(super) const fn wait_token(self) -> WaitToken {
        self.wait_token
    }

    /// Exact immutable lifecycle key validated before async detachment.
    pub(super) const fn lifecycle_key(self) -> LifecycleKey {
        self.lifecycle_key
    }

    /// Exact immutable lifecycle stage validated before async detachment.
    pub(super) const fn lifecycle_stage(self) -> LifecycleStage {
        self.lifecycle_stage
    }

    /// Return whether the waiting row retains this completion's exact frame.
    pub(super) fn matches_durable_payload(self, payload: DurablePayloadReference) -> bool {
        self.payload == payload
            && super::body_pipeline_transition::durable_validate_payload_is_exact(
                self.lifecycle_key,
                payload,
            )
    }

    /// Whether this exact result must remain Waiting for merge-sidecar service.
    pub(super) const fn is_deferred_merge_sidecar(self) -> bool {
        matches!(
            self.outcome_kind,
            DurableValidateOutcomeKind::DeferredMergeSidecar
        )
    }

    /// Construct the only Ready event authorized by this executable outcome.
    pub(super) fn ready_event(self) -> Option<ReadyEvent> {
        let replacement_digest = self.replacement_digest?;
        if self.is_deferred_merge_sidecar() {
            return None;
        }
        Some(ReadyEvent::new(
            self.address.ordinal,
            self.address.owner,
            self.wait_token,
            Some(PhysicalReplacement::new(
                self.address.slot,
                PhysicalSlot::new(self.address.slot, replacement_digest),
            )),
        ))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'a> PreparedExecutedDurableValidateCompletion<'a> {
    /// Borrow the sealed coordinator publication projection.
    pub(super) const fn authority(&self) -> DurableValidateCompletionAuthority {
        self.authority
    }

    /// Return this preflight only as an ownership-preserving typed failure.
    #[allow(clippy::result_large_err)]
    pub(super) fn fail(
        self,
        error: DurableValidateCompletionPublicationError,
    ) -> (
        DurableValidateCompletionPublicationError,
        ExecutedDurableValidateDispatch,
    ) {
        (error, self.dispatch)
    }

    /// Retain a missing merge-sidecar result without changing either live row.
    ///
    /// TODO: Consume this token only in a sealed sidecar registration plus
    /// same-row wake transaction; raw wait authority remains inaccessible.
    pub(super) fn defer_merge_sidecar(self) -> DeferredDurableValidateDispatch {
        debug_assert!(self.authority.is_deferred_merge_sidecar());
        debug_assert!(self.dispatch.outcome().missing_merge_sidecar().is_some());
        DeferredDurableValidateDispatch {
            dispatch: self.dispatch,
        }
    }

    /// Stage the exact executable outcome as a same-address closed carrier.
    ///
    /// Every CAS and outcome comparison precedes mutation. Once installed, the
    /// returned guard owns rollback until its infallible commit is called.
    #[allow(clippy::result_large_err)]
    pub(super) fn stage_executable_carrier(
        self,
    ) -> Result<
        StagedDurableValidateCompletion<'a>,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        let authority = self.authority;
        let Some(replacement_digest) = authority.replacement_digest else {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidReplacementDigest,
                )),
            );
        };
        if authority.is_deferred_merge_sidecar() || replacement_digest == authority.incumbent_digest
        {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                )),
            );
        }

        let request = &self.dispatch.executed.request;
        let outcome = self.dispatch.outcome();
        let validation_error = match self.registry.entries.get(&authority.address) {
            None => Some(DurableValidateExecutionError::Registry(
                RegistryError::Missing,
            )),
            Some(work) if !work.validates_at(authority.address) => Some(
                DurableValidateExecutionError::Registry(RegistryError::CorruptWork),
            ),
            Some(work) if work.digest != authority.incumbent_digest => Some(
                DurableValidateExecutionError::Registry(RegistryError::DigestMismatch),
            ),
            Some(work) => match &work.kind {
                ConcreteLifecycleWorkKind::DurableValidateBody(incumbent)
                    if incumbent.address == request.address
                        && incumbent.durable_receipt == request.durable_receipt
                        && incumbent.expected_manifest_hash == request.expected_manifest_hash
                        && incumbent.pending.causal_lifecycle_key()
                            == &request.causal_lifecycle_key
                        && incumbent.pending.candidate_statement()
                            == request.candidate_statement
                        && outcome.durable_body() == &incumbent.durable_receipt
                        && durable_validate_completion_digest(
                            authority.incumbent_digest,
                            incumbent.expected_manifest_hash,
                            outcome,
                        ) == Some(replacement_digest) =>
                {
                    None
                }
                ConcreteLifecycleWorkKind::DurableValidateBody(_) => {
                    Some(DurableValidateExecutionError::InvalidValidateShape)
                }
                _ => Some(DurableValidateExecutionError::WrongWorkKind),
            },
        };
        if let Some(error) = validation_error {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(error),
                )),
            );
        }

        let location = DurableValidatePublishedLocation {
            address: authority.address,
            incumbent_digest: authority.incumbent_digest,
            replacement_digest,
        };
        let publication = match authority.outcome_kind {
            DurableValidateOutcomeKind::Validated => {
                PublishedDurableValidateCompletion::Validated(PublishedValidated { location })
            }
            DurableValidateOutcomeKind::Rejected => {
                PublishedDurableValidateCompletion::Rejected(PublishedRejected { location })
            }
            DurableValidateOutcomeKind::DeferredMergeSidecar => unreachable!(
                "deferred Validate outcome was rejected before same-address conversion"
            ),
        };
        let PreparedExecutedDurableValidateCompletion {
            registry,
            dispatch,
            authority: _,
        } = self;
        let ExecutedDurableValidateDispatch { executed, wake } = dispatch;
        let ExecutedDurableValidateExecution { request, outcome } = executed;
        let Some(incumbent) = registry.entries.remove(&authority.address) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::Registry(RegistryError::Missing),
                    ),
                ),
                ExecutedDurableValidateDispatch {
                    executed: ExecutedDurableValidateExecution { request, outcome },
                    wake,
                },
            ));
        };
        let ConcreteLifecycleWork {
            digest: incumbent_digest,
            kind,
        } = incumbent;
        let incumbent = match kind {
            ConcreteLifecycleWorkKind::DurableValidateBody(incumbent) => incumbent,
            kind => {
                let _ = registry.entries.insert(
                    authority.address,
                    ConcreteLifecycleWork {
                        digest: incumbent_digest,
                        kind,
                    },
                );
                return Err((
                    DurableValidateCompletionPublicationError::Registry(
                        DurableValidateCompletionConversionError::Execution(
                            DurableValidateExecutionError::WrongWorkKind,
                        ),
                    ),
                    ExecutedDurableValidateDispatch {
                        executed: ExecutedDurableValidateExecution { request, outcome },
                        wake,
                    },
                ));
            }
        };
        let completion = DurableValidateCompletion {
            address: authority.address,
            incumbent,
            incumbent_digest,
            outcome,
        };
        let installed = ConcreteLifecycleWork {
            digest: replacement_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(completion),
        };
        let displaced = registry.entries.insert(authority.address, installed);
        let staged = StagedDurableValidateCompletion {
            entries: &mut registry.entries,
            address: authority.address,
            request: Some(request),
            wake: Some(wake),
            publication,
            armed: true,
        };
        debug_assert!(displaced.is_none());
        debug_assert!(staged.entries.get(&authority.address).is_some_and(|work| {
            work.validates_at(authority.address) && work.digest == replacement_digest
        }));
        drop(displaced);
        Ok(staged)
    }
}

impl StagedDurableValidateCompletion<'_> {
    fn restore(&mut self) -> Option<ExecutedDurableValidateDispatch> {
        if !self.armed {
            return None;
        }
        self.armed = false;
        let request = self.request.take();
        let wake = self.wake.take();
        let Some(installed) = self.entries.remove(&self.address) else {
            drop(request);
            drop(wake);
            return None;
        };
        let ConcreteLifecycleWork {
            digest: replacement_digest,
            kind,
        } = installed;
        let completion = match kind {
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => completion,
            kind => {
                let _ = self.entries.insert(
                    self.address,
                    ConcreteLifecycleWork {
                        digest: replacement_digest,
                        kind,
                    },
                );
                drop(request);
                drop(wake);
                return None;
            }
        };
        let DurableValidateCompletion {
            address,
            incumbent,
            incumbent_digest,
            outcome,
        } = completion;
        let _ = self.entries.insert(
            address,
            ConcreteLifecycleWork {
                digest: incumbent_digest,
                kind: ConcreteLifecycleWorkKind::DurableValidateBody(incumbent),
            },
        );
        let (Some(request), Some(wake)) = (request, wake) else {
            return None;
        };
        Some(ExecutedDurableValidateDispatch {
            executed: ExecutedDurableValidateExecution { request, outcome },
            wake,
        })
    }

    /// Permanently retain the already-installed carrier and return only its
    /// precomputed Copy publication metadata.
    pub(super) fn commit(mut self) -> PublishedDurableValidateCompletion {
        self.armed = false;
        self.publication
    }
}

impl Drop for StagedDurableValidateCompletion<'_> {
    fn drop(&mut self) {
        drop(self.restore());
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl DeferredDurableValidateDispatch {
    /// Borrow the exact missing sidecar reference without exposing wake parts.
    pub(super) fn missing_reference(&self) -> &CertifiedMergeLedgerReference {
        self.dispatch
            .outcome()
            .missing_merge_sidecar()
            .expect("deferred Validate token retains one exact merge-sidecar reference")
    }

    #[cfg(test)]
    const fn dispatch_for_test(&self) -> &ExecutedDurableValidateDispatch {
        &self.dispatch
    }
}

#[cfg(test)]
impl PublishedValidated {
    const fn location_for_test(&self) -> DurableValidatePublishedLocation {
        self.location
    }
}

#[cfg(test)]
impl PublishedRejected {
    const fn location_for_test(&self) -> DurableValidatePublishedLocation {
        self.location
    }
}
// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_END

impl<'a> PreparedCertifiedFetchCompletion<'a> {
    /// Bind this drop-inert preflight to the exact body-store durability proof.
    ///
    /// Every comparison is read-only. Failure or drop leaves the incumbent
    /// registry row unchanged, while success moves the exclusive borrow and
    /// receipt into the sole post-dequeue authority.
    #[allow(dead_code)]
    pub(super) fn bind_durable_body_receipt(
        self,
        durable_receipt: DurableCertifiedFetchBodyReceipt,
    ) -> Result<PreparedDurableCertifiedFetchCompletion<'a>, CertifiedFetchCompletionError> {
        let address = self.location.address();
        let incumbent = self
            .registry
            .entries
            .get(&address)
            .ok_or(CertifiedFetchCompletionError::MissingIncumbent)?;
        if !incumbent.validates_at(address) {
            return Err(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let Some((incumbent_effect, _incumbent_pending)) = incumbent.pending_adapter_pair() else {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if self.location.owner.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if incumbent.digest != self.location.incumbent_digest {
            return Err(CertifiedFetchCompletionError::IncumbentDigestMismatch);
        }
        if self.location.replacement_digest != self.ingress_identity.digest() {
            return Err(CertifiedFetchCompletionError::ReplacementDigestMismatch);
        }
        if !durable_receipt_matches_fetch(
            &durable_receipt,
            incumbent_effect,
            self.request_hash,
            self.response_hash,
            self.response_round,
            self.response_subject,
            self.response_manifest_hash,
        ) {
            return Err(CertifiedFetchCompletionError::DurableReceiptMismatch);
        }
        let replay_evidence = self
            .replay_origin
            .bind_durable_body(&durable_receipt)
            .ok_or(CertifiedFetchCompletionError::InvalidReplayEvidence)?;

        Ok(PreparedDurableCertifiedFetchCompletion {
            registry: self.registry,
            location: self.location,
            ingress_identity: self.ingress_identity,
            request_hash: self.request_hash,
            response_hash: self.response_hash,
            authenticated_responder: self.authenticated_responder,
            durable_receipt,
            replay_evidence,
        })
    }
}

impl PreparedDurableCertifiedFetchCompletion<'_> {
    /// Install the closed completion only after checked dequeue returned its
    /// exact owned response carrier.
    ///
    /// Every fallible comparison precedes the first map mutation. Once those
    /// comparisons succeed, the exclusive registry borrow guarantees that the
    /// previously validated incumbent still occupies `location`; removal,
    /// construction, and same-address insertion are then infallible.
    ///
    /// `Err` leaves this volatile registry untouched for diagnostic tests, but
    /// cannot roll back the already-successful external dequeue. The future
    /// composite caller must convert every such result directly into its
    /// process fail-stop path; it must never retry, restore, or continue.
    // TODO: Make that fail-stop conversion structural when the output-permit
    // transaction exposes this private post-CAS method.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    fn commit_after_exact_dequeue(
        self,
        dequeued: CertifiedFetchDequeuedResponse,
    ) -> Result<
        (),
        (
            CertifiedFetchCompletionError,
            CertifiedFetchDequeuedResponse,
        ),
    > {
        if dequeued.ingress_identity != self.ingress_identity {
            return Err((
                CertifiedFetchCompletionError::DequeuedResponseMismatch,
                dequeued,
            ));
        }
        let address = self.location.address();
        let Some(incumbent) = self.registry.entries.get(&address) else {
            return Err((CertifiedFetchCompletionError::MissingIncumbent, dequeued));
        };
        if !incumbent.validates_at(address) {
            return Err((CertifiedFetchCompletionError::CorruptIncumbent, dequeued));
        }
        let Some((incumbent_effect, _incumbent_pending)) = incumbent.pending_adapter_pair() else {
            return Err((CertifiedFetchCompletionError::WrongIncumbentShape, dequeued));
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            return Err((CertifiedFetchCompletionError::WrongIncumbentShape, dequeued));
        }
        if self.location.owner.causal_root() != incumbent.causal_root() {
            return Err((CertifiedFetchCompletionError::ForeignCausalOwner, dequeued));
        }
        if incumbent.digest != self.location.incumbent_digest {
            return Err((
                CertifiedFetchCompletionError::IncumbentDigestMismatch,
                dequeued,
            ));
        }
        if self.location.replacement_digest != self.ingress_identity.digest() {
            return Err((
                CertifiedFetchCompletionError::ReplacementDigestMismatch,
                dequeued,
            ));
        }
        if !exact_dequeued_response_matches(
            &dequeued,
            incumbent_effect,
            self.request_hash,
            self.response_hash,
            &self.authenticated_responder,
            &self.durable_receipt,
        ) {
            return Err((
                CertifiedFetchCompletionError::DequeuedResponseMismatch,
                dequeued,
            ));
        }

        let incumbent = self
            .registry
            .entries
            .remove(&address)
            .expect("exclusively borrowed validated incumbent remains installed");
        let ConcreteLifecycleWork {
            digest: incumbent_digest,
            kind,
        } = incumbent;
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
        } = kind
        else {
            panic!("validated certified-Fetch incumbent remains a pending adapter")
        };
        let completion = CertifiedFetchCompletion {
            address,
            incumbent_effect,
            incumbent_pending,
            incumbent_digest,
            request_hash: self.request_hash,
            response_hash: self.response_hash,
            authenticated_responder: self.authenticated_responder,
            durable_receipt: self.durable_receipt,
            replay_evidence: self.replay_evidence,
            dequeued,
        };
        let installed = ConcreteLifecycleWork {
            digest: self.location.replacement_digest,
            kind: ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
        };
        assert!(
            self.registry.entries.insert(address, installed).is_none(),
            "removed completion address remains vacant until same-address install"
        );
        Ok(())
    }
}

fn ingress_identity_matches_round(
    identity: PendingFairIngressIdentity,
    round: wire::ConsensusRound,
) -> bool {
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(round.context_id.0.as_ref());
    identity.context().height() == round.height
        && identity.context().id() == LifecycleDigest::new(context_id)
}

fn fetch_effect_matches_response(
    effect: &AdapterEffect,
    response: &wire::CertifiedBodyResponse,
) -> bool {
    fetch_effect_matches_manifest(effect, response.manifest.round, response.manifest.subject)
}

fn fetch_effect_matches_manifest(
    effect: &AdapterEffect,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> bool {
    matches!(
        effect,
        AdapterEffect::FetchBody {
            round: fetch_round,
            subject: fetch_subject,
            ..
        } if *fetch_round == round && *fetch_subject == subject
    )
}

fn durable_receipt_matches_fetch(
    receipt: &DurableCertifiedFetchBodyReceipt,
    effect: &AdapterEffect,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
) -> bool {
    let durable_body = receipt.durable_body();
    receipt.request_hash() == request_hash
        && receipt.response_hash() == response_hash
        && durable_body.context_id() == round.context_id
        && durable_body.round() == round
        && durable_body.subject() == subject
        && durable_body.manifest_hash() == manifest_hash
        && fetch_effect_matches_manifest(effect, round, subject)
}

fn exact_dequeued_response_matches(
    dequeued: &CertifiedFetchDequeuedResponse,
    effect: &AdapterEffect,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: &PeerId,
    durable_receipt: &DurableCertifiedFetchBodyReceipt,
) -> bool {
    if dequeued.ingress_identity.physical_admission_ordinal() == 0 {
        return false;
    }
    let Some(response) = dequeued_certified_response(dequeued) else {
        return false;
    };
    dequeued.inbound.sender() == Some(authenticated_responder)
        && ingress_identity_matches_round(dequeued.ingress_identity, response.manifest.round)
        && response.request_hash == request_hash
        && HashOf::new(response) == response_hash
        && fetch_effect_matches_response(effect, response)
        && durable_receipt_matches_fetch(
            durable_receipt,
            effect,
            request_hash,
            response_hash,
            response.manifest.round,
            response.manifest.subject,
            HashOf::new(&response.manifest),
        )
}

fn dequeued_certified_response(
    dequeued: &CertifiedFetchDequeuedResponse,
) -> Option<&wire::CertifiedBodyResponse> {
    let BlockMessage::V2(message) = dequeued.inbound.message() else {
        return None;
    };
    if message.validate_version().is_err() {
        return None;
    }
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload else {
        return None;
    };
    Some(response)
}

#[cfg(test)]
fn certified_pipeline_prepare_certificate_for_test(
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
) -> wire::QuorumCertificate {
    wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: ValidatedBodyReceipt::for_test(receipt.clone())
            .execution_commitment(),
        signers: vec![0],
        aggregate_signature: vec![0xC1],
    }
}

#[cfg(test)]
fn certified_pipeline_replay_evidence_for_test(
    tag: EventTag,
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
    validate_pending: &PendingRuntimeEffectBinding,
) -> Option<(
    CertifiedStoreReplayEvidenceV1,
    CertifiedValidateReplayEvidenceV1,
)> {
    let certificate = certified_pipeline_prepare_certificate_for_test(manifest, receipt);
    let fetch_effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate),
    };
    let response = wire::CertifiedBodyResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"certified pipeline replay fixture request",
        )),
        manifest: manifest.clone(),
        body: vec![0xC2],
        responder: 0,
        signature: vec![0xC3],
    };
    let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
        &fetch_effect,
        &response,
        receipt,
    )?;
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store = fetch.project_store_for_test(&store_effect, receipt)?;
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate =
        store.project_validate(&store_effect, receipt, &validate_effect, validate_pending)?;
    Some((store, validate))
}

fn digest_from_hash(hash: &iroha_crypto::Hash) -> LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    LifecycleDigest::new(bytes)
}

fn durable_validate_body_payload(receipt: &DurableBodyReceipt) -> Option<DurablePayloadReference> {
    let mut context = [0_u8; 32];
    context.copy_from_slice(receipt.context_id().0.as_ref());
    let active_context =
        LifecycleContext::new(LifecycleDigest::new(context), receipt.round().height);
    projection::durable_body_frame_reference(active_context, receipt)
        .map(DurablePayloadReference::BodyFrame)
}

fn validate_validated_receipt_authority(
    validate: &DurableValidateBody,
    validated_receipt: &ValidatedBodyReceipt,
) -> Result<(), DurableValidateExecutionError> {
    let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    };
    if validated_receipt.durable() != &validate.durable_receipt
        || validated_receipt.execution_commitment().validate().is_err()
    {
        return Err(DurableValidateExecutionError::InvalidValidationReceipt);
    }
    let Some(statement) = validate.pending.candidate_statement() else {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    };
    if statement.context_id() != round.context_id
        || statement.proposal_round() != *round
        || statement.subject() != Some(*subject)
    {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    }
    if statement
        .execution_commitment()
        .is_some_and(|commitment| commitment != validated_receipt.execution_commitment())
    {
        return Err(DurableValidateExecutionError::ConflictingValidationCommitment);
    }
    Ok(())
}

fn validated_body_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    validated_receipt: &ValidatedBodyReceipt,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:validated-body-completion:v1";
    let commitment = validated_receipt.execution_commitment().encode();
    let mut preimage = Vec::with_capacity(DOMAIN.len() + 1 + 32 + 32 + 32 + 8 + commitment.len());
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(incumbent_digest.as_bytes());
    preimage.extend_from_slice(expected_manifest_hash.as_ref());
    preimage.extend_from_slice(validated_receipt.durable().frame_hash().as_ref());
    preimage.extend_from_slice(
        &u64::try_from(commitment.len())
            .expect("bounded execution commitment encoding fits u64")
            .to_le_bytes(),
    );
    preimage.extend_from_slice(&commitment);
    digest_from_hash(&Hash::new(preimage))
}

fn rejected_body_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    durable_receipt: &DurableBodyReceipt,
    identity: &BodyValidationRejectionIdentity,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:rejected-body-completion:v2";
    let mut preimage = Vec::with_capacity(DOMAIN.len() + 1 + 32 + 32 + 32 + 1);
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(incumbent_digest.as_bytes());
    preimage.extend_from_slice(expected_manifest_hash.as_ref());
    preimage.extend_from_slice(durable_receipt.frame_hash().as_ref());
    preimage.push(identity.canonical_code());
    digest_from_hash(&Hash::new(preimage))
}

fn durable_validate_outcome_kind(
    outcome: &DurableBodyValidationOutcome,
) -> Option<DurableValidateOutcomeKind> {
    match (
        outcome.validated_receipt().is_some(),
        outcome.rejection_reason().is_some(),
        outcome.rejection_identity().is_some(),
        outcome.missing_merge_sidecar().is_some(),
    ) {
        (true, false, false, false) => Some(DurableValidateOutcomeKind::Validated),
        (false, true, true, false) => Some(DurableValidateOutcomeKind::Rejected),
        (false, false, false, true) => Some(DurableValidateOutcomeKind::DeferredMergeSidecar),
        _ => None,
    }
}

fn durable_validate_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    outcome: &DurableBodyValidationOutcome,
) -> Option<LifecycleDigest> {
    match durable_validate_outcome_kind(outcome)? {
        DurableValidateOutcomeKind::Validated => {
            let receipt = outcome.validated_receipt()?;
            (receipt.execution_commitment().validate().is_ok()
                && receipt.durable() == outcome.durable_body())
            .then(|| {
                validated_body_completion_digest(incumbent_digest, expected_manifest_hash, receipt)
            })
        }
        DurableValidateOutcomeKind::Rejected => {
            let identity = outcome.rejection_identity()?;
            Some(rejected_body_completion_digest(
                incumbent_digest,
                expected_manifest_hash,
                outcome.durable_body(),
                identity,
            ))
        }
        DurableValidateOutcomeKind::DeferredMergeSidecar => None,
    }
}

fn durable_validation_wait_source_for_request(
    request: &DetachedDurableValidateExecution,
) -> WaitSource {
    durable_validation_wait_source_from_exact_parts(
        request.address,
        request.incumbent_digest,
        &request.causal_lifecycle_key,
        request.candidate_statement,
        &request.durable_receipt,
        request.expected_manifest_hash,
        request.lifecycle_key,
        request.lifecycle_stage,
    )
}

fn durable_validation_wait_source_from_exact_parts(
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    causal_lifecycle_key: &Hash,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    durable_receipt: &DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
) -> WaitSource {
    let durable_frame_hash = durable_receipt.frame_hash();
    projection::durable_validation_wait_source(
        address.owner,
        address.ordinal,
        address.slot,
        incumbent_digest,
        causal_lifecycle_key,
        candidate_statement,
        &durable_frame_hash,
        expected_manifest_hash,
        lifecycle_key,
        lifecycle_stage,
    )
}

#[cfg(test)]
mod tests {
    include!("tests/v2_lifecycle_work_registry_00.rs");
    include!("tests/v2_lifecycle_work_registry_01.rs");
    include!("tests/v2_lifecycle_work_registry_02.rs");
    include!("tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs");
    include!("tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs");
    include!("tests/v2_lifecycle_work_registry_exact_registry_cases.rs");
    include!("tests/v2_lifecycle_work_registry_replay_evidence_cases.rs");
}
