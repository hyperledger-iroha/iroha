//! Scheduler-free registry for exact concrete lifecycle work.
//!
//! The logical coordinator retains only authenticated slot digests. This
//! module keeps the corresponding process-local effect values in a separate,
//! deterministic map so planning never makes the coordinator own physical
//! bytes or service handles.

use std::{collections::BTreeMap, fmt, path::Path, sync::Arc};

use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    peer::PeerId,
};
use norito::codec::Encode;

#[cfg(test)]
use super::{AdmissionRequest, schema::DurableBodyFrameReference};
use super::{
    AuthenticatedLifecycleRecoveryCut, CandidateAdmission, CapacityClass, InitialLifecycleState,
    LeaseId, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey, LifecyclePhase,
    LifecycleStage, LifecycleStageKind, LifecycleWorkClass, OwnerId, PhysicalReplacement,
    PhysicalSlot, PhysicalSlotId, PredecessorScope, ReadyEvent, TurnLease, WaitSource, WaitToken,
    authority,
    body_pipeline_transition::{
        SealedInvalidBodyReportProjection, SealedInvalidBodyReportProjectionPermit,
        SealedValidateNoSuccessorProjection, SealedValidateNoSuccessorProjectionPermit,
        SealedValidateSignProjection, SealedValidateSignProjectionPermit,
    },
    ingress_position::PendingFairIngressIdentity,
    open::{LifecycleOpenCommitError, LifecycleOpenError, PreparedLifecycleCoordinatorOpen},
    projection::{self, AdapterEffectAdmissionError, certified_fetch_lifecycle_key},
    replay_authority::{
        AuthenticatedCertifiedFetchReplayOriginV1, CertifiedFetchReplayEvidenceV1,
        CertifiedServeReplayEvidencePairV1, CertifiedServeTerminalReplayAuthorityPairV1,
        CertifiedStoreReplayEvidenceV1, CertifiedValidateReplayEvidenceV1,
        DurableCertifiedFetchReplayProjectionV1, DurableValidateReplayEvidenceV1,
        LifecycleReplayAuthorityV1, PreparedDurableCertifiedFetchStartupV1,
        RemoteProposalFetchReplayEvidenceV1, RemoteProposalStoreReplayEvidenceV1,
        RemoteProposalStoredReplayEvidenceV1, RemoteProposalValidateReplayEvidenceV1,
        SealedLiveWalPersistedEffectV1, SignedBroadcastReplayEvidenceV1,
        SignedEquivocationReplayEvidenceV1,
    },
    schema::DurablePayloadReference,
    selector::{CertifiedFetchCompletionAuthority, CertifiedFetchDequeuedResponse},
    wal_recovery::{
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection, AuthenticatedWalVoteLifecycleRepair,
        DurableAuthenticatedWalVoteLifecycleRepair, DurableRecoveredWalControlSignCarrierV1,
        DurableRecoveredWalDecisionFetchCarrierV1, RecoveredWalVoteLifecycleRepairError,
        authenticate_recovered_wal_vote_lifecycle_from_durable_body,
        authenticate_recovered_wal_vote_lifecycle_from_ledger_parent,
    },
};

/// Exact process-local carrier for one nonterminal durable Certified-Serve row.
///
/// It shares the one non-decomposable authenticated payload-store replay family
/// with its adjacent ProducerTurn and retains only coordinates copied from the
/// admitted LedgerV1 row. There is no generic adapter effect or route ownership
/// in this carrier.
struct DurableCertifiedServeWork {
    context: LifecycleContext,
    address: ConcreteWorkAddress,
    key: LifecycleKey,
    stage: LifecycleStage,
    payload: DurablePayloadReference,
    reconstruction_source: LifecycleDigest,
    replay_authority: LifecycleReplayAuthorityV1,
    replay_evidence: Arc<CertifiedServeReplayEvidencePairV1>,
}

impl fmt::Debug for DurableCertifiedServeWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableCertifiedServeWork")
            .field("address", &self.address)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl DurableCertifiedServeWork {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        self.address.slot
            == PhysicalSlotId::for_capacity(LifecycleWorkClass::CertifiedServe.capacity_class(), 0)
            && self.address.owner.causal_root().digest() == self.reconstruction_source
            && self.key.context() == self.context.id()
            && self.replay_evidence.exactly_matches_serve_carrier(
                self.context,
                self.key,
                self.stage,
                self.payload,
                installed_digest,
                &self.replay_authority,
            )
    }

    fn matches_record(
        &self,
        record: &super::LifecycleRecord,
        metadata: &super::schema::DurableRecordMetadata,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.validates(installed_digest)
            && record.work_class == LifecycleWorkClass::CertifiedServe
            && record.state == super::LifecycleState::Ready
            && record.key == self.key
            && record.owner == self.address.owner
            && record.ordinal == self.address.ordinal
            && record.stage == self.stage
            && record.physical_slots == BTreeMap::from([(self.address.slot, installed_digest)])
            && record.episode.consumed_slots
                == std::collections::BTreeSet::from([self.address.slot])
            && record.episode.slot_universe == std::collections::BTreeSet::from([self.address.slot])
            && metadata.reconstruction_source == self.reconstruction_source
            && metadata.payload == self.payload
            && metadata.replay_authority == self.replay_authority
    }

    fn matches_claimed_record(
        &self,
        record: &super::LifecycleRecord,
        metadata: &super::schema::DurableRecordMetadata,
        installed_digest: LifecycleDigest,
        lease: &TurnLease,
    ) -> bool {
        self.validates(installed_digest)
            && record.work_class == LifecycleWorkClass::CertifiedServe
            && record.state == super::LifecycleState::Claimed(lease.id)
            && record.key == self.key
            && record.owner == self.address.owner
            && record.ordinal == self.address.ordinal
            && record.stage == self.stage
            && record.physical_slots == BTreeMap::from([(self.address.slot, installed_digest)])
            && record.episode.consumed_slots
                == std::collections::BTreeSet::from([self.address.slot])
            && record.episode.slot_universe == std::collections::BTreeSet::from([self.address.slot])
            && metadata.reconstruction_source == self.reconstruction_source
            && metadata.payload == self.payload
            && metadata.replay_authority == self.replay_authority
            && lease.ordinal == record.ordinal
            && lease.owner == record.owner
            && lease.key == record.key
            && lease.work_class == record.work_class
            && lease.stage == record.stage
            && lease.physical_slots == record.physical_slots
            && lease.output_reservation.is_none()
    }
}

/// Exact process-local carrier for one nonterminal durable ProducerTurn row.
///
/// This is the adjacent owner of the same opaque replay allocation as its Serve
/// origin. It intentionally owns no volatile producer reply route.
struct DurableProducerTurnWork {
    context: LifecycleContext,
    address: ConcreteWorkAddress,
    serve_ordinal: u128,
    key: LifecycleKey,
    stage: LifecycleStage,
    reconstruction_source: LifecycleDigest,
    replay_authority: LifecycleReplayAuthorityV1,
    replay_evidence: Arc<CertifiedServeReplayEvidencePairV1>,
}

impl fmt::Debug for DurableProducerTurnWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableProducerTurnWork")
            .field("address", &self.address)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl DurableProducerTurnWork {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        self.address.slot
            == PhysicalSlotId::for_capacity(LifecycleWorkClass::ProducerTurn.capacity_class(), 0)
            && self.serve_ordinal.checked_add(1) == Some(self.address.ordinal)
            && self.address.owner.causal_root().digest() == self.reconstruction_source
            && self.key.context() == self.context.id()
            && self.replay_evidence.exactly_matches_producer_carrier(
                self.context,
                self.key,
                self.stage,
                DurablePayloadReference::None,
                installed_digest,
                &self.replay_authority,
            )
    }

    fn matches_record(
        &self,
        record: &super::LifecycleRecord,
        metadata: &super::schema::DurableRecordMetadata,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.validates(installed_digest)
            && record.work_class == LifecycleWorkClass::ProducerTurn
            && (record.state == super::LifecycleState::Ready
                || matches!(
                    record.state,
                    super::LifecycleState::Waiting(WaitToken {
                        source: WaitSource::ProducerTurn(serve),
                        observed_generation: 0,
                    }) if serve == self.serve_ordinal
                ))
            && record.key == self.key
            && record.owner == self.address.owner
            && record.ordinal == self.address.ordinal
            && record.stage == self.stage
            && record.physical_slots == BTreeMap::from([(self.address.slot, installed_digest)])
            && record.episode.consumed_slots
                == std::collections::BTreeSet::from([self.address.slot])
            && record.episode.slot_universe == std::collections::BTreeSet::from([self.address.slot])
            && metadata.reconstruction_source == self.reconstruction_source
            && metadata.payload == DurablePayloadReference::None
            && metadata.replay_authority == self.replay_authority
    }
}
use crate::sumeragi::{
    FairV2IngressDequeueDisposition, InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterEffect, PreparedInvalidBodyReportAdapterReplay,
        PreparedReadyDurableValidateAdapterPublication, PreparedReadyDurableValidatePersistedSign,
        ReadyDurableValidateAdapterPublicationKind, ReadyDurableValidateSignWalError,
        RecoveredWalVoteSign, RegisteredPrepareValidateSignCapability, SignRequest,
        SumeragiV2Adapter, VerifiedHeightContext,
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
    v2_transport::AuthenticatedCertifiedBodyRequest,
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

/// Read-only coordinates of one exact Waiting Fetch incumbent.
///
/// The canonical replacement digest is deliberately absent: it is derived
/// only after authenticated response persistence binds the replay family to
/// the exact body frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CertifiedFetchWaitingLocation {
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    incumbent_digest: LifecycleDigest,
}

impl CertifiedFetchWaitingLocation {
    /// Seal one exact logical incumbent at an already admitted address.
    pub(super) const fn new(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        incumbent_digest: LifecycleDigest,
    ) -> Option<Self> {
        if ConcreteWorkAddress::new(owner, ordinal, slot).is_none() {
            return None;
        }
        Some(Self {
            owner,
            ordinal,
            slot,
            incumbent_digest,
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

    const fn address(self) -> ConcreteWorkAddress {
        ConcreteWorkAddress {
            owner: self.owner,
            ordinal: self.ordinal,
            slot: self.slot,
        }
    }
}

/// Closed installed form of an authenticated certified-Fetch completion.
///
/// This payload directly owns the incumbent effect and pending binding moved
/// unchanged from the pending-adapter variant, plus only the restart-stable
/// body receipt and replay evidence. Queue occurrences are consumed before
/// this carrier is installed and are never retained here.
#[derive(Debug)]
pub(super) struct CertifiedFetchCompletion {
    address: ConcreteWorkAddress,
    incumbent_effect: AdapterEffect,
    incumbent_pending: PendingRuntimeEffectBinding,
    incumbent_digest: LifecycleDigest,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: CertifiedFetchReplayEvidenceV1,
}

impl CertifiedFetchCompletion {
    /// Close one storage-authenticated restart row directly into the same
    /// carrier used by the live post-fsync path.
    ///
    /// The constructor is restricted to the lifecycle authority. It accepts
    /// the complete move-only authority in one call and returns no parts on
    /// success, so restart cannot mint a second executable Fetch binding.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_recovered_durable_fetch(
        owner: OwnerId,
        ordinal: u128,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        durable_receipt: DurableBodyReceipt,
        replay_evidence: CertifiedFetchReplayEvidenceV1,
        ready_projection: &DurableCertifiedFetchReplayProjectionV1,
    ) -> Result<Self, ()> {
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Fetch.capacity_class(), 0);
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return Err(());
        };
        let incumbent_digest = digest_from_hash(pending.exact_effect_identity());
        let completion = Self {
            address,
            incumbent_effect: effect,
            incumbent_pending: pending,
            incumbent_digest,
            durable_receipt,
            replay_evidence,
        };
        completion
            .validates(ready_projection.completion_digest())
            .then_some(completion)
            .ok_or(())
    }

    /// Compare this closed carrier with its exact logical recovery candidate.
    pub(super) fn matches_recovered_candidate(&self, candidate: &CandidateAdmission) -> bool {
        self.replay_evidence
            .project_durable_ready_fetch(
                &self.incumbent_effect,
                &self.incumbent_pending,
                &self.durable_receipt,
            )
            .is_some_and(|projection| {
                projection.exactly_matches_recovered_candidate(candidate, self.address.owner)
                    && self.validates(projection.completion_digest())
            })
    }

    /// Return the sealed same-address location for registry insertion.
    pub(super) const fn address(&self) -> ConcreteWorkAddress {
        self.address
    }

    /// Return the immutable logical owner only to the sealed startup census.
    pub(super) const fn owner(&self) -> OwnerId {
        self.address.owner
    }

    /// Reconstruct the canonical Ready digest without exposing replay parts.
    pub(super) fn ready_digest(&self) -> Option<LifecycleDigest> {
        self.replay_evidence
            .project_durable_ready_fetch(
                &self.incumbent_effect,
                &self.incumbent_pending,
                &self.durable_receipt,
            )
            .map(|projection| projection.completion_digest())
    }

    pub(super) fn validates(&self, installed_digest: LifecycleDigest) -> bool {
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
            && self
                .replay_evidence
                .project_durable_ready_fetch(
                    &self.incumbent_effect,
                    &self.incumbent_pending,
                    &self.durable_receipt,
                )
                .is_some_and(|projection| projection.completion_digest() == installed_digest)
    }

    /// Corrupt only the sealed incumbent digest for a focused startup test.
    #[cfg(test)]
    pub(super) fn corrupt_for_startup_test(&mut self) {
        self.incumbent_digest = LifecycleDigest::new([0xFF; 32]);
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

/// Closed concrete carrier for one exact standalone recovered control Sign.
///
/// The projection remains whole. The registry can only compare it with a
/// durable row, installed address/digest, or current coordinator state; it
/// cannot obtain the effect, pending binding, WAL locator, replay bytes, or
/// candidate admission.
struct DurableRecoveredWalControlSignWork {
    carrier: DurableRecoveredWalControlSignCarrierV1,
    address: ConcreteWorkAddress,
}

impl fmt::Debug for DurableRecoveredWalControlSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalControlSignWork")
            .finish_non_exhaustive()
    }
}

impl DurableRecoveredWalControlSignWork {
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        self.carrier.validates_at(
            self.address.owner,
            self.address.ordinal,
            self.address.slot,
            installed_digest,
        )
    }

    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address && self.validates_digest(installed_digest)
    }

    fn validates_in_store(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> bool {
        self.validates_at(address, installed_digest) && self.carrier.validates_in_store(store)
    }

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.carrier.matches_current_ready_record(coordinator)
    }
}

/// Closed concrete carrier for one exact recovered Decision Fetch.
///
/// The complete authenticated WAL projection remains sealed in the dedicated
/// carrier. The registry exposes only equality checks needed by the atomic
/// startup census; generic Fetch execution cannot extract its effect or
/// pending authority.
struct DurableRecoveredWalDecisionFetchWork {
    carrier: DurableRecoveredWalDecisionFetchCarrierV1,
    address: ConcreteWorkAddress,
}

impl fmt::Debug for DurableRecoveredWalDecisionFetchWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalDecisionFetchWork")
            .finish_non_exhaustive()
    }
}

impl DurableRecoveredWalDecisionFetchWork {
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        self.carrier.validates_at(
            self.address.owner,
            self.address.ordinal,
            self.address.slot,
            installed_digest,
        )
    }

    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address && self.validates_digest(installed_digest)
    }

    fn validates_in_store(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> bool {
        self.validates_at(address, installed_digest) && self.carrier.validates_in_store(store)
    }

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.carrier.matches_current_ready_record(coordinator)
    }
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

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let candidate = self.repair.repair().child();
        let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
            return false;
        };
        let Some(record) = coordinator.records.get(&address.ordinal) else {
            return false;
        };
        let Some(metadata) = coordinator.durable_records.get(&address.ordinal) else {
            return false;
        };
        self.validates_at(address, installed_digest)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == candidate.key.context()
            && coordinator.active_context.height() == candidate.key.round().height()
            && coordinator.high_water >= address.ordinal
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.producer_turn.is_none()
            && record.key == candidate.key
            && record.owner == address.owner
            && record.owner.causal_root() == candidate.causal_root
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::SignVote
            && record.stage == candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&address.slot) == Some(&installed_digest)
            && metadata.matches_admission(candidate)
            && coordinator.key_index.get(&candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&candidate.causal_root) == Some(&record.owner)
            && coordinator.ready_index.contains(&address.ordinal)
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
    DurableRecoveredWalControlSign(DurableRecoveredWalControlSignWork),
    DurableRecoveredWalDecisionFetch(DurableRecoveredWalDecisionFetchWork),
    DurableCertifiedServe(DurableCertifiedServeWork),
    DurableProducerTurn(DurableProducerTurnWork),
}

/// One move-only concrete effect paired with its sealed pending authority.
#[derive(Debug)]
#[must_use = "dropping concrete lifecycle work abandons its exact physical owner"]
pub(super) struct ConcreteLifecycleWork {
    digest: LifecycleDigest,
    kind: ConcreteLifecycleWorkKind,
}

impl ConcreteLifecycleWork {
    /// Seal one recovered durable Fetch completion as registry work.
    fn from_recovered_durable_fetch(
        completion: CertifiedFetchCompletion,
    ) -> Result<Self, CertifiedFetchCompletion> {
        let Some(digest) = completion.ready_digest() else {
            return Err(completion);
        };
        let work = Self {
            digest,
            kind: ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
        };
        if work.validate_exact() {
            Ok(work)
        } else {
            let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = work.kind else {
                unreachable!("new recovered Fetch work retains its closed completion kind")
            };
            Err(completion)
        }
    }

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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.validates_digest(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.validates_digest(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => serve.validates(self.digest),
            ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => {
                producer.validates(self.digest)
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
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                    sign.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                    fetch.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => serve.address == address,
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => {
                    producer.address == address
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => {
                serve.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => {
                producer.address.owner.causal_root()
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => {
                panic!(
                    "closed recovered control, Serve, and ProducerTurn work require typed executors"
                )
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_) => None,
            ConcreteLifecycleWorkKind::DurableCertifiedServe(_) => None,
            ConcreteLifecycleWorkKind::DurableProducerTurn(_) => None,
        }
    }
}

/// One completely preflighted Certified-Serve/ProducerTurn carrier batch.
///
/// Construction first checks every supplied replay pair and the complete live
/// Serve/Producer census. Only then do both exact adjacent carriers retain the
/// same whole common replay family. Registry installation performs a second
/// complete vacancy/validity preflight before changing any entry.
#[must_use = "the Certified-Serve concrete carrier batch has not been installed"]
pub(super) struct PreparedCertifiedServeRegistryBatchV1 {
    entries: Vec<(ConcreteWorkAddress, ConcreteLifecycleWork)>,
}

impl fmt::Debug for PreparedCertifiedServeRegistryBatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCertifiedServeRegistryBatchV1")
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl PreparedCertifiedServeRegistryBatchV1 {
    /// Close all authenticated recovery pairs over the complete coordinator
    /// census. Failure returns every still-common replay pair unchanged.
    pub(super) fn from_recovered_pairs(
        coordinator: &LifecycleCoordinator,
        pairs: BTreeMap<LifecycleKey, CertifiedServeReplayEvidencePairV1>,
    ) -> Result<Self, BTreeMap<LifecycleKey, CertifiedServeReplayEvidencePairV1>> {
        let context = coordinator.active_context;
        let Some(expected_live) = recovered_serve_pairs_preflight(coordinator, &pairs) else {
            return Err(pairs);
        };

        let mut entries = Vec::with_capacity(expected_live);
        for (serve_key, pair) in pairs {
            let serve_ordinal = coordinator.key_index[&serve_key];
            let producer_ordinal = serve_ordinal
                .checked_add(1)
                .expect("complete Serve-pair preflight fixed the adjacent ordinal");
            let serve = &coordinator.records[&serve_ordinal];
            let serve_metadata = &coordinator.durable_records[&serve_ordinal];
            let producer = &coordinator.records[&producer_ordinal];
            let producer_metadata = &coordinator.durable_records[&producer_ordinal];
            let (producer_slot, producer_digest) = exact_single_record_slot(
                producer,
                LifecycleWorkClass::ProducerTurn.capacity_class(),
            )
            .expect("complete ProducerTurn preflight fixes one slot");
            let replay_evidence = Arc::new(pair);
            if !matches!(serve.state, super::LifecycleState::Terminal(_)) {
                let (serve_slot, serve_digest) = exact_single_record_slot(
                    serve,
                    LifecycleWorkClass::CertifiedServe.capacity_class(),
                )
                .expect("complete live Serve preflight fixes one slot");
                let address = ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)
                    .expect("complete Serve address preflight");
                entries.push((
                    address,
                    ConcreteLifecycleWork {
                        digest: serve_digest,
                        kind: ConcreteLifecycleWorkKind::DurableCertifiedServe(
                            DurableCertifiedServeWork {
                                context,
                                address,
                                key: serve.key,
                                stage: serve.stage,
                                payload: serve_metadata.payload,
                                reconstruction_source: serve_metadata.reconstruction_source,
                                replay_authority: serve_metadata.replay_authority.clone(),
                                replay_evidence: Arc::clone(&replay_evidence),
                            },
                        ),
                    },
                ));
            }
            if !matches!(producer.state, super::LifecycleState::Terminal(_)) {
                let address =
                    ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot)
                        .expect("complete ProducerTurn address preflight");
                entries.push((
                    address,
                    ConcreteLifecycleWork {
                        digest: producer_digest,
                        kind: ConcreteLifecycleWorkKind::DurableProducerTurn(
                            DurableProducerTurnWork {
                                context,
                                address,
                                serve_ordinal,
                                key: producer.key,
                                stage: producer.stage,
                                reconstruction_source: producer_metadata.reconstruction_source,
                                replay_authority: producer_metadata.replay_authority.clone(),
                                replay_evidence,
                            },
                        ),
                    },
                ));
            }
        }
        let batch = Self { entries };
        debug_assert!(
            batch
                .entries
                .iter()
                .all(|(address, work)| work.validates_at(*address))
        );
        Ok(batch)
    }

    /// Close the one adjacent pair allocated by a fresh staged coordinator.
    /// Failure returns the still-whole replay family without exposing either
    /// ordinal, slot, or physical digest to the caller.
    pub(super) fn from_fresh_admitted_pair(
        coordinator: &LifecycleCoordinator,
        serve_key: LifecycleKey,
        pair: CertifiedServeReplayEvidencePairV1,
    ) -> Result<Self, CertifiedServeReplayEvidencePairV1> {
        let Some(&serve_ordinal) = coordinator.key_index.get(&serve_key) else {
            return Err(pair);
        };
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&serve_ordinal) else {
            return Err(pair);
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            coordinator.records.get(&serve_ordinal),
            coordinator.durable_records.get(&serve_ordinal),
            coordinator.records.get(&producer_ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return Err(pair);
        };
        let Some((serve_slot, serve_digest)) =
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class())
        else {
            return Err(pair);
        };
        let Some((producer_slot, producer_digest)) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())
        else {
            return Err(pair);
        };
        if coordinator.fault.is_some()
            || serve_ordinal.checked_add(1) != Some(producer_ordinal)
            || serve.key != serve_key
            || serve.work_class != LifecycleWorkClass::CertifiedServe
            || serve.state != super::LifecycleState::Ready
            || producer.work_class != LifecycleWorkClass::ProducerTurn
            || !matches!(
                producer.state,
                super::LifecycleState::Waiting(WaitToken {
                    source: WaitSource::ProducerTurn(origin),
                    observed_generation: 0,
                }) if origin == serve_ordinal
            )
            || !super::schema::serve_and_producer_keys_match(serve.key, producer.key)
            || serve.owner != producer.owner
            || serve_metadata.reconstruction_source != producer_metadata.reconstruction_source
            || serve_metadata.reconstruction_source != serve.owner.causal_root().digest()
            || producer_metadata.payload != DurablePayloadReference::None
            || !pair.exactly_matches_serve_carrier(
                coordinator.active_context,
                serve.key,
                serve.stage,
                serve_metadata.payload,
                serve_digest,
                &serve_metadata.replay_authority,
            )
            || !pair.exactly_matches_producer_carrier(
                coordinator.active_context,
                producer.key,
                producer.stage,
                producer_metadata.payload,
                producer_digest,
                &producer_metadata.replay_authority,
            )
        {
            return Err(pair);
        }
        let Some(serve_address) = ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)
        else {
            return Err(pair);
        };
        let Some(producer_address) =
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot)
        else {
            return Err(pair);
        };
        let replay_evidence = Arc::new(pair);
        let batch = Self {
            entries: vec![
                (
                    serve_address,
                    ConcreteLifecycleWork {
                        digest: serve_digest,
                        kind: ConcreteLifecycleWorkKind::DurableCertifiedServe(
                            DurableCertifiedServeWork {
                                context: coordinator.active_context,
                                address: serve_address,
                                key: serve.key,
                                stage: serve.stage,
                                payload: serve_metadata.payload,
                                reconstruction_source: serve_metadata.reconstruction_source,
                                replay_authority: serve_metadata.replay_authority.clone(),
                                replay_evidence: Arc::clone(&replay_evidence),
                            },
                        ),
                    },
                ),
                (
                    producer_address,
                    ConcreteLifecycleWork {
                        digest: producer_digest,
                        kind: ConcreteLifecycleWorkKind::DurableProducerTurn(
                            DurableProducerTurnWork {
                                context: coordinator.active_context,
                                address: producer_address,
                                serve_ordinal,
                                key: producer.key,
                                stage: producer.stage,
                                reconstruction_source: producer_metadata.reconstruction_source,
                                replay_authority: producer_metadata.replay_authority.clone(),
                                replay_evidence,
                            },
                        ),
                    },
                ),
            ],
        };
        debug_assert!(
            batch
                .entries
                .iter()
                .all(|(address, work)| work.validates_at(*address))
        );
        Ok(batch)
    }

    fn preflights_registry(&self, registry: &ConcreteLifecycleWorkRegistry) -> bool {
        let mut addresses = std::collections::BTreeSet::new();
        registry
            .entries
            .iter()
            .all(|(&address, work)| work.validates_at(address))
            && self.entries.iter().all(|(address, work)| {
                addresses.insert(*address)
                    && !registry.entries.contains_key(address)
                    && work.validates_at(*address)
                    && address.owner.causal_root() == work.causal_root()
            })
    }

    /// Prove the complete post-install startup census before LedgerV1 fsync.
    ///
    /// Sealed startup callers arrive with exactly the recovered Fetch census,
    /// optionally beside one exclusive recovered-WAL work carrier. Any other valid but
    /// unrelated registry entry therefore rejects before the batch mutates the
    /// registry or invokes durable publication.
    pub(super) fn preflights_startup_registry(
        &self,
        registry: &ConcreteLifecycleWorkRegistry,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        if !self.preflights_registry(registry) {
            return false;
        }
        let existing_is_exact = registry
            .exact_recovered_wal_registry_slot()
            .is_some_and(|slot| {
                registry.exactly_covers_recovered_ready_fetches_with_extra(coordinator, slot)
            });
        let live_serve_or_producer = coordinator
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.work_class,
                    LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn
                ) && !matches!(record.state, super::LifecycleState::Terminal(_))
            })
            .count();
        existing_is_exact && self.entries.len() == live_serve_or_producer
    }

    /// Prove the complete prospective Serve/Producer census for one fresh
    /// admission before LedgerV1 publication.
    pub(super) fn preflights_fresh_registry(
        &self,
        registry: &ConcreteLifecycleWorkRegistry,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
    ) -> bool {
        self.entries.len() == 2
            && self.preflights_registry(registry)
            && (registry.exactly_covers_recovered_ready_work(current)
                || registry.exactly_covers_recovered_ready_work_and_wal_authority(current))
            && current.active_context == staged.active_context
            && current.high_water.checked_add(2) == Some(staged.high_water)
            && self.exactly_matches_fresh_staged_append(current, staged)
            && registry
                .serve_and_producer_carrier_count()
                .checked_add(self.entries.len())
                == Some(
                    staged
                        .records
                        .values()
                        .filter(|record| {
                            matches!(
                                record.work_class,
                                LifecycleWorkClass::CertifiedServe
                                    | LifecycleWorkClass::ProducerTurn
                            ) && !matches!(record.state, super::LifecycleState::Terminal(_))
                        })
                        .count(),
                )
    }

    fn exactly_matches_fresh_staged_append(
        &self,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
    ) -> bool {
        let mut serve = None;
        let mut producer = None;
        for (address, work) in &self.entries {
            let Some(record) = staged.records.get(&address.ordinal) else {
                return false;
            };
            let Some(metadata) = staged.durable_records.get(&address.ordinal) else {
                return false;
            };
            let exact = match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(carrier) => {
                    serve.replace(address.ordinal).is_none()
                        && carrier.matches_record(record, metadata, work.digest)
                }
                ConcreteLifecycleWorkKind::DurableProducerTurn(carrier) => {
                    producer.replace(address.ordinal).is_none()
                        && carrier.matches_record(record, metadata, work.digest)
                }
                _ => false,
            };
            if !exact {
                return false;
            }
        }
        let (Some(serve), Some(producer)) = (serve, producer) else {
            return false;
        };
        if serve.checked_add(1) != Some(producer)
            || producer != staged.high_water
            || current.records.len().checked_add(2) != Some(staged.records.len())
            || current.durable_records.len().checked_add(2) != Some(staged.durable_records.len())
            || current.key_index.len().checked_add(2) != Some(staged.key_index.len())
            || current.owner_index.len().checked_add(1) != Some(staged.owner_index.len())
            || current.ready_index.len().checked_add(1) != Some(staged.ready_index.len())
            || current.producer_debts.len().checked_add(1) != Some(staged.producer_debts.len())
            || current.admission_waits != staged.admission_waits
            || current.active_lease != staged.active_lease
            || current.next_lease != staged.next_lease
            || current.capacity_geometry != staged.capacity_geometry
            || current.capacity_generation != staged.capacity_generation
            || current.observed_generation != staged.observed_generation
            || current.fault != staged.fault
            || !current
                .records
                .iter()
                .all(|(ordinal, record)| staged.records.get(ordinal) == Some(record))
            || !current
                .durable_records
                .iter()
                .all(|(ordinal, metadata)| staged.durable_records.get(ordinal) == Some(metadata))
            || !current
                .key_index
                .iter()
                .all(|(key, ordinal)| staged.key_index.get(key) == Some(ordinal))
            || !current
                .owner_index
                .iter()
                .all(|(root, owner)| staged.owner_index.get(root) == Some(owner))
            || !current.ready_index.is_subset(&staged.ready_index)
            || !current
                .producer_debts
                .iter()
                .all(|(serve, producer)| staged.producer_debts.get(serve) == Some(producer))
            || staged.producer_debts.get(&serve) != Some(&producer)
        {
            return false;
        }
        let mut expected_capacity = current.capacity_used.clone();
        let Some(serve_used) = expected_capacity.get_mut(&CapacityClass::Serve) else {
            return false;
        };
        let Some(next_serve) = serve_used.checked_add(1) else {
            return false;
        };
        *serve_used = next_serve;
        let Some(producer_used) = expected_capacity.get_mut(&CapacityClass::Producer) else {
            return false;
        };
        let Some(next_producer) = producer_used.checked_add(1) else {
            return false;
        };
        *producer_used = next_producer;
        staged.capacity_used == expected_capacity
    }
}

fn recovered_serve_pairs_preflight(
    coordinator: &LifecycleCoordinator,
    pairs: &BTreeMap<LifecycleKey, CertifiedServeReplayEvidencePairV1>,
) -> Option<usize> {
    let context = coordinator.active_context;
    let mut expected_live = 0usize;
    for record in coordinator.records.values() {
        if matches!(
            record.work_class,
            LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn
        ) && !matches!(record.state, super::LifecycleState::Terminal(_))
        {
            expected_live = expected_live.checked_add(1)?;
            if record.work_class == LifecycleWorkClass::CertifiedServe
                && record.state != super::LifecycleState::Ready
            {
                return None;
            }
        }
    }

    let mut projected_live = 0usize;
    let mut seen_ordinals = std::collections::BTreeSet::new();
    for (serve_key, pair) in pairs {
        let &serve_ordinal = coordinator.key_index.get(serve_key)?;
        let producer_ordinal = serve_ordinal.checked_add(1)?;
        let serve = coordinator.records.get(&serve_ordinal)?;
        let serve_metadata = coordinator.durable_records.get(&serve_ordinal)?;
        let producer = coordinator.records.get(&producer_ordinal)?;
        let producer_metadata = coordinator.durable_records.get(&producer_ordinal)?;
        let serve_live = !matches!(serve.state, super::LifecycleState::Terminal(_));
        let producer_live = !matches!(producer.state, super::LifecycleState::Terminal(_));
        if !serve_live && !producer_live {
            return None;
        }
        let (producer_slot, producer_digest) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())?;
        let serve_is_exact = if serve_live {
            let (_, serve_digest) = exact_single_record_slot(
                serve,
                LifecycleWorkClass::CertifiedServe.capacity_class(),
            )?;
            pair.exactly_matches_serve_carrier(
                context,
                serve.key,
                serve.stage,
                serve_metadata.payload,
                serve_digest,
                &serve_metadata.replay_authority,
            )
        } else {
            pair.exactly_matches_terminal_serve_record(context, serve, serve_metadata)
        };
        let producer_debt_is_exact = if producer_live {
            coordinator.producer_debts.get(&serve_ordinal) == Some(&producer_ordinal)
        } else {
            !coordinator.producer_debts.contains_key(&serve_ordinal)
        };
        if serve.key != *serve_key
            || serve.work_class != LifecycleWorkClass::CertifiedServe
            || producer.work_class != LifecycleWorkClass::ProducerTurn
            || !super::schema::serve_and_producer_keys_match(serve.key, producer.key)
            || serve.owner != producer.owner
            || serve_metadata.reconstruction_source != producer_metadata.reconstruction_source
            || serve_metadata.reconstruction_source != serve.owner.causal_root().digest()
            || producer_metadata.payload != DurablePayloadReference::None
            || (serve_live && !producer_live)
            || !producer_debt_is_exact
            || (serve_live && serve.state != super::LifecycleState::Ready)
            || (producer_live
                && producer.state != super::LifecycleState::Ready
                && !matches!(
                    producer.state,
                    super::LifecycleState::Waiting(WaitToken {
                        source: WaitSource::ProducerTurn(serve),
                        observed_generation: 0,
                    }) if serve == serve_ordinal
                ))
            || !serve_is_exact
            || !pair.exactly_matches_producer_carrier(
                context,
                producer.key,
                producer.stage,
                producer_metadata.payload,
                producer_digest,
                &producer_metadata.replay_authority,
            )
            || !seen_ordinals.insert(serve_ordinal)
            || !seen_ordinals.insert(producer_ordinal)
            || (serve_live
                && exact_single_record_slot(
                    serve,
                    LifecycleWorkClass::CertifiedServe.capacity_class(),
                )
                .and_then(|(serve_slot, _)| {
                    ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)
                })
                .is_none())
            || ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot).is_none()
        {
            return None;
        }
        projected_live = projected_live
            .checked_add(usize::from(serve_live))?
            .checked_add(usize::from(producer_live))?;
    }
    (projected_live == expected_live).then_some(expected_live)
}

fn exact_single_record_slot(
    record: &super::LifecycleRecord,
    capacity: CapacityClass,
) -> Option<(PhysicalSlotId, LifecycleDigest)> {
    let (&slot, &digest) = record.physical_slots.first_key_value()?;
    (record.physical_slots.len() == 1
        && slot == PhysicalSlotId::for_capacity(capacity, 0)
        && record.episode.consumed_slots == std::collections::BTreeSet::from([slot])
        && record.episode.slot_universe == std::collections::BTreeSet::from([slot]))
    .then_some((slot, digest))
}

fn serve_ordinal_pair_is_exact(
    serve: &super::LifecycleRecord,
    producer: &super::LifecycleRecord,
) -> bool {
    serve.ordinal.checked_add(1) == Some(producer.ordinal)
        && serve.work_class == LifecycleWorkClass::CertifiedServe
        && producer.work_class == LifecycleWorkClass::ProducerTurn
        && serve.owner == producer.owner
        && super::schema::serve_and_producer_keys_match(serve.key, producer.key)
}

/// Typed error from installing a complete Serve/Producer batch around one
/// durable publication attempt.
pub(super) enum CertifiedServeRegistryBatchPublicationError<E> {
    /// Complete registry validation rejected before any mutation.
    Preflight(PreparedCertifiedServeRegistryBatchV1),
    /// Durable publication failed; every installed carrier was removed and
    /// returned in the reconstructed batch.
    Publication(E, PreparedCertifiedServeRegistryBatchV1),
}

/// Closed carrier mutation prepared from one exact active Serve lease and its
/// post-fsync terminal replay family.
#[must_use = "the Certified-Serve terminal carrier transition has not been published"]
pub(super) struct PreparedCertifiedServeTerminalRegistryTransitionV1 {
    serve_address: ConcreteWorkAddress,
    producer_address: ConcreteWorkAddress,
    outcome: super::TerminalOutcome,
    pending_replay_evidence: Arc<CertifiedServeReplayEvidencePairV1>,
    terminal_replay_evidence: Arc<CertifiedServeReplayEvidencePairV1>,
}

impl fmt::Debug for PreparedCertifiedServeTerminalRegistryTransitionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCertifiedServeTerminalRegistryTransitionV1")
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

impl PreparedCertifiedServeTerminalRegistryTransitionV1 {
    fn preflights_current(
        &self,
        registry: &ConcreteLifecycleWorkRegistry,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if !registry.exactly_covers_active_certified_serve_lease(coordinator, lease) {
            return false;
        }
        let (Some(serve), Some(producer)) = (
            registry.entries.get(&self.serve_address),
            registry.entries.get(&self.producer_address),
        ) else {
            return false;
        };
        matches!(
            (&serve.kind, &producer.kind),
            (
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve),
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer),
            ) if Arc::ptr_eq(&serve.replay_evidence, &self.pending_replay_evidence)
                && Arc::ptr_eq(&producer.replay_evidence, &self.pending_replay_evidence)
        )
    }

    /// Prove that `staged` is the sole complete successor produced by terminal
    /// settlement of this exact active Serve lease.
    ///
    /// The terminal replay pair has already authenticated the only fields that
    /// may change inside the Serve/Producer rows. Every other coordinator field
    /// is compared exhaustively here before registry mutation or Ledger fsync.
    fn preflights_exact_staged_successor(
        &self,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        let serve_ordinal = self.serve_address.ordinal;
        let producer_ordinal = self.producer_address.ordinal;
        let (Some(current_serve), Some(current_producer)) = (
            current.records.get(&serve_ordinal),
            current.records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some(staged_serve), Some(staged_producer)) = (
            staged.records.get(&serve_ordinal),
            staged.records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some(current_serve_metadata), Some(current_producer_metadata)) = (
            current.durable_records.get(&serve_ordinal),
            current.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some(staged_serve_metadata), Some(staged_producer_metadata)) = (
            staged.durable_records.get(&serve_ordinal),
            staged.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };

        let mut expected_serve = current_serve.clone();
        expected_serve.state = super::LifecycleState::Terminal(self.outcome);
        let mut expected_producer = current_producer.clone();
        expected_producer.state = if self.outcome == super::TerminalOutcome::Cancelled {
            super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled)
        } else {
            super::LifecycleState::Ready
        };
        if staged_serve != &expected_serve || staged_producer != &expected_producer {
            return false;
        }

        let Some((_, producer_digest)) = exact_single_record_slot(
            staged_producer,
            LifecycleWorkClass::ProducerTurn.capacity_class(),
        ) else {
            return false;
        };
        if !self
            .terminal_replay_evidence
            .exactly_matches_terminal_serve_record(
                staged.active_context,
                staged_serve,
                staged_serve_metadata,
            )
            || !self
                .terminal_replay_evidence
                .exactly_matches_producer_carrier(
                    staged.active_context,
                    staged_producer.key,
                    staged_producer.stage,
                    staged_producer_metadata.payload,
                    producer_digest,
                    &staged_producer_metadata.replay_authority,
                )
            || staged_serve_metadata.reconstruction_source
                != current_serve_metadata.reconstruction_source
            || staged_serve_metadata.continuation != current_serve_metadata.continuation
            || staged_producer_metadata.reconstruction_source
                != current_producer_metadata.reconstruction_source
            || staged_producer_metadata.payload != current_producer_metadata.payload
            || staged_producer_metadata.continuation != current_producer_metadata.continuation
        {
            return false;
        }

        if current.records.len() != staged.records.len()
            || current.durable_records.len() != staged.durable_records.len()
            || current.records.iter().any(|(ordinal, record)| {
                *ordinal != serve_ordinal
                    && *ordinal != producer_ordinal
                    && staged.records.get(ordinal) != Some(record)
            })
            || current.durable_records.iter().any(|(ordinal, metadata)| {
                *ordinal != serve_ordinal
                    && *ordinal != producer_ordinal
                    && staged.durable_records.get(ordinal) != Some(metadata)
            })
        {
            return false;
        }

        let mut expected_ready = current.ready_index.clone();
        expected_ready.remove(&serve_ordinal);
        if self.outcome == super::TerminalOutcome::Cancelled {
            expected_ready.remove(&producer_ordinal);
        } else {
            expected_ready.insert(producer_ordinal);
        }
        let mut expected_debts = current.producer_debts.clone();
        if self.outcome == super::TerminalOutcome::Cancelled {
            expected_debts.remove(&serve_ordinal);
        }
        let mut expected_capacity_used = current.capacity_used.clone();
        let mut expected_capacity_generation = current.capacity_generation.clone();
        let released_classes = [
            Some(current_serve.work_class.capacity_class()),
            (self.outcome == super::TerminalOutcome::Cancelled)
                .then_some(current_producer.work_class.capacity_class()),
        ];
        for class in released_classes.into_iter().flatten() {
            let Some(used) = expected_capacity_used.get_mut(&class) else {
                return false;
            };
            let Some(next_used) = used.checked_sub(1) else {
                return false;
            };
            *used = next_used;
            let Some(generation) = expected_capacity_generation.get_mut(&class) else {
                return false;
            };
            let Some(next_generation) = generation.checked_add(1) else {
                return false;
            };
            *generation = next_generation;
        }
        let same_ledger_target = matches!(
            (&current.ledger_store, &staged.ledger_store),
            (Some(current_store), Some(staged_store))
                if current_store.same_publication_target(staged_store)
        );

        current.active_lease.as_ref() == Some(lease)
            && staged.active_lease.is_none()
            && staged.episode_authority == current.episode_authority
            && staged.active_context == current.active_context
            && staged.key_index == current.key_index
            && staged.owner_index == current.owner_index
            && staged.ready_index == expected_ready
            && staged.admission_waits == current.admission_waits
            && staged.high_water == current.high_water
            && staged.next_lease == current.next_lease
            && staged.capacity_geometry == current.capacity_geometry
            && staged.capacity_used == expected_capacity_used
            && staged.capacity_generation == expected_capacity_generation
            && staged.observed_generation == current.observed_generation
            && staged.producer_debts == expected_debts
            && same_ledger_target
            && current.fault.is_none()
            && staged.fault.is_none()
    }

    fn producer_replacement(&self, staged: &LifecycleCoordinator) -> Option<ConcreteLifecycleWork> {
        let serve = staged.records.get(&self.serve_address.ordinal)?;
        let serve_metadata = staged.durable_records.get(&self.serve_address.ordinal)?;
        let producer = staged.records.get(&self.producer_address.ordinal)?;
        let producer_metadata = staged.durable_records.get(&self.producer_address.ordinal)?;
        let (_, producer_digest) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())?;
        if serve.state != super::LifecycleState::Terminal(self.outcome)
            || producer.state != super::LifecycleState::Ready
            || !self
                .terminal_replay_evidence
                .exactly_matches_terminal_serve_record(staged.active_context, serve, serve_metadata)
            || !self
                .terminal_replay_evidence
                .exactly_matches_producer_carrier(
                    staged.active_context,
                    producer.key,
                    producer.stage,
                    producer_metadata.payload,
                    producer_digest,
                    &producer_metadata.replay_authority,
                )
        {
            return None;
        }
        let work = ConcreteLifecycleWork {
            digest: producer_digest,
            kind: ConcreteLifecycleWorkKind::DurableProducerTurn(DurableProducerTurnWork {
                context: staged.active_context,
                address: self.producer_address,
                serve_ordinal: self.serve_address.ordinal,
                key: producer.key,
                stage: producer.stage,
                reconstruction_source: producer_metadata.reconstruction_source,
                replay_authority: producer_metadata.replay_authority.clone(),
                replay_evidence: Arc::clone(&self.terminal_replay_evidence),
            }),
        };
        work.validates_at(self.producer_address).then_some(work)
    }

    fn preflights_cancelled_successor(&self, staged: &LifecycleCoordinator) -> bool {
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            staged.records.get(&self.serve_address.ordinal),
            staged.durable_records.get(&self.serve_address.ordinal),
            staged.records.get(&self.producer_address.ordinal),
            staged.durable_records.get(&self.producer_address.ordinal),
        ) else {
            return false;
        };
        let Some((_, producer_digest)) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())
        else {
            return false;
        };
        self.outcome == super::TerminalOutcome::Cancelled
            && serve.state == super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled)
            && producer.state == super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled)
            && self
                .terminal_replay_evidence
                .exactly_matches_terminal_serve_record(staged.active_context, serve, serve_metadata)
            && self
                .terminal_replay_evidence
                .exactly_matches_producer_carrier(
                    staged.active_context,
                    producer.key,
                    producer.stage,
                    producer_metadata.payload,
                    producer_digest,
                    &producer_metadata.replay_authority,
                )
    }
}

/// Failure from the carrier-before-LedgerV1 terminal publication boundary.
pub(super) enum CertifiedServeTerminalRegistryPublicationError<E> {
    /// Current or staged whole-census validation failed before mutation.
    Preflight(PreparedCertifiedServeTerminalRegistryTransitionV1),
    /// LedgerV1 publication failed and the exact incumbent Producer carrier
    /// was restored before returning.
    Publication(E, PreparedCertifiedServeTerminalRegistryTransitionV1),
}

struct StagedCertifiedServeRegistryBatch<'registry> {
    entries: &'registry mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    addresses: Vec<ConcreteWorkAddress>,
}

impl StagedCertifiedServeRegistryBatch<'_> {
    fn commit(mut self) {
        self.addresses.clear();
    }

    fn rollback(mut self) -> PreparedCertifiedServeRegistryBatchV1 {
        let entries = self
            .addresses
            .drain(..)
            .map(|address| {
                let work = self
                    .entries
                    .remove(&address)
                    .expect("staged Serve/Producer carrier remains at its exact address");
                (address, work)
            })
            .collect();
        PreparedCertifiedServeRegistryBatchV1 { entries }
    }
}

impl Drop for StagedCertifiedServeRegistryBatch<'_> {
    fn drop(&mut self) {
        for address in self.addresses.drain(..) {
            self.entries.remove(&address);
        }
    }
}

struct StagedCertifiedServeTerminalProducer<'registry> {
    entries: &'registry mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    producer_address: ConcreteWorkAddress,
    incumbent: Option<ConcreteLifecycleWork>,
}

impl StagedCertifiedServeTerminalProducer<'_> {
    fn commit(mut self) {
        drop(
            self.incumbent
                .take()
                .expect("terminal Producer replacement retains its incumbent"),
        );
    }

    fn rollback(mut self) {
        let incumbent = self
            .incumbent
            .take()
            .expect("terminal Producer replacement retains its incumbent");
        let replacement = std::mem::replace(
            self.entries
                .get_mut(&self.producer_address)
                .expect("terminal Producer replacement remains at its exact address"),
            incumbent,
        );
        drop(replacement);
    }
}

impl Drop for StagedCertifiedServeTerminalProducer<'_> {
    fn drop(&mut self) {
        let Some(incumbent) = self.incumbent.take() else {
            return;
        };
        let replacement = std::mem::replace(
            self.entries
                .get_mut(&self.producer_address)
                .expect("unwinding terminal Producer remains at its exact address"),
            incumbent,
        );
        drop(replacement);
    }
}

/// Opaque ordinary Sign row prepared from the post-WAL continuation seal.
///
/// The concrete carrier never crosses this wrapper's API. Its fixed oracle
/// binds the sealed pending causal root and exact child address before fsync;
/// the only consuming operation is installation through the retained live
/// parent/child reservation.
#[must_use = "prepared live Sign work has not entered its reserved registry row"]
pub(in crate::sumeragi) struct PreparedLiveValidateSignRegistryWork {
    work: ConcreteLifecycleWork,
}

impl PreparedLiveValidateSignRegistryWork {
    /// Close exact effect/pending authority without exposing concrete work.
    pub(super) fn from_exact(
        _permit: LiveValidateSignWorkProjectionPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        ConcreteLifecycleWork::from_exact(effect, pending).map(|work| Self { work })
    }

    /// Revalidate the still-closed effect/pending binding.
    pub(in crate::sumeragi) fn validates_exact(&self) -> bool {
        self.work.validate_exact()
    }

    /// Match the exact staged Sign row, including its inherited causal owner.
    pub(in crate::sumeragi) fn validates_publication(
        &self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return false;
        };
        self.work.validate_exact()
            && self.work.digest() == digest
            && self.work.causal_root() == owner.causal_root()
            && matches!(
                &self.work.kind,
                ConcreteLifecycleWorkKind::PendingAdapter {
                    effect:
                        AdapterEffect::Sign {
                            request: SignRequest::Vote(vote),
                            ..
                        },
                    ..
                } if matches!(
                    vote.phase,
                    wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit
                )
            )
            && address.owner == owner
            && address.ordinal == ordinal
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
    }

    /// Consume this closed row into its prevalidated exclusive reservation.
    pub(in crate::sumeragi) fn install_into(
        self,
        reservation: LiveValidateSignRegistryReservation<'_>,
    ) {
        reservation.install_live_sign(self.work);
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
    pub(super) const fn matches(
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

    /// Return whether the exact carrier must enter the bounded I/O worker.
    pub(super) const fn requires_io_dispatch(self) -> bool {
        matches!(self.kind, ReadyValidateCarrierKind::ExecuteBody)
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

/// Exact post-fsync LedgerV1 frame beside its uninstalled recovered Sign.
#[must_use = "the fsynced recovered WAL repair must install its Sign child"]
pub(crate) struct PersistedRecoveredWalValidateLedger<'registry> {
    store: super::ledger::LifecycleLedgerStoreV1,
    repaired: super::ledger::LifecycleLedgerV1,
    repair: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
}

/// Exact repaired storage and installed Sign retained under one registry borrow.
#[must_use = "the installed recovered WAL storage must complete lifecycle open"]
pub(crate) struct InstalledRecoveredWalSignStorage<'registry> {
    store: super::ledger::LifecycleLedgerStoreV1,
    repaired: super::ledger::LifecycleLedgerV1,
    installed: InstalledRecoveredWalSignRegistryCut<'registry>,
}

/// Typed fail-stop classification for the final exact-store recovery join.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed recovered WAL storage completion requires restart"]
pub(crate) struct ProductionRecoveredWalStorageError {
    kind: ProductionRecoveredWalStorageErrorKind,
}

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
enum ProductionRecoveredWalStorageErrorKind {
    #[error("repaired lifecycle ledger changed before unified open")]
    StaleLedger,
    #[error("durable Ready-Fetch recovery failed after WAL repair: {0}")]
    Fetch(#[source] super::ledger::DurableCertifiedFetchRecoveryError),
    #[error("unified lifecycle storage census is inconsistent: {0}")]
    Recovery(#[source] super::open::LifecycleRecoveryAssemblyError),
    #[error("recovered Sign and Ready-Fetch registry carriers conflict")]
    Registry,
    #[error("exact recovered lifecycle open failed: {0}")]
    Open(&'static str),
}

impl ProductionRecoveredWalStorageError {
    fn new(kind: ProductionRecoveredWalStorageErrorKind) -> Self {
        Self { kind }
    }

    /// Return a stable diagnostic without exposing retained startup authority.
    pub(crate) fn reason(&self) -> &'static str {
        match &self.kind {
            ProductionRecoveredWalStorageErrorKind::StaleLedger => {
                "repaired lifecycle ledger changed before unified open"
            }
            ProductionRecoveredWalStorageErrorKind::Fetch(_) => {
                "durable Ready-Fetch recovery failed after WAL repair"
            }
            ProductionRecoveredWalStorageErrorKind::Recovery(_) => {
                "unified lifecycle storage census is inconsistent"
            }
            ProductionRecoveredWalStorageErrorKind::Registry => {
                "recovered Sign and Ready-Fetch registry carriers conflict"
            }
            ProductionRecoveredWalStorageErrorKind::Open(reason) => reason,
        }
    }
}

/// Fail-stop error retaining the exact opened frame beside a failed fsync splice.
#[must_use = "failed exact-store recovered WAL persistence requires restart"]
pub(crate) struct ExactStoreRecoveredWalPersistError<'registry> {
    _ledger: OpenedRecoveredWalValidateLedger,
    error: RecoveredWalValidateLedgerPersistError<'registry>,
}

impl ExactStoreRecoveredWalPersistError<'_> {
    /// Return a stable diagnostic without exposing storage or repair authority.
    pub(crate) const fn reason(&self) -> &'static str {
        self.error.reason()
    }
}

/// Fail-stop error retaining repaired storage beside an uninstalled Sign.
#[must_use = "failed exact-store recovered Sign installation requires restart"]
pub(crate) struct ExactStoreRecoveredWalSignInstallError<'registry> {
    _store: super::ledger::LifecycleLedgerStoreV1,
    _repaired: super::ledger::LifecycleLedgerV1,
    error: RecoveredWalSignInstallError<'registry>,
}

impl ExactStoreRecoveredWalSignInstallError<'_> {
    /// Return a stable diagnostic without exposing storage or registry authority.
    pub(crate) const fn reason(&self) -> &'static str {
        self.error.reason()
    }
}

impl OpenedRecoveredWalValidateLedger {
    /// Fsync the authenticated repair only against this retained store/frame pair.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist_recovered_wal_repair<'registry>(
        self,
        repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ) -> Result<
        PersistedRecoveredWalValidateLedger<'registry>,
        ExactStoreRecoveredWalPersistError<'registry>,
    > {
        let Self { store, opened } = self;
        match repair.persist_in_opened_ledger(&store, &opened) {
            Ok((repaired, repair, _changed)) => Ok(PersistedRecoveredWalValidateLedger {
                store,
                repaired,
                repair,
            }),
            Err(error) => Err(ExactStoreRecoveredWalPersistError {
                _ledger: Self { store, opened },
                error,
            }),
        }
    }
}

impl<'registry> PersistedRecoveredWalValidateLedger<'registry> {
    /// Install the exact recovered Sign without reopening or substituting storage.
    #[allow(clippy::result_large_err)]
    pub(crate) fn install_recovered_wal_sign(
        self,
    ) -> Result<
        InstalledRecoveredWalSignStorage<'registry>,
        ExactStoreRecoveredWalSignInstallError<'registry>,
    > {
        let Self {
            store,
            repaired,
            repair,
        } = self;
        match repair.install_recovered_sign(&store) {
            Ok(installed) => Ok(InstalledRecoveredWalSignStorage {
                store,
                repaired,
                installed,
            }),
            Err(error) => Err(ExactStoreRecoveredWalSignInstallError {
                _store: store,
                _repaired: repaired,
                error,
            }),
        }
    }
}

impl<'registry> InstalledRecoveredWalSignStorage<'registry> {
    /// Complete the final-frame Fetch/Serve/Validate census and exact coordinator open.
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_production_lifecycle(
        self,
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        body_store: &mut V2BodyStore,
        payload_store: &mut CertifiedServePayloadStoreV1,
        serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<
        ProductionOpenedRecoveredWalSignLifecycleCut<'registry>,
        ProductionRecoveredWalStorageError,
    > {
        let body_store_identity = body_store.instance_identity();
        let payload_store_identity = payload_store.instance_identity();
        let Self {
            store,
            repaired,
            mut installed,
        } = self;
        if !store.load().is_ok_and(|loaded| loaded == repaired) {
            return Err(ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::StaleLedger,
            ));
        }
        let projection = installed.authenticated_projection().ok_or_else(|| {
            ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::Registry,
            )
        })?;
        let fetches = repaired
            .authenticate_durable_certified_fetch_startup(verified, body_store)
            .map_err(|error| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Fetch(error),
                )
            })?;
        let (recovery, fetches) = AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign_and_durable_fetch_startup(
            repaired,
            serve_payloads,
            body_store,
            &projection,
            fetches,
        )
        .map_err(|error| {
            ProductionRecoveredWalStorageError::new(
                ProductionRecoveredWalStorageErrorKind::Recovery(error),
            )
        })?;
        fetches
            .install_alongside_recovered_wal_authority(&mut *installed.registry)
            .map_err(|_fetches| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Registry,
                )
            })?;
        let authority =
            authority::production_authority(verified, config, reply_route_source_capacity).ok_or(
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Open(
                        "verified height cannot derive recovered lifecycle authority",
                    ),
                ),
            )?;
        installed
            .open_with_exact_store_authority(authority, store, payload_store, recovery)
            .map_err(|error| {
                ProductionRecoveredWalStorageError::new(
                    ProductionRecoveredWalStorageErrorKind::Open(error.reason()),
                )
            })
            .map(|opened| ProductionOpenedRecoveredWalSignLifecycleCut {
                opened,
                verified: verified.clone(),
                body_store_identity,
                payload_store_identity,
            })
    }
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

/// Fail-stop live use of the recovered-WAL detached-parent reservation.
///
/// The exact validated parent is retained but can no longer restore itself.
/// Its child address is bound before LedgerV1 fsync. After fsync the sole
/// operation inserts prechecked ordinary Sign work at that reserved address;
/// no fallible check or allocation-dependent staging remains.
#[must_use = "a live Validate-to-Sign registry reservation has not been published"]
pub(in crate::sumeragi) struct LiveValidateSignRegistryReservation<'registry> {
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
    _detached_parent: ConcreteLifecycleWork,
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
/// take, or execute the child before the unified startup transaction commits
/// its remaining coordinator and adapter publications.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "an installed recovered WAL Sign child still seals startup"]
pub(crate) struct InstalledRecoveredWalSignRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    parent_address: ConcreteWorkAddress,
    child_address: ConcreteWorkAddress,
    child_digest: LifecycleDigest,
}

/// Exclusive installed view of one standalone recovered control Sign.
///
/// The dedicated carrier remains in the registry while this cut is alive.
/// The cut exposes no address, ordinal, digest, projection, or registry parts;
/// its only production path installs the Fetch census and opens the exact
/// coordinator/store join.
#[must_use = "the installed recovered control Sign must complete startup"]
pub(super) struct InstalledRecoveredWalControlSignRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}

/// Exclusive installed view of one recovered Decision Fetch.
///
/// The dedicated carrier stays in the registry while the complete durable
/// Fetch/Serve/Producer census is installed and joined to the coordinator.
/// No effect, pending binding, locator, or candidate can be extracted.
#[must_use = "the installed recovered Decision Fetch must complete startup"]
pub(super) struct InstalledRecoveredWalDecisionFetchRegistryCut<'registry> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}

/// Fail-stop diagnostic for a rejected recovered control carrier install.
#[must_use = "failed recovered control installation requires restart"]
pub(super) struct RecoveredWalControlSignInstallError {
    failure: RecoveredWalControlSignInstallFailure,
}

#[allow(variant_size_differences)]
enum RecoveredWalControlSignInstallFailure {
    Projection {
        _projection: AuthenticatedRecoveredWalControlProjection,
    },
    Carrier {
        _carrier: DurableRecoveredWalControlSignCarrierV1,
    },
}

impl RecoveredWalControlSignInstallError {
    /// Return a stable diagnostic without exposing retained authority.
    pub(super) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalControlSignInstallFailure::Projection { .. } => {
                "recovered control Sign failed exact registry preflight"
            }
            RecoveredWalControlSignInstallFailure::Carrier { .. } => {
                "recovered control Sign carrier disagrees with durable storage"
            }
        }
    }
}

/// Fail-stop diagnostic from the installed control-carrier coordinator join.
#[must_use = "failed recovered control lifecycle open requires restart"]
pub(super) struct RecoveredWalControlSignLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredWalControlSignLifecycleOpenError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Fail-stop diagnostic for a rejected recovered Decision Fetch install.
#[must_use = "failed recovered Decision Fetch installation requires restart"]
pub(super) struct RecoveredWalDecisionFetchInstallError {
    failure: RecoveredWalDecisionFetchInstallFailure,
}

#[allow(variant_size_differences)]
enum RecoveredWalDecisionFetchInstallFailure {
    Projection {
        _projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    },
    Carrier {
        _carrier: DurableRecoveredWalDecisionFetchCarrierV1,
    },
}

impl RecoveredWalDecisionFetchInstallError {
    /// Return a stable diagnostic without exposing retained authority.
    pub(super) const fn reason(&self) -> &'static str {
        match &self.failure {
            RecoveredWalDecisionFetchInstallFailure::Projection { .. } => {
                "recovered Decision Fetch failed exact registry preflight"
            }
            RecoveredWalDecisionFetchInstallFailure::Carrier { .. } => {
                "recovered Decision Fetch carrier disagrees with durable storage"
            }
        }
    }
}

/// Fail-stop diagnostic from the installed Decision-Fetch coordinator join.
#[must_use = "failed recovered Decision Fetch lifecycle open requires restart"]
pub(super) struct RecoveredWalDecisionFetchLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredWalDecisionFetchLifecycleOpenError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
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

impl LiveValidateSignRegistryReservation<'_> {
    fn bind_exact_child(&mut self, address: ConcreteWorkAddress, digest: LifecycleDigest) -> bool {
        self.reservation.bind_child_if_vacant(address, digest)
    }

    /// Install prechecked ordinary Sign work at the already-reserved child.
    ///
    /// This is called only after exact LedgerV1 fsync. All validation and
    /// vacancy checks happened while the same exclusive registry borrow was
    /// retained, so the remaining map publication is structurally infallible.
    fn install_live_sign(self, work: ConcreteLifecycleWork) {
        let Self {
            reservation,
            _detached_parent: _,
        } = self;
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child,
        } = reservation;
        let (child_address, child_digest) =
            child.expect("pre-fsync live Sign reservation binds one exact child");
        debug_assert_ne!(parent_address, child_address);
        debug_assert!(!registry.entries.contains_key(&parent_address));
        debug_assert!(!registry.entries.contains_key(&child_address));
        debug_assert_eq!(work.digest(), child_digest);
        debug_assert!(work.validates_at(child_address));
        let std::collections::btree_map::Entry::Vacant(entry) =
            registry.entries.entry(child_address)
        else {
            unreachable!("exclusive live Sign reservation kept its child address vacant")
        };
        entry.insert(work);
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

/// Production-only recovered open with the exact stores used by authentication.
///
/// The comparison seals are captured inside the storage-authenticated open,
/// before any borrow is released. A later owner constructor therefore cannot
/// relabel the opened coordinator with same-context foreign store instances.
#[must_use = "the production recovered open must enter its exact lifecycle owner"]
pub(crate) struct ProductionOpenedRecoveredWalSignLifecycleCut<'registry> {
    opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
    verified: VerifiedHeightContext,
    body_store_identity: crate::sumeragi::v2_body_store::V2BodyStoreInstanceIdentity,
    payload_store_identity:
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreInstanceIdentity,
}

/// No-lifetime exact-open seal used only to construct the owning production service.
#[must_use = "the exact recovered WAL open must enter its production owner"]
pub(crate) struct RecoveredWalProductionOwnerOpenV1 {
    pub(super) coordinator: LifecycleCoordinator,
    pub(super) verified: VerifiedHeightContext,
    pub(super) registry_identity: ConcreteLifecycleWorkRegistryInstanceIdentity,
    pub(super) body_store_identity:
        crate::sumeragi::v2_body_store::V2BodyStoreInstanceIdentity,
    pub(super) payload_store_identity:
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreInstanceIdentity,
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
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    RecoveredWalRegistrySlotV1::PhaseVote(self.child_address),
                )
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
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    RecoveredWalRegistrySlotV1::PhaseVote(self.child_address),
                )
    }
}

impl InstalledRecoveredWalControlSignRegistryCut<'_> {
    fn exact_control_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredWalControlSignWork> {
        if self
            .registry
            .entries
            .keys()
            .filter(|address| address.owner == self.address.owner)
            .count()
            != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && sign.validates_in_store(self.address, self.digest, store))
        .then_some(sign)
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.exact_control_work(prepared.store())
            .is_some_and(|sign| {
                sign.carrier.owns_recovery(recovery)
                    && sign.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            })
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    RecoveredWalRegistrySlotV1::ControlSign(self.address),
                )
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.exact_control_work(store).is_some_and(|sign| {
            sign.carrier.owns_recovery(recovery)
                && sign.matches_current_ready_record(self.address, self.digest, coordinator)
        }) && self
            .registry
            .exactly_covers_recovered_ready_work_with_extra(
                coordinator,
                RecoveredWalRegistrySlotV1::ControlSign(self.address),
            )
    }

    /// Install the complete durable Fetch census beside this sole WAL authority.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredWalControlSignLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredWalControlSignLifecycleOpenError::new(
                    "recovered control Sign and Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredWalControlSignRegistryCut<'registry> {
    /// Open and commit the exact control/Fetch/Serve/Producer recovery census.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, RecoveredWalControlSignLifecycleOpenError> {
        if self.exact_control_work(&store).is_none() {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "installed recovered control carrier is not exact",
            ));
        }
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        )
        .map_err(|_error| {
            RecoveredWalControlSignLifecycleOpenError::new(
                "recovered control coordinator preparation failed",
            )
        })?;
        if !self.prepared_join_is_exact(&prepared, &recovery) {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "prepared recovered control registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &recovery)
            .map_err(|_error| {
                RecoveredWalControlSignLifecycleOpenError::new(
                    "recovered control coordinator commit failed",
                )
            })?;
        if !self.opened_join_is_exact(&coordinator, &recovery) {
            return Err(RecoveredWalControlSignLifecycleOpenError::new(
                "opened recovered control registry/coordinator census changed",
            ));
        }
        Ok(coordinator)
    }
}

impl InstalledRecoveredWalDecisionFetchRegistryCut<'_> {
    fn exact_decision_fetch_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredWalDecisionFetchWork> {
        if self
            .registry
            .entries
            .keys()
            .filter(|address| address.owner == self.address.owner)
            .count()
            != 1
        {
            return None;
        }
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && fetch.validates_in_store(self.address, self.digest, store))
        .then_some(fetch)
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        self.exact_decision_fetch_work(prepared.store())
            .is_some_and(|fetch| {
                fetch.carrier.owns_recovery(recovery)
                    && fetch.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            })
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    RecoveredWalRegistrySlotV1::DecisionFetch(self.address),
                )
    }

    fn opened_join_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.exact_decision_fetch_work(store).is_some_and(|fetch| {
            fetch.carrier.owns_recovery(recovery)
                && fetch.matches_current_ready_record(self.address, self.digest, coordinator)
        }) && self
            .registry
            .exactly_covers_recovered_ready_work_with_extra(
                coordinator,
                RecoveredWalRegistrySlotV1::DecisionFetch(self.address),
            )
    }

    /// Install the complete body-backed Fetch census beside the WAL Fetch.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredWalDecisionFetchLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredWalDecisionFetchLifecycleOpenError::new(
                    "recovered Decision Fetch and body-backed Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredWalDecisionFetchRegistryCut<'registry> {
    /// Open and commit the exact Decision-Fetch/Fetch/Serve/Producer census.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<LifecycleCoordinator, RecoveredWalDecisionFetchLifecycleOpenError> {
        if self.exact_decision_fetch_work(&store).is_none() {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "installed recovered Decision Fetch carrier is not exact",
            ));
        }
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
            payload_store,
            &recovery,
        )
        .map_err(|_error| {
            RecoveredWalDecisionFetchLifecycleOpenError::new(
                "recovered Decision Fetch coordinator preparation failed",
            )
        })?;
        if !self.prepared_join_is_exact(&prepared, &recovery) {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "prepared recovered Decision Fetch registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &recovery)
            .map_err(|_error| {
                RecoveredWalDecisionFetchLifecycleOpenError::new(
                    "recovered Decision Fetch coordinator commit failed",
                )
            })?;
        if !self.opened_join_is_exact(&coordinator, &recovery) {
            return Err(RecoveredWalDecisionFetchLifecycleOpenError::new(
                "opened recovered Decision Fetch registry/coordinator census changed",
            ));
        }
        Ok(coordinator)
    }
}

impl<'registry> InstalledRecoveredWalSignRegistryCut<'registry> {
    #[cfg(test)]
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
        let coordinator =
            match prepared.commit_with_registry(&mut *self.registry, payload_store, &recovery) {
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

    /// Open against the exact store retained continuously since parent reconstruction.
    #[allow(clippy::result_large_err)]
    fn open_with_exact_store_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        store: super::ledger::LifecycleLedgerStoreV1,
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
        let prepared = match LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            store,
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
        let coordinator =
            match prepared.commit_with_registry(&mut *self.registry, payload_store, &recovery) {
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
    #[cfg(test)]
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

impl<'registry> ProductionOpenedRecoveredWalSignLifecycleCut<'registry> {
    /// Consume the exclusive registry borrow into a no-lifetime owner-open seal.
    pub(crate) fn into_production_owner_open(
        self,
    ) -> Result<RecoveredWalProductionOwnerOpenV1, Self> {
        let Self {
            opened,
            verified,
            body_store_identity,
            payload_store_identity,
        } = self;
        let Some(projection) = opened.installed.authenticated_projection() else {
            return Err(Self {
                opened,
                verified,
                body_store_identity,
                payload_store_identity,
            });
        };
        if !opened.installed.opened_join_is_exact(
            &opened.coordinator,
            &opened.recovery,
            &projection,
        ) || opened.coordinator.active_context()
            != projection::lifecycle_context(verified.context())
        {
            return Err(Self {
                opened,
                verified,
                body_store_identity,
                payload_store_identity,
            });
        }
        let OpenedRecoveredWalSignLifecycleCut {
            installed,
            recovery,
            coordinator,
        } = opened;
        let registry_identity = installed.registry.instance_identity();
        drop(installed);
        drop(recovery);
        Ok(RecoveredWalProductionOwnerOpenV1 {
            coordinator,
            verified,
            registry_identity,
            body_store_identity,
            payload_store_identity,
        })
    }

    /// Seal a focused opened-cut fixture with the exact stores it used.
    #[cfg(test)]
    pub(crate) fn from_opened_for_test(
        opened: OpenedRecoveredWalSignLifecycleCut<'registry>,
        verified: VerifiedHeightContext,
        body_store: &V2BodyStore,
        payload_store: &CertifiedServePayloadStoreV1,
    ) -> Self {
        Self {
            opened,
            verified,
            body_store_identity: body_store.instance_identity(),
            payload_store_identity: payload_store.instance_identity(),
        }
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
            && self.recovered_wal_sign_census_rejects_mutations_for_test()
    }

    fn recovered_wal_sign_census_rejects_mutations_for_test(&self) -> bool {
        let address = self.installed.child_address;
        let registry = &*self.installed.registry;
        if !registry.exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator) {
            return false;
        }

        let mut missing = self.coordinator.clone();
        missing.records.remove(&address.ordinal);

        let mut terminal = self.coordinator.clone();
        let Some(terminal_record) = terminal.records.get_mut(&address.ordinal) else {
            return false;
        };
        terminal_record.state = super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled);

        let mut stale = self.coordinator.clone();
        stale.ready_index.remove(&address.ordinal);

        let mut mutated = self.coordinator.clone();
        let Some(metadata) = mutated.durable_records.get_mut(&address.ordinal) else {
            return false;
        };
        let mut foreign_source = *metadata.reconstruction_source.as_bytes();
        foreign_source[0] ^= 1;
        metadata.reconstruction_source = LifecycleDigest::new(foreign_source);

        [&missing, &terminal, &stale, &mutated]
            .into_iter()
            .all(|coordinator| {
                !registry.exactly_covers_recovered_ready_work_and_wal_authority(coordinator)
            })
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

/// Non-forgeable successful-validation input accepted only by the adapter's
/// sealed direct-preview entry point.
///
/// Construction stays private to the exact Ready registry preflight. The only
/// consuming projection is used by `v2` and this value is never returned from
/// the registry-owned join.
#[must_use = "validated adapter authority must enter the sealed preview join"]
pub(crate) struct ReadyValidatedAdapterAuthority<'a> {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    receipt: &'a ValidatedBodyReceipt,
}

/// Non-forgeable installed-Validate predecessor accepted only by the
/// adapter's sealed WAL-sign binding step.
///
/// The exact effect and pending binding remain borrowed from the closed Ready
/// completion. Only the adapter may consume this view, so no caller can supply
/// a detached Validate effect, causal root, or candidate statement.
#[must_use = "Validate predecessor authority must bind the sealed WAL Sign preview"]
pub(in crate::sumeragi) struct ReadyValidateSignPredecessorAuthority<'a> {
    effect: &'a AdapterEffect,
    pending: &'a PendingRuntimeEffectBinding,
    _linearity: ReadyValidateSignPredecessorLinearity,
}

struct ReadyValidateSignPredecessorLinearity;

impl Drop for ReadyValidateSignPredecessorLinearity {
    fn drop(&mut self) {}
}

impl<'a> ReadyValidateSignPredecessorAuthority<'a> {
    /// Project only the exact Prepare/Commit vote successor retained by the
    /// adapter preflight. No predecessor parts or certificate can escape.
    pub(in crate::sumeragi) fn project_successor(
        self,
        successor: &AdapterEffect,
        registered_prepare: Option<&RegisteredPrepareValidateSignCapability>,
    ) -> Option<PendingRuntimeEffectBinding> {
        let AdapterEffect::Sign {
            request: crate::sumeragi::v2::SignRequest::Vote(vote),
            ..
        } = successor
        else {
            return None;
        };
        match vote.phase {
            wire::GlobalPhase::Prepare if registered_prepare.is_none() => self
                .pending
                .project_validate_sign_prepare_successor(self.effect, successor),
            wire::GlobalPhase::Commit => self
                .pending
                .project_validate_sign_commit_successor(self.effect, successor)
                .or_else(|| {
                    self.pending
                        .project_validate_sign_commit_successor_with_registered_prepare(
                            self.effect,
                            successor,
                            registered_prepare?,
                        )
                }),
            wire::GlobalPhase::Prepare => None,
        }
    }

    /// Construct the same opaque view for focused adapter tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(
        effect: &'a AdapterEffect,
        pending: &'a PendingRuntimeEffectBinding,
    ) -> Self {
        Self {
            effect,
            pending,
            _linearity: ReadyValidateSignPredecessorLinearity,
        }
    }
}

impl<'a> ReadyValidatedAdapterAuthority<'a> {
    /// Consume the unforgeable registry authority inside the adapter module.
    pub(crate) fn into_parts(
        self,
    ) -> (
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        &'a ValidatedBodyReceipt,
    ) {
        (self.tag, self.round, self.subject, self.receipt)
    }
}

/// Non-forgeable rejected-validation input accepted only by the adapter's
/// sealed direct-preview entry point.
///
/// Diagnostic text is deliberately absent. The registry constructs this value
/// only after proving the one canonical reducer-level rejection identity.
#[must_use = "rejected adapter authority must enter the sealed preview join"]
pub(crate) struct ReadyRejectedAdapterAuthority<'a> {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    receipt: &'a DurableBodyReceipt,
}

impl<'a> ReadyRejectedAdapterAuthority<'a> {
    /// Consume the unforgeable registry authority inside the adapter module.
    pub(crate) fn into_parts(
        self,
    ) -> (
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        &'a DurableBodyReceipt,
    ) {
        (self.tag, self.round, self.subject, self.receipt)
    }
}

/// Fixed dual-borrow result of joining one Ready Validate carrier to the
/// adapter's fully preflighted, still-inert publication.
///
/// The fields remain private and there is no extraction or commit operation.
/// Dropping this inert token releases both borrows without mutating either
/// subsystem.
#[allow(dead_code)]
#[must_use = "a Ready Validate adapter preview retains both authority borrows"]
pub(super) struct PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _adapter: PreparedReadyDurableValidateAdapterPublication<'adapter>,
}

/// Opaque post-fsync Validate-to-Sign authority retaining both subsystem
/// borrows until the sole coordinator/LedgerV1 publication consumes it.
#[allow(dead_code)]
#[must_use = "a persisted Validate Sign has not entered lifecycle publication"]
pub(super) struct PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
}

/// Pre-fsync live registry publication using the recovered-WAL exclusive
/// detached-parent/child-vacancy reservation.
///
/// The detached parent is deliberately non-restoring. The adapter half owns
/// one closed ordinary Sign carrier whose digest already matches the reserved
/// child. Dropping this token therefore requires restart and cannot resurrect
/// a volatile Validate row after the WAL may be durable.
#[must_use = "a live Validate-to-Sign registry publication awaits LedgerV1 fsync"]
pub(super) struct PreparedLiveValidateSignRegistryPublication<'registry, 'adapter> {
    reservation: LiveValidateSignRegistryReservation<'registry>,
    adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
}

/// Opaque fail-stop error from live Sign registry preparation.
#[must_use = "failed live Sign registry preparation retains post-WAL authority"]
pub(super) struct LiveValidateSignRegistryPublicationError<'registry, 'adapter> {
    _failure: LiveValidateSignRegistryPublicationFailure<'registry, 'adapter>,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LiveValidateSignRegistryPublicationFailure<'registry, 'adapter> {
    AdapterWork {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    InvalidCoordinates {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    Detach {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    Reservation {
        _reservation: LiveValidateSignRegistryReservation<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
}

/// One-shot authority for consuming the nested post-WAL Sign into closed
/// ordinary registry work.
///
/// Construction remains private to the fixed live publication transaction.
/// The replay module accepts this token only so no sibling can extract the
/// effect or pending binding from the post-fsync seal.
pub(in crate::sumeragi) struct LiveValidateSignWorkProjectionPermit {
    _linearity: LiveValidateSignWorkProjectionLinearity,
}

struct LiveValidateSignWorkProjectionLinearity;

impl Drop for LiveValidateSignWorkProjectionLinearity {
    fn drop(&mut self) {}
}

impl LiveValidateSignWorkProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: LiveValidateSignWorkProjectionLinearity,
        }
    }
}

/// Ownership-retaining failure from the fixed live-WAL Validate-to-Sign join.
#[allow(dead_code)]
#[must_use = "failed Validate Sign sealing still owns both subsystem borrows"]
pub(super) struct ReadyDurableValidateSignPreAdmissionError<'registry, 'adapter> {
    failure: ReadyDurableValidateSignPreAdmissionFailure<'registry, 'adapter>,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum ReadyDurableValidateSignPreAdmissionFailure<'registry, 'adapter> {
    PreWal {
        _preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
    },
    Wal {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _error: ReadyDurableValidateSignWalError<'adapter>,
    },
}

/// Closed inert replay pre-admission for one exact invalid certified body report.
///
/// The Ready registry row and staged adapter rejection remain exclusively
/// borrowed. The report effect, derived child pending binding, and canonical
/// runtime evidence stay sealed in the adapter half; no installation or
/// execution surface exists on this token.
#[allow(dead_code)]
#[must_use = "invalid-body replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter> {
    registry: PreparedReadyDurableValidateExecution<'registry>,
    adapter: PreparedInvalidBodyReportAdapterReplay<'adapter>,
}

/// Ownership-preserving failure from the fixed invalid-body replay join.
#[allow(dead_code)]
#[must_use = "failed invalid-body replay preparation retains both authority borrows"]
pub(super) struct InvalidBodyReportReplayPreAdmissionError<'registry, 'adapter> {
    preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
}

impl PreparedReadyDurableValidateAdapterPreview<'_, '_> {
    /// Project one exact no-successor cut without exposing the retained body.
    ///
    /// The transition module supplies its private one-shot permit. The frame is
    /// derived from the still-installed completion, and only inactive or
    /// no-effect branches can return the opaque projection.
    pub(super) fn project_no_successor_for_body_transition(
        &self,
        permit: SealedValidateNoSuccessorProjectionPermit,
        lease: &TurnLease,
    ) -> Result<SealedValidateNoSuccessorProjection, SealedValidateTerminalProjectionError> {
        if !self._registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        let release_consensus_reservation = sealed_validate_no_successor_reservation(
            self._adapter.kind(),
            self._registry.outcome_kind,
        )?;
        let completion = self
            ._registry
            .completion()
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        if completion.outcome.durable_body() != &completion.incumbent.durable_receipt {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedValidateNoSuccessorProjection::from_registry(
            permit,
            lease.clone(),
            parent_payload,
            release_consensus_reservation,
        ))
    }
}

impl<'registry, 'adapter> PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter> {
    /// Consume only the exact validated Persist branch into a real post-fsync
    /// vote-sign seal.
    ///
    /// The registry mints the predecessor authority directly from its still-
    /// installed completion. A branch or lineage mismatch returns the whole
    /// dual-borrow preview before WAL I/O. Once append is attempted, every
    /// error is opaque and restart-only.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_live_wal_validate_sign(
        self,
    ) -> Result<
        PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>,
        ReadyDurableValidateSignPreAdmissionError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let Some(predecessor) = registry.validate_sign_predecessor_authority() else {
            return Err(ReadyDurableValidateSignPreAdmissionError {
                failure: ReadyDurableValidateSignPreAdmissionFailure::PreWal {
                    _preview: Self {
                        _registry: registry,
                        _adapter: adapter,
                    },
                },
            });
        };
        let adapter = match adapter.bind_validate_sign_predecessor(predecessor) {
            Ok(adapter) => adapter,
            Err(adapter) => {
                return Err(ReadyDurableValidateSignPreAdmissionError {
                    failure: ReadyDurableValidateSignPreAdmissionFailure::PreWal {
                        _preview: Self {
                            _registry: registry,
                            _adapter: adapter,
                        },
                    },
                });
            }
        };
        match adapter.append_live_wal() {
            Ok(adapter) => Ok(PreparedReadyDurableValidatePersistedSignPreAdmission {
                _registry: registry,
                _adapter: adapter,
            }),
            Err(error) => Err(ReadyDurableValidateSignPreAdmissionError {
                failure: ReadyDurableValidateSignPreAdmissionFailure::Wal {
                    _registry: registry,
                    _error: error,
                },
            }),
        }
    }

    /// Consume only the exact Ready/rejected report preview into replay pre-admission.
    ///
    /// All inputs are read from the still-installed completion and staged
    /// adapter publication. Failure reconstructs the complete dual-borrow
    /// preview, while success remains publication-inert.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_invalid_body_report_replay(
        self,
    ) -> Result<
        PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>,
        InvalidBodyReportReplayPreAdmissionError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let Some(completion) = registry.completion() else {
            return Err(InvalidBodyReportReplayPreAdmissionError {
                preview: Self {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        };
        if registry.outcome_kind != ReadyDurableValidateOutcomeKind::Rejected
            || completion.outcome.validated_receipt().is_some()
            || completion.outcome.rejection_identity()
                != Some(&BodyValidationRejectionIdentity::Rejected)
            || completion.outcome.missing_merge_sidecar().is_some()
        {
            return Err(InvalidBodyReportReplayPreAdmissionError {
                preview: Self {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        }
        let validate_origin = completion.incumbent.replay_evidence.clone();
        let adapter = match adapter.seal_invalid_body_report_replay(
            validate_origin,
            &completion.incumbent.effect,
            &completion.incumbent.pending,
            &completion.incumbent.durable_receipt,
        ) {
            Ok(adapter) => adapter,
            Err(adapter) => {
                return Err(InvalidBodyReportReplayPreAdmissionError {
                    preview: Self {
                        _registry: registry,
                        _adapter: adapter,
                    },
                });
            }
        };
        let sealed = PreparedInvalidBodyReportReplayPreAdmission { registry, adapter };
        debug_assert!(sealed.validates());
        Ok(sealed)
    }
}

impl PreparedInvalidBodyReportReplayPreAdmission<'_, '_> {
    fn validates(&self) -> bool {
        self.registry.completion().is_some_and(|completion| {
            self.registry.outcome_kind == ReadyDurableValidateOutcomeKind::Rejected
                && completion.outcome.rejection_identity()
                    == Some(&BodyValidationRejectionIdentity::Rejected)
                && completion.outcome.validated_receipt().is_none()
                && completion.outcome.missing_merge_sidecar().is_none()
                && self.adapter.exactly_matches(
                    &completion.incumbent.effect,
                    &completion.incumbent.pending,
                    &completion.incumbent.durable_receipt,
                )
        })
    }

    /// Project the invalid-body child while every raw part remains sealed.
    ///
    /// Only the body-transition module can mint the one-shot permit. Candidate
    /// projection occurs inside this adapter/registry join and the returned
    /// value has no field or candidate accessor outside that module.
    pub(super) fn project_for_body_transition(
        &self,
        permit: SealedInvalidBodyReportProjectionPermit,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<SealedInvalidBodyReportProjection, SealedValidateTerminalProjectionError> {
        if !self.registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        if !self.validates() {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        let completion = self
            .registry
            .completion()
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let candidate = self
            .adapter
            .project_invalid_body_report_candidate(
                &permit,
                verified,
                &completion.incumbent.effect,
                &completion.incumbent.pending,
                &completion.incumbent.durable_receipt,
            )
            .map_err(SealedValidateTerminalProjectionError::Projection)?;
        let expected_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let mut context = [0_u8; 32];
        context.copy_from_slice(completion.incumbent.durable_receipt.context_id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context),
            completion.incumbent.durable_receipt.round().height,
        );
        if candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::InvalidBodyReport
            || candidate.stage.kind() != LifecycleStageKind::ReportInvalidBody
            || candidate.stage.predecessor_scope() != PredecessorScope::Independent
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != DurablePayloadReference::None
            || !candidate.replay_authority_is_exact(active_context)
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || !projected_slots.contains_key(&expected_slot)
            || projected_universe.len() != 1
            || !projected_universe.contains(&expected_slot)
            || projected_consumed != projected_universe
        {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedInvalidBodyReportProjection::from_registry(
            permit,
            lease.clone(),
            candidate,
            parent_payload,
        ))
    }

    /// Compare one retained lease without exposing registry or report parts.
    #[cfg(test)]
    fn exactly_matches_lease_for_test(&self, lease: &TurnLease) -> bool {
        self.validates() && self.registry.matches_exact_lease(lease)
    }

    /// Compare one retained body receipt without exposing the canonical frame.
    #[cfg(test)]
    fn exactly_matches_receipt_for_test(&self, receipt: &DurableBodyReceipt) -> bool {
        self.validates() && self.registry.matches_exact_durable_receipt(receipt)
    }
}

impl PreparedReadyDurableValidatePersistedSignPreAdmission<'_, '_> {
    /// Project the exact post-WAL Sign child while all effect, pending, replay,
    /// and durable-body authority remains nested in this fixed join.
    pub(super) fn project_for_body_transition(
        &self,
        permit: SealedValidateSignProjectionPermit,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<SealedValidateSignProjection, SealedValidateTerminalProjectionError> {
        if !self._registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        let completion = self
            ._registry
            .completion()
            .filter(|completion| {
                self._registry.outcome_kind == ReadyDurableValidateOutcomeKind::Validated
                    && completion.outcome.validated_receipt().is_some()
                    && completion.outcome.rejection_identity().is_none()
                    && completion.outcome.missing_merge_sidecar().is_none()
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let candidate = self
            ._adapter
            .project_validate_sign_candidate(&permit, verified)
            .map_err(SealedValidateTerminalProjectionError::Projection)?;
        let expected_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let expected_stage = match candidate.key.phase() {
            LifecyclePhase::Prepare => LifecycleStageKind::SignPrepareVote,
            LifecyclePhase::Commit => LifecycleStageKind::SignCommitVote,
            _ => return Err(SealedValidateTerminalProjectionError::InvalidCarrier),
        };
        let mut context = [0_u8; 32];
        context.copy_from_slice(completion.incumbent.durable_receipt.context_id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context),
            completion.incumbent.durable_receipt.round().height,
        );
        if candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::SignVote
            || candidate.stage.kind() != expected_stage
            || candidate.stage.predecessor_scope() != PredecessorScope::Independent
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != DurablePayloadReference::None
            || !candidate.replay_authority_is_exact(active_context)
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || !projected_slots.contains_key(&expected_slot)
            || projected_universe.len() != 1
            || !projected_universe.contains(&expected_slot)
            || projected_consumed != projected_universe
        {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedValidateSignProjection::from_registry(
            permit,
            lease.clone(),
            candidate,
            parent_payload,
        ))
    }
}

impl<'registry, 'adapter>
    PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>
{
    /// Prepare the exact detached-parent and reserved-child registry half.
    ///
    /// The adapter first consumes its nested replay seal into closed ordinary
    /// Sign work without exposing parts. Only after every coordinate, digest,
    /// and vacancy check succeeds is the existing restorable recovered-WAL cut
    /// converted into a non-restoring live reservation.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_registry_publication(
        self,
        lease: &TurnLease,
        child_ordinal: u128,
        child_slot: PhysicalSlotId,
        child_digest: LifecycleDigest,
    ) -> Result<
        PreparedLiveValidateSignRegistryPublication<'registry, 'adapter>,
        LiveValidateSignRegistryPublicationError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let adapter =
            match adapter.prepare_registry_work(LiveValidateSignWorkProjectionPermit::new()) {
                Ok(adapter) => adapter,
                Err(adapter) => {
                    return Err(LiveValidateSignRegistryPublicationError {
                        _failure: LiveValidateSignRegistryPublicationFailure::AdapterWork {
                            _registry: registry,
                            _adapter: adapter,
                        },
                    });
                }
            };
        let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot);
        let coordinates_are_exact = registry.matches_exact_lease(lease)
            && registry.outcome_kind == ReadyDurableValidateOutcomeKind::Validated
            && registry.completion().is_some()
            && child_address.is_some_and(|address| {
                address != registry.address
                    && address.owner == registry.address.owner
                    && address.ordinal == child_ordinal
                    && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
                    && !registry.registry.entries.contains_key(&address)
            })
            && adapter.registry_work_matches(
                lease.owner(),
                child_ordinal,
                child_slot,
                child_digest,
            );
        if !coordinates_are_exact {
            return Err(LiveValidateSignRegistryPublicationError {
                _failure: LiveValidateSignRegistryPublicationFailure::InvalidCoordinates {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        }
        let child_address = child_address.expect("exact coordinates retain one child address");
        let cut = match registry.into_recovered_wal_validate_registry_cut() {
            Ok(cut) => cut,
            Err(registry) => {
                return Err(LiveValidateSignRegistryPublicationError {
                    _failure: LiveValidateSignRegistryPublicationFailure::Detach {
                        _registry: registry,
                        _adapter: adapter,
                    },
                });
            }
        };
        let mut reservation = cut
            .into_live_validate_sign_reservation()
            .expect("validated recovered cut transfers both retained fields");
        if !reservation.bind_exact_child(child_address, child_digest) {
            return Err(LiveValidateSignRegistryPublicationError {
                _failure: LiveValidateSignRegistryPublicationFailure::Reservation {
                    _reservation: reservation,
                    _adapter: adapter,
                },
            });
        }
        Ok(PreparedLiveValidateSignRegistryPublication {
            reservation,
            adapter,
        })
    }
}

impl PreparedLiveValidateSignRegistryPublication<'_, '_> {
    /// Complete the already-fsynced registry and adapter publication.
    ///
    /// All checks ran before LedgerV1 persistence. This method contains only
    /// the fixed reserved-row insertion and staged adapter swaps.
    pub(super) fn publish_after_ledger_fsync(self) {
        self.adapter
            .install_registry_and_commit_adapter(self.reservation);
    }
}

/// Ownership-preserving failure from the fixed Ready Validate adapter join.
#[allow(dead_code)]
#[must_use = "a failed Ready Validate preview still retains its registry cut"]
pub(super) struct ReadyDurableValidateAdapterPreviewError<'registry> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _failure: ReadyDurableValidateAdapterPreviewFailure,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum ReadyDurableValidateAdapterPreviewFailure {
    RegistryAuthority,
    Adapter(crate::sumeragi::v2::AdapterError),
}

// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN
/// Move-only registry authority detached from one exact durable Validate row.
///
/// All fields are private. The exact address and incumbent digest exist only
/// to recheck the unchanged registry row after storage work; the validation
/// service can neither decompose this value nor derive scheduling authority
/// from it.
#[derive(Debug)]
#[must_use = "detached durable Validate authority must be executed or retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DetachedDurableValidateExecution {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    causal_lifecycle_key: Hash,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
}

/// Move-only result of executing one detached durable Validate request.
///
/// The request remains sealed beside the body-store-minted closed outcome, so
/// every later registry check retains the original physical and durable
/// authority instead of accepting caller-supplied coordinates.
#[derive(Debug)]
#[must_use = "executed durable Validate authority has not been reattached"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ExecutedDurableValidateExecution {
    request: DetachedDurableValidateExecution,
    outcome: DurableBodyValidationOutcome,
}

/// Borrow-bound exact-row reattachment of one executed Validate outcome.
///
/// Reattachment and drop mutate nothing. This token deliberately exposes no
/// registry replacement or coordinator publication operation.
#[must_use = "reattached durable Validate outcome has not entered atomic publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableValidateCompletion<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    executed: ExecutedDurableValidateExecution,
}
// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_END

// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_BEGIN
/// Move-only wake authority paired only with its exact detached validation.
///
/// The token is deliberately private: neither its source nor observed
/// generation can be separated from the request and used to wake another
/// lifecycle row.
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
struct DurableValidateWakeAuthority {
    wait_token: WaitToken,
}

/// One exact durable validation whose claimed lifecycle lease has already
/// become an explicit external wait.
#[derive(Debug)]
#[must_use = "a durable Validate dispatch must be executed or retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DurableValidateDispatch {
    request: DetachedDurableValidateExecution,
    wake: DurableValidateWakeAuthority,
}

/// Closed validation result retaining the exact external-wait authority.
///
/// The sole volatile completion transaction reattaches `executed`, installs
/// its executable typed outcome carrier, and publishes `wake` at the same
/// physical address atomically. Sidecar deferral retains this value intact.
#[derive(Debug)]
#[must_use = "an executed durable Validate dispatch awaits typed completion publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ExecutedDurableValidateDispatch {
    executed: ExecutedDurableValidateExecution,
    wake: DurableValidateWakeAuthority,
}
// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_END

// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_BEGIN
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurableValidateOutcomeKind {
    Validated,
    Rejected,
    DeferredMergeSidecar,
}

/// Sealed exact authority for one Waiting-to-Ready Validate publication.
///
/// Construction is private to exact registry reattachment. The coordinator
/// receives this typed projection instead of caller-supplied address, wait, or
/// digest parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DurableValidateCompletionAuthority {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: Option<LifecycleDigest>,
    wait_token: WaitToken,
    outcome_kind: DurableValidateOutcomeKind,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
    payload: DurablePayloadReference,
}

/// Typed location of one published successful validation carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PublishedValidated {
    location: DurableValidatePublishedLocation,
}

/// Typed location of one published deterministic-rejection carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PublishedRejected {
    location: DurableValidatePublishedLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
struct DurableValidatePublishedLocation {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: LifecycleDigest,
}

/// Move-only merge-sidecar dependency retaining its exact executed dispatch.
#[derive(Debug)]
#[must_use = "a deferred Validate dispatch still requires sealed sidecar registration"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DeferredDurableValidateDispatch {
    dispatch: ExecutedDurableValidateDispatch,
}

/// Closed result of the volatile Validate completion transaction.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[must_use = "published or deferred Validate completion authority must be retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum DurableValidateCompletionPublication {
    /// The exact validated carrier and logical Ready replacement committed.
    PublishedValidated(PublishedValidated),
    /// The exact deterministic rejection carrier and Ready replacement committed.
    PublishedRejected(PublishedRejected),
    /// Merge-sidecar absence left both volatile sides exactly Waiting/original.
    DeferredMergeSidecar(DeferredDurableValidateDispatch),
}

/// Borrow-bound exact executed-dispatch reattachment.
///
/// Drop changes nothing. The only consuming paths either return the dispatch
/// with a typed failure, retain it in a deferral, or stage the specialized
/// unwind-safe same-address carrier conversion.
#[must_use = "an exact executed Validate reattachment has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedExecutedDurableValidateCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    dispatch: ExecutedDurableValidateDispatch,
    authority: DurableValidateCompletionAuthority,
}

/// Armed same-address Validate conversion restored automatically on unwind.
#[must_use = "the staged Validate carrier must commit or roll back"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct StagedDurableValidateCompletion<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    request: Option<DetachedDurableValidateExecution>,
    wake: Option<DurableValidateWakeAuthority>,
    publication: PublishedDurableValidateCompletion,
    armed: bool,
}

/// Infallible Copy metadata returned when an armed carrier is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PublishedDurableValidateCompletion {
    /// Exact successful-validation publication metadata.
    Validated(PublishedValidated),
    /// Exact deterministic-rejection publication metadata.
    Rejected(PublishedRejected),
}
// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_END

/// Receipt-bound successful validation of one closed Validate carrier.
///
/// The live registry row remains untouched and exclusively borrowed. The
/// deterministic completion digest is ready for a future same-address
/// coordinator replacement, but this token deliberately exposes no registry
/// installation, removal, or commit operation.
#[must_use = "a validated body completion has not entered its atomic publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedValidatedBodyCompletion<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: LifecycleDigest,
    validated_receipt: ValidatedBodyReceipt,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}

/// Closed live-WAL replay preflight retained until future exact admission.
///
/// Payload-free stages own the canonical source seal immediately. `Apply`
/// additionally owns the exact receipt-bound Validate completion that supplied
/// its body frame. No field, effect, pending binding, receipt, or replay parts
/// can be extracted, and dropping the token publishes no lifecycle work.
#[must_use = "live WAL replay evidence has not entered lifecycle admission"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedLiveWalReplayPreAdmission<'a> {
    _persisted: SealedLiveWalPersistedEffectV1,
    _origin: LiveWalReplayPreAdmissionOrigin<'a>,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionOrigin<'a> {
    PayloadFree,
    Apply(PreparedValidatedBodyCompletion<'a>),
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionFailure<'a> {
    PayloadFree {
        _persisted: SealedLiveWalPersistedEffectV1,
    },
    Apply {
        _completion: PreparedValidatedBodyCompletion<'a>,
        _persisted: SealedLiveWalPersistedEffectV1,
        _pending: PendingRuntimeEffectBinding,
    },
}

/// Ownership-preserving failure from exact live-WAL replay preflight.
pub(super) struct LiveWalReplayPreAdmissionError<'a> {
    _failure: LiveWalReplayPreAdmissionFailure<'a>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl PreparedLiveWalReplayPreAdmission<'static> {
    /// Seal one of the five payload-free WAL continuations with its exact pending owner.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_payload_free(
        persisted: SealedLiveWalPersistedEffectV1,
    ) -> Result<Self, LiveWalReplayPreAdmissionError<'static>> {
        if !persisted.exactly_binds_payload_free_pending() {
            return Err(LiveWalReplayPreAdmissionError {
                _failure: LiveWalReplayPreAdmissionFailure::PayloadFree {
                    _persisted: persisted,
                },
            });
        }
        Ok(Self {
            _persisted: persisted,
            _origin: LiveWalReplayPreAdmissionOrigin::PayloadFree,
        })
    }
}

/// Move-only Validate projection sealed under its closed durable Store parent.
///
/// No field can be extracted. Its only cross-module consuming path retains the
/// whole token inside inert coordinator staging; no registry installation or
/// publication exists in this tranche.
///
/// TODO: Add publication only when the registry, coordinator, durable-catalog,
/// and adapter cuts can commit together.
#[must_use = "a sealed Validate successor has not entered a parent-to-child transaction"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableStoreValidateSuccessor<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    _store_address: ConcreteWorkAddress,
    _validate_effect: AdapterEffect,
    _validate_digest: LifecycleDigest,
    _validate_pending: PendingRuntimeEffectBinding,
    _durable_body: DurableBodyReceipt,
    _expected_manifest_hash: HashOf<wire::PayloadManifest>,
    _replay_evidence: CertifiedValidateReplayEvidenceV1,
}

/// Move-only Store-successor projection sealed under its closed Fetch parent.
///
/// The projected pending binding never escapes this token. In particular,
/// callers cannot clone or install it independently of the still-borrowed
/// completion. Its inert coordinator staging path retains this entire token;
/// no child installation or publication is exposed.
///
/// TODO: Add publication only with a typed output from the real checked-dequeue
/// witness; never add a constructor from raw response parts.
#[must_use = "a sealed Store successor has not entered a parent-to-child transaction"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    _completion_address: ConcreteWorkAddress,
    _store_effect: AdapterEffect,
    _store_digest: LifecycleDigest,
    _store_pending: PendingRuntimeEffectBinding,
    _durable_body: DurableBodyReceipt,
    _expected_manifest_hash: HashOf<wire::PayloadManifest>,
    _replay_evidence: CertifiedStoreReplayEvidenceV1,
}

/// Closed failure while projecting a sealed body-stage successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SealedBodySuccessorProjectionError {
    /// The retained registry parent is not the lease's exact owner/ordinal/slot.
    ForeignParent,
    /// The move-only successor no longer matches its retained parent or body frame.
    InvalidCarrier,
    /// Authenticated replay projection rejected the verified height context.
    Projection(AdapterEffectAdmissionError),
}

/// Closed failure inventory for sealed terminal Validate projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SealedValidateTerminalProjectionError {
    /// The retained completion is not the supplied coordinator parent.
    ForeignParent,
    /// The installed completion or its nested adapter seal is inconsistent.
    InvalidCarrier,
    /// This adapter branch cannot enter the requested terminal edge.
    InvalidBranch,
    /// Canonical report projection rejected the verified height context.
    Projection(AdapterEffectAdmissionError),
}

const fn sealed_validate_no_successor_reservation(
    publication: ReadyDurableValidateAdapterPublicationKind,
    outcome: ReadyDurableValidateOutcomeKind,
) -> Result<bool, SealedValidateTerminalProjectionError> {
    match (publication, outcome) {
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect,
            ReadyDurableValidateOutcomeKind::Validated,
        ) => Ok(false),
        (
            ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
            ReadyDurableValidateOutcomeKind::Rejected,
        ) => Ok(true),
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedBusy
            | ReadyDurableValidateAdapterPublicationKind::ValidatedApply
            | ReadyDurableValidateAdapterPublicationKind::ValidatedPersist
            | ReadyDurableValidateAdapterPublicationKind::RejectedBusy
            | ReadyDurableValidateAdapterPublicationKind::RejectedReport,
            _,
        ) => Err(SealedValidateTerminalProjectionError::InvalidBranch),
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
            | ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
            _,
        ) => Err(SealedValidateTerminalProjectionError::InvalidCarrier),
    }
}

/// Borrow-bound registry conversion prepared before the exact queue CAS.
///
/// Preparation is read-only. Dropping this value therefore leaves every map
/// allocation, key, and move-only incumbent untouched. This token has no
/// dequeue commit; it must first consume a store-minted durable receipt whose
/// complete response and body bindings match this preflight.
#[must_use = "prepared completion conversion has not observed a successful queue CAS"]
pub(super) struct PreparedCertifiedFetchCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    location: CertifiedFetchWaitingLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    response_round: wire::ConsensusRound,
    response_subject: wire::BlockSubject,
    response_manifest_hash: HashOf<wire::PayloadManifest>,
    authenticated_responder: PeerId,
    replay_origin: AuthenticatedCertifiedFetchReplayOriginV1,
}

/// Receipt-bound completion conversion authorized to consume one exact
/// checked-dequeue result.
///
/// This is the sole owner of the post-CAS registry commit. Construction
/// consumes the drop-inert selector preflight plus a sealed body-store receipt;
/// neither raw response parts nor a caller-minted body acknowledgement are
/// accepted.
#[must_use = "durable completion conversion has not observed a successful queue CAS"]
pub(super) struct PreparedDurableCertifiedFetchCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    location: CertifiedFetchWaitingLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    durable_receipt: DurableCertifiedFetchBodyReceipt,
    replay_evidence: CertifiedFetchReplayEvidenceV1,
    ready_projection: DurableCertifiedFetchReplayProjectionV1,
}

/// Failure from the registry-before-ledger publication boundary.
pub(super) enum RegistryPublicationError<E> {
    /// Exact-address installation failed before publication was attempted.
    Install(RegistryError, ConcreteLifecycleWork),
    /// Durable publication failed and the just-installed work was removed.
    Publication(E, ConcreteLifecycleWork),
}

/// Failure from an exact same-address replacement boundary.
#[derive(Debug)]
pub(super) enum RegistryReplacementError<E> {
    /// The incumbent or replacement failed exact validation before mutation.
    Validation(RegistryError, ConcreteLifecycleWork),
    /// The callback rejected the staged replacement and the incumbent was restored.
    Publication(E, ConcreteLifecycleWork),
}

/// Unwind-safe staging guard for one new registry installation.
struct StagedRegistryInstall<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    armed: bool,
}

impl StagedRegistryInstall<'_> {
    fn commit(mut self) {
        self.armed = false;
    }

    fn rollback(mut self) -> ConcreteLifecycleWork {
        self.armed = false;
        self.entries
            .remove(&self.address)
            .expect("staged installation remains at its exact address")
    }
}

impl Drop for StagedRegistryInstall<'_> {
    fn drop(&mut self) {
        if self.armed {
            let removed = self
                .entries
                .remove(&self.address)
                .expect("unwinding installation remains at its exact address");
            drop(removed);
        }
    }
}

/// Unwind-safe staging guard for one exact registry replacement.
struct StagedRegistryReplacement<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    incumbent: Option<ConcreteLifecycleWork>,
}

impl StagedRegistryReplacement<'_> {
    fn commit(mut self) -> ConcreteLifecycleWork {
        self.incumbent
            .take()
            .expect("staged replacement retains its incumbent until commit")
    }

    fn rollback(mut self) -> ConcreteLifecycleWork {
        let incumbent = self
            .incumbent
            .take()
            .expect("staged replacement retains its incumbent until rollback");
        self.entries
            .insert(self.address, incumbent)
            .expect("staged replacement remains installed at its exact address")
    }
}

impl Drop for StagedRegistryReplacement<'_> {
    fn drop(&mut self) {
        let Some(incumbent) = self.incumbent.take() else {
            return;
        };
        let replacement = self
            .entries
            .insert(self.address, incumbent)
            .expect("unwinding replacement remains installed at its exact address");
        drop(replacement);
    }
}

/// Closed failure inventory for concrete-work registration and resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegistryError {
    /// The sealed pending authority does not name the supplied effect.
    UnboundEffect,
    /// Owner and ordinal do not form a valid admitted address.
    InvalidAddress,
    /// The admitted owner does not name the pending work's causal root.
    CausalOwnerMismatch,
    /// The coordinator's slot digest and concrete effect digest disagree.
    DigestMismatch,
    /// One concrete work value already occupies the exact logical address.
    Occupied,
    /// No concrete work value exists at the lease's exact address.
    Missing,
    /// A stored value lost its sealed effect binding.
    CorruptWork,
    /// The exact row is a closed carrier and cannot re-enter generic adapter execution.
    WrongWorkKind,
    /// The coordinator's admitted record did not name exactly one effect slot.
    InvalidAdmissionShape,
}

/// Deterministic process-local map from admitted slots to concrete effects.
///
/// This registry is deliberately not a scheduler. It owns no readiness,
/// ordinal allocation, rank, retry, wait, generation, capacity, or lease state.
#[derive(Debug)]
pub(super) struct ConcreteLifecycleWorkRegistry {
    identity: std::sync::Arc<ConcreteLifecycleWorkRegistryInstanceIdentityMarker>,
    entries: BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
}

/// Exclusive optional WAL-owned registry slot at startup.
#[derive(Clone, Copy)]
enum RecoveredWalRegistrySlotV1 {
    None,
    PhaseVote(ConcreteWorkAddress),
    ControlSign(ConcreteWorkAddress),
    DecisionFetch(ConcreteWorkAddress),
}

impl RecoveredWalRegistrySlotV1 {
    const fn address(self) -> Option<ConcreteWorkAddress> {
        match self {
            Self::None => None,
            Self::PhaseVote(address)
            | Self::ControlSign(address)
            | Self::DecisionFetch(address) => Some(address),
        }
    }
}

#[derive(Debug)]
struct ConcreteLifecycleWorkRegistryInstanceIdentityMarker;

/// Comparison-only identity for one exact concrete registry instance.
#[derive(Clone, Debug)]
pub(super) struct ConcreteLifecycleWorkRegistryInstanceIdentity(
    std::sync::Arc<ConcreteLifecycleWorkRegistryInstanceIdentityMarker>,
);

impl ConcreteLifecycleWorkRegistryInstanceIdentity {
    /// Return whether both seals came from the same registry owner.
    pub(super) fn same_instance(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for ConcreteLifecycleWorkRegistry {
    fn default() -> Self {
        Self {
            identity: std::sync::Arc::new(ConcreteLifecycleWorkRegistryInstanceIdentityMarker),
            entries: BTreeMap::new(),
        }
    }
}

impl ConcreteLifecycleWorkRegistry {
    /// Project a comparison-only seal for this exact registry instance.
    pub(super) fn instance_identity(&self) -> ConcreteLifecycleWorkRegistryInstanceIdentity {
        ConcreteLifecycleWorkRegistryInstanceIdentity(std::sync::Arc::clone(&self.identity))
    }

    /// Whether this registry has no installed concrete authority.
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consume one exact durable control projection into its dedicated carrier.
    ///
    /// Every projection, opened-frame, unique-row, standalone-owner, address,
    /// geometry, digest, and vacancy check precedes the sole insertion. The
    /// existing durable row is never rewritten here; a coalesced restart only
    /// reconstructs this volatile carrier.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_control_sign<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalControlProjection,
    ) -> Result<
        InstalledRecoveredWalControlSignRegistryCut<'registry>,
        RecoveredWalControlSignInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalControlSignInstallError {
                        failure: RecoveredWalControlSignInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(
                DurableRecoveredWalControlSignWork { carrier, address },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalControlSignRegistryCut {
            registry: self,
            address,
            digest,
        })
    }

    /// Consume one exact durable Decision Fetch projection into its carrier.
    ///
    /// All projection, row, owner, address, geometry, digest, store, and
    /// vacancy checks precede the sole insertion. An exact coalesced ledger
    /// row is read-only; this method reconstructs only process-local authority.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_decision_fetch<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<
        InstalledRecoveredWalDecisionFetchRegistryCut<'registry>,
        RecoveredWalDecisionFetchInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalDecisionFetchInstallError {
                        failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(
                DurableRecoveredWalDecisionFetchWork { carrier, address },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalDecisionFetchRegistryCut {
            registry: self,
            address,
            digest,
        })
    }

    /// Install the startup Serve/Producer batch only after proving the exact
    /// prospective Fetch/(optional Sign)/Serve/Producer census. Rejection is
    /// before both registry mutation and the publication callback.
    pub(super) fn install_certified_serve_startup_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        coordinator: &LifecycleCoordinator,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_startup_registry(self, coordinator) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }

    /// Install one fresh adjacent Serve/Producer batch only after comparing the
    /// complete current and prospective concrete census. No raw ordinal or
    /// digest enters this boundary.
    pub(super) fn install_certified_serve_fresh_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_fresh_registry(self, current, staged) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }

    /// Install a complete Certified-Serve/ProducerTurn batch immediately
    /// around one durable publication. The full registry and batch are checked
    /// before the first insertion. Publication failure removes every inserted
    /// carrier and returns the reconstructed move-only batch.
    pub(super) fn install_certified_serve_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_registry(self) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        let mut staged = StagedCertifiedServeRegistryBatch {
            entries: &mut self.entries,
            addresses: Vec::with_capacity(batch.entries.len()),
        };
        for (address, work) in batch.entries {
            staged.addresses.push(address);
            let displaced = staged.entries.insert(address, work);
            debug_assert!(displaced.is_none(), "complete preflight fixed vacancy");
            if displaced.is_some() {
                unreachable!("exclusive registry borrow cannot change after preflight")
            }
        }
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => Err(CertifiedServeRegistryBatchPublicationError::Publication(
                error,
                staged.rollback(),
            )),
        }
    }

    /// Publish the exact terminal LedgerV1 successor while the registry's
    /// eventual Producer replacement is staged at the same address.
    ///
    /// Ledger failure restores the byte-for-byte incumbent before returning.
    /// Ledger success is followed only by infallible exact-address removals:
    /// Serve always leaves the registry, and cancellation removes Producer as
    /// well. No allocation or fallible callback occurs after Ledger fsync.
    pub(super) fn publish_certified_serve_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedCertifiedServeTerminalRegistryTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeTerminalRegistryPublicationError<E>> {
        if !prepared.preflights_current(self, current, lease)
            || !prepared.preflights_exact_staged_successor(current, staged, lease)
        {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        }

        if prepared.outcome == super::TerminalOutcome::Cancelled {
            if !prepared.preflights_cancelled_successor(staged) {
                return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                    prepared,
                ));
            }
            return match publish() {
                Ok(published) => {
                    drop(
                        self.entries
                            .remove(&prepared.serve_address)
                            .expect("terminal preflight retained the exact Serve carrier"),
                    );
                    drop(
                        self.entries
                            .remove(&prepared.producer_address)
                            .expect("cancel preflight retained the exact Producer carrier"),
                    );
                    Ok(published)
                }
                Err(error) => Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                )),
            };
        }

        let Some(replacement) = prepared.producer_replacement(staged) else {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        };
        let incumbent = std::mem::replace(
            self.entries
                .get_mut(&prepared.producer_address)
                .expect("terminal preflight retained the exact Producer carrier"),
            replacement,
        );
        let staged_registry = StagedCertifiedServeTerminalProducer {
            entries: &mut self.entries,
            producer_address: prepared.producer_address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => {
                staged_registry.commit();
                drop(
                    self.entries
                        .remove(&prepared.serve_address)
                        .expect("terminal preflight retained the exact Serve carrier"),
                );
                Ok(published)
            }
            Err(error) => {
                staged_registry.rollback();
                Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                ))
            }
        }
    }

    /// Whether the registry contains exactly one internally consistent
    /// recovered-WAL authority carrier and no other work.
    ///
    /// This is the only non-empty startup shape into which the post-repair
    /// Ready-Fetch census may install. The phase-vote, control, or Decision
    /// Fetch carrier
    /// remains the exclusive durable authority for its causal owner; Fetch
    /// carriers must use disjoint owners and addresses.
    pub(super) fn contains_only_exact_recovered_wal_authority(&self) -> bool {
        let Some((&address, work)) = self.entries.first_key_value() else {
            return false;
        };
        if self.entries.len() != 1 || !work.validates_at(address) {
            return false;
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.validates_at(address, work.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.validates_at(address, work.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.validates_at(address, work.digest)
            }
            _ => false,
        }
    }

    /// Classify zero or one exact WAL-owned startup carrier.
    ///
    /// `None` from this function means ambiguity (including phase and control
    /// together), while `Some(None)` is the exact zero-carrier shape.
    fn exact_recovered_wal_registry_slot(&self) -> Option<RecoveredWalRegistrySlotV1> {
        let mut signs = self
            .entries
            .iter()
            .filter_map(|(&address, work)| match &work.kind {
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::PhaseVote(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::ControlSign(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                    if work.validates_at(address) && fetch.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionFetch(address))
                }
                _ => None,
            });
        let first = signs.next().unwrap_or(RecoveredWalRegistrySlotV1::None);
        signs.next().is_none().then_some(first)
    }

    /// Preflight a complete recovered-Fetch batch beside the sole WAL authority.
    pub(super) fn preflights_recovered_fetches_alongside_wal_authority(
        &self,
        completions: &[&CertifiedFetchCompletion],
    ) -> bool {
        let Some((&sign_address, _)) = self.entries.first_key_value() else {
            return false;
        };
        let mut addresses = std::collections::BTreeSet::new();
        let mut owners = std::collections::BTreeSet::new();
        self.contains_only_exact_recovered_wal_authority()
            && completions.iter().all(|completion| {
                let address = completion.address();
                completion
                    .ready_digest()
                    .is_some_and(|digest| completion.validates(digest))
                    && address.owner != sign_address.owner
                    && !self.entries.contains_key(&address)
                    && addresses.insert(address)
                    && owners.insert(address.owner)
            })
    }

    /// Install one already-closed recovered Fetch completion.
    ///
    /// Callers must complete a whole-census empty-registry preflight first;
    /// failure still returns the exact move-only completion.
    pub(super) fn install_recovered_durable_fetch(
        &mut self,
        completion: CertifiedFetchCompletion,
    ) -> Result<(), (RegistryError, CertifiedFetchCompletion)> {
        let address = completion.address();
        let work = match ConcreteLifecycleWork::from_recovered_durable_fetch(completion) {
            Ok(work) => work,
            Err(completion) => return Err((RegistryError::CorruptWork, completion)),
        };
        let digest = work.digest();
        match self.install(address, digest, work) {
            Ok(()) => Ok(()),
            Err((error, work)) => {
                let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = work.kind
                else {
                    unreachable!("recovered Fetch installation retains its closed work kind")
                };
                Err((error, completion))
            }
        }
    }

    /// Verify complete equality between all installed startup Fetch carriers
    /// and all live coordinator Fetch rows.
    pub(super) fn exactly_covers_recovered_ready_fetches(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.exactly_covers_recovered_ready_fetches_with_extra(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
        )
    }

    /// Verify the complete post-repair startup registry: one exact recovered
    /// WAL authority plus every live Ready-Fetch row and no other carrier.
    pub(super) fn exactly_covers_recovered_ready_fetches_and_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_fetches_with_extra(coordinator, sign)
    }

    fn exactly_covers_recovered_ready_fetches_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let live_fetches = coordinator
            .records
            .values()
            .filter(|record| {
                record.work_class == LifecycleWorkClass::Fetch
                    && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && extra.address().is_none_or(|address| {
                        record.owner != address.owner || record.ordinal != address.ordinal
                    })
            })
            .collect::<Vec<_>>();
        self.entries.len() == live_fetches.len() + usize::from(extra.address().is_some())
            && self.exact_optional_recovered_wal_authority(coordinator, extra)
            && live_fetches.into_iter().all(|record| {
                if record.state != super::LifecycleState::Ready || record.physical_slots.len() != 1
                {
                    return false;
                }
                let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
                    return false;
                };
                if record.episode.consumed_slots != std::collections::BTreeSet::from([slot])
                    || !record.episode.slot_universe.contains(&slot)
                {
                    return false;
                }
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                let candidate = CandidateAdmission::new(
                    record.key,
                    record.owner.causal_root(),
                    record.work_class,
                    record.stage,
                    InitialLifecycleState::Ready,
                    metadata.reconstruction_source,
                    metadata.payload,
                    metadata.replay_authority.clone(),
                    super::PhysicalGeometry::new(
                        record
                            .physical_slots
                            .iter()
                            .map(|(id, digest)| PhysicalSlot::new(*id, *digest)),
                        record.episode.slot_universe.iter().copied(),
                    ),
                    None,
                );
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && matches!(
                            &work.kind,
                            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion)
                                if completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                        )
                })
            })
    }

    fn serve_and_producer_carrier_count(&self) -> usize {
        self.entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
                        | ConcreteLifecycleWorkKind::DurableProducerTurn(_)
                )
            })
            .count()
    }

    /// Verify exact startup coverage for every live durable Fetch, Serve, and
    /// ProducerTurn row, with no additional concrete carrier.
    pub(super) fn exactly_covers_recovered_ready_work(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.exactly_covers_recovered_ready_work_with_extra(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
        )
    }

    /// Verify exact startup coverage beside the one recovered-WAL authority.
    pub(super) fn exactly_covers_recovered_ready_work_and_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_work_with_extra(coordinator, sign)
    }

    fn exactly_covers_recovered_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        self.exactly_covers_ready_work_with_extra(coordinator, extra, None)
    }

    fn exactly_covers_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
        active_serve: Option<&TurnLease>,
    ) -> bool {
        let live = coordinator
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.work_class,
                    LifecycleWorkClass::Fetch
                        | LifecycleWorkClass::CertifiedServe
                        | LifecycleWorkClass::ProducerTurn
                ) && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && extra.address().is_none_or(|address| {
                        record.owner != address.owner || record.ordinal != address.ordinal
                    })
            })
            .collect::<Vec<_>>();
        self.entries.len() == live.len() + usize::from(extra.address().is_some())
            && self.exact_optional_recovered_wal_authority(coordinator, extra)
            && live.into_iter().all(|record| {
                let is_active_serve = active_serve.is_some_and(|lease| {
                    record.work_class == LifecycleWorkClass::CertifiedServe
                        && record.ordinal == lease.ordinal
                        && record.state == super::LifecycleState::Claimed(lease.id)
                });
                if record.work_class != LifecycleWorkClass::ProducerTurn
                    && record.state != super::LifecycleState::Ready
                    && !is_active_serve
                {
                    return false;
                }
                let Some((slot, digest)) =
                    exact_single_record_slot(record, record.work_class.capacity_class())
                else {
                    return false;
                };
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && match (&work.kind, record.work_class) {
                            (
                                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
                                LifecycleWorkClass::Fetch,
                            ) => {
                                let candidate = CandidateAdmission::new(
                                    record.key,
                                    record.owner.causal_root(),
                                    record.work_class,
                                    record.stage,
                                    InitialLifecycleState::Ready,
                                    metadata.reconstruction_source,
                                    metadata.payload,
                                    metadata.replay_authority.clone(),
                                    super::PhysicalGeometry::new(
                                        [PhysicalSlot::new(slot, digest)],
                                        [slot],
                                    ),
                                    None,
                                );
                                completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                            }
                            (
                                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve),
                                LifecycleWorkClass::CertifiedServe,
                            ) => active_serve.map_or_else(
                                || serve.matches_record(record, metadata, digest),
                                |lease| {
                                    if record.ordinal == lease.ordinal {
                                        serve
                                            .matches_claimed_record(record, metadata, digest, lease)
                                    } else {
                                        serve.matches_record(record, metadata, digest)
                                    }
                                },
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableProducerTurn(producer),
                                LifecycleWorkClass::ProducerTurn,
                            ) => producer.matches_record(record, metadata, digest),
                            _ => false,
                        }
                })
            })
    }

    fn exactly_covers_active_certified_serve_lease(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class != LifecycleWorkClass::CertifiedServe
        {
            return false;
        }
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        self.exactly_covers_ready_work_with_extra(coordinator, sign, Some(lease))
    }

    /// Prove the complete private registry and exact active Serve lease without
    /// consulting caller-supplied request material.
    pub(super) fn preflight_certified_serve_terminal_owner_state(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if !self.exactly_covers_active_certified_serve_lease(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            coordinator.records.get(&lease.ordinal),
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.records.get(&producer_ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some((serve_slot, _)), Some((producer_slot, _))) = (
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class()),
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class()),
        ) else {
            return false;
        };
        let (Some(serve_address), Some(producer_address)) = (
            ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot),
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot),
        ) else {
            return false;
        };
        let (Some(serve_work), Some(producer_work)) = (
            self.entries.get(&serve_address),
            self.entries.get(&producer_address),
        ) else {
            return false;
        };
        matches!(
            (&serve_work.kind, &producer_work.kind),
            (
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier),
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer_carrier),
            ) if serve_ordinal_pair_is_exact(serve, producer)
                && serve_metadata
                    .replay_authority
                    .same_persisted_family(&producer_metadata.replay_authority)
                && Arc::ptr_eq(
                    &serve_carrier.replay_evidence,
                    &producer_carrier.replay_evidence,
                )
        )
    }

    /// Join an exact signed request only after the complete owner-private state
    /// has independently passed preflight.
    pub(super) fn preflight_certified_serve_terminal_settlement(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        if !self.preflight_certified_serve_terminal_owner_state(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve_metadata), Some(producer_metadata)) = (
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        serve_metadata
            .replay_authority
            .exactly_matches_certified_serve_request(authenticated)
            && producer_metadata
                .replay_authority
                .exactly_matches_certified_serve_request(authenticated)
    }

    /// Close one post-fsync terminal replay family over the already-preflighted
    /// active Serve and adjacent Producer carriers.
    pub(super) fn prepare_certified_serve_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        terminal: &CertifiedServeTerminalReplayAuthorityPairV1,
    ) -> Option<PreparedCertifiedServeTerminalRegistryTransitionV1> {
        if !self.preflight_certified_serve_terminal_settlement(coordinator, lease, authenticated) {
            return None;
        }
        let producer_ordinal = *coordinator.producer_debts.get(&lease.ordinal)?;
        let serve = coordinator.records.get(&lease.ordinal)?;
        let serve_metadata = coordinator.durable_records.get(&lease.ordinal)?;
        let producer = coordinator.records.get(&producer_ordinal)?;
        let producer_metadata = coordinator.durable_records.get(&producer_ordinal)?;
        if !terminal.exactly_advances_pending_records(
            coordinator.active_context,
            serve,
            serve_metadata,
            producer,
            producer_metadata,
        ) {
            return None;
        }
        let (serve_slot, _) =
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class())?;
        let (producer_slot, _) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())?;
        let serve_address = ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)?;
        let producer_address =
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot)?;
        let serve_work = self.entries.get(&serve_address)?;
        let ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier) = &serve_work.kind
        else {
            return None;
        };
        let terminal_replay_evidence = terminal.terminal_carrier_replay_evidence()?;
        Some(PreparedCertifiedServeTerminalRegistryTransitionV1 {
            serve_address,
            producer_address,
            outcome: terminal.terminal_outcome(),
            pending_replay_evidence: Arc::clone(&serve_carrier.replay_evidence),
            terminal_replay_evidence: Arc::new(terminal_replay_evidence),
        })
    }

    fn exact_optional_recovered_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let unsupported_live = coordinator
            .records
            .values()
            .filter(|record| {
                !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !matches!(
                        record.work_class,
                        LifecycleWorkClass::Fetch
                            | LifecycleWorkClass::CertifiedServe
                            | LifecycleWorkClass::ProducerTurn
                    )
            })
            .collect::<Vec<_>>();
        match extra {
            RecoveredWalRegistrySlotV1::None => unsupported_live.is_empty(),
            RecoveredWalRegistrySlotV1::PhaseVote(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::ControlSign(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::DecisionFetch(address) => {
                if !unsupported_live.is_empty() {
                    return false;
                }
                let Some(record) = coordinator.records.get(&address.ordinal) else {
                    return false;
                };
                if record.owner != address.owner
                    || record.ordinal != address.ordinal
                    || record.work_class != LifecycleWorkClass::Fetch
                    || matches!(record.state, super::LifecycleState::Terminal(_))
                {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && fetch.matches_current_ready_record(
                                    address,
                                    work.digest,
                                    coordinator,
                                )
                    )
                })
            }
        }
    }

    /// Install one work value without overwriting an incumbent address.
    ///
    /// Failure returns the move-only value to the caller so a higher-level
    /// admission transaction can roll back without cloning physical work.
    pub(super) fn install(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
    ) -> Result<(), (RegistryError, ConcreteLifecycleWork)> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err((RegistryError::InvalidAddress, work));
        }
        if !work.validates_at(address) {
            return Err((RegistryError::CorruptWork, work));
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err((RegistryError::CausalOwnerMismatch, work));
        }
        if work.digest != expected_digest {
            return Err((RegistryError::DigestMismatch, work));
        }
        if self.entries.contains_key(&address) {
            return Err((RegistryError::Occupied, work));
        }
        self.entries.insert(address, work);
        Ok(())
    }

    /// Install exact work, invoke durable publication, and synchronously undo
    /// the installation when publication fails or unwinds.
    ///
    /// The callback cannot access this exclusively borrowed registry, so the
    /// entry installed immediately before it remains the exact rollback target.
    pub(super) fn install_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, RegistryPublicationError<E>> {
        if let Err((error, work)) = self.install(address, expected_digest, work) {
            return Err(RegistryPublicationError::Install(error, work));
        }
        let staged = StagedRegistryInstall {
            entries: &mut self.entries,
            address,
            armed: true,
        };
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => {
                let work = staged.rollback();
                debug_assert!(work.validate_exact());
                debug_assert_eq!(work.digest, expected_digest);
                Err(RegistryPublicationError::Publication(error, work))
            }
        }
    }

    /// Replace one exact address before invoking a reversible publication.
    ///
    /// The incumbent remains recoverable until the callback succeeds. A
    /// callback error removes the replacement and restores the byte-for-byte
    /// incumbent before returning the replacement to the caller. Unwinding
    /// also restores the incumbent through an RAII guard. This map is
    /// exclusively borrowed across the callback, so no other registry entry
    /// can observe the staged value or invalidate the rollback address.
    ///
    /// `Err` is valid only when the callback proves that its external target
    /// did not commit. A durability-ambiguous dequeue or publication must
    /// instead cross the process fail-stop boundary; restoring this volatile
    /// map cannot undo an external transition.
    /// This generic seam accepts pending adapter work only. Certified-Fetch
    /// completion must use the specialized conversion below, which moves the
    /// incumbent binding into its closed carrier rather than constructing an
    /// independent replacement proof.
    pub(super) fn replace_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_incumbent_digest: LifecycleDigest,
        expected_replacement_digest: LifecycleDigest,
        replacement: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<(T, ConcreteLifecycleWork), RegistryReplacementError<E>> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::InvalidAddress,
                replacement,
            ));
        }
        if !replacement.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !replacement.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != replacement.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if replacement.digest != expected_replacement_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }
        let Some(incumbent) = self.entries.get(&address) else {
            return Err(RegistryReplacementError::Validation(
                RegistryError::Missing,
                replacement,
            ));
        };
        if !incumbent.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !incumbent.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != incumbent.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if incumbent.digest != expected_incumbent_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }

        let incumbent = self
            .entries
            .insert(address, replacement)
            .expect("validated replacement address retains its incumbent");
        let staged = StagedRegistryReplacement {
            entries: &mut self.entries,
            address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => Ok((published, staged.commit())),
            Err(error) => {
                let replacement = staged.rollback();
                debug_assert!(replacement.validate_exact());
                debug_assert_eq!(replacement.digest, expected_replacement_digest);
                Err(RegistryReplacementError::Publication(error, replacement))
            }
        }
    }

    /// Prepare an exact incumbent-to-completion conversion without mutation.
    ///
    /// The sealed selector capability is borrowed only for equality validation.
    /// It is deliberately not stored in the returned token: successful
    /// conversion moves the incumbent registry binding and never mints or
    /// retains a second causal proof. Raw response, responder, hash, queue
    /// identity, and pending-binding inputs are not accepted here.
    pub(super) fn prepare_certified_fetch_completion(
        &mut self,
        location: CertifiedFetchWaitingLocation,
        authority: CertifiedFetchCompletionAuthority<'_>,
    ) -> Result<PreparedCertifiedFetchCompletion<'_>, CertifiedFetchCompletionError> {
        let ingress_identity = authority.ingress_identity();
        let request_hash = authority.request_hash();
        let response_hash = authority.response_hash();
        let authenticated_responder = authority.authenticated_responder();
        let authenticated_response = authority.authenticated_response();
        let candidate_pending = authority.candidate_pending();
        let address = location.address();
        if ConcreteWorkAddress::new(location.owner, location.ordinal, location.slot)
            != Some(address)
        {
            return Err(CertifiedFetchCompletionError::InvalidLocation);
        }
        if ingress_identity.physical_admission_ordinal() == 0
            || !ingress_identity_matches_round(
                ingress_identity,
                authenticated_response.manifest.round,
            )
        {
            return Err(CertifiedFetchCompletionError::InvalidQueueIdentity);
        }
        if authenticated_response.request_hash != request_hash
            || HashOf::new(authenticated_response) != response_hash
        {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }

        let incumbent = self
            .entries
            .get(&address)
            .ok_or(CertifiedFetchCompletionError::MissingIncumbent)?;
        if !incumbent.validates_at(address) {
            return Err(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
        } = &incumbent.kind
        else {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if location.owner.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if authority.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if incumbent.digest != location.incumbent_digest {
            return Err(CertifiedFetchCompletionError::IncumbentDigestMismatch);
        }
        if candidate_pending != incumbent_pending
            || !candidate_pending.exactly_binds_adapter_effect(incumbent_effect)
        {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if !fetch_effect_matches_response(incumbent_effect, authenticated_response) {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }
        let replay_origin = AuthenticatedCertifiedFetchReplayOriginV1::from_completion_authority(
            &authority,
            incumbent_effect,
        )
        .ok_or(CertifiedFetchCompletionError::InvalidReplayEvidence)?;

        Ok(PreparedCertifiedFetchCompletion {
            registry: self,
            location,
            ingress_identity,
            request_hash,
            response_hash,
            response_round: authenticated_response.manifest.round,
            response_subject: authenticated_response.manifest.subject,
            response_manifest_hash: HashOf::new(&authenticated_response.manifest),
            authenticated_responder: authenticated_responder.clone(),
            replay_origin,
        })
    }

    /// Prepare execution of one exact closed certified-Fetch completion.
    ///
    /// The lease must name the completion's immutable owner, record ordinal,
    /// sole physical slot, and installed response digest, and it must retain
    /// the coordinator's exact independent `FetchBody` stage. No row is taken
    /// or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_certified_fetch_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<PreparedCertifiedFetchExecution<'_>, CertifiedFetchExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Fetch
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(lease.work_class().capacity_class())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(CertifiedFetchExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated certified-Fetch execution address remains present");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            return Err(CertifiedFetchExecutionError::WrongWorkKind);
        };
        let AdapterEffect::FetchBody {
            certificate: Some(certificate),
            ..
        } = &completion.incumbent_effect
        else {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        };
        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        if certified_fetch_lifecycle_key(
            active_context,
            certificate.round,
            certificate.proposal_round,
            certificate.subject,
            certificate.phase,
            certificate.execution_commitment,
        ) != Some(lease.key())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }
        if !completion.validates(work.digest) {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        }

        Ok(PreparedCertifiedFetchExecution {
            registry: self,
            address,
        })
    }

    /// Prepare execution of one exact closed durable Store carrier.
    ///
    /// In addition to the address and digest checks shared by all registry
    /// leases, this replays the authenticated adapter projection under the
    /// supplied height context. The projected semantic key, causal owner, and
    /// complete one-slot physical geometry must be identical to the claimed
    /// Store lease. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_store_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableStoreExecution<'_>, DurableStoreExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Store
            || lease.key().phase() != LifecyclePhase::Store
            || lease.stage().kind() != LifecycleStageKind::StoreBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Store.capacity_class())
        {
            return Err(DurableStoreExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableStoreExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Store execution address remains present");
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            return Err(DurableStoreExecutionError::WrongWorkKind);
        };
        if !store.validates(work.digest) {
            return Err(DurableStoreExecutionError::InvalidStoreShape);
        }

        let candidate = store
            .project_candidate(verified)
            .map_err(DurableStoreExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&store.durable_receipt)
            .ok_or(DurableStoreExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableStoreExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Store
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableStoreExecutionError::InvalidProjection);
        }

        Ok(PreparedDurableStoreExecution {
            registry: self,
            address,
        })
    }

    /// Prepare execution of one exact closed durable Validate carrier.
    ///
    /// The lease, installed carrier, verified projection, and normalized
    /// physical geometry must all describe the same independent one-slot
    /// `ValidateBody` work. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableValidateExecution<'_>, DurableValidateExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Validate
            || lease.key().phase() != LifecyclePhase::Validate
            || lease.stage().kind() != LifecycleStageKind::ValidateBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(DurableValidateExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Validate execution address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return Err(DurableValidateExecutionError::WrongWorkKind);
        };
        if !validate.validates(work.digest) {
            return Err(DurableValidateExecutionError::InvalidValidateShape);
        }

        let candidate = validate
            .project_candidate(verified)
            .map_err(DurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&validate.durable_receipt)
            .ok_or(DurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || projected_universe.len() != 1
            || projected_consumed.len() != 1
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableValidateExecutionError::InvalidProjection);
        }

        Ok(PreparedDurableValidateExecution {
            registry: self,
            address,
            lifecycle_key: lease.key(),
            lifecycle_stage: lease.stage(),
        })
    }

    /// Classify one exact Ready Validate carrier without granting scheduler authority.
    ///
    /// The caller supplies coordinator-owned address and digest coordinates.
    /// Successful classification proves only the process-local carrier shape;
    /// the coordinator must still bind it into its complete Ready census.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn classify_ready_validate_carrier(
        &self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ReadyValidateCarrierSeal, ReadyValidateCarrierError> {
        let work = self
            .entries
            .get(&address)
            .ok_or(ReadyValidateCarrierError::Registry(RegistryError::Missing))?;
        if !work.validates_at(address) {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != expected_digest {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::DurableValidateBody(validate)
                if validate.validates(expected_digest) =>
            {
                let payload = durable_validate_body_payload(&validate.durable_receipt)
                    .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                Ok(ReadyValidateCarrierSeal {
                    address,
                    digest: expected_digest,
                    kind: ReadyValidateCarrierKind::ExecuteBody,
                    payload,
                })
            }
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                if completion.validates(expected_digest) =>
            {
                match (
                    completion.outcome.validated_receipt(),
                    completion.outcome.rejection_identity(),
                    completion.outcome.missing_merge_sidecar(),
                ) {
                    (Some(receipt), None, None)
                        if validate_validated_receipt_authority(&completion.incumbent, receipt)
                            .is_ok() =>
                    {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::ValidatedCompletion,
                            payload,
                        })
                    }
                    (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::RejectedCompletion,
                            payload,
                        })
                    }
                    _ => Err(ReadyValidateCarrierError::InvalidCarrier),
                }
            }
            ConcreteLifecycleWorkKind::PendingAdapter { .. }
            | ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
            | ConcreteLifecycleWorkKind::DurableStoreBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateCompletion(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => {
                Err(ReadyValidateCarrierError::WrongWorkKind)
            }
        }
    }

    /// Prepare execution of one exact Ready durable Validate completion.
    ///
    /// The claimed lease must retain the original independent Validate
    /// lifecycle identity while its sole physical slot names the installed
    /// outcome-bound replacement digest. The retained incumbent is replayed
    /// through authenticated projection, and the complete closed outcome is
    /// revalidated before an exclusive, drop-inert registry borrow is issued.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_ready_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedReadyDurableValidateExecution<'_>, ReadyDurableValidateExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Validate
            || lease.key().phase() != LifecyclePhase::Validate
            || lease.stage().kind() != LifecycleStageKind::ValidateBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(ReadyDurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated Ready Validate completion address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind else {
            return Err(ReadyDurableValidateExecutionError::WrongWorkKind);
        };
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        let Some(candidate_statement) = completion.incumbent.pending.candidate_statement() else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        if !completion.validates(work.digest)
            || completion.address != address
            || completion.incumbent.address != address
            || candidate_statement.context_id() != round.context_id
            || candidate_statement.proposal_round() != *round
            || candidate_statement.subject() != Some(*subject)
            || completion.incumbent.durable_receipt.context_id() != round.context_id
            || completion.incumbent.durable_receipt.round() != *round
            || completion.incumbent.durable_receipt.subject() != *subject
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || completion.outcome.durable_body() != &completion.incumbent.durable_receipt
        {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        }

        let outcome_kind = match (
            completion.outcome.validated_receipt(),
            completion.outcome.rejection_identity(),
            completion.outcome.missing_merge_sidecar(),
        ) {
            (Some(receipt), None, None)
                if receipt.durable() == &completion.incumbent.durable_receipt
                    && receipt.durable().manifest_hash()
                        == completion.incumbent.expected_manifest_hash
                    && validate_validated_receipt_authority(&completion.incumbent, receipt)
                        .is_ok() =>
            {
                ReadyDurableValidateOutcomeKind::Validated
            }
            (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                ReadyDurableValidateOutcomeKind::Rejected
            }
            _ => return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape),
        };
        let expected_reservation = match outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => None,
            ReadyDurableValidateOutcomeKind::Rejected => Some(CapacityClass::Consensus),
        };
        if lease
            .output_reservation()
            .map(|reservation| reservation.class())
            != expected_reservation
        {
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }

        let candidate = completion
            .incumbent
            .project_candidate(verified)
            .map_err(ReadyDurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .ok_or(ReadyDurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| ReadyDurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let incumbent_slots = BTreeMap::from([(slot, completion.incumbent_digest)]);
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != incumbent_slots
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(ReadyDurableValidateExecutionError::InvalidProjection);
        }

        Ok(PreparedReadyDurableValidateExecution {
            registry: self,
            address,
            outcome_kind,
            lease: lease.clone(),
        })
    }

    /// Reattach one executed Validate outcome only if its original closed row
    /// remains byte-for-byte authoritative at the exact address and digest.
    ///
    /// Failure returns the complete move-only execution token. Success only
    /// establishes a new exclusive borrow; neither path changes the registry.
    // The sole outer consumer joins this reattachment with typed same-address
    // carrier installation and the coordinator Ready replacement. Waiting,
    // Ready, and physical carriers are excluded from the lifecycle ledger, so
    // that volatile cut deliberately performs no ledger rewrite.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn reattach_durable_validate_execution(
        &mut self,
        executed: ExecutedDurableValidateExecution,
    ) -> Result<
        PreparedDurableValidateCompletion<'_>,
        (
            DurableValidateExecutionError,
            ExecutedDurableValidateExecution,
        ),
    > {
        let request = &executed.request;
        let exact = (|| {
            if ConcreteWorkAddress::new(
                request.address.owner,
                request.address.ordinal,
                request.address.slot,
            ) != Some(request.address)
            {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::InvalidAddress,
                ));
            }
            let work = self.entries.get(&request.address).ok_or(
                DurableValidateExecutionError::Registry(RegistryError::Missing),
            )?;
            if !work.validates_at(request.address) {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork,
                ));
            }
            if request.address.owner.causal_root() != work.causal_root() {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CausalOwnerMismatch,
                ));
            }
            if work.digest != request.incumbent_digest {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::DigestMismatch,
                ));
            }
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                return Err(DurableValidateExecutionError::WrongWorkKind);
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            };
            if validate.address != request.address
                || *tag != request.tag
                || *round != request.round
                || *subject != request.subject
                || validate.durable_receipt != request.durable_receipt
                || validate.expected_manifest_hash != request.expected_manifest_hash
                || !validate
                    .pending
                    .exactly_binds_adapter_effect(&validate.effect)
                || validate.pending.causal_lifecycle_key() != &request.causal_lifecycle_key
                || validate.pending.candidate_statement() != request.candidate_statement
                || request.lifecycle_key.phase() != LifecyclePhase::Validate
                || request.lifecycle_stage.kind() != LifecycleStageKind::ValidateBody
                || request.lifecycle_stage.predecessor_scope() != PredecessorScope::Independent
            {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            }
            if executed.outcome.durable_body() != &request.durable_receipt {
                return Err(DurableValidateExecutionError::InvalidValidationReceipt);
            }
            if let Some(receipt) = executed.outcome.validated_receipt() {
                validate_validated_receipt_authority(validate, receipt)?;
            }
            Ok(())
        })();
        if let Err(error) = exact {
            return Err((error, executed));
        }

        Ok(PreparedDurableValidateCompletion {
            _registry: self,
            executed,
        })
    }

    /// Reattach the complete executed dispatch and its exact wake authority.
    ///
    /// This is the sole registry entry to volatile Validate completion. Every
    /// failure returns the original move-only dispatch and leaves the map
    /// untouched; success retains the exclusive borrow in a sealed preflight.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_executed_durable_validate_completion(
        &mut self,
        dispatch: ExecutedDurableValidateDispatch,
    ) -> Result<
        PreparedExecutedDurableValidateCompletion<'_>,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        let ExecutedDurableValidateDispatch { executed, wake } = dispatch;
        let prepared = match self.reattach_durable_validate_execution(executed) {
            Ok(prepared) => prepared,
            Err((error, executed)) => {
                return Err((
                    DurableValidateCompletionPublicationError::Registry(
                        DurableValidateCompletionConversionError::Execution(error),
                    ),
                    ExecutedDurableValidateDispatch { executed, wake },
                ));
            }
        };
        let PreparedDurableValidateCompletion {
            _registry: registry,
            executed,
        } = prepared;
        let dispatch = ExecutedDurableValidateDispatch { executed, wake };
        let request = &dispatch.executed.request;
        let expected_source = durable_validation_wait_source_for_request(request);
        if dispatch.wake.wait_token.source() != expected_source
            || dispatch.wake.wait_token.observed_generation() == u64::MAX
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidWakeAuthority,
                ),
                dispatch,
            ));
        }
        let Some(outcome_kind) = durable_validate_outcome_kind(dispatch.outcome()) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        let replacement_digest = durable_validate_completion_digest(
            request.incumbent_digest,
            request.expected_manifest_hash,
            dispatch.outcome(),
        );
        if matches!(
            outcome_kind,
            DurableValidateOutcomeKind::Validated | DurableValidateOutcomeKind::Rejected
        ) && replacement_digest.is_none_or(|digest| digest == request.incumbent_digest)
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidReplacementDigest,
                ),
                dispatch,
            ));
        }
        if outcome_kind == DurableValidateOutcomeKind::DeferredMergeSidecar
            && replacement_digest.is_some()
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let Some(payload) = durable_validate_body_payload(&request.durable_receipt) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        if !super::body_pipeline_transition::durable_validate_payload_is_exact(
            request.lifecycle_key,
            payload,
        ) {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let authority = DurableValidateCompletionAuthority {
            address: request.address,
            incumbent_digest: request.incumbent_digest,
            replacement_digest,
            wait_token: dispatch.wake.wait_token,
            outcome_kind,
            lifecycle_key: request.lifecycle_key,
            lifecycle_stage: request.lifecycle_stage,
            payload,
        };
        Ok(PreparedExecutedDurableValidateCompletion {
            registry,
            dispatch,
            authority,
        })
    }

    /// Borrow the still-pending adapter effect advertised by one lease slot.
    /// Closed carriers fail rather than re-executing their retained effects.
    pub(super) fn borrow_for_lease(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<&AdapterEffect, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated lease address remains present");
        if !work.is_pending_adapter() {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(work.effect())
    }

    /// Consume the complete still-pending adapter work advertised by one lease slot once.
    ///
    /// Returning the sealed pending authority together with the effect is
    /// essential: execution may report `Blocked` or `Replenished`, in which
    /// case a later atomic settlement must be able to restore the incumbent
    /// without reminting its causal binding. Closed-carrier consumption
    /// remains unavailable until its typed executor lands.
    pub(super) fn take_for_lease(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        if !self
            .entries
            .get(&address)
            .expect("validated lease address remains present")
            .is_pending_adapter()
        {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated lease address remains present"))
    }

    /// Remove only the exact digest installed by a failed outer transaction.
    pub(super) fn rollback_exact(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated rollback address remains present"))
    }

    fn validated_lease_address(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteWorkAddress, RegistryError> {
        let address = ConcreteWorkAddress::new(lease.owner, lease.ordinal, slot)
            .ok_or(RegistryError::InvalidAddress)?;
        let expected_digest = lease
            .physical_slots
            .get(&slot)
            .ok_or(RegistryError::DigestMismatch)?;
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != *expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(address)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn certified_serve_and_producer_carrier_counts(&self) -> (usize, usize) {
        self.entries
            .values()
            .fold((0, 0), |counts, work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_) => (counts.0 + 1, counts.1),
                ConcreteLifecycleWorkKind::DurableProducerTurn(_) => (counts.0, counts.1 + 1),
                _ => counts,
            })
    }

    #[cfg(test)]
    pub(super) fn one_certified_serve_pair_shares_replay_family(&self) -> bool {
        let serves = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => Some(serve),
                _ => None,
            })
            .collect::<Vec<_>>();
        let producers = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => Some(producer),
                _ => None,
            })
            .collect::<Vec<_>>();
        let ([serve], [producer]) = (serves.as_slice(), producers.as_slice()) else {
            return false;
        };
        Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)
    }

    #[cfg(test)]
    /// Remove one exact Serve carrier to exercise owner-private census faults.
    pub(super) fn remove_one_certified_serve_carrier_for_test(&mut self) -> bool {
        let address = self.entries.iter().find_map(|(address, work)| {
            matches!(
                &work.kind,
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            )
            .then_some(*address)
        });
        address.is_some_and(|address| self.entries.remove(&address).is_some())
    }

    #[cfg(test)]
    pub(super) fn exactly_contains(
        &self,
        address: ConcreteWorkAddress,
        effect: &AdapterEffect,
    ) -> bool {
        self.entries
            .get(&address)
            .is_some_and(|work| work.validates_at(address) && work.effect() == effect)
    }
}

#[allow(dead_code)]
impl<'a> PreparedCertifiedFetchExecution<'a> {
    /// Return the exact reducer tag and authenticated manifest accepted by the
    /// direct adapter preview. Both are derived from the installed completion;
    /// neither can be supplied independently by the caller.
    pub(super) fn adapter_preview_inputs(&self) -> (EventTag, &wire::PayloadManifest) {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        completion
            .replay_evidence
            .adapter_preview_inputs(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            )
            .expect("prepared certified-Fetch completion retains exact durable replay inputs")
    }

    /// Borrow the durable body proof retained by the exact completion.
    ///
    /// The receipt remains nested and non-decomposable; callers may use it only
    /// for the future body-catalog equality check and canonical reload.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        &completion.durable_receipt
    }

    /// Seal the ordinal-free pending binding for the exact Store effect emitted
    /// by the direct adapter preview.
    ///
    /// This pure projection checks the certified predecessor, exact tag/round/
    /// subject, inherited candidate statement, unchanged causal key, and a new
    /// physical effect identity. Neither success nor failure changes the
    /// installed completion.
    pub(super) fn seal_store_successor(
        self,
        successor: &AdapterEffect,
    ) -> Result<PreparedCertifiedFetchStoreSuccessor<'a>, CertifiedFetchExecutionError> {
        let (
            store_effect,
            store_pending,
            store_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared certified-Fetch completion remains installed");
            let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            if !completion.validates(work.digest) {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            }
            let Some(store_pending) = completion
                .incumbent_pending
                .project_certified_fetch_store_successor(&completion.incumbent_effect, successor)
            else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            if store_pending.causal_lifecycle_key()
                != completion.incumbent_pending.causal_lifecycle_key()
                || store_pending.candidate_statement()
                    != completion.incumbent_pending.candidate_statement()
                || store_pending.exact_effect_identity()
                    == completion.incumbent_pending.exact_effect_identity()
                || !store_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            }
            let store_digest = digest_from_hash(store_pending.exact_effect_identity());
            let durable_body = completion.durable_receipt.clone();
            let Some(ready_projection) = completion.replay_evidence.project_durable_ready_fetch(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            let expected_manifest_hash = ready_projection.expected_manifest_hash();
            let Some(replay_evidence) = completion.replay_evidence.project_store(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
                successor,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            (
                successor.clone(),
                store_pending,
                store_digest,
                durable_body,
                expected_manifest_hash,
                replay_evidence,
            )
        };

        Ok(PreparedCertifiedFetchStoreSuccessor {
            _registry: self.registry,
            _completion_address: self.address,
            _store_effect: store_effect,
            _store_digest: store_digest,
            _store_pending: store_pending,
            _durable_body: durable_body,
            _expected_manifest_hash: expected_manifest_hash,
            _replay_evidence: replay_evidence,
        })
    }
}

#[allow(dead_code)]
impl<'a> PreparedDurableStoreExecution<'a> {
    fn durable_store(&self) -> &DurableStoreBody {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Store carrier remains installed");
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            unreachable!("prepared durable Store execution retains its closed carrier")
        };
        store
    }

    /// Return the exact reducer coordinates accepted by the direct
    /// `BodyStored` adapter preview.
    pub(super) fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = &self.durable_store().effect
        else {
            unreachable!("prepared durable Store carrier retains its Store effect")
        };
        (*tag, *round, *subject)
    }

    /// Borrow the exact post-fsync body receipt retained by the Store carrier.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_store().durable_receipt
    }

    /// Return the manifest hash transferred independently from the parent response.
    pub(super) fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.durable_store().expected_manifest_hash
    }

    /// Seal the ordinal-free pending binding for the exact Validate effect
    /// emitted by the direct `BodyStored` adapter preview.
    ///
    /// The Store's full inherited candidate statement and causal root must be
    /// unchanged, while the concrete effect identity must be replaced by the
    /// exact Validate identity. Neither success nor failure changes the Store
    /// row retained under the exclusive registry borrow.
    pub(super) fn seal_validate_successor(
        self,
        successor: &AdapterEffect,
    ) -> Result<PreparedDurableStoreValidateSuccessor<'a>, DurableStoreExecutionError> {
        let (
            validate_effect,
            validate_pending,
            validate_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Store carrier remains installed");
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
                return Err(DurableStoreExecutionError::InvalidStoreShape);
            };
            if !store.validates(work.digest) {
                return Err(DurableStoreExecutionError::InvalidStoreShape);
            }
            let Some(validate_pending) = store
                .pending
                .project_store_validate_successor(&store.effect, successor)
            else {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            };
            if validate_pending.causal_lifecycle_key() != store.pending.causal_lifecycle_key()
                || super::CausalRoot::new(digest_from_hash(validate_pending.causal_lifecycle_key()))
                    != store.address.owner.causal_root()
                || validate_pending.candidate_statement() != store.pending.candidate_statement()
                || validate_pending.exact_effect_identity() == store.pending.exact_effect_identity()
                || !validate_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            let validate_digest = digest_from_hash(validate_pending.exact_effect_identity());
            if validate_digest == work.digest {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            let Some(replay_evidence) = store.replay_evidence.project_validate(
                &store.effect,
                &store.durable_receipt,
                successor,
                &validate_pending,
            ) else {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            };
            (
                successor.clone(),
                validate_pending,
                validate_digest,
                store.durable_receipt.clone(),
                store.expected_manifest_hash,
                replay_evidence,
            )
        };

        Ok(PreparedDurableStoreValidateSuccessor {
            _registry: self.registry,
            _store_address: self.address,
            _validate_effect: validate_effect,
            _validate_digest: validate_digest,
            _validate_pending: validate_pending,
            _durable_body: durable_body,
            _expected_manifest_hash: expected_manifest_hash,
            _replay_evidence: replay_evidence,
        })
    }
}

fn sealed_successor_parent<'a>(
    registry: &'a ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    lease: &TurnLease,
) -> Result<&'a ConcreteLifecycleWork, SealedBodySuccessorProjectionError> {
    let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    };
    if lease.physical_slots().len() != 1
        || slot.capacity_class() != Some(CapacityClass::Effect)
        || ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot) != Some(address)
    {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    }
    let work = registry
        .entries
        .get(&address)
        .ok_or(SealedBodySuccessorProjectionError::ForeignParent)?;
    if !work.validates_at(address) || work.digest != digest {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    }
    Ok(work)
}

fn sealed_successor_candidate_has_exact_geometry(
    candidate: &CandidateAdmission,
    expected_class: LifecycleWorkClass,
    expected_digest: LifecycleDigest,
) -> bool {
    let expected_slot = PhysicalSlotId::for_capacity(expected_class.capacity_class(), 0);
    candidate
        .physical_geometry
        .normalized()
        .is_ok_and(|(slots, universe, consumed)| {
            slots.len() == 1
                && slots.get(&expected_slot) == Some(&expected_digest)
                && universe.len() == 1
                && universe.contains(&expected_slot)
                && consumed == universe
        })
}

#[allow(dead_code)]
impl PreparedCertifiedFetchStoreSuccessor<'_> {
    /// Project the exact Store candidate while retaining its Fetch registry cut.
    ///
    /// The lease supplies only coordinator ownership coordinates. Effect,
    /// pending binding, durable frame, replay authority, and child digest stay
    /// sealed in this token and are revalidated before projection.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        let work = sealed_successor_parent(self._registry, self._completion_address, lease)?;
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        };
        let ready_projection = completion.replay_evidence.project_durable_ready_fetch(
            &completion.incumbent_effect,
            &completion.incumbent_pending,
            &completion.durable_receipt,
        );
        if !completion.validates(work.digest)
            || completion.address != self._completion_address
            || completion.durable_receipt != self._durable_body
            || ready_projection
                .as_ref()
                .map(DurableCertifiedFetchReplayProjectionV1::expected_manifest_hash)
                != Some(self._expected_manifest_hash)
            || self._durable_body.manifest_hash() != self._expected_manifest_hash
            || !self
                ._store_pending
                .exactly_binds_adapter_effect(&self._store_effect)
            || super::CausalRoot::new(digest_from_hash(self._store_pending.causal_lifecycle_key()))
                != self._completion_address.owner.causal_root()
            || digest_from_hash(self._store_pending.exact_effect_identity()) != self._store_digest
            || !self
                ._replay_evidence
                .exactly_matches_store(&self._store_effect, &self._durable_body)
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        let candidate = self
            ._replay_evidence
            .project_sealed_store_successor_candidate(
                SealedBodySuccessorProjectionPermit::new(),
                verified,
                &self._store_effect,
                &self._durable_body,
                &self._store_pending,
            )
            .map_err(SealedBodySuccessorProjectionError::Projection)?;
        if candidate.causal_root != self._completion_address.owner.causal_root()
            || candidate.payload
                != durable_validate_body_payload(&self._durable_body)
                    .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)?
            || !sealed_successor_candidate_has_exact_geometry(
                &candidate,
                LifecycleWorkClass::Store,
                self._store_digest,
            )
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        Ok(candidate)
    }
}

#[allow(dead_code)]
impl PreparedDurableStoreValidateSuccessor<'_> {
    /// Project the exact Validate candidate while retaining its Store registry cut.
    ///
    /// The candidate is derived only from the Store-projected pending binding,
    /// independently transferred manifest hash, exact durable frame, and
    /// certified replay evidence already owned by this move-only token.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        let work = sealed_successor_parent(self._registry, self._store_address, lease)?;
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        };
        if !store.validates(work.digest)
            || store.address != self._store_address
            || store.durable_receipt != self._durable_body
            || store.expected_manifest_hash != self._expected_manifest_hash
            || self._durable_body.manifest_hash() != self._expected_manifest_hash
            || !self
                ._validate_pending
                .exactly_binds_adapter_effect(&self._validate_effect)
            || super::CausalRoot::new(digest_from_hash(
                self._validate_pending.causal_lifecycle_key(),
            )) != self._store_address.owner.causal_root()
            || digest_from_hash(self._validate_pending.exact_effect_identity())
                != self._validate_digest
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        let replay_evidence =
            DurableValidateReplayEvidenceV1::certified(self._replay_evidence.clone());
        let candidate = replay_evidence
            .project_sealed_validate_successor_candidate(
                SealedBodySuccessorProjectionPermit::new(),
                verified,
                &self._validate_effect,
                &self._durable_body,
                &self._validate_pending,
            )
            .map_err(SealedBodySuccessorProjectionError::Projection)?;
        if candidate.causal_root != self._store_address.owner.causal_root()
            || candidate.payload
                != durable_validate_body_payload(&self._durable_body)
                    .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)?
            || !sealed_successor_candidate_has_exact_geometry(
                &candidate,
                LifecycleWorkClass::Validate,
                self._validate_digest,
            )
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        Ok(candidate)
    }
}

// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN
#[allow(dead_code)]
impl<'registry> PreparedReadyDurableValidateExecution<'registry> {
    fn completion(&self) -> Option<&DurableValidateCompletion> {
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind else {
            return None;
        };
        completion.validates(work.digest).then_some(completion)
    }

    /// Return only the closed reducer-level outcome discriminator.
    pub(crate) const fn outcome_kind(&self) -> ReadyDurableValidateOutcomeKind {
        self.outcome_kind
    }

    fn matches_exact_lease(&self, lease: &TurnLease) -> bool {
        &self.lease == lease
    }

    fn matches_exact_durable_receipt(&self, receipt: &DurableBodyReceipt) -> bool {
        self.completion()
            .is_some_and(|completion| &completion.incumbent.durable_receipt == receipt)
    }

    fn validated_authority(&self) -> Option<ReadyValidatedAdapterAuthority<'_>> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Validated {
            return None;
        }
        let completion = self.completion()?;
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return None;
        };
        let receipt = completion.outcome.validated_receipt()?;
        if completion.outcome.rejection_identity().is_some()
            || completion.outcome.missing_merge_sidecar().is_some()
            || receipt.durable() != &completion.incumbent.durable_receipt
            || receipt.durable().manifest_hash() != completion.incumbent.expected_manifest_hash
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || validate_validated_receipt_authority(&completion.incumbent, receipt).is_err()
        {
            return None;
        }
        Some(ReadyValidatedAdapterAuthority {
            tag: *tag,
            round: *round,
            subject: *subject,
            receipt,
        })
    }

    fn validate_sign_predecessor_authority(
        &self,
    ) -> Option<ReadyValidateSignPredecessorAuthority<'_>> {
        self.validated_authority()?;
        let completion = self.completion()?;
        Some(ReadyValidateSignPredecessorAuthority {
            effect: &completion.incumbent.effect,
            pending: &completion.incumbent.pending,
            _linearity: ReadyValidateSignPredecessorLinearity,
        })
    }

    fn rejected_authority(&self) -> Option<ReadyRejectedAdapterAuthority<'_>> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Rejected {
            return None;
        }
        let completion = self.completion()?;
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return None;
        };
        if completion.outcome.validated_receipt().is_some()
            || completion.outcome.rejection_identity()
                != Some(&BodyValidationRejectionIdentity::Rejected)
            || completion.outcome.missing_merge_sidecar().is_some()
            || completion.outcome.durable_body() != &completion.incumbent.durable_receipt
            || completion.outcome.durable_body().manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
        {
            return None;
        }
        Some(ReadyRejectedAdapterAuthority {
            tag: *tag,
            round: *round,
            subject: *subject,
            receipt: completion.outcome.durable_body(),
        })
    }

    /// Consume this exact registry cut into the adapter's sealed direct preview.
    ///
    /// The fixed join exposes no generic callback or raw receipt result. Every
    /// successful and failed classification retains the complete registry
    /// authority, so the operation is single-use and drop-inert.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_adapter_preview<'adapter>(
        self,
        adapter: &'adapter mut SumeragiV2Adapter,
    ) -> Result<
        PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
        ReadyDurableValidateAdapterPreviewError<'registry>,
    > {
        let adapter_preview = match self.outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => {
                let Some(authority) = self.validated_authority() else {
                    return Err(ReadyDurableValidateAdapterPreviewError {
                        _registry: self,
                        _failure: ReadyDurableValidateAdapterPreviewFailure::RegistryAuthority,
                    });
                };
                adapter.prepare_sealed_ready_durable_validate_succeeded(authority)
            }
            ReadyDurableValidateOutcomeKind::Rejected => {
                let Some(authority) = self.rejected_authority() else {
                    return Err(ReadyDurableValidateAdapterPreviewError {
                        _registry: self,
                        _failure: ReadyDurableValidateAdapterPreviewFailure::RegistryAuthority,
                    });
                };
                adapter.prepare_sealed_ready_durable_validate_failed(authority)
            }
        };

        match adapter_preview {
            Ok(adapter_preview) => match adapter_preview.preflight_publication() {
                Ok(_adapter) => Ok(PreparedReadyDurableValidateAdapterPreview {
                    _registry: self,
                    _adapter,
                }),
                Err(error) => Err(ReadyDurableValidateAdapterPreviewError {
                    _registry: self,
                    _failure: ReadyDurableValidateAdapterPreviewFailure::Adapter(error),
                }),
            },
            Err(error) => Err(ReadyDurableValidateAdapterPreviewError {
                _registry: self,
                _failure: ReadyDurableValidateAdapterPreviewFailure::Adapter(error),
            }),
        }
    }
}
// READY_DURABLE_VALIDATE_ADAPTER_JOIN_END

// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_BEGIN
impl<'registry> PreparedReadyDurableValidateExecution<'registry> {
    /// Detach the exact Ready/validated carrier for the fixed recovered-WAL join.
    ///
    /// Rejected completions and malformed carriers are returned unchanged. A
    /// successfully detached cut restores the byte-for-byte carrier on drop
    /// until its recovered vote consumes the predecessor binding.
    #[allow(clippy::result_large_err)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn into_recovered_wal_validate_registry_cut(
        self,
    ) -> Result<RecoveredWalValidateRegistryCut<'registry>, Self> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Validated
            || self.completion().is_none()
        {
            return Err(self);
        }
        let address = self.address;
        let Some(work) = self.registry.entries.remove(&address) else {
            return Err(self);
        };
        let Self {
            registry,
            address: _,
            outcome_kind: _,
            lease: _,
        } = self;
        Ok(RecoveredWalValidateRegistryCut {
            registry: Some(registry),
            address,
            work: Some(work),
        })
    }
}
// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_END

/// Reconstruct one exact recovered Validate parent directly from durable storage.
///
/// This is the production restart-only replacement for the scheduler/lease
/// preparation path used by live work. LedgerV1 supplies the immutable owner
/// and ordinal, the body store transfers one exact revalidated marker, and the
/// runtime consumes the authenticated WAL vote into its successor. The holder
/// remains the only concrete-registry owner and returns only the existing
/// opaque authenticated repair plus its exact opened ledger wrapper.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::result_large_err, clippy::too_many_lines)]
pub(super) fn reconstruct_recovered_wal_validate_parent<'registry, 'body>(
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    verified: &VerifiedHeightContext,
    body_store: &'body mut V2BodyStore,
    ledger_root: &Path,
    recovered: RecoveredWalVoteSign,
) -> Result<
    (
        OpenedRecoveredWalValidateLedger,
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ),
    RecoveredWalParentFactoryError<'body>,
> {
    let context = projection::lifecycle_context(verified.context());
    let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(ledger_root, context) {
        Ok(opened) => opened,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::LedgerOpen {
                    _error: error,
                    _recovered: recovered,
                },
            });
        }
    };
    let ledger = OpenedRecoveredWalValidateLedger { store, opened };
    let body = match body_store.detach_recovered_validated_parent(&recovered) {
        Ok(body) => body,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::BodyMarker {
                    _error: error,
                    _ledger: ledger,
                    _recovered: recovered,
                },
            });
        }
    };
    if !body.exactly_matches_vote(&recovered) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let Some(parent) = ledger
        .opened
        .authenticate_recovered_wal_validate_parent(&recovered)
    else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    };
    if !body.exactly_matches_ledger_parent(context, &parent) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let successor = match reconstruct_recovered_wal_vote_successor(&parent, recovered) {
        Ok(successor) => successor,
        Err(recovered) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::RuntimeParent {
                    _ledger: ledger,
                    _body: body,
                    _recovered: recovered,
                },
            });
        }
    };
    let repair = match authenticate_recovered_wal_vote_lifecycle_from_ledger_parent(
        verified, &parent, successor,
    ) {
        Ok(repair) => repair,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::Lifecycle {
                    _ledger: ledger,
                    _body: body,
                    _error: error,
                },
            });
        }
    };
    let registry_preflight = (|| {
        if !parent.matches_candidate(repair.parent())
            || ledger
                .opened
                .stage_authenticated_wal_vote_repair(&repair)
                .is_err()
        {
            return None;
        }
        let (physical, universe, consumed) = repair.parent().physical_geometry.normalized().ok()?;
        if physical.len() != 1 || universe.len() != 1 || consumed != universe {
            return None;
        }
        let (&slot, &incumbent_digest) = physical.first_key_value()?;
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return None;
        }
        let address = ConcreteWorkAddress::new(parent.owner(), parent.ordinal(), slot)?;
        registry
            .entries
            .keys()
            .all(|installed| installed.owner != parent.owner())
            .then_some((address, incumbent_digest))
    })();
    let Some((address, incumbent_digest)) = registry_preflight else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::RegistryParent {
                _ledger: ledger,
                _repair: repair,
                _body: body,
            },
        });
    };

    // All fallible parent, ledger, body, and registry checks precede this
    // transfer. From here the detached marker moves directly into the sealed
    // completion and no pre-join error can discard it.
    let outcome = body.into_validation_outcome();
    let validated = outcome
        .validated_receipt()
        .expect("a recovered validated-body cut transfers one success outcome");
    let durable_receipt = validated.durable().clone();
    // Restart recovery obtains this hash from the semantically revalidated
    // marker reopened by this exact body-store instance. Unlike the live
    // transport path, there is no independently in-flight manifest carrier;
    // the checksummed receipt and store manifest were already compared before
    // the marker entered the validated recovery catalog.
    let expected_manifest_hash = durable_receipt.manifest_hash();
    let recovered_body_marker = durable_receipt.clone();
    let installed_digest =
        durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
            .expect("a validated recovered parent has one completion digest");
    let validation = DetachedRecoveredValidateCompletion {
        address,
        installed_digest,
        incumbent_address: address,
        incumbent_digest,
        durable_receipt,
        expected_manifest_hash,
        replay_evidence: DetachedValidateReplayEvidenceV1::RecoveredBodyMarker(
            recovered_body_marker,
        ),
        outcome,
    };
    let authority = AuthenticatedRecoveredWalValidateLifecycleRepair {
        repair,
        validation,
        reservation: RecoveredWalValidateRegistryReservation {
            registry,
            parent_address: address,
            child: None,
        },
    };
    debug_assert!(authority.concrete_pair_and_validation_are_exact());
    Ok((ledger, authority))
}

#[cfg(test)]
impl super::concrete_admission::LifecycleWorkRegistryHolder {
    /// Count only closed recovered-WAL Sign rows after an installed cut drops.
    /// This test oracle exposes no address, effect, pending binding, or receipt.
    pub(crate) fn recovered_wal_sign_entry_count_for_test(&self) -> usize {
        self.registry_for_test()
            .entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
                )
            })
            .count()
    }

    /// Assemble and install a genuine ordinary-Proposal validated completion.
    ///
    /// The retained signed Proposal enters the same authenticated fair-ingress
    /// replay mint as production dispatch. This helper then projects its exact
    /// Fetch-to-Store-to-Validate lineage, installs the closed completion, and
    /// returns only its exact scheduler coordinates. Callers must still enter
    /// the production Ready preparation path to borrow or detach the carrier.
    #[allow(clippy::too_many_lines)]
    fn install_remote_proposal_validate_completion_for_test(
        &mut self,
        verified: &VerifiedHeightContext,
        tag: EventTag,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> (TurnLease, PhysicalSlotId, CandidateAdmission) {
        assert_eq!(proposal.manifest, manifest);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round: proposal.round,
            subject: proposal.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let mut fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 1)],
        )
        .expect("bind genuine remote-Proposal Fetch fixture")
        .pop()
        .expect("one remote-Proposal Fetch fixture owner");
        assert!(
            fetch_ownership
                .bind_authenticated_remote_proposal_replay_for_test(proposal, &fetch_effect,)
        );
        let fetch_pending = fetch_ownership
            .pending_adapter_effect_binding(&fetch_effect)
            .expect("remote-Proposal Fetch retains one pending binding");
        let fetch_replay = fetch_ownership
            .exact_remote_proposal_fetch_replay(&fetch_effect)
            .expect("authenticated Proposal retains exact Fetch replay evidence");
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store_pending = fetch_pending
            .project_proposal_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("remote-Proposal Fetch projects exact Store binding");
        let store_replay = fetch_replay
            .project_exact_store(&store_effect, &store_pending)
            .expect("remote-Proposal Fetch projects exact Store replay evidence");
        let durable_receipt = validated_receipt.durable().clone();
        let stored_replay = store_replay
            .bind_durable_body(&store_effect, &durable_receipt)
            .expect("remote-Proposal Store binds its exact durable frame");
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let pending = store_pending
            .project_store_validate_successor(&store_effect, &effect)
            .expect("remote-Proposal Store projects exact Validate binding");
        let replay_evidence = stored_replay
            .project_exact_validate(&store_effect, &durable_receipt, &effect, &pending)
            .expect("remote-Proposal Store projects exact Validate replay evidence");
        let replay_evidence = DurableValidateReplayEvidenceV1::remote_proposal(replay_evidence);
        let projected = replay_evidence
            .project_installed_validate_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project genuine remote-Proposal recovered-WAL Validate fixture");
        let coordinator_candidate = projected.clone();
        assert_eq!(projected.work_class, LifecycleWorkClass::Validate);
        assert_eq!(projected.key.phase(), LifecyclePhase::Validate);
        assert_eq!(projected.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(
            projected.stage.predecessor_scope(),
            PredecessorScope::Independent
        );
        assert_eq!(projected.initial_state, InitialLifecycleState::Ready);
        let (physical_slots, universe, consumed) = projected
            .physical_geometry
            .normalized()
            .expect("normalize recovered-WAL Validate fixture geometry");
        assert_eq!(physical_slots.len(), 1);
        assert_eq!(universe.len(), 1);
        assert_eq!(consumed, universe);
        let (&slot, &incumbent_digest) = physical_slots
            .first_key_value()
            .expect("one recovered-WAL Validate fixture slot");
        let ordinal = 1;
        let owner = OwnerId::new(projected.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact recovered-WAL Validate fixture address");
        let expected_manifest_hash = durable_receipt.manifest_hash();
        assert_eq!(HashOf::new(&manifest), expected_manifest_hash);
        let incumbent = DurableValidateBody {
            address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(validate_validated_receipt_authority(&incumbent, &validated_receipt).is_ok());
        let outcome = DurableBodyValidationOutcome::validated_for_test(validated_receipt);
        let replacement_digest =
            durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
                .expect("validated recovered-WAL completion has one digest");
        assert_ne!(replacement_digest, incumbent_digest);
        let work = ConcreteLifecycleWork {
            digest: replacement_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(DurableValidateCompletion {
                address,
                incumbent,
                incumbent_digest,
                outcome,
            }),
        };
        self.registry_for_test_mut()
            .install(address, replacement_digest, work)
            .unwrap_or_else(|(error, _work)| {
                panic!("install recovered-WAL Validate fixture: {error:?}")
            });
        let mut ready_slots = physical_slots;
        assert_eq!(
            ready_slots.insert(slot, replacement_digest),
            Some(incumbent_digest)
        );
        let lease = TurnLease {
            id: LeaseId(1),
            ordinal,
            owner,
            key: projected.key,
            work_class: projected.work_class,
            stage: projected.stage,
            rank: super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: ready_slots,
            output_reservation: None,
        };
        (lease, slot, coordinator_candidate)
    }

    /// Assemble and install a genuine validated completion fixture, then reach
    /// the recovered-WAL cut through the production Ready preparation and
    /// detachment path.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn recovered_wal_validate_registry_cut_for_test<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        recovered: &RecoveredWalVoteSign,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> RecoveredWalValidateRegistryCut<'registry> {
        let tag = recovered.tag();
        let vote = recovered.vote();
        assert_eq!(proposal.round, vote.proposal_round);
        assert_eq!(proposal.subject, vote.subject);
        let (lease, slot, _candidate) = self.install_remote_proposal_validate_completion_for_test(
            verified,
            tag,
            proposal,
            manifest,
            validated_receipt,
        );
        let prepared = self
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, verified)
            .expect("prepare installed recovered-WAL Validate completion");
        prepared
            .into_recovered_wal_validate_registry_cut()
            .unwrap_or_else(|_prepared| panic!("validated recovered-WAL completion must detach"))
    }
}

// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN
impl<'registry> RecoveredWalValidateRegistryCut<'registry> {
    /// Convert the existing restorable recovery cut into the sole fail-stop
    /// live parent/child reservation after WAL fsync.
    ///
    /// Taking both optional fields disarms the recovery cut's restoring Drop.
    /// The returned parent is retained opaquely and is never reinstalled: any
    /// later error must restart through the durable WAL.
    fn into_live_validate_sign_reservation(
        mut self,
    ) -> Option<LiveValidateSignRegistryReservation<'registry>> {
        let registry = self.registry.take()?;
        let parent = self.work.take()?;
        let parent_address = self.address;
        Some(LiveValidateSignRegistryReservation {
            reservation: RecoveredWalValidateRegistryReservation {
                registry,
                parent_address,
                child: None,
            },
            _detached_parent: parent,
        })
    }

    #[cfg(test)]
    fn detached_work_is_exact_for_test(&self) -> bool {
        self.work.as_ref().is_some_and(|work| {
            work.validates_at(self.address)
                && matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                        if completion.address == self.address
                            && completion.outcome.validated_receipt().is_some()
                            && completion.outcome.rejection_identity().is_none()
                            && completion.outcome.missing_merge_sidecar().is_none()
                )
        })
    }

    /// Consume this exact closed Validate carrier and the adapter-authenticated
    /// current reducer vote from its latest exact WAL owner into one typed
    /// lifecycle repair.
    ///
    /// The pending binding never leaves this module. Projection failure
    /// reconstructs the detached completion so dropping the returned error
    /// restores it at the exact address. A later lifecycle-authentication
    /// failure retains every move-only input and requires restart.
    #[allow(clippy::result_large_err)]
    pub(crate) fn join_recovered_vote(
        mut self,
        verified: &VerifiedHeightContext,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        RecoveredWalValidateRegistryJoinError<'registry>,
    > {
        let recovered_commitment = recovered.vote().execution_commitment;
        let valid = self.work.as_ref().is_some_and(|work| {
            work.validates_at(self.address)
                && matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                        if completion.address == self.address
                            && completion.outcome.validated_receipt().is_some_and(|receipt| {
                                receipt.execution_commitment() == recovered_commitment
                            })
                            && completion.outcome.rejection_identity().is_none()
                            && completion.outcome.missing_merge_sidecar().is_none()
                )
        });
        if !valid {
            return Err(RecoveredWalValidateRegistryJoinError {
                failure: RecoveredWalValidateRegistryJoinFailure::InvalidCarrier {
                    _cut: self,
                    _recovered: recovered,
                },
            });
        }

        let work = self
            .work
            .take()
            .expect("validated recovered WAL cut retains its detached carrier");
        let ConcreteLifecycleWork {
            digest: installed_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(completion),
        } = work
        else {
            unreachable!("recovered WAL cut validated one completion carrier")
        };
        let DurableValidateCompletion {
            address,
            incumbent,
            incumbent_digest,
            outcome,
        } = completion;
        let DurableValidateBody {
            address: incumbent_address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        } = incumbent;
        let completion = DetachedRecoveredValidateCompletion {
            address,
            installed_digest,
            incumbent_address,
            incumbent_digest,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence: DetachedValidateReplayEvidenceV1::Retained(replay_evidence),
            outcome,
        };

        let successor = match pending.project_recovered_wal_vote_successor(&effect, recovered) {
            Ok(successor) => successor,
            Err((pending, recovered)) => {
                self.work = Some(completion.restore(effect, pending));
                return Err(RecoveredWalValidateRegistryJoinError {
                    failure: RecoveredWalValidateRegistryJoinFailure::Projection {
                        _cut: self,
                        _recovered: recovered,
                    },
                });
            }
        };
        let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) =
            &completion.replay_evidence
        else {
            unreachable!("a live detached Validate completion retains its replay origin")
        };
        match authenticate_recovered_wal_vote_lifecycle_from_durable_body(
            verified,
            &completion.durable_receipt,
            replay_evidence,
            successor,
        ) {
            Ok(repair) => {
                let registry = self.registry.take();
                let registry =
                    registry.expect("recovered WAL join retains its exclusive registry borrow");
                Ok(AuthenticatedRecoveredWalValidateLifecycleRepair {
                    repair,
                    validation: completion,
                    reservation: RecoveredWalValidateRegistryReservation {
                        registry,
                        parent_address: self.address,
                        child: None,
                    },
                })
            }
            Err(error) => Err(RecoveredWalValidateRegistryJoinError {
                failure: RecoveredWalValidateRegistryJoinFailure::Lifecycle {
                    _cut: self,
                    _error: error,
                    _completion: completion,
                },
            }),
        }
    }
}
// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END

#[allow(dead_code)]
impl<'a> PreparedDurableValidateExecution<'a> {
    fn durable_validate(&self) -> &DurableValidateBody {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Validate carrier remains installed");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            unreachable!("prepared durable Validate execution retains its closed carrier")
        };
        validate
    }

    /// Return the exact reducer coordinates accepted by the future body
    /// validation preview.
    pub(super) fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &self.durable_validate().effect
        else {
            unreachable!("prepared durable Validate carrier retains its Validate effect")
        };
        (*tag, *round, *subject)
    }

    /// Borrow the exact post-fsync body receipt retained by the Validate carrier.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_validate().durable_receipt
    }

    /// Match the coordinator's durable payload against this exact body receipt.
    pub(super) fn matches_durable_payload(&self, payload: DurablePayloadReference) -> bool {
        durable_validate_body_payload(&self.durable_validate().durable_receipt).is_some_and(
            |expected| {
                expected == payload
                    && super::body_pipeline_transition::durable_validate_payload_is_exact(
                        self.lifecycle_key,
                        payload,
                    )
            },
        )
    }

    /// Return the manifest hash transferred independently through the Store parent.
    pub(super) fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.durable_validate().expected_manifest_hash
    }

    /// Derive the external wake source only from this revalidated closed row.
    ///
    /// Callers cannot supply address, digest, causal-key, or inherited
    /// statement parts independently. The coordinator samples the generation
    /// for this source before consuming the preflight into a dispatch.
    pub(super) fn durable_validation_wait_source(&self) -> WaitSource {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Validate carrier remains installed");
        let validate = self.durable_validate();
        durable_validation_wait_source_from_exact_parts(
            self.address,
            work.digest,
            validate.pending.causal_lifecycle_key(),
            validate.pending.candidate_statement(),
            &validate.durable_receipt,
            validate.expected_manifest_hash,
            self.lifecycle_key,
            self.lifecycle_stage,
        )
    }

    /// Seal this preflight beside the exact coordinator-minted external wait.
    ///
    /// A foreign source or the reserved maximum generation returns the
    /// borrow-bound preflight intact and mints no detached request.
    pub(super) fn seal_waiting_dispatch(
        self,
        wait_token: WaitToken,
    ) -> Result<DurableValidateDispatch, Self> {
        if wait_token.source() != self.durable_validation_wait_source()
            || wait_token.observed_generation() == u64::MAX
        {
            return Err(self);
        }
        Ok(DurableValidateDispatch {
            request: self.detach(),
            wake: DurableValidateWakeAuthority { wait_token },
        })
    }

    /// Consume the borrow-bound preflight into an owned validation request.
    ///
    /// The registry row is not removed or changed. Returning the owned token
    /// ends the exclusive registry borrow before any body-store I/O or
    /// deterministic validation callback can run.
    pub(super) fn detach(self) -> DetachedDurableValidateExecution {
        let (
            incumbent_digest,
            tag,
            round,
            subject,
            durable_receipt,
            expected_manifest_hash,
            causal_lifecycle_key,
            candidate_statement,
            lifecycle_key,
            lifecycle_stage,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Validate carrier remains installed");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                unreachable!("prepared durable Validate execution retains its closed carrier")
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                unreachable!("prepared durable Validate carrier retains its Validate effect")
            };
            (
                work.digest,
                *tag,
                *round,
                *subject,
                validate.durable_receipt.clone(),
                validate.expected_manifest_hash,
                validate.pending.causal_lifecycle_key().clone(),
                validate.pending.candidate_statement(),
                self.lifecycle_key,
                self.lifecycle_stage,
            )
        };
        DetachedDurableValidateExecution {
            address: self.address,
            incumbent_digest,
            tag,
            round,
            subject,
            durable_receipt,
            expected_manifest_hash,
            causal_lifecycle_key,
            candidate_statement,
            lifecycle_key,
            lifecycle_stage,
        }
    }

    /// Bind one store-minted successful-validation receipt to this exact
    /// Validate carrier without changing the registry row.
    ///
    /// Existing Prepare or Commit authority must name the same deterministic
    /// execution result. An ordinary body may acquire its first commitment,
    /// but only the later receipt-bound Apply projection may use it.
    pub(super) fn bind_validated_receipt(
        self,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<
        PreparedValidatedBodyCompletion<'a>,
        (DurableValidateExecutionError, ValidatedBodyReceipt),
    > {
        let (tag, round, subject, incumbent_digest, replacement_digest) = {
            let validate = self.durable_validate();
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                return Err((
                    DurableValidateExecutionError::InvalidValidateShape,
                    validated_receipt,
                ));
            };
            if let Err(error) = validate_validated_receipt_authority(validate, &validated_receipt) {
                return Err((error, validated_receipt));
            }
            let incumbent_digest = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Validate carrier remains installed")
                .digest;
            let replacement_digest = validated_body_completion_digest(
                incumbent_digest,
                validate.expected_manifest_hash,
                &validated_receipt,
            );
            if replacement_digest == incumbent_digest {
                return Err((
                    DurableValidateExecutionError::InvalidValidationCompletionDigest,
                    validated_receipt,
                ));
            }
            (*tag, *round, *subject, incumbent_digest, replacement_digest)
        };

        Ok(PreparedValidatedBodyCompletion {
            _registry: self.registry,
            address: self.address,
            incumbent_digest,
            replacement_digest,
            validated_receipt,
            tag,
            round,
            subject,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'a> PreparedValidatedBodyCompletion<'a> {
    fn retained_validated_receipt_is_exact(&self) -> bool {
        let Some(work) = self._registry.entries.get(&self.address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return false;
        };
        work.digest == self.incumbent_digest
            && validate.validates(self.incumbent_digest)
            && validate_validated_receipt_authority(validate, &self.validated_receipt).is_ok()
            && validated_body_completion_digest(
                self.incumbent_digest,
                validate.expected_manifest_hash,
                &self.validated_receipt,
            ) == self.replacement_digest
    }

    fn retained_apply_join_is_exact(&self, persisted: &SealedLiveWalPersistedEffectV1) -> bool {
        let Some(work) = self._registry.entries.get(&self.address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return false;
        };
        self.retained_validated_receipt_is_exact()
            && persisted.exactly_binds_validated_apply_successor(
                &validate.effect,
                &validate.pending,
                self.validated_receipt.durable(),
            )
    }

    /// Join one exact live `Decision -> Apply` WAL seal to this retained Validate result.
    ///
    /// This is the sole production receipt-bearing completion surface. It
    /// first revalidates the installed carrier and store-minted receipt, then
    /// consumes the source-only WAL seal into its canonical body-frame-bound
    /// replay envelope. Failure retains every move-only input.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_live_wal_apply(
        self,
        persisted: SealedLiveWalPersistedEffectV1,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<PreparedLiveWalReplayPreAdmission<'a>, LiveWalReplayPreAdmissionError<'a>> {
        if !self.retained_validated_receipt_is_exact() {
            return Err(LiveWalReplayPreAdmissionError {
                _failure: LiveWalReplayPreAdmissionFailure::Apply {
                    _completion: self,
                    _persisted: persisted,
                    _pending: pending,
                },
            });
        }
        let (predecessor_effect, predecessor_pending) = {
            let work = self
                ._registry
                .entries
                .get(&self.address)
                .expect("revalidated Apply join retains its installed Validate row");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                unreachable!("revalidated Apply join retains its durable Validate carrier")
            };
            (&validate.effect, &validate.pending)
        };
        let persisted = match persisted.complete_exact_apply(
            predecessor_effect,
            predecessor_pending,
            pending,
            self.validated_receipt.durable(),
        ) {
            Ok(persisted) => persisted,
            Err((persisted, pending)) => {
                return Err(LiveWalReplayPreAdmissionError {
                    _failure: LiveWalReplayPreAdmissionFailure::Apply {
                        _completion: self,
                        _persisted: persisted,
                        _pending: pending,
                    },
                });
            }
        };
        debug_assert!(self.retained_apply_join_is_exact(&persisted));
        Ok(PreparedLiveWalReplayPreAdmission {
            _persisted: persisted,
            _origin: LiveWalReplayPreAdmissionOrigin::Apply(self),
        })
    }

    /// Exact reducer coordinates retained by the completed Validate carrier.
    pub(super) const fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        (self.tag, self.round, self.subject)
    }

    /// Borrow the exact store-minted deterministic validation result.
    pub(super) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }

    /// Digest currently installed for the closed Validate work.
    pub(super) const fn incumbent_digest(&self) -> LifecycleDigest {
        self.incumbent_digest
    }

    /// Domain-separated physical digest for the receipt-bound completion.
    pub(super) const fn replacement_digest(&self) -> LifecycleDigest {
        self.replacement_digest
    }
}

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
impl PreparedExecutedDurableValidateCompletion<'_> {
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
        StagedDurableValidateCompletion<'_>,
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
    ) -> Result<
        PreparedDurableCertifiedFetchCompletion<'a>,
        (
            CertifiedFetchCompletionError,
            DurableCertifiedFetchBodyReceipt,
        ),
    > {
        macro_rules! retain_receipt {
            ($error:expr) => {
                return Err(($error, durable_receipt))
            };
        }
        let address = self.location.address();
        let Some(incumbent) = self.registry.entries.get(&address) else {
            retain_receipt!(CertifiedFetchCompletionError::MissingIncumbent);
        };
        if !incumbent.validates_at(address) {
            retain_receipt!(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
        } = &incumbent.kind
        else {
            retain_receipt!(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            retain_receipt!(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if self.location.owner.causal_root() != incumbent.causal_root() {
            retain_receipt!(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if incumbent.digest != self.location.incumbent_digest {
            retain_receipt!(CertifiedFetchCompletionError::IncumbentDigestMismatch);
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
            retain_receipt!(CertifiedFetchCompletionError::DurableReceiptMismatch);
        }
        let Some(replay_evidence) = self.replay_origin.bind_durable_body(&durable_receipt) else {
            retain_receipt!(CertifiedFetchCompletionError::InvalidReplayEvidence);
        };
        let Some(ready_projection) = replay_evidence.project_durable_ready_fetch(
            incumbent_effect,
            incumbent_pending,
            durable_receipt.durable_body(),
        ) else {
            retain_receipt!(CertifiedFetchCompletionError::InvalidReplayEvidence);
        };
        if ready_projection.completion_digest() == self.location.incumbent_digest {
            retain_receipt!(CertifiedFetchCompletionError::ReplacementDigestMismatch);
        }

        Ok(PreparedDurableCertifiedFetchCompletion {
            registry: self.registry,
            location: self.location,
            ingress_identity: self.ingress_identity,
            request_hash: self.request_hash,
            response_hash: self.response_hash,
            authenticated_responder: self.authenticated_responder,
            durable_receipt,
            replay_evidence,
            ready_projection,
        })
    }
}

impl PreparedDurableCertifiedFetchCompletion<'_> {
    /// Borrow the opaque durable projection used by the coordinator's staged cut.
    pub(super) const fn ready_projection(&self) -> &DurableCertifiedFetchReplayProjectionV1 {
        &self.ready_projection
    }

    /// Return the exact Waiting incumbent address authenticated before persistence.
    pub(super) const fn waiting_location(&self) -> CertifiedFetchWaitingLocation {
        self.location
    }

    /// Revalidate the selector-retained exact response before LedgerV1 fsync.
    ///
    /// The later checked dequeue can then mint only an ownership carrier; its
    /// registry install has no fallible response or durable-receipt checks.
    pub(super) fn matches_selected_response(
        &self,
        ingress_identity: PendingFairIngressIdentity,
        inbound: &InboundBlockMessage,
        disposition: FairV2IngressDequeueDisposition,
    ) -> bool {
        exact_selected_response_matches(
            ingress_identity,
            inbound,
            disposition,
            self.registry
                .entries
                .get(&self.location.address())
                .and_then(|work| match &work.kind {
                    ConcreteLifecycleWorkKind::PendingAdapter { effect, .. } => Some(effect),
                    _ => None,
                }),
            self.request_hash,
            self.response_hash,
            &self.authenticated_responder,
            &self.durable_receipt,
        )
    }

    /// Return the sealed receipt before any external queue mutation.
    ///
    /// The selector uses this only to reconstruct the complete opaque Phase-B
    /// input after a retryable checked-dequeue rejection. The registry remains
    /// byte-for-byte unchanged.
    pub(super) fn abort_before_dequeue(self) -> DurableCertifiedFetchBodyReceipt {
        self.durable_receipt
    }

    /// Install the closed completion only after checked dequeue returned its
    /// exact owned response carrier. The occurrence is authenticated here and
    /// then dropped; installed work retains only restart-stable material.
    ///
    /// Every fallible comparison precedes the first map mutation. Once those
    /// comparisons succeed, the exclusive registry borrow guarantees that the
    /// previously validated incumbent still occupies `location`; removal,
    /// construction, and same-address insertion are then infallible.
    ///
    /// The caller invokes this only after LedgerV1 fsync and exact dequeue,
    /// under an armed fail-stop operation. Assertions therefore represent a
    /// process-fatal invariant violation, never a retryable completion error.
    pub(super) fn commit_after_exact_dequeue(self, dequeued: CertifiedFetchDequeuedResponse) {
        assert_eq!(dequeued.ingress_identity(), self.ingress_identity);
        let address = self.location.address();
        let incumbent = self
            .registry
            .entries
            .get(&address)
            .expect("preflighted certified-Fetch incumbent remains installed");
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            ..
        } = &incumbent.kind
        else {
            panic!("preflighted certified-Fetch incumbent changed work kind")
        };
        assert!(incumbent.validates_at(address));
        assert!(matches!(incumbent_effect, AdapterEffect::FetchBody { .. }));
        assert_eq!(self.location.owner.causal_root(), incumbent.causal_root());
        assert_eq!(incumbent.digest, self.location.incumbent_digest);
        assert!(exact_selected_response_matches(
            dequeued.ingress_identity(),
            dequeued.inbound(),
            dequeued.disposition(),
            Some(incumbent_effect),
            self.request_hash,
            self.response_hash,
            &self.authenticated_responder,
            &self.durable_receipt,
        ));
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
        let durable_receipt = self.durable_receipt.durable_body().clone();
        let installed_digest = self.ready_projection.completion_digest();
        let completion = CertifiedFetchCompletion {
            address,
            incumbent_effect,
            incumbent_pending,
            incumbent_digest,
            durable_receipt,
            replay_evidence: self.replay_evidence,
        };
        let installed = ConcreteLifecycleWork {
            digest: installed_digest,
            kind: ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
        };
        assert!(installed.validate_exact());
        assert!(
            self.registry.entries.insert(address, installed).is_none(),
            "removed completion address remains vacant until same-address install"
        );
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

fn exact_selected_response_matches(
    ingress_identity: PendingFairIngressIdentity,
    inbound: &InboundBlockMessage,
    disposition: FairV2IngressDequeueDisposition,
    effect: Option<&AdapterEffect>,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: &PeerId,
    durable_receipt: &DurableCertifiedFetchBodyReceipt,
) -> bool {
    if ingress_identity.physical_admission_ordinal() == 0
        || disposition != FairV2IngressDequeueDisposition::Admit
    {
        return false;
    }
    let Some(effect) = effect else {
        return false;
    };
    let Some(response) = selected_certified_response(inbound) else {
        return false;
    };
    inbound.sender() == Some(authenticated_responder)
        && ingress_identity_matches_round(ingress_identity, response.manifest.round)
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

fn selected_certified_response(
    inbound: &InboundBlockMessage,
) -> Option<&wire::CertifiedBodyResponse> {
    let BlockMessage::V2(message) = inbound.message() else {
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
    use std::{
        cell::Cell,
        collections::BTreeMap,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;

    #[cfg(feature = "bls")]
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::consensus_v2 as wire;
    #[cfg(feature = "bls")]
    use iroha_data_model::block::{
        BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
    };
    #[cfg(feature = "bls")]
    use iroha_data_model::merge::MergeQuorumCertificate;
    #[cfg(feature = "bls")]
    use iroha_data_model::peer::PeerId;
    #[cfg(feature = "bls")]
    use tempfile::TempDir;

    #[cfg(feature = "bls")]
    use super::super::{
        AdmissionDecision, CapacityClass, LifecycleCoordinator, LifecycleState,
        ProductionSchedulerInputsError, TerminalOutcome, WaitToken,
        concrete_admission::{
            DurableValidateDispatchError, LifecycleWorkRegistryHolder,
            ReadyValidateDemandAttestationError,
        },
        schema::CapacityGeometry,
    };
    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2::{
        AdapterError, AdapterFingerprints, AuthenticatedConsensusMessage,
        DeferredAdmissionOrdinalSource,
    };
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2_chunks::encode_payload;
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2_core as reducer;
    use crate::sumeragi::{
        v2::{ExactLiveWalPersistedContinuationCause, LiveWalFrameIdentity},
        v2_core::{EventTag, Generation},
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    #[test]
    fn registry_instance_identity_rejects_a_distinct_empty_registry() {
        let first = ConcreteLifecycleWorkRegistry::default();
        let identity = first.instance_identity();
        assert!(identity.same_instance(&first.instance_identity()));

        let second = ConcreteLifecycleWorkRegistry::default();
        assert!(
            !identity.same_instance(&second.instance_identity()),
            "equal empty registry contents cannot substitute for exact instance ownership"
        );
    }

    fn effect_at_generation(marker: u8, generation: u64) -> AdapterEffect {
        let tag = EventTag::new(7, 2, Generation::new(generation.max(1)));
        AdapterEffect::StoreBody {
            tag,
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"lifecycle-work-registry-context",
                ))),
                height: 7,
                view: 2,
            },
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
                payload_hash: Hash::new([marker, 2]),
            },
        }
    }

    fn effect(marker: u8) -> AdapterEffect {
        effect_at_generation(marker, u64::from(marker))
    }

    fn direct_signed_pending(
        effect: &AdapterEffect,
        tag: EventTag,
        ordinal: u128,
    ) -> PendingRuntimeEffectBinding {
        bind_adapter_effect_batch_ownership(
            core::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind direct signed registry fixture")
        .pop()
        .expect("one direct signed registry fixture owner")
        .pending_adapter_effect_binding(effect)
        .expect("mint direct signed pending binding")
    }

    fn direct_signed_vote(marker: u8, subject_marker: u8) -> wire::Vote {
        let context_id =
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new([marker, 0xD1])));
        let round = wire::ConsensusRound {
            context_id,
            height: 7,
            view: 2,
        };
        wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([subject_marker, 0xD2])),
                payload_hash: Hash::new([subject_marker, 0xD3]),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 0xD4]),
                Hash::new([marker, 0xD5]),
                Hash::new([marker, 0xD6]),
                1,
                Hash::new([marker, 0xD7]),
            ),
            signer: 0,
            signature: vec![subject_marker, 0xD8],
        }
    }

    fn recovered_wal_projection_candidate(
        phase: LifecyclePhase,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
        marker: u8,
    ) -> CandidateAdmission {
        let context = LifecycleContext::new(LifecycleDigest::new([0x31; 32]), 7);
        let replay =
            super::super::replay_authority::exact_record_fixture(context, stage_kind, marker);
        assert_eq!((replay.key.phase(), replay.work_class), (phase, work_class));
        let root = super::super::CausalRoot::new(LifecycleDigest::new([0x34; 32]));
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        CandidateAdmission::new(
            replay.key,
            root,
            work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            InitialLifecycleState::Ready,
            root.digest(),
            replay.payload,
            replay.authority,
            super::super::PhysicalGeometry::new(
                [PhysicalSlot::new(slot, LifecycleDigest::new([marker; 32]))],
                [slot],
            ),
            None,
        )
    }

    #[test]
    fn recovered_wal_projection_never_overwrites_foreign_opposite_key_occupants() {
        let parent = recovered_wal_projection_candidate(
            LifecyclePhase::Validate,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
            0x41,
        );
        let child = recovered_wal_projection_candidate(
            LifecyclePhase::Prepare,
            LifecycleWorkClass::SignVote,
            LifecycleStageKind::SignPrepareVote,
            0x42,
        );
        let owner = OwnerId::new(parent.causal_root, 1);
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let projection = AuthenticatedRecoveredWalSignProjection {
            parent: parent.clone(),
            child: child.clone(),
            parent_address: ConcreteWorkAddress::new(owner, 1, effect_slot)
                .expect("exact recovered parent address"),
            child_address: ConcreteWorkAddress::new(owner, 2, effect_slot)
                .expect("exact recovered child address"),
        };

        let mut foreign_child = child.clone();
        foreign_child.reconstruction_source = LifecycleDigest::new([0x51; 32]);
        let mut parent_with_foreign_child =
            BTreeMap::from([(parent.key, parent.clone()), (child.key, foreign_child)]);
        let before = parent_with_foreign_child.clone();
        assert!(!projection.splice_candidates(&mut parent_with_foreign_child));
        assert_eq!(parent_with_foreign_child, before);

        let mut foreign_parent = parent.clone();
        foreign_parent.reconstruction_source = LifecycleDigest::new([0x52; 32]);
        let mut child_with_foreign_parent =
            BTreeMap::from([(parent.key, foreign_parent), (child.key, child)]);
        let before = child_with_foreign_parent.clone();
        assert!(!projection.splice_candidates(&mut child_with_foreign_parent));
        assert_eq!(child_with_foreign_parent, before);
    }

    fn concrete(effect: AdapterEffect, legacy_ordinal: u128) -> ConcreteLifecycleWork {
        let tag = match &effect {
            AdapterEffect::StoreBody { tag, .. } => *tag,
            _ => unreachable!("registry fixture uses one StoreBody effect"),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, legacy_ordinal)],
        )
        .expect("bind exact registry fixture")
        .pop()
        .expect("one registry fixture owner");
        let pending = ownership
            .pending_adapter_effect_binding(&effect)
            .expect("mint pending registry fixture");
        ConcreteLifecycleWork::from_exact(effect, pending).expect("construct exact concrete work")
    }

    fn owner(seed: u8, first_ordinal: u128) -> OwnerId {
        OwnerId::new(
            super::super::CausalRoot::new(LifecycleDigest::new([seed; 32])),
            first_ordinal,
        )
    }

    fn admitted_owner(work: &ConcreteLifecycleWork, first_ordinal: u128) -> OwnerId {
        OwnerId::new(work.causal_root(), first_ordinal)
    }

    #[test]
    fn prospective_startup_census_rejects_extra_valid_carrier_before_publication() {
        let work = concrete(effect(0x61), 1);
        let expected_effect = work.effect().clone();
        let digest = work.digest();
        let owner = admitted_owner(&work, 1);
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 1, slot).expect("exact extra address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, digest, work)
            .expect("install internally valid but extraneous startup carrier");
        let coordinator = super::super::LifecycleCoordinator::new(
            LifecycleContext::new(LifecycleDigest::new([0x62; 32]), 7),
            0,
            super::super::schema::CapacityGeometry::new(
                CapacityClass::ALL.into_iter().map(|class| (class, 8)),
            ),
        );
        let batch = PreparedCertifiedServeRegistryBatchV1 {
            entries: Vec::new(),
        };
        let invoked = Cell::new(false);
        let result = registry.install_certified_serve_startup_batch_before_publication(
            batch,
            &coordinator,
            || {
                invoked.set(true);
                Ok::<(), ()>(())
            },
        );
        assert!(matches!(
            result,
            Err(CertifiedServeRegistryBatchPublicationError::Preflight(_))
        ));
        assert!(!invoked.get(), "Ledger publication must not be invoked");
        assert!(registry.exactly_contains(address, &expected_effect));
    }

    #[test]
    fn complete_startup_census_rejects_live_store_without_a_carrier() {
        let candidate = recovered_wal_projection_candidate(
            LifecyclePhase::Store,
            LifecycleWorkClass::Store,
            LifecycleStageKind::StoreBody,
            0x63,
        );
        let context =
            LifecycleContext::new(candidate.key.context(), candidate.key.round().height());
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            super::super::schema::CapacityGeometry::new(
                CapacityClass::ALL.into_iter().map(|class| (class, 8)),
            ),
        );
        assert!(matches!(
            coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)),
            super::super::AdmissionDecision::Admitted { .. }
        ));
        let registry = ConcreteLifecycleWorkRegistry::default();

        assert!(!registry.exactly_covers_recovered_ready_work(&coordinator));
        assert!(!registry.exactly_covers_recovered_ready_fetches(&coordinator));
    }

    fn key(seed: u8) -> super::super::LifecycleKey {
        super::super::LifecycleKey::new(
            LifecycleDigest::new([seed; 32]),
            super::super::LifecycleRound::new(7, 2),
            Some(super::super::LifecycleRound::new(7, 2)),
            Some(LifecycleDigest::new([seed.wrapping_add(1); 32])),
            super::super::LifecyclePhase::Store,
            None,
        )
    }

    fn lease(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> TurnLease {
        TurnLease {
            id: super::super::LeaseId(1),
            ordinal,
            owner,
            key: key(u8::try_from(ordinal).unwrap_or(0)),
            work_class: super::super::LifecycleWorkClass::Store,
            stage: super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::StoreBody,
                super::super::PredecessorScope::Independent,
            ),
            rank: super::super::SchedulerRank::new(4, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: BTreeMap::from([(slot, digest)]),
            output_reservation: None,
        }
    }

    fn fetch_lease(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> TurnLease {
        TurnLease {
            id: super::super::LeaseId(2),
            ordinal,
            owner,
            key: super::super::LifecycleKey::new(
                LifecycleDigest::new([u8::try_from(ordinal).unwrap_or(0); 32]),
                super::super::LifecycleRound::new(7, 2),
                Some(super::super::LifecycleRound::new(7, 2)),
                Some(LifecycleDigest::new([0xA5; 32])),
                super::super::LifecyclePhase::Fetch,
                None,
            ),
            work_class: super::super::LifecycleWorkClass::Fetch,
            stage: super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::FetchBody,
                super::super::PredecessorScope::Independent,
            ),
            rank: super::super::SchedulerRank::new(5, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: BTreeMap::from([(slot, digest)]),
            output_reservation: None,
        }
    }

    #[cfg(feature = "bls")]
    struct DurableStoreFixture {
        registry: ConcreteLifecycleWorkRegistry,
        verified: VerifiedHeightContext,
        address: ConcreteWorkAddress,
        lease: TurnLease,
        slot: PhysicalSlotId,
        effect: AdapterEffect,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
    }

    #[cfg(feature = "bls")]
    struct DurableValidateFixture {
        registry: ConcreteLifecycleWorkRegistry,
        verified: VerifiedHeightContext,
        address: ConcreteWorkAddress,
        lease: TurnLease,
        slot: PhysicalSlotId,
        effect: AdapterEffect,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
        canonical_wire: Vec<u8>,
        manifest: wire::PayloadManifest,
        store_ownership: RuntimeEffectOwnership,
    }

    #[cfg(feature = "bls")]
    fn durable_store_keys(marker: u8) -> Vec<KeyPair> {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed.wrapping_add(marker); 32], Algorithm::BlsNormal)
                    .expect("deterministic durable Store BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        keys
    }

    #[cfg(feature = "bls")]
    fn verified_store_context(marker: u8) -> (VerifiedHeightContext, wire::HeightContext) {
        let keys = durable_store_keys(marker);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("durable Store proof of possession")
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
            network_id: crate::sumeragi::synthetic_network_id(&format!(
                "durable-store-registry-{marker}"
            )),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("durable Store fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new([marker, 0xA1]),
            execution_policy_hash: Hash::new([marker, 0xA2]),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [marker; 32],
        };
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified durable Store height context");
        (verified, context)
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn durable_store_fixture(marker: u8) -> DurableStoreFixture {
        let (verified, context) = verified_store_context(marker);
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let tag = EventTag::new(
            round.height,
            round.view,
            Generation::new(u64::from(marker) + 1),
        );
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xB1])),
            payload_hash: Hash::new([marker, 0xB2]),
        };
        let manifest = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: context.da_layout,
            chunk_hashes: vec![Hash::new([marker, 0xC1])],
            chunk_root: Hash::new([marker, 0xC2]),
        };
        let expected_manifest_hash = HashOf::new(&manifest);
        let durable_receipt =
            DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(certified_pipeline_prepare_certificate_for_test(
                &manifest,
                &durable_receipt,
            )),
        };
        let effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                tag,
                u128::from(marker) + 1,
            )],
        )
        .expect("bind exact certified Fetch fixture")
        .pop()
        .expect("one certified Fetch fixture owner");
        let ownership = fetch_ownership
            .rebind_as_inherited_adapter_effect(&effect)
            .expect("carry certified Fetch authority into Store");
        let pending = ownership
            .pending_adapter_effect_binding(&effect)
            .expect("mint sealed durable Store binding");
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let validate_pending = pending
            .project_store_validate_successor(&effect, &validate_effect)
            .expect("project exact certified Validate fixture pending");
        let (replay_evidence, _validate_evidence) = certified_pipeline_replay_evidence_for_test(
            tag,
            &manifest,
            &durable_receipt,
            &validate_pending,
        )
        .expect("build exact certified Store replay evidence");
        let candidate = replay_evidence
            .project_installed_store_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                &verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project exact replay-authorized durable Store fixture");
        let (physical_slots, slot_universe, consumed_slots) = candidate
            .physical_geometry
            .normalized()
            .expect("normalize durable Store fixture geometry");
        assert_eq!(slot_universe, consumed_slots);
        assert_eq!(physical_slots.len(), 1);
        let (&slot, &digest) = physical_slots
            .first_key_value()
            .expect("one durable Store fixture slot");
        let ordinal = u128::from(marker) + 1;
        let owner = OwnerId::new(candidate.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact durable Store registry address");
        let lease = TurnLease {
            id: super::super::LeaseId(u128::from(marker) + 1),
            ordinal,
            owner,
            key: candidate.key,
            work_class: candidate.work_class,
            stage: candidate.stage,
            rank: super::super::SchedulerRank::new(4, 0, 0, 0, 0, 0, 0, 0),
            physical_slots,
            output_reservation: None,
        };
        let store = DurableStoreBody {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(store.validates(digest));
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableStoreBody(store),
        };
        assert!(work.validate_exact());
        assert!(work.validates_at(address));
        assert_eq!(work.effect(), &effect);
        assert_eq!(work.causal_root(), owner.causal_root());
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        assert!(registry.entries.insert(address, work).is_none());
        DurableStoreFixture {
            registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
        }
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn durable_validate_fixture(marker: u8) -> DurableValidateFixture {
        durable_validate_fixture_at_view(marker, 2)
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn durable_validate_fixture_at_view(marker: u8, view: wire::View) -> DurableValidateFixture {
        let (verified, context) = verified_store_context(marker);
        let keys = durable_store_keys(marker);
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let tag = EventTag::new(
            round.height,
            round.view,
            Generation::new(u64::from(marker) + 1),
        );
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("durable Validate leader index");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero durable Validate height"),
            None,
            None,
            None,
            1_000,
            round.view,
        );
        let signature = SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
            .expect("sign durable Validate fixture body");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block
            .encode_wire()
            .expect("encode durable Validate fixture body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(&context, round, subject, &canonical_wire)
            .expect("encode durable Validate fixture payload")
            .manifest()
            .clone();
        let expected_manifest_hash = HashOf::new(&manifest);
        let durable_receipt =
            DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(certified_pipeline_prepare_certificate_for_test(
                &manifest,
                &durable_receipt,
            )),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                tag,
                u128::from(marker) + 1,
            )],
        )
        .expect("bind exact certified Validate Fetch fixture")
        .pop()
        .expect("one certified Validate Fetch fixture owner");
        let ownership = fetch_ownership
            .rebind_as_inherited_adapter_effect(&store_effect)
            .expect("carry certified Fetch authority into Validate parent Store");
        let store_pending = ownership
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint sealed durable Validate parent binding");
        let effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let pending = store_pending
            .project_store_validate_successor(&store_effect, &effect)
            .expect("project exact Store-to-Validate fixture lineage");
        assert_eq!(
            pending.causal_lifecycle_key(),
            store_pending.causal_lifecycle_key()
        );
        assert_eq!(
            pending.candidate_statement(),
            store_pending.candidate_statement()
        );
        assert_ne!(
            pending.exact_effect_identity(),
            store_pending.exact_effect_identity()
        );
        let (_store_evidence, replay_evidence) =
            certified_pipeline_replay_evidence_for_test(tag, &manifest, &durable_receipt, &pending)
                .expect("build exact certified Validate replay evidence");
        let replay_evidence = DurableValidateReplayEvidenceV1::certified(replay_evidence);
        let candidate = replay_evidence
            .project_installed_validate_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                &verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project exact replay-authorized durable Validate fixture");
        let (physical_slots, slot_universe, consumed_slots) = candidate
            .physical_geometry
            .normalized()
            .expect("normalize durable Validate fixture geometry");
        assert_eq!(slot_universe, consumed_slots);
        assert_eq!(physical_slots.len(), 1);
        let (&slot, &digest) = physical_slots
            .first_key_value()
            .expect("one durable Validate fixture slot");
        let ordinal = u128::from(marker) + 1;
        let owner = OwnerId::new(candidate.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact durable Validate registry address");
        let lease = TurnLease {
            id: super::super::LeaseId(u128::from(marker) + 1),
            ordinal,
            owner,
            key: candidate.key,
            work_class: candidate.work_class,
            stage: candidate.stage,
            rank: super::super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
            physical_slots,
            output_reservation: None,
        };
        let validate = DurableValidateBody {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(validate.validates(digest));
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
        };
        assert!(work.validate_exact());
        assert!(work.validates_at(address));
        assert_eq!(work.effect(), &effect);
        assert_eq!(work.causal_root(), owner.causal_root());
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        assert!(registry.entries.insert(address, work).is_none());
        DurableValidateFixture {
            registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
            canonical_wire,
            manifest,
            store_ownership: ownership,
        }
    }

    #[cfg(feature = "bls")]
    #[derive(Debug)]
    #[allow(variant_size_differences, clippy::large_enum_variant)]
    enum DetachedValidationError {
        Invalid(&'static str),
        MissingMergeSidecar(CertifiedMergeLedgerReference),
    }

    #[cfg(feature = "bls")]
    impl std::fmt::Display for DetachedValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Invalid(reason) => formatter.write_str(reason),
                Self::MissingMergeSidecar(reference) => {
                    write!(formatter, "missing merge sidecar {}", reference.entry_hash)
                }
            }
        }
    }

    #[cfg(feature = "bls")]
    impl BodyValidationError for DetachedValidationError {
        fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
            match self {
                Self::MissingMergeSidecar(reference) => Some(reference),
                Self::Invalid(_) => None,
            }
        }
    }

    #[cfg(feature = "bls")]
    fn detached_validation_merge_reference(
        durable: &DurableBodyReceipt,
    ) -> CertifiedMergeLedgerReference {
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"detached Validate missing merge sidecar",
            )),
            encoded_len: 512,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                durable.round().view,
                7,
                durable.round().height,
                HashOf::from_untyped_unchecked(Hash::new(b"detached Validate merge parent")),
                Hash::new(b"detached Validate merge chain"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"detached Validate merge certificate"),
            ),
        }
    }

    #[cfg(feature = "bls")]
    fn durable_validate_store_fixture(
        marker: u8,
    ) -> (
        DurableValidateFixture,
        TempDir,
        V2BodyStore,
        DurableBodyReceipt,
    ) {
        durable_validate_store_fixture_at_view(marker, 2)
    }

    #[cfg(feature = "bls")]
    fn durable_validate_store_fixture_at_view(
        marker: u8,
        view: wire::View,
    ) -> (
        DurableValidateFixture,
        TempDir,
        V2BodyStore,
        DurableBodyReceipt,
    ) {
        let mut fixture = durable_validate_fixture_at_view(marker, view);
        let directory = TempDir::new().expect("temporary detached Validate body store");
        let mut store = V2BodyStore::open(directory.path(), fixture.verified.context().clone())
            .expect("open detached Validate body store");
        let durable = store
            .store(fixture.manifest.clone(), fixture.canonical_wire.clone())
            .expect("persist detached Validate fixture body");
        assert_eq!(durable.manifest_hash(), fixture.expected_manifest_hash);
        let work = fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("detached Validate fixture retains its closed row");
        let digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("detached Validate fixture retains one closed Validate")
        };
        validate.durable_receipt = durable.clone();
        assert!(validate.validates(digest));
        assert!(work.validates_at(fixture.address));
        (fixture, directory, store, durable)
    }

    #[cfg(feature = "bls")]
    fn seal_validate_fixture_commitment(
        fixture: &mut DurableValidateFixture,
        execution_commitment: wire::ExecutionCommitment,
    ) {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = fixture.effect.clone()
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let certified_fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: Vec::new(),
                aggregate_signature: Vec::new(),
            }),
        };
        let certified_fetch_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&certified_fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 60_001)],
        )
        .expect("bind one commitment-authorized Fetch")
        .pop()
        .expect("one commitment-authorized Fetch owner");
        let incoming_store_owner = certified_fetch_owner
            .rebind_as_inherited_adapter_effect(&store_effect)
            .expect("carry commitment authority into Store");
        let adopted_store_owner = fixture
            .store_ownership
            .adopt_incumbent_body_stage_for_retry_or_authority(&incoming_store_owner, &store_effect)
            .expect("retain physical Store owner while sealing commitment authority");
        let upgraded_store = adopted_store_owner
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint commitment-authorized Store binding");
        let upgraded_validate = upgraded_store
            .project_store_validate_successor(&store_effect, &fixture.effect)
            .expect("carry commitment authority into Validate");

        let work = fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("commitment fixture retains exact Validate row");
        let digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("commitment fixture retains one closed Validate")
        };
        let (_store_replay, validate_replay) = certified_pipeline_replay_evidence_for_test(
            tag,
            &fixture.manifest,
            &validate.durable_receipt,
            &upgraded_validate,
        )
        .expect("rebind certified Validate replay to upgraded pending authority");
        validate.pending = upgraded_validate;
        validate.replay_evidence = DurableValidateReplayEvidenceV1::certified(validate_replay);
        assert!(validate.validates(digest));

        let candidate = validate
            .project_candidate(&fixture.verified)
            .expect("project commitment-authorized Validate fixture");
        assert!(fixture.registry.entries[&fixture.address].validates_at(fixture.address));
        assert_eq!(candidate.causal_root, fixture.lease.owner().causal_root());
        assert_eq!(candidate.work_class, fixture.lease.work_class());
        assert_eq!(candidate.stage, fixture.lease.stage());
        assert_eq!(
            candidate
                .physical_geometry
                .normalized()
                .expect("normalize commitment-authorized Validate geometry")
                .0,
            *fixture.lease.physical_slots()
        );
        fixture.lease.key = candidate.key;
    }

    #[cfg(feature = "bls")]
    fn claimed_durable_validate_coordinator(
        fixture: &DurableValidateFixture,
    ) -> LifecycleCoordinator {
        let work = fixture
            .registry
            .entries
            .get(&fixture.address)
            .expect("dispatch fixture retains its closed Validate row");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            unreachable!("dispatch fixture retains one closed Validate carrier")
        };
        let context = fixture.verified.context();
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context_id),
            fixture.lease.key().round().height(),
        );
        let candidate = validate
            .project_candidate(&fixture.verified)
            .expect("project dispatch fixture Validate carrier");
        let high_water = fixture
            .lease
            .ordinal()
            .checked_sub(1)
            .expect("dispatch fixture ordinal is non-zero");
        let mut coordinator = LifecycleCoordinator::new(
            active_context,
            high_water,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
        );
        assert!(matches!(
            coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)),
            AdmissionDecision::Admitted {
                owner,
                ordinal,
                producer_turn_ordinal: None,
            } if owner == fixture.lease.owner() && ordinal == fixture.lease.ordinal()
        ));
        coordinator.ready_index.remove(&fixture.lease.ordinal());
        coordinator
            .records
            .get_mut(&fixture.lease.ordinal())
            .expect("dispatch fixture admitted its Validate row")
            .state = LifecycleState::Claimed(fixture.lease.id());
        coordinator.active_lease = Some(fixture.lease.clone());
        coordinator
    }

    #[cfg(feature = "bls")]
    fn durable_validation_source(fixture: &mut DurableValidateFixture) -> WaitSource {
        let prepared = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare dispatch fixture source");
        prepared.durable_validation_wait_source()
    }

    #[cfg(feature = "bls")]
    fn take_dispatch_registry(fixture: &mut DurableValidateFixture) -> LifecycleWorkRegistryHolder {
        LifecycleWorkRegistryHolder::from_registry_for_test(core::mem::take(&mut fixture.registry))
    }

    #[cfg(feature = "bls")]
    struct WaitingDurableValidateFixture {
        fixture: DurableValidateFixture,
        _directory: TempDir,
        store: V2BodyStore,
        durable: DurableBodyReceipt,
        coordinator: LifecycleCoordinator,
        holder: LifecycleWorkRegistryHolder,
        dispatch: DurableValidateDispatch,
    }

    #[cfg(feature = "bls")]
    fn waiting_durable_validate_fixture(marker: u8) -> WaitingDurableValidateFixture {
        waiting_durable_validate_fixture_at_view(marker, 2)
    }

    #[cfg(feature = "bls")]
    fn waiting_durable_validate_fixture_at_view(
        marker: u8,
        view: wire::View,
    ) -> WaitingDurableValidateFixture {
        let (mut fixture, directory, store, durable) =
            durable_validate_store_fixture_at_view(marker, view);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one waiting dispatch");
        WaitingDurableValidateFixture {
            fixture,
            _directory: directory,
            store,
            durable,
            coordinator,
            holder,
            dispatch,
        }
    }

    #[cfg(feature = "bls")]
    #[derive(Clone, Copy)]
    enum ReadyDurableValidateFixtureOutcome {
        Validated,
        Rejected,
    }

    #[cfg(feature = "bls")]
    struct ReadyDurableValidateFixture {
        fixture: DurableValidateFixture,
        _directory: TempDir,
        holder: LifecycleWorkRegistryHolder,
        lease: TurnLease,
        durable: DurableBodyReceipt,
    }

    #[cfg(feature = "bls")]
    fn ready_durable_validate_fixture(
        marker: u8,
        outcome: ReadyDurableValidateFixtureOutcome,
    ) -> ReadyDurableValidateFixture {
        ready_durable_validate_fixture_at_view(marker, 2, outcome)
    }

    #[cfg(feature = "bls")]
    fn ready_durable_validate_fixture_at_view(
        marker: u8,
        view: wire::View,
        outcome: ReadyDurableValidateFixtureOutcome,
    ) -> ReadyDurableValidateFixture {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture_at_view(marker, view);
        let executed = match outcome {
            ReadyDurableValidateFixtureOutcome::Validated => {
                let commitment =
                    ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
                dispatch
                    .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                    .expect("execute successful Ready Validate fixture")
            }
            ReadyDurableValidateFixtureOutcome::Rejected => dispatch
                .execute(&mut store, |_| {
                    Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                        "Ready Validate rejection diagnostic",
                    ))
                })
                .expect("execute rejected Ready Validate fixture"),
        };
        coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish Ready Validate completion fixture");
        let replacement_digest = holder.registry_for_test().entries[&fixture.address].digest;
        let mut lease = fixture.lease.clone();
        assert_eq!(
            lease
                .physical_slots
                .insert(fixture.slot, replacement_digest),
            Some(fixture.lease.physical_slots()[&fixture.slot])
        );
        lease.output_reservation = match outcome {
            ReadyDurableValidateFixtureOutcome::Validated => None,
            ReadyDurableValidateFixtureOutcome::Rejected => {
                Some(super::super::schema::LeaseCapacityReservation::new(
                    CapacityClass::Consensus,
                    coordinator.capacity_generation[&CapacityClass::Consensus],
                ))
            }
        };
        assert_eq!(
            coordinator.records[&lease.ordinal()].state,
            LifecycleState::Ready
        );
        ReadyDurableValidateFixture {
            fixture,
            _directory,
            holder,
            lease,
            durable,
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_moves_claim_to_current_external_wait_and_executes() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB0);
        let source = durable_validation_source(&mut fixture);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        coordinator.observed_generation.insert(source, 7);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        assert_eq!(wait, WaitToken::new(source, 7));
        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(coordinator.observed_generation.get(&source), Some(&7));
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);

        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact waiting Validate request");
        assert_eq!(executed.wait_token_for_test(), wait);
        assert_eq!(executed.outcome().durable_body(), &durable);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn dropping_unexecuted_durable_validate_dispatch_preserves_wait_and_registry() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB1);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        drop(dispatch);

        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn committed_durable_validate_dispatch_cannot_mint_a_second_request() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB2);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, lease.clone(), &fixture.verified)
            .expect("first exact claimed Validate mints one dispatch");
        let coordinator_after = format!("{coordinator:?}");
        let Err((error, returned_lease)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("waiting Validate must not mint a second dispatch")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned_lease, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_after);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        drop(dispatch);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_store_error_returns_the_exact_dispatch() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB3);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        let empty_directory = TempDir::new().expect("temporary empty Validate body store");
        let mut empty_store =
            V2BodyStore::open(empty_directory.path(), fixture.verified.context().clone())
                .expect("open empty Validate body store");

        let (error, dispatch) = dispatch
            .execute(&mut empty_store, |_| {
                Ok::<_, DetachedValidationError>(
                    ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment(),
                )
            })
            .expect_err("missing durable catalog row returns the dispatch");
        assert!(matches!(error, V2BodyStoreError::ReceiptMismatch));
        assert_eq!(dispatch.wait_token_for_test(), wait);

        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("returned dispatch remains executable against its exact store");
        assert_eq!(executed.wait_token_for_test(), wait);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_stale_foreign_and_wrong_kind_without_mutation() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB4);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let mut stale = fixture.lease.clone();
            stale.id = super::super::LeaseId(stale.id().0 + 1);

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                stale.clone(),
                &fixture.verified,
            ) else {
                panic!("stale lease must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, stale);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB5);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let mut holder = LifecycleWorkRegistryHolder::empty();
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("foreign empty registry must not mint a Validate dispatch")
            };
            assert_eq!(
                error,
                DurableValidateDispatchError::Registry(DurableValidateExecutionError::Registry(
                    RegistryError::Missing
                ))
            );
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB6);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let incumbent = fixture
                .registry
                .entries
                .remove(&fixture.address)
                .expect("wrong-kind fixture removes its closed Validate");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
                unreachable!("wrong-kind fixture starts with one closed Validate")
            };
            let DurableValidateBody {
                effect, pending, ..
            } = validate;
            let pending = ConcreteLifecycleWork::from_exact(effect, pending)
                .expect("rebuild exact pending Validate work");
            assert!(
                fixture
                    .registry
                    .entries
                    .insert(fixture.address, pending)
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("pending Validate row must not cross the closed-carrier dispatch")
            };
            assert_eq!(
                error,
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::WrongWorkKind
                )
            );
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_a_substituted_ledger_body_frame() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBE);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let metadata = coordinator
            .durable_records
            .get_mut(&fixture.lease.ordinal())
            .expect("claimed Validate retains durable metadata");
        let DurablePayloadReference::BodyFrame(mut substituted) = metadata.payload else {
            panic!("claimed Validate must retain one durable body frame")
        };
        substituted.frame = LifecycleDigest::new([0xEE; 32]);
        metadata.payload = DurablePayloadReference::BodyFrame(substituted);
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("a ledger frame foreign to the installed carrier must fail closed")
        };
        assert_eq!(
            error,
            DurableValidateDispatchError::Registry(
                DurableValidateExecutionError::InvalidValidateShape
            )
        );
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_max_generation_and_wait_source_alias() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB7);
            let source = durable_validation_source(&mut fixture);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            coordinator.observed_generation.insert(source, u64::MAX);
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("maximum wait generation must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::WaitGenerationExhausted);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB8);
            let source = durable_validation_source(&mut fixture);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Waiting(WaitToken::new(source, 0));
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("aliased external wait source must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::AliasedWaitSource);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_reverse_identity_aliases() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB9);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_key = fixture.lease.ordinal() + 1000;
            let alias = coordinator.records[&fixture.lease.ordinal()].clone();
            assert!(coordinator.records.insert(alias_key, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse internal-ordinal alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBA);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let key = fixture.lease.key();
            let alias_key = super::super::LifecycleKey::new(
                key.context(),
                key.round(),
                key.proposal_round(),
                key.subject(),
                super::super::LifecyclePhase::Apply,
                key.execution_commitment(),
            );
            assert_ne!(alias_key, key);
            assert!(
                coordinator
                    .key_index
                    .insert(alias_key, fixture.lease.ordinal())
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse key-index alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBB);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_root = super::super::CausalRoot::new(LifecycleDigest::new([0xBB; 32]));
            assert_ne!(alias_root, fixture.lease.owner().causal_root());
            assert!(
                coordinator
                    .owner_index
                    .insert(alias_root, fixture.lease.owner())
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse owner-index alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBC);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Ready;
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("duplicate lifecycle record key must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_capacity_classifier_is_exact_and_drop_inert() {
        let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBF);
        let before = format!("{:?}", fixture.registry);
        let digest = fixture.lease.physical_slots()[&fixture.slot];

        let seal = fixture
            .registry
            .classify_ready_validate_carrier(fixture.address, digest)
            .expect("exact durable Validate carrier mints one opaque seal");
        assert!(seal.matches(
            fixture.address.owner,
            fixture.address.ordinal,
            fixture.address.slot,
            digest,
        ));
        assert!(!seal.requires_consensus_capacity());
        assert!(seal.requires_io_dispatch());
        assert_eq!(
            fixture.registry.classify_ready_validate_carrier(
                fixture.address,
                LifecycleDigest::new([0xFF; 32]),
            ),
            Err(ReadyValidateCarrierError::Registry(
                RegistryError::DigestMismatch
            ))
        );
        assert_eq!(format!("{:?}", fixture.registry), before);

        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        coordinator.active_lease = None;
        coordinator.ready_index.insert(fixture.lease.ordinal());
        coordinator
            .records
            .get_mut(&fixture.lease.ordinal())
            .expect("exact Validate row remains installed")
            .state = LifecycleState::Ready;
        let ordinal = fixture.lease.ordinal();
        let holder = LifecycleWorkRegistryHolder::from_registry_for_test(fixture.registry);
        let coordinator_before = format!("{coordinator:?}");
        assert_eq!(
            coordinator.direct_registry_scheduler_inputs_for_test(&holder),
            Err(ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal })
        );
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validated_completion_atomically_publishes_exact_ready_carrier() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC0);
        let ordinal = fixture.lease.ordinal();
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let wait = dispatch.wait_token_for_test();
        let before_record = coordinator.records[&ordinal].clone();
        let before_records = coordinator.records.len();
        let before_high_water = coordinator.high_water;
        let before_capacity = coordinator.capacity_used.clone();
        let before_capacity_generation = coordinator.capacity_generation.clone();
        let before_durable = coordinator.durable_records.clone();
        let before_debts = coordinator.producer_debts.clone();
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact successful Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish exact successful Validate completion");
        let DurableValidateCompletionPublication::PublishedValidated(published) = publication
        else {
            panic!("successful body validation publishes the validated carrier")
        };
        let location = published.location_for_test();
        assert_eq!(location.address, fixture.address);
        assert_eq!(location.incumbent_digest, old_digest);
        assert_ne!(location.replacement_digest, old_digest);

        let record = &coordinator.records[&ordinal];
        assert_eq!(record.owner, fixture.lease.owner());
        assert_eq!(record.ordinal, ordinal);
        assert_eq!(record.state, LifecycleState::Ready);
        assert_eq!(record.physical_slots.len(), 1);
        assert_eq!(
            record.physical_slots.get(&fixture.slot),
            Some(&location.replacement_digest)
        );
        assert_eq!(record.episode, before_record.episode);
        assert_eq!(coordinator.records.len(), before_records);
        assert_eq!(coordinator.high_water, before_high_water);
        assert_eq!(coordinator.capacity_used, before_capacity);
        assert_eq!(coordinator.capacity_generation, before_capacity_generation);
        assert_eq!(coordinator.durable_records, before_durable);
        assert_eq!(coordinator.producer_debts, before_debts);
        assert_eq!(coordinator.observed_generation[&wait.source()], 1);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.active_lease.is_none());
        assert!(coordinator.ledger_store.is_none());
        assert_eq!(
            coordinator
                .attest_ready_validate_demand(&holder, ordinal)
                .expect("validated completion mints one exact scheduler attestation")
                .capacity_class(),
            None
        );
        let inputs = coordinator
            .direct_registry_scheduler_inputs_for_test(&holder)
            .expect("validated completion has no nested service episode");
        let (generations, ready) = inputs.into_parts();
        assert!(generations.is_empty());
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[&ordinal].live_debts(), [0; 6]);
        let plan_inputs = super::super::SchedulerInputs::new(generations, ready)
            .expect("one unique direct validated-completion row");
        let mut planned = coordinator.clone();
        let super::super::TurnPlan::Execute(lease) = planned.plan_turn(plan_inputs) else {
            panic!("direct validated completion must be selectable")
        };
        assert_eq!(lease.ordinal(), ordinal);
        assert!(lease.output_reservation().is_none());

        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let installed = &holder.registry_for_test().entries[&fixture.address];
        assert_eq!(installed.digest, location.replacement_digest);
        assert!(installed.validates_at(fixture.address));
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind
        else {
            panic!("successful validation installs one closed completion carrier")
        };
        assert_eq!(completion.address, fixture.address);
        assert_eq!(completion.incumbent_digest, old_digest);
        assert!(completion.incumbent.validates(old_digest));
        assert_eq!(completion.outcome.durable_body(), &durable);
        assert_eq!(
            completion
                .outcome
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );

        let corrupt_digest = if location.replacement_digest != LifecycleDigest::new([0xE7; 32]) {
            LifecycleDigest::new([0xE7; 32])
        } else {
            LifecycleDigest::new([0xE8; 32])
        };
        holder
            .registry_for_test_mut()
            .entries
            .get_mut(&fixture.address)
            .expect("validated completion remains installed")
            .digest = corrupt_digest;
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        assert_eq!(
            coordinator.direct_registry_scheduler_inputs_for_test(&holder),
            Err(ProductionSchedulerInputsError::InvalidValidateCarrier { ordinal })
        );
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validated_completion_rejects_conflicting_inherited_commitment_intact() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xCD);
        let yielded_commitment =
            ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let inherited_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"inherited commitment parent"),
            Hash::new(b"inherited commitment post"),
            Hash::new(b"inherited commitment writes"),
            1,
            Hash::new(b"inherited commitment wire"),
        );
        assert!(inherited_commitment.validate().is_ok());
        assert_ne!(inherited_commitment, yielded_commitment);
        seal_validate_fixture_commitment(&mut fixture, inherited_commitment);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("commitment-authorized Validate becomes one waiting dispatch");
        let executed = dispatch
            .execute(&mut store, |_| {
                Ok::<_, DetachedValidationError>(yielded_commitment)
            })
            .expect("body store retains the conflicting deterministic success");
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("inherited commitment must constrain asynchronous validation success")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::ConflictingValidationCommitment
                )
            )
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            returned
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(yielded_commitment)
        );
        assert_eq!(
            returned
                .executed
                .request
                .candidate_statement
                .and_then(RuntimeCandidateSemanticStatement::execution_commitment),
            Some(inherited_commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn rejected_completion_atomically_publishes_exact_ready_carrier() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC1);
        let ordinal = fixture.lease.ordinal();
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let wait = dispatch.wait_token_for_test();
        let before_record = coordinator.records[&ordinal].clone();
        let before_records = coordinator.records.len();
        let before_high_water = coordinator.high_water;
        let before_capacity = coordinator.capacity_used.clone();
        let before_capacity_generation = coordinator.capacity_generation.clone();
        let before_durable = coordinator.durable_records.clone();
        let before_debts = coordinator.producer_debts.clone();
        let executed = dispatch
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "deterministic rejected completion",
                ))
            })
            .expect("execute exact rejected Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish exact rejected Validate completion");
        let DurableValidateCompletionPublication::PublishedRejected(published) = publication else {
            panic!("deterministic rejection publishes the rejected carrier")
        };
        let location = published.location_for_test();
        assert_eq!(location.address, fixture.address);
        assert_eq!(location.incumbent_digest, old_digest);
        assert_ne!(location.replacement_digest, old_digest);

        let record = &coordinator.records[&ordinal];
        assert_eq!(record.owner, fixture.lease.owner());
        assert_eq!(record.ordinal, ordinal);
        assert_eq!(record.state, LifecycleState::Ready);
        assert_eq!(record.physical_slots.len(), 1);
        assert_eq!(
            record.physical_slots.get(&fixture.slot),
            Some(&location.replacement_digest)
        );
        assert_eq!(record.episode, before_record.episode);
        assert_eq!(coordinator.records.len(), before_records);
        assert_eq!(coordinator.high_water, before_high_water);
        assert_eq!(coordinator.capacity_used, before_capacity);
        assert_eq!(coordinator.capacity_generation, before_capacity_generation);
        assert_eq!(coordinator.durable_records, before_durable);
        assert_eq!(coordinator.producer_debts, before_debts);
        assert_eq!(coordinator.observed_generation[&wait.source()], 1);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.ledger_store.is_none());
        let attestation = coordinator
            .attest_ready_validate_demand(&holder, ordinal)
            .expect("rejected completion mints one exact scheduler attestation");
        assert_eq!(attestation.capacity_class(), Some(CapacityClass::Consensus));
        let inputs = coordinator
            .direct_registry_scheduler_inputs_for_test(&holder)
            .expect("rejected completion has no nested service episode");
        let (generations, ready) = inputs.into_parts();
        assert!(generations.is_empty());
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[&ordinal].live_debts(), [0; 6]);

        let mut stale = coordinator.clone();
        stale
            .records
            .get_mut(&ordinal)
            .expect("rejected completion row")
            .physical_slots
            .insert(fixture.slot, LifecycleDigest::new([0xEF; 32]));
        let stale_before = format!("{stale:?}");
        assert_eq!(
            stale.attest_ready_validate_demand(&holder, ordinal),
            Err(ReadyValidateDemandAttestationError::Registry(
                ReadyValidateCarrierError::Registry(RegistryError::DigestMismatch)
            ))
        );
        assert_eq!(format!("{stale:?}"), stale_before);

        let mut substituted = coordinator.clone();
        let metadata = substituted
            .durable_records
            .get_mut(&ordinal)
            .expect("rejected completion retains durable metadata");
        let DurablePayloadReference::BodyFrame(mut foreign_frame) = metadata.payload else {
            panic!("rejected completion must retain one durable body frame")
        };
        foreign_frame.manifest = LifecycleDigest::new([0xED; 32]);
        metadata.payload = DurablePayloadReference::BodyFrame(foreign_frame);
        let substituted_before = format!("{substituted:?}");
        assert_eq!(
            substituted.attest_ready_validate_demand(&holder, ordinal),
            Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)
        );
        assert_eq!(format!("{substituted:?}"), substituted_before);

        let inputs = super::super::SchedulerInputs::new(generations, ready)
            .expect("one unique registry-attested Ready row");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("registry-attested rejected Validate must claim with its reservation")
        };
        assert_eq!(lease.ordinal(), ordinal);
        assert_eq!(
            lease
                .output_reservation()
                .map(|reservation| reservation.class()),
            Some(CapacityClass::Consensus)
        );

        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let installed = &holder.registry_for_test().entries[&fixture.address];
        assert_eq!(installed.digest, location.replacement_digest);
        assert!(installed.validates_at(fixture.address));
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind
        else {
            panic!("rejection installs one closed completion carrier")
        };
        assert_eq!(completion.incumbent_digest, old_digest);
        assert!(completion.incumbent.validates(old_digest));
        assert_eq!(completion.outcome.durable_body(), &durable);
        assert_eq!(
            completion.outcome.rejection_reason(),
            Some("deterministic rejected completion")
        );
        assert!(completion.outcome.validated_receipt().is_none());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_execution_preflight_binds_closed_outcomes_and_is_drop_inert() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable,
            } = ready_durable_validate_fixture(0xD0, ReadyDurableValidateFixtureOutcome::Validated);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact validated Ready carrier");
            assert_eq!(
                prepared.outcome_kind(),
                ReadyDurableValidateOutcomeKind::Validated
            );
            assert!(prepared.matches_exact_lease(&lease));
            assert!(prepared.matches_exact_durable_receipt(&durable));
            let foreign_receipt = DurableBodyReceipt::for_test(
                durable.context_id(),
                durable.round(),
                durable.subject(),
                HashOf::from_untyped_unchecked(Hash::new(b"foreign Ready Validate manifest")),
            );
            assert!(!prepared.matches_exact_durable_receipt(&foreign_receipt));
            let mut foreign_lease = lease.clone();
            foreign_lease.id = LeaseId(
                foreign_lease
                    .id()
                    .0
                    .checked_add(1)
                    .expect("fixture lease id remains bounded"),
            );
            assert!(!prepared.matches_exact_lease(&foreign_lease));
            assert!(prepared.validated_authority().is_some());
            assert!(prepared.rejected_authority().is_none());
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable,
            } = ready_durable_validate_fixture(0xD1, ReadyDurableValidateFixtureOutcome::Rejected);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact rejected Ready carrier");
            assert_eq!(
                prepared.outcome_kind(),
                ReadyDurableValidateOutcomeKind::Rejected
            );
            assert!(prepared.matches_exact_durable_receipt(&durable));
            assert!(prepared.rejected_authority().is_some());
            assert!(prepared.validated_authority().is_none());
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDA, ReadyDurableValidateFixtureOutcome::Rejected);
            lease.output_reservation = None;
            let before = format!("{:?}", holder.registry_for_test());
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDB, ReadyDurableValidateFixtureOutcome::Validated);
            lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
                CapacityClass::Consensus,
                0,
            ));
            let before = format!("{:?}", holder.registry_for_test());
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn assert_ready_validate_commit_sign_live_transaction(attach_ledger: bool) {
        let marker = 0xE0;
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            holder: _,
            lease: _,
            durable,
        } = ready_durable_validate_fixture_at_view(
            marker,
            0,
            ReadyDurableValidateFixtureOutcome::Validated,
        );
        let (tag, round, subject) = match &fixture.effect {
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => (*tag, *round, *subject),
            _ => unreachable!("Ready fixture retains one Validate effect"),
        };
        let adapter_directory = TempDir::new().expect("temporary Ready Validate adapter");
        let wal_path = adapter_directory.path().join("safety.wal");
        let (mut adapter, startup) = SumeragiV2Adapter::open(
            &wal_path,
            fixture.verified.clone(),
            Some(0),
            tag.generation(),
            [0xE0; 32],
            AdapterFingerprints {
                node: Hash::new(b"Ready Validate registry join node"),
                build: Hash::new(b"Ready Validate registry join build"),
                config: Hash::new(b"Ready Validate registry join config"),
            },
            DeferredAdmissionOrdinalSource::new(1),
        )
        .expect("open exact Ready Validate adapter");
        assert!(startup.is_empty());

        let proposal = wire::Proposal {
            round,
            proposer: fixture.verified.context().leader(round.view),
            subject,
            manifest: fixture.manifest.clone(),
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![marker],
        };
        let fetch = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                    proposal.clone(),
                )),
            ))
            .expect("admit exact Ready Validate proposal")
            .into_effects();
        assert!(matches!(
            fetch.as_slice(),
            [AdapterEffect::FetchBody {
                tag: effect_tag,
                manifest: Some(effect_manifest),
                ..
            }] if *effect_tag == tag && effect_manifest == &fixture.manifest
        ));
        let stored = adapter
            .body_available(tag, fixture.manifest.clone())
            .expect("advance exact Ready Validate body to Store")
            .into_effects();
        assert!(matches!(
            stored.as_slice(),
            [AdapterEffect::StoreBody {
                tag: effect_tag,
                round: effect_round,
                subject: effect_subject,
            }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
        ));
        let validate = adapter
            .body_stored(tag, round, subject, &durable)
            .expect("advance exact Ready Validate body to Validate")
            .into_effects();
        assert!(matches!(
            validate.as_slice(),
            [AdapterEffect::ValidateBody {
                tag: effect_tag,
                round: effect_round,
                subject: effect_subject,
            }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
        ));

        let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: validated_receipt.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    prepare,
                )),
            ))
            .expect("register exact concurrent PrepareQC");
        assert!(observed.effects().is_empty());
        let mut holder = LifecycleWorkRegistryHolder::empty();
        let (lease, slot, coordinator_candidate) = holder
            .install_remote_proposal_validate_completion_for_test(
                &fixture.verified,
                tag,
                proposal,
                fixture.manifest.clone(),
                validated_receipt,
            );
        let registry_before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, &fixture.verified)
            .expect("prepare exact Ready Validate registry carrier");
        let preview = prepared
            .prepare_adapter_preview(&mut adapter)
            .unwrap_or_else(|_| panic!("join exact registry carrier to adapter preview"));
        let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
        let persisted = preview
            .seal_live_wal_validate_sign()
            .unwrap_or_else(|_| panic!("seal exact Ready Validate Sign to real WAL"));
        let wal_after = std::fs::read(&wal_path).expect("read persisted Ready Validate WAL");
        assert!(wal_after.len() > wal_before.len());

        let active_context = LifecycleContext::new(
            coordinator_candidate.key.context(),
            coordinator_candidate.key.round().height(),
        );
        let mut coordinator = LifecycleCoordinator::new(
            active_context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
        );
        assert!(matches!(
            coordinator.reduce_admit(AdmissionRequest::Candidate(coordinator_candidate)),
            AdmissionDecision::Admitted {
                owner,
                ordinal,
                producer_turn_ordinal: None,
            } if owner == lease.owner() && ordinal == lease.ordinal()
        ));
        coordinator.ready_index.remove(&lease.ordinal());
        let parent = coordinator
            .records
            .get_mut(&lease.ordinal())
            .expect("admitted Validate parent");
        parent.physical_slots = lease.physical_slots().clone();
        parent.state = LifecycleState::Claimed(lease.id());
        coordinator.active_lease = Some(lease.clone());
        if !attach_ledger {
            let result = coordinator.prepare_sealed_validate_sign_transition(
                &lease,
                &fixture.verified,
                persisted,
            );
            assert!(result.is_err());
            drop(result);
            assert!(coordinator.ledger_store.is_none());
            assert_eq!(
                coordinator.records[&lease.ordinal()].state,
                LifecycleState::Claimed(lease.id())
            );
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
            assert_eq!(
                std::fs::read(&wal_path).expect("read WAL after missing-store rejection"),
                wal_after
            );
            assert!(matches!(
                adapter.body_available(tag, fixture.manifest.clone()),
                Err(AdapterError::FailClosed)
            ));
            return;
        }
        let ledger_directory = TempDir::new().expect("temporary live publication ledger");
        coordinator
            .attach_empty_test_ledger(ledger_directory.path())
            .expect("attach exact current LedgerV1");

        coordinator
            .prepare_sealed_validate_sign_transition(&lease, &fixture.verified, persisted)
            .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Commit transaction"))
            .persist_and_publish()
            .unwrap_or_else(|_| panic!("fsync and publish exact live Validate-to-Commit cut"));

        let child_ordinal = lease.ordinal() + 1;
        let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)
            .expect("exact live Sign child address");
        assert_ne!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let child_work = holder
            .registry_for_test()
            .entries
            .get(&child_address)
            .expect("reserved Sign child is installed");
        assert!(child_work.validate_exact());
        assert_eq!(child_work.causal_root(), lease.owner().causal_root());
        assert!(matches!(
            &child_work.kind,
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect:
                    AdapterEffect::Sign {
                        request: SignRequest::Vote(vote),
                        ..
                    },
                ..
            } if vote.phase == wire::GlobalPhase::Commit
        ));
        assert_eq!(
            coordinator.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            coordinator.durable_records[&lease.ordinal()].continuation,
            super::super::schema::DurableContinuation::successor(
                super::super::schema::DurableContinuationEdge::ValidateToSignCommit,
                child_ordinal,
            )
        );
        assert_eq!(
            coordinator.records[&child_ordinal].state,
            LifecycleState::Ready
        );
        assert_eq!(
            coordinator.records[&child_ordinal].stage.kind(),
            LifecycleStageKind::SignCommitVote
        );
        assert!(coordinator.active_lease.is_none());
        assert!(adapter.signature_fence_is_active());
        assert!(matches!(
            adapter.signature_fence_identity(),
            Some((identity_tag, reducer::SignableMessage::Vote(vote)))
                if identity_tag == tag && vote.phase() == reducer::Phase::Commit
        ));
        let (_, reopened) = super::super::ledger::LifecycleLedgerStoreV1::open(
            ledger_directory.path(),
            active_context,
        )
        .expect("reopen exact committed LedgerV1");
        assert_eq!(reopened.high_water(), child_ordinal);
        assert_eq!(reopened.records().len(), 2);
        assert_eq!(
            reopened.records()[0].terminal(),
            Some(Some(TerminalOutcome::Advanced))
        );
        assert_eq!(
            reopened.records()[0].continuation(),
            Some(super::super::schema::DurableContinuation::successor(
                super::super::schema::DurableContinuationEdge::ValidateToSignCommit,
                child_ordinal,
            ))
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_commit_sign_publishes_one_atomic_live_transaction() {
        assert_ready_validate_commit_sign_live_transaction(true);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_commit_sign_rejects_missing_ledger_store_and_fails_closed() {
        assert_ready_validate_commit_sign_live_transaction(false);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_validate_cut_detaches_only_validated_completion_and_restores_on_drop() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDC, ReadyDurableValidateFixtureOutcome::Validated);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact validated recovered-WAL parent");
            let cut = match prepared.into_recovered_wal_validate_registry_cut() {
                Ok(cut) => cut,
                Err(_prepared) => panic!("validated completion must detach into WAL parent cut"),
            };
            assert!(cut.detached_work_is_exact_for_test());
            drop(cut);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDD, ReadyDurableValidateFixtureOutcome::Rejected);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact rejected recovered-WAL parent candidate");
            let prepared = match prepared.into_recovered_wal_validate_registry_cut() {
                Ok(_cut) => panic!("rejected completion cannot become a WAL vote parent"),
                Err(prepared) => prepared,
            };
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn ready_validate_execution_preflight_rejects_foreign_or_malformed_authority() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD2, ReadyDurableValidateFixtureOutcome::Validated);
            lease.owner = OwnerId::new(
                super::super::CausalRoot::new(LifecycleDigest::new([0xD2; 32])),
                lease.owner.first_admission_ordinal(),
            );
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::Missing
                ))
            ));
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD3, ReadyDurableValidateFixtureOutcome::Validated);
            lease
                .physical_slots
                .insert(fixture.slot, LifecycleDigest::new([0xD3; 32]));
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::DigestMismatch
                ))
            ));
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD4, ReadyDurableValidateFixtureOutcome::Rejected);
            lease.stage = super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::StoreBody,
                super::super::PredecessorScope::Independent,
            );
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xD5);
            assert!(matches!(
                fixture.registry.prepare_ready_durable_validate_execution(
                    &fixture.lease,
                    fixture.slot,
                    &fixture.verified,
                ),
                Err(ReadyDurableValidateExecutionError::WrongWorkKind)
            ));
        }

        {
            let mut exact =
                ready_durable_validate_fixture(0xD6, ReadyDurableValidateFixtureOutcome::Validated);
            let WaitingDurableValidateFixture {
                fixture: deferred_fixture,
                _directory: deferred_directory,
                mut store,
                durable,
                coordinator: _,
                holder: _,
                dispatch,
            } = waiting_durable_validate_fixture(0xD7);
            let reference = detached_validation_merge_reference(&durable);
            let deferred = dispatch
                .execute(&mut store, |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        DetachedValidationError::MissingMergeSidecar(reference),
                    )
                })
                .expect("execute foreign deferred outcome");
            let ExecutedDurableValidateDispatch {
                executed: ExecutedDurableValidateExecution { outcome, .. },
                ..
            } = deferred;
            let work = exact
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&exact.fixture.address)
                .expect("exact fixture retains Ready carrier");
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &mut work.kind
            else {
                unreachable!("exact fixture retains Ready completion")
            };
            completion.outcome = outcome;
            let _keep_foreign_files = deferred_directory;
            assert_ne!(deferred_fixture.address, exact.fixture.address);
            assert!(matches!(
                exact
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &exact.lease,
                        exact.fixture.slot,
                        &exact.fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork
                ))
            ));
        }

        {
            let mut first =
                ready_durable_validate_fixture(0xD8, ReadyDurableValidateFixtureOutcome::Validated);
            let mut foreign =
                ready_durable_validate_fixture(0xD9, ReadyDurableValidateFixtureOutcome::Rejected);
            let first_work = first
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&first.fixture.address)
                .expect("first fixture retains Ready carrier");
            let foreign_work = foreign
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&foreign.fixture.address)
                .expect("foreign fixture retains Ready carrier");
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(first_completion) =
                &mut first_work.kind
            else {
                unreachable!("first fixture retains Ready completion")
            };
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(foreign_completion) =
                &mut foreign_work.kind
            else {
                unreachable!("foreign fixture retains Ready completion")
            };
            core::mem::swap(
                &mut first_completion.outcome,
                &mut foreign_completion.outcome,
            );
            assert!(matches!(
                first
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &first.lease,
                        first.fixture.slot,
                        &first.fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork
                ))
            ));
        }

        {
            let mut exact =
                ready_durable_validate_fixture(0xDE, ReadyDurableValidateFixtureOutcome::Rejected);
            let foreign = durable_validate_fixture(0xDF);
            let before = format!("{:?}", exact.holder.registry_for_test());
            assert!(matches!(
                exact
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &exact.lease,
                        exact.fixture.slot,
                        &foreign.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Projection(_))
            ));
            assert_eq!(format!("{:?}", exact.holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn rejected_completion_digest_ignores_diagnostic_display_text() {
        let first = waiting_durable_validate_fixture(0xCE);
        let second = waiting_durable_validate_fixture(0xCE);
        let WaitingDurableValidateFixture {
            fixture: first_fixture,
            _directory: first_directory,
            store: mut first_store,
            durable: first_durable,
            coordinator: _first_coordinator,
            holder: _first_holder,
            dispatch: first_dispatch,
        } = first;
        let WaitingDurableValidateFixture {
            fixture: second_fixture,
            _directory: second_directory,
            store: mut second_store,
            durable: second_durable,
            coordinator: _second_coordinator,
            holder: _second_holder,
            dispatch: second_dispatch,
        } = second;
        assert_eq!(first_fixture.address, second_fixture.address);
        assert_eq!(first_durable, second_durable);
        let first_executed = first_dispatch
            .execute(&mut first_store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "diagnostic wording alpha",
                ))
            })
            .expect("execute first deterministic rejection");
        let second_executed = second_dispatch
            .execute(&mut second_store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "diagnostic wording beta",
                ))
            })
            .expect("execute second deterministic rejection");
        assert_ne!(
            first_executed.outcome().rejection_reason(),
            second_executed.outcome().rejection_reason()
        );
        assert_eq!(
            first_executed.outcome().rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert_eq!(
            first_executed.outcome().rejection_identity(),
            second_executed.outcome().rejection_identity()
        );
        let incumbent_digest = first_fixture.lease.physical_slots()[&first_fixture.slot];
        let first_digest = durable_validate_completion_digest(
            incumbent_digest,
            first_fixture.expected_manifest_hash,
            first_executed.outcome(),
        )
        .expect("first rejection derives one replacement digest");
        let second_digest = durable_validate_completion_digest(
            incumbent_digest,
            second_fixture.expected_manifest_hash,
            second_executed.outcome(),
        )
        .expect("second rejection derives one replacement digest");
        assert_ne!(first_digest, incumbent_digest);
        assert_eq!(first_digest, second_digest);
        drop(first_directory);
        drop(second_directory);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn merge_sidecar_deferral_retains_dispatch_and_leaves_waiting_row_original() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC2);
        let reference = detached_validation_merge_reference(&durable);
        let wait = dispatch.wait_token_for_test();
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let executed = dispatch
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("execute exact deferred Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("retain exact merge-sidecar deferral");
        let DurableValidateCompletionPublication::DeferredMergeSidecar(deferred) = publication
        else {
            panic!("missing merge sidecar must not publish an executable carrier")
        };
        assert_eq!(deferred.missing_reference(), &reference);
        assert_eq!(deferred.dispatch_for_test().wait_token_for_test(), wait);
        assert_eq!(
            deferred.dispatch_for_test().outcome().durable_body(),
            &durable
        );
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].physical_slots[&fixture.slot],
            old_digest
        );
        assert!(!coordinator.ready_index.contains(&fixture.lease.ordinal()));
        assert!(matches!(
            holder.registry_for_test().entries[&fixture.address].kind,
            ConcreteLifecycleWorkKind::DurableValidateBody(_)
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn validate_completion_precommit_failures_preserve_both_sides_and_dispatch() {
        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC3);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let mut executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute stale-digest completion fixture");
            executed.executed.request.incumbent_digest = LifecycleDigest::new([0xC3; 32]);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("stale incumbent digest must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                    )
                )
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
            assert_eq!(returned.outcome().durable_body(), &durable);
            assert_eq!(returned.executed.request.address, fixture.address);
        }

        {
            let WaitingDurableValidateFixture {
                fixture: _,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC4);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let mut executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute stale-address completion fixture");
            executed.executed.request.address.slot = PhysicalSlotId::for_capacity(
                CapacityClass::Effect,
                executed.executed.request.address.slot.1.saturating_add(1),
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((_, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("foreign Validate address must fail before publication")
            };
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
            assert_eq!(returned.outcome().durable_body(), &durable);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC5);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute wrong-carrier completion fixture");
            let incumbent = holder
                .registry_for_test_mut()
                .entries
                .remove(&fixture.address)
                .expect("wrong-carrier fixture removes exact Validate incumbent");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
                unreachable!("wrong-carrier fixture starts with durable Validate")
            };
            let pending = ConcreteLifecycleWork::from_exact(validate.effect, validate.pending)
                .expect("rebuild pending Validate wrong carrier");
            assert!(
                holder
                    .registry_for_test_mut()
                    .entries
                    .insert(fixture.address, pending)
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("wrong concrete carrier must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::WrongWorkKind
                    )
                )
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC6);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute key-mutation completion fixture");
            let old_key = fixture.lease.key();
            let foreign_subject = LifecycleDigest::new([0xC6; 32]);
            let foreign_key = super::super::LifecycleKey::new(
                old_key.context(),
                old_key.round(),
                old_key.proposal_round(),
                Some(foreign_subject),
                LifecyclePhase::Validate,
                old_key.execution_commitment(),
            );
            assert_ne!(foreign_key, old_key);
            assert_eq!(
                coordinator.key_index.remove(&old_key),
                Some(fixture.lease.ordinal())
            );
            coordinator
                .records
                .get_mut(&fixture.lease.ordinal())
                .expect("key-mutation fixture retains target record")
                .key = foreign_key;
            assert!(
                coordinator
                    .key_index
                    .insert(foreign_key, fixture.lease.ordinal())
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("consistent key/index mutation must fail exact async authority")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC7);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute corrupt-episode completion fixture");
            coordinator
                .records
                .get_mut(&fixture.lease.ordinal())
                .expect("episode corruption fixture retains target record")
                .episode
                .frozen_predecessors
                .insert(fixture.lease.ordinal() + 1000);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("corrupt independent episode must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validate_completion_rejects_reverse_index_and_duplicate_record_key_intact() {
        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xCA);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute reverse-index completion fixture");
            let key = fixture.lease.key();
            let alias_key = super::super::LifecycleKey::new(
                key.context(),
                key.round(),
                key.proposal_round(),
                key.subject(),
                LifecyclePhase::Apply,
                key.execution_commitment(),
            );
            assert_ne!(alias_key, key);
            assert!(
                coordinator
                    .key_index
                    .insert(alias_key, fixture.lease.ordinal())
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("reverse key-index alias must fail completion preflight")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xCB);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute duplicate-key completion fixture");
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Ready;
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("duplicate lifecycle record key must fail completion preflight")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validate_completion_guard_restores_incumbent_on_unwind_before_swap() {
        let WaitingDurableValidateFixture {
            fixture: _,
            _directory,
            mut store,
            durable,
            coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC8);
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute unwind completion fixture");
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_executed_durable_validate_completion(executed)
            .expect("reattach unwind completion fixture");

        let unwind = catch_unwind(AssertUnwindSafe(move || {
            let _staged = prepared
                .stage_executable_carrier()
                .expect("stage unwind-safe Validate carrier");
            panic!("test-only panic before coordinator swap");
        }));
        assert!(unwind.is_err());
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn duplicate_old_digest_completion_cas_returns_exact_dispatch_intact() {
        let first = waiting_durable_validate_fixture(0xC9);
        let second = waiting_durable_validate_fixture(0xC9);
        let WaitingDurableValidateFixture {
            fixture: first_fixture,
            _directory: first_directory,
            store: mut first_store,
            durable: first_durable,
            coordinator: mut first_coordinator,
            holder: mut first_holder,
            dispatch: first_dispatch,
        } = first;
        let WaitingDurableValidateFixture {
            fixture: second_fixture,
            _directory: second_directory,
            store: mut second_store,
            durable: second_durable,
            coordinator: _second_coordinator,
            holder: _second_holder,
            dispatch: second_dispatch,
        } = second;
        assert_eq!(first_fixture.address, second_fixture.address);
        assert_eq!(first_durable, second_durable);
        let first_commitment =
            ValidatedBodyReceipt::for_test(first_durable.clone()).execution_commitment();
        let second_commitment =
            ValidatedBodyReceipt::for_test(second_durable).execution_commitment();
        let first_executed = first_dispatch
            .execute(&mut first_store, |_| {
                Ok::<_, DetachedValidationError>(first_commitment)
            })
            .expect("execute first duplicate-CAS fixture");
        let second_executed = second_dispatch
            .execute(&mut second_store, |_| {
                Ok::<_, DetachedValidationError>(second_commitment)
            })
            .expect("execute second duplicate-CAS fixture");
        let mut waiting_again = first_coordinator.clone();
        first_coordinator
            .complete_durable_validate_dispatch(&mut first_holder, first_executed)
            .expect("publish first exact completion carrier");
        let coordinator_before = format!("{waiting_again:?}");
        let registry_before = format!("{:?}", first_holder.registry_for_test());
        let dispatch_before = format!("{second_executed:?}");

        let Err((error, returned)) =
            waiting_again.complete_durable_validate_dispatch(&mut first_holder, second_executed)
        else {
            panic!("old-digest completion must not replace an installed completion")
        };
        assert!(matches!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                        | DurableValidateExecutionError::WrongWorkKind
                )
            )
        ));
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{waiting_again:?}"), coordinator_before);
        assert_eq!(
            format!("{:?}", first_holder.registry_for_test()),
            registry_before
        );
        drop(first_directory);
        drop(second_directory);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_store_prepare_seal_and_drop_preserve_the_closed_row() {
        let DurableStoreFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
        } = durable_store_fixture(0x41);
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = effect.clone()
        else {
            unreachable!("durable Store fixture retains its Store effect")
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let before = format!("{registry:?}");

        let prepared = registry
            .prepare_durable_store_execution(&lease, slot, &verified)
            .expect("prepare exact durable Store execution");
        assert_eq!(prepared.adapter_preview_inputs(), (tag, round, subject));
        assert_eq!(prepared.durable_body_receipt().round(), round);
        assert_eq!(prepared.durable_body_receipt().subject(), subject);
        assert_eq!(
            prepared.durable_body_receipt().manifest_hash(),
            expected_manifest_hash
        );
        assert_eq!(prepared.expected_manifest_hash(), expected_manifest_hash);
        let sealed = prepared
            .seal_validate_successor(&validate_effect)
            .expect("seal exact ordinal-free Validate successor");
        assert_eq!(sealed._store_address, address);
        assert_eq!(sealed._validate_effect, validate_effect);
        assert!(
            sealed
                ._validate_pending
                .exactly_binds_adapter_effect(&sealed._validate_effect)
        );
        assert_eq!(
            sealed._validate_digest,
            digest_from_hash(sealed._validate_pending.exact_effect_identity())
        );
        assert_eq!(
            super::super::CausalRoot::new(digest_from_hash(
                sealed._validate_pending.causal_lifecycle_key()
            )),
            lease.owner().causal_root()
        );
        assert_eq!(
            sealed._durable_body.manifest_hash(),
            sealed._expected_manifest_hash
        );
        drop(sealed);

        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
        assert_eq!(
            registry.borrow_for_lease(&lease, slot),
            Err(RegistryError::WrongWorkKind)
        );
        assert!(matches!(
            registry.take_for_lease(&lease, slot),
            Err(RegistryError::WrongWorkKind)
        ));
        assert_eq!(format!("{registry:?}"), before);

        let mut disposable = durable_store_fixture(0x42);
        let closed = disposable
            .registry
            .entries
            .remove(&disposable.address)
            .expect("remove disposable closed Store only for into-pair rejection test");
        let unwind = catch_unwind(AssertUnwindSafe(|| closed.into_pair()));
        assert!(unwind.is_err(), "closed Store must not expose a raw pair");
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_store_prepare_rejects_foreign_retained_origin_without_mutation() {
        let DurableStoreFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            ..
        } = durable_store_fixture(0x43);
        {
            let work = registry
                .entries
                .get_mut(&address)
                .expect("foreign-origin fixture retains its Store carrier");
            let installed_digest = work.digest;
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("foreign-origin fixture retains one Store carrier")
            };
            assert!(store.replay_evidence.replace_with_foreign_origin_for_test());
            assert!(!store.validates(installed_digest));
            assert!(matches!(
                store.project_candidate(&verified),
                Err(AdapterEffectAdmissionError::InvalidCarrier)
            ));
        }
        let before = format!("{registry:?}");
        assert!(matches!(
            registry.prepare_durable_store_execution(&lease, slot, &verified),
            Err(DurableStoreExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
        assert_eq!(format!("{registry:?}"), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_store_prepare_rejects_wrong_lease_projection_and_context_without_mutation() {
        let DurableStoreFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            ..
        } = durable_store_fixture(0x51);
        let before = format!("{registry:?}");

        let mut wrong_class = lease.clone();
        wrong_class.work_class = LifecycleWorkClass::Fetch;
        assert!(matches!(
            registry.prepare_durable_store_execution(&wrong_class, slot, &verified),
            Err(DurableStoreExecutionError::InvalidLeaseShape)
        ));

        let other_slot = PhysicalSlotId::for_capacity(lease.work_class().capacity_class(), 1);
        assert!(matches!(
            registry.prepare_durable_store_execution(&lease, other_slot, &verified),
            Err(DurableStoreExecutionError::InvalidLeaseShape)
        ));

        let mut wrong_digest = lease.clone();
        wrong_digest
            .physical_slots
            .insert(slot, LifecycleDigest::new([0xD1; 32]));
        assert!(matches!(
            registry.prepare_durable_store_execution(&wrong_digest, slot, &verified),
            Err(DurableStoreExecutionError::Registry(
                RegistryError::DigestMismatch
            ))
        ));

        let mut stale = lease.clone();
        stale.ordinal = stale.ordinal.saturating_add(1);
        assert!(matches!(
            registry.prepare_durable_store_execution(&stale, slot, &verified),
            Err(DurableStoreExecutionError::Registry(RegistryError::Missing))
        ));

        let exact_key = lease.key();
        let mut wrong_key = lease.clone();
        wrong_key.key = super::super::LifecycleKey::new(
            exact_key.context(),
            exact_key.round(),
            exact_key.proposal_round(),
            Some(LifecycleDigest::new([0xE1; 32])),
            exact_key.phase(),
            exact_key.execution_commitment(),
        );
        assert!(matches!(
            registry.prepare_durable_store_execution(&wrong_key, slot, &verified),
            Err(DurableStoreExecutionError::InvalidProjection)
        ));

        let (foreign_verified, _) = verified_store_context(0x52);
        assert!(matches!(
            registry.prepare_durable_store_execution(&lease, slot, &foreign_verified),
            Err(DurableStoreExecutionError::Projection(
                AdapterEffectAdmissionError::ForeignContext
            ))
        ));

        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_store_seal_rejects_wrong_kind_or_tag_and_wrong_row_kind() {
        let DurableStoreFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            ..
        } = durable_store_fixture(0x61);
        let before = format!("{registry:?}");

        let prepared = registry
            .prepare_durable_store_execution(&lease, slot, &verified)
            .expect("prepare Store before wrong-kind successor");
        assert!(matches!(
            prepared.seal_validate_successor(&effect),
            Err(DurableStoreExecutionError::InvalidValidateSuccessor)
        ));
        assert_eq!(format!("{registry:?}"), before);

        let AdapterEffect::StoreBody { round, subject, .. } = effect.clone() else {
            unreachable!("durable Store fixture retains its Store effect")
        };
        let wrong_tag_validate = AdapterEffect::ValidateBody {
            tag: EventTag::new(round.height, round.view, Generation::new(999)),
            round,
            subject,
        };
        let prepared = registry
            .prepare_durable_store_execution(&lease, slot, &verified)
            .expect("prepare Store before wrong-tag successor");
        assert!(matches!(
            prepared.seal_validate_successor(&wrong_tag_validate),
            Err(DurableStoreExecutionError::InvalidValidateSuccessor)
        ));
        assert_eq!(format!("{registry:?}"), before);

        let closed = registry
            .entries
            .remove(&address)
            .expect("test-only conversion of closed row to pending kind");
        let ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableStoreBody(store),
        } = closed
        else {
            unreachable!("fixture retains one closed Store row")
        };
        let DurableStoreBody {
            effect, pending, ..
        } = store;
        let pending_work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
        };
        assert!(pending_work.validate_exact());
        assert!(registry.entries.insert(address, pending_work).is_none());
        assert!(matches!(
            registry.prepare_durable_store_execution(&lease, slot, &verified),
            Err(DurableStoreExecutionError::WrongWorkKind)
        ));
    }

    #[cfg(feature = "bls")]
    fn assert_corrupt_durable_store_rejected(
        marker: u8,
        corrupt: impl FnOnce(&mut ConcreteLifecycleWork),
    ) {
        let DurableStoreFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            ..
        } = durable_store_fixture(marker);
        let work = registry
            .entries
            .get_mut(&address)
            .expect("corruption fixture retains its closed Store row");
        corrupt(work);
        assert!(!work.validate_exact());
        let before = format!("{registry:?}");
        assert!(matches!(
            registry.prepare_durable_store_execution(&lease, slot, &verified),
            Err(DurableStoreExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
        assert_eq!(format!("{registry:?}"), before);
        assert_eq!(registry.len(), 1);
        assert!(registry.entries.contains_key(&address));
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn durable_store_validation_rejects_every_corrupt_closed_coordinate() {
        assert_corrupt_durable_store_rejected(0x71, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            store.address.ordinal = 0;
        });
        assert_corrupt_durable_store_rejected(0x72, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            let foreign_owner = owner(0xF2, store.address.ordinal);
            assert_ne!(
                foreign_owner.causal_root(),
                super::super::CausalRoot::new(digest_from_hash(
                    store.pending.causal_lifecycle_key()
                ))
            );
            store.address.owner = foreign_owner;
        });

        let mut foreign = durable_store_fixture(0x73);
        let foreign_work = foreign
            .registry
            .entries
            .remove(&foreign.address)
            .expect("take foreign pending only inside private fixture");
        let ConcreteLifecycleWorkKind::DurableStoreBody(foreign_store) = foreign_work.kind else {
            unreachable!("foreign fixture retains one closed Store")
        };
        let foreign_pending = foreign_store.pending;
        assert_corrupt_durable_store_rejected(0x74, move |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            store.pending = foreign_pending;
        });

        assert_corrupt_durable_store_rejected(0x75, |work| {
            work.digest = LifecycleDigest::new([0xD5; 32]);
        });
        assert_corrupt_durable_store_rejected(0x76, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
                unreachable!("corruption fixture retains one Store effect")
            };
            store.durable_receipt = DurableBodyReceipt::for_test(
                wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"foreign durable Store context",
                ))),
                *round,
                *subject,
                store.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_store_rejected(0x77, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
                unreachable!("corruption fixture retains one Store effect")
            };
            let wrong_round = wire::ConsensusRound {
                view: round.view.saturating_add(1),
                ..*round
            };
            store.durable_receipt = DurableBodyReceipt::for_test(
                round.context_id,
                wrong_round,
                *subject,
                store.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_store_rejected(0x78, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
                unreachable!("corruption fixture retains one Store effect")
            };
            let wrong_subject = wire::BlockSubject {
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"foreign durable Store subject",
                )),
                ..*subject
            };
            store.durable_receipt = DurableBodyReceipt::for_test(
                round.context_id,
                *round,
                wrong_subject,
                store.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_store_rejected(0x79, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
                unreachable!("corruption fixture retains one Store effect")
            };
            store.durable_receipt = DurableBodyReceipt::for_test(
                round.context_id,
                *round,
                *subject,
                HashOf::from_untyped_unchecked(Hash::new(b"foreign manifest hash")),
            );
        });
        assert_corrupt_durable_store_rejected(0x7A, |work| {
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Store")
            };
            store.expected_manifest_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"altered parent manifest hash"));
        });
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_prepare_and_drop_preserve_the_closed_row() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
            ..
        } = durable_validate_fixture(0x81);
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = effect.clone()
        else {
            unreachable!("durable Validate fixture retains its Validate effect")
        };
        let before = format!("{registry:?}");

        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("prepare exact durable Validate execution");
        assert_eq!(prepared.adapter_preview_inputs(), (tag, round, subject));
        assert_eq!(
            prepared.durable_body_receipt().context_id(),
            round.context_id
        );
        assert_eq!(prepared.durable_body_receipt().round(), round);
        assert_eq!(prepared.durable_body_receipt().subject(), subject);
        assert_eq!(
            prepared.durable_body_receipt().manifest_hash(),
            expected_manifest_hash
        );
        assert_eq!(prepared.expected_manifest_hash(), expected_manifest_hash);
        drop(prepared);

        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
        assert_eq!(
            registry.borrow_for_lease(&lease, slot),
            Err(RegistryError::WrongWorkKind)
        );
        assert!(matches!(
            registry.take_for_lease(&lease, slot),
            Err(RegistryError::WrongWorkKind)
        ));
        assert_eq!(format!("{registry:?}"), before);

        let mut disposable = durable_validate_fixture(0x82);
        let closed = disposable
            .registry
            .entries
            .remove(&disposable.address)
            .expect("remove disposable closed Validate only for into-pair rejection test");
        let unwind = catch_unwind(AssertUnwindSafe(|| closed.into_pair()));
        assert!(
            unwind.is_err(),
            "closed Validate must not expose a raw pair"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_prepare_rejects_foreign_retained_origin_without_mutation() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            ..
        } = durable_validate_fixture(0x86);
        {
            let work = registry
                .entries
                .get_mut(&address)
                .expect("foreign-origin fixture retains its Validate carrier");
            let installed_digest = work.digest;
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("foreign-origin fixture retains one Validate carrier")
            };
            assert!(
                validate
                    .replay_evidence
                    .replace_with_foreign_origin_for_test()
            );
            assert!(!validate.validates(installed_digest));
            assert!(matches!(
                validate.project_candidate(&verified),
                Err(AdapterEffectAdmissionError::InvalidCarrier)
            ));
        }
        let before = format!("{registry:?}");
        assert!(matches!(
            registry.prepare_durable_validate_execution(&lease, slot, &verified),
            Err(DurableValidateExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
        assert_eq!(format!("{registry:?}"), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn durable_validate_prepare_rejects_wrong_lease_projection_and_context_without_mutation() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            ..
        } = durable_validate_fixture(0x83);
        let before = format!("{registry:?}");

        let mut wrong_class = lease.clone();
        wrong_class.work_class = LifecycleWorkClass::Store;
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_class, slot, &verified),
            Err(DurableValidateExecutionError::InvalidLeaseShape)
        ));

        let exact_key = lease.key();
        let mut wrong_phase = lease.clone();
        wrong_phase.key = super::super::LifecycleKey::new(
            exact_key.context(),
            exact_key.round(),
            exact_key.proposal_round(),
            exact_key.subject(),
            LifecyclePhase::Store,
            exact_key.execution_commitment(),
        );
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_phase, slot, &verified),
            Err(DurableValidateExecutionError::InvalidLeaseShape)
        ));

        let mut wrong_stage = lease.clone();
        wrong_stage.stage = super::super::LifecycleStage::new(
            LifecycleStageKind::StoreBody,
            PredecessorScope::Independent,
        );
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_stage, slot, &verified),
            Err(DurableValidateExecutionError::InvalidLeaseShape)
        ));

        let mut wrong_scope = lease.clone();
        wrong_scope.stage = super::super::LifecycleStage::new(
            LifecycleStageKind::ValidateBody,
            PredecessorScope::ReadyOrdinalPrefix,
        );
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_scope, slot, &verified),
            Err(DurableValidateExecutionError::InvalidLeaseShape)
        ));

        let other_slot = PhysicalSlotId::for_capacity(lease.work_class().capacity_class(), 1);
        assert!(matches!(
            registry.prepare_durable_validate_execution(&lease, other_slot, &verified),
            Err(DurableValidateExecutionError::InvalidLeaseShape)
        ));

        let mut wrong_digest = lease.clone();
        wrong_digest
            .physical_slots
            .insert(slot, LifecycleDigest::new([0xD4; 32]));
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_digest, slot, &verified),
            Err(DurableValidateExecutionError::Registry(
                RegistryError::DigestMismatch
            ))
        ));

        let mut stale_address = lease.clone();
        stale_address.ordinal = stale_address.ordinal.saturating_add(1);
        assert!(matches!(
            registry.prepare_durable_validate_execution(&stale_address, slot, &verified),
            Err(DurableValidateExecutionError::Registry(
                RegistryError::Missing
            ))
        ));

        let mut wrong_key = lease.clone();
        wrong_key.key = super::super::LifecycleKey::new(
            exact_key.context(),
            exact_key.round(),
            exact_key.proposal_round(),
            Some(LifecycleDigest::new([0xE4; 32])),
            exact_key.phase(),
            exact_key.execution_commitment(),
        );
        assert!(matches!(
            registry.prepare_durable_validate_execution(&wrong_key, slot, &verified),
            Err(DurableValidateExecutionError::InvalidProjection)
        ));

        let (foreign_verified, _) = verified_store_context(0x84);
        assert!(matches!(
            registry.prepare_durable_validate_execution(&lease, slot, &foreign_verified),
            Err(DurableValidateExecutionError::Projection(
                AdapterEffectAdmissionError::ForeignContext
            ))
        ));

        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_prepare_rejects_an_executable_adapter_at_the_exact_address() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            ..
        } = durable_validate_fixture(0x85);
        let closed = registry
            .entries
            .remove(&address)
            .expect("test-only conversion of closed Validate row to pending kind");
        let ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
        } = closed
        else {
            unreachable!("fixture retains one closed Validate row")
        };
        let DurableValidateBody {
            effect, pending, ..
        } = validate;
        let pending_work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
        };
        assert!(pending_work.validate_exact());
        assert!(registry.entries.insert(address, pending_work).is_none());
        assert!(matches!(
            registry.prepare_durable_validate_execution(&lease, slot, &verified),
            Err(DurableValidateExecutionError::WrongWorkKind)
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_binds_exact_success_receipt_without_registry_mutation() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            ..
        } = durable_validate_fixture(0x95);
        let before = format!("{registry:?}");
        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("prepare exact closed Validate carrier");
        let preview_inputs = prepared.adapter_preview_inputs();
        let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
        let expected_commitment = validated.execution_commitment();
        let completion = prepared
            .bind_validated_receipt(validated)
            .expect("bind exact store-minted validation receipt");
        assert_eq!(completion.address, address);
        assert_eq!(completion.adapter_preview_inputs(), preview_inputs);
        assert_eq!(
            completion.validated_receipt().execution_commitment(),
            expected_commitment
        );
        assert_eq!(completion.incumbent_digest(), lease.physical_slots()[&slot]);
        assert_ne!(
            completion.replacement_digest(),
            completion.incumbent_digest()
        );
        let first_replacement = completion.replacement_digest();
        drop(completion);
        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));

        let repeated = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("repeat exact closed Validate preflight");
        let repeated_receipt =
            ValidatedBodyReceipt::for_test(repeated.durable_body_receipt().clone());
        let repeated = repeated
            .bind_validated_receipt(repeated_receipt)
            .expect("repeat deterministic validation binding");
        assert_eq!(repeated.replacement_digest(), first_replacement);
        drop(repeated);
        assert_eq!(format!("{registry:?}"), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn live_wal_apply_join_rejects_foreign_receipt_and_root_before_exact_retry() {
        let DurableValidateFixture {
            mut registry,
            verified,
            lease,
            slot,
            ..
        } = durable_validate_fixture(0x97);
        let before = format!("{registry:?}");
        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("prepare exact Validate for live Apply");
        let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
        let (apply, exact_child_pending, foreign_child_pending) = {
            let validate = prepared.durable_validate();
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                unreachable!("fixture retains Validate")
            };
            let apply = AdapterEffect::Apply {
                tag: *tag,
                subject: *subject,
                certificate: wire::QuorumCertificate {
                    round: *round,
                    proposal_round: *round,
                    phase: wire::GlobalPhase::Commit,
                    subject: *subject,
                    execution_commitment: validated.execution_commitment(),
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0x97; 96],
                },
            };
            let exact_child_pending = validate
                .pending
                .project_validate_apply_successor(&validate.effect, &apply)
                .expect("retained Validate projects exact Apply pending");
            let foreign_owner = bind_adapter_effect_batch_ownership(
                core::slice::from_ref(&validate.effect),
                vec![RuntimeEffectOwnership::fresh_for_test(*tag, 9_700)],
            )
            .expect("bind same effect under foreign causal root")
            .pop()
            .expect("one foreign Validate owner");
            let foreign_predecessor = foreign_owner
                .pending_adapter_effect_binding(&validate.effect)
                .expect("mint foreign Validate pending");
            assert_ne!(
                foreign_predecessor.causal_lifecycle_key(),
                validate.pending.causal_lifecycle_key()
            );
            let foreign_child_pending = foreign_predecessor
                .project_validate_apply_successor(&validate.effect, &apply)
                .expect("foreign Validate projects its own same-effect child");
            (apply, exact_child_pending, foreign_child_pending)
        };
        let foreign_manifest =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign same-coordinate Apply manifest"));
        let exact_durable = prepared.durable_body_receipt();
        let foreign_receipt = DurableBodyReceipt::for_test(
            exact_durable.context_id(),
            exact_durable.round(),
            exact_durable.subject(),
            foreign_manifest,
        );
        let foreign_validated = ValidatedBodyReceipt::for_test(foreign_receipt);
        let Err((error, returned)) = prepared.bind_validated_receipt(foreign_validated) else {
            panic!("foreign same-coordinate receipt cannot construct Apply completion")
        };
        assert_eq!(
            error,
            DurableValidateExecutionError::InvalidValidationReceipt
        );
        drop(returned);
        assert_eq!(format!("{registry:?}"), before);

        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("repeat exact Validate after foreign receipt rejection");
        let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
        let persisted = SealedLiveWalPersistedEffectV1::from_exact_live_append(
            ExactLiveWalPersistedContinuationCause::Apply {
                wal_identity: LiveWalFrameIdentity::for_test(0, 1, [0; 32]),
                effect: apply.clone(),
            },
        )
        .expect("zero-valued exact live WAL hash seals Apply source");
        let completion = prepared
            .bind_validated_receipt(validated)
            .expect("bind exact retained validation receipt");
        let Ok(exact) = completion.seal_live_wal_apply(persisted, exact_child_pending) else {
            panic!("exact retained receipt must complete Apply authority")
        };
        let LiveWalReplayPreAdmissionOrigin::Apply(completion) = &exact._origin else {
            unreachable!("Apply join retains its Validate completion")
        };
        assert!(completion.retained_apply_join_is_exact(&exact._persisted));
        drop(exact);
        assert_eq!(format!("{registry:?}"), before);

        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("repeat exact Validate after drop");
        let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
        let persisted = SealedLiveWalPersistedEffectV1::from_exact_live_append(
            ExactLiveWalPersistedContinuationCause::Apply {
                wal_identity: LiveWalFrameIdentity::for_test(1, 2, [0x97; 32]),
                effect: apply.clone(),
            },
        )
        .expect("seal repeated exact Apply source");
        let completion = prepared
            .bind_validated_receipt(validated)
            .expect("bind repeated exact validation receipt");
        let Err(error) = completion.seal_live_wal_apply(persisted, foreign_child_pending) else {
            panic!("foreign causal root cannot splice into retained Validate")
        };
        let LiveWalReplayPreAdmissionFailure::Apply {
            _completion: completion,
            _persisted: persisted,
            _pending: foreign_pending,
        } = error._failure
        else {
            unreachable!("Apply rejection retains every input")
        };
        drop(foreign_pending);
        let exact_child_pending = {
            let validate = completion
                ._registry
                .entries
                .get(&completion.address)
                .expect("completion keeps Validate installed");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &validate.kind else {
                unreachable!("completion retains durable Validate")
            };
            validate
                .pending
                .project_validate_apply_successor(&validate.effect, &apply)
                .expect("retained predecessor still projects exact Apply")
        };
        let Ok(exact) = completion.seal_live_wal_apply(persisted, exact_child_pending) else {
            panic!("foreign-root rejection must leave source-only seal retryable")
        };
        drop(exact);
        assert_eq!(format!("{registry:?}"), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_rejects_foreign_success_receipt_without_registry_mutation() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
            ..
        } = durable_validate_fixture(0x96);
        let before = format!("{registry:?}");
        let prepared = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("prepare exact closed Validate carrier");
        let (_, round, subject) = prepared.adapter_preview_inputs();
        let foreign_durable = DurableBodyReceipt::for_test(
            round.context_id,
            wire::ConsensusRound {
                view: round.view.saturating_add(1),
                ..round
            },
            subject,
            expected_manifest_hash,
        );
        let foreign = ValidatedBodyReceipt::for_test(foreign_durable);
        let Err((error, returned)) = prepared.bind_validated_receipt(foreign) else {
            panic!("foreign durable receipt must not bind Validate completion")
        };
        assert_eq!(
            error,
            DurableValidateExecutionError::InvalidValidationReceipt
        );
        assert_ne!(returned.durable().round(), round);
        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_detach_and_drop_release_the_registry_without_mutation() {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            effect,
            ..
        } = durable_validate_fixture(0xA0);
        let before = format!("{registry:?}");
        let detached = registry
            .prepare_durable_validate_execution(&lease, slot, &verified)
            .expect("prepare detached durable Validate")
            .detach();

        assert_eq!(format!("{registry:?}"), before);
        assert!(registry.exactly_contains(address, &effect));
        drop(detached);
        assert_eq!(format!("{registry:?}"), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_detached_success_reattaches_and_repeats_idempotently() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA1);
        let before = format!("{:?}", fixture.registry);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let detached = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare exact durable Validate")
            .detach();
        assert_eq!(format!("{:?}", fixture.registry), before);
        let executed = detached
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute detached durable validation");
        assert_eq!(executed.outcome().durable_body(), &durable);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
        let completed = fixture
            .registry
            .reattach_durable_validate_execution(executed)
            .expect("reattach exact durable Validate success");
        assert_eq!(
            completed.adapter_preview_inputs(),
            match fixture.effect {
                AdapterEffect::ValidateBody {
                    tag,
                    round,
                    subject,
                } => (tag, round, subject),
                _ => unreachable!("fixture retains one Validate effect"),
            }
        );
        assert!(completed.outcome().validated_receipt().is_some());
        drop(completed);
        assert_eq!(format!("{:?}", fixture.registry), before);

        let repeated = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("repeat exact durable Validate preflight")
            .detach()
            .execute(
                &mut store,
                |_| -> Result<wire::ExecutionCommitment, DetachedValidationError> {
                    panic!("durable validation marker must bypass the callback")
                },
            )
            .expect("repeat reuses durable validation marker");
        assert_eq!(
            repeated
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
        let repeated = fixture
            .registry
            .reattach_durable_validate_execution(repeated)
            .expect("reattach repeated deterministic success");
        drop(repeated);
        assert_eq!(format!("{:?}", fixture.registry), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_reattach_rejects_row_and_digest_changes_with_outcome_intact() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA2);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare exact durable Validate")
            .detach()
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact detached validation");

        fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("fixture retains exact Validate row")
            .digest = LifecycleDigest::new([0xEF; 32]);
        let mutated = format!("{:?}", fixture.registry);
        let Err((error, executed)) = fixture
            .registry
            .reattach_durable_validate_execution(executed)
        else {
            panic!("mutated incumbent digest must reject reattachment")
        };
        assert_eq!(
            error,
            DurableValidateExecutionError::Registry(RegistryError::CorruptWork)
        );
        assert_eq!(format!("{:?}", fixture.registry), mutated);
        assert_eq!(executed.outcome().durable_body(), &durable);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_reattach_rejects_foreign_registry_address_and_carrier() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA3);
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let executed = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare exact durable Validate")
            .detach()
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact detached validation");

        let mut foreign_registry = ConcreteLifecycleWorkRegistry::default();
        let Err((error, mut executed)) =
            foreign_registry.reattach_durable_validate_execution(executed)
        else {
            panic!("foreign empty registry must reject reattachment")
        };
        assert_eq!(
            error,
            DurableValidateExecutionError::Registry(RegistryError::Missing)
        );

        let exact_address = executed.request.address;
        executed.request.address = ConcreteWorkAddress::new(
            exact_address.owner,
            exact_address.ordinal.saturating_add(1),
            exact_address.slot,
        )
        .expect("construct foreign detached address");
        let Err((error, mut executed)) = fixture
            .registry
            .reattach_durable_validate_execution(executed)
        else {
            panic!("foreign detached address must reject reattachment")
        };
        assert_eq!(
            error,
            DurableValidateExecutionError::Registry(RegistryError::Missing)
        );
        executed.request.address = exact_address;

        let closed = fixture
            .registry
            .entries
            .remove(&fixture.address)
            .expect("replace exact carrier only in this rejection fixture");
        let ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
        } = closed
        else {
            unreachable!("fixture retains one closed Validate carrier")
        };
        let DurableValidateBody {
            effect, pending, ..
        } = validate;
        let pending = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
        };
        assert!(pending.validates_at(fixture.address));
        assert!(
            fixture
                .registry
                .entries
                .insert(fixture.address, pending)
                .is_none()
        );
        let foreign_carrier = format!("{:?}", fixture.registry);
        let Err((error, returned)) = fixture
            .registry
            .reattach_durable_validate_execution(executed)
        else {
            panic!("foreign carrier kind must reject reattachment")
        };
        assert_eq!(error, DurableValidateExecutionError::WrongWorkKind);
        assert_eq!(format!("{:?}", fixture.registry), foreign_carrier);
        assert!(returned.outcome().validated_receipt().is_some());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_detached_rejection_and_sidecar_deferral_remain_bound() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA4);
        let before = format!("{:?}", fixture.registry);
        let rejected = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare rejected detached Validate")
            .detach()
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "detached candidate rejected",
                ))
            })
            .expect("execute deterministic rejection");
        assert_eq!(rejected.outcome().durable_body(), &durable);
        assert_eq!(
            rejected.outcome().rejection_reason(),
            Some("detached candidate rejected")
        );
        let rejected = fixture
            .registry
            .reattach_durable_validate_execution(rejected)
            .expect("reattach exact deterministic rejection");
        assert_eq!(
            rejected.outcome().rejection_reason(),
            Some("detached candidate rejected")
        );
        drop(rejected);
        assert_eq!(format!("{:?}", fixture.registry), before);

        let reference = detached_validation_merge_reference(&durable);
        let deferred = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare deferred detached Validate")
            .detach()
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("execute typed sidecar deferral");
        assert_eq!(deferred.outcome().durable_body(), &durable);
        assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
        let deferred = fixture
            .registry
            .reattach_durable_validate_execution(deferred)
            .expect("reattach exact sidecar deferral");
        assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
        drop(deferred);
        assert_eq!(format!("{:?}", fixture.registry), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_reattach_rejects_an_inflight_authority_upgrade() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA5);
        let executed = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare exact durable Validate")
            .detach()
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "authority-upgrade fixture rejection",
                ))
            })
            .expect("execute detached validation before authority upgrade");
        let original_statement = executed.request.candidate_statement;
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = fixture.effect.clone()
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let certified_fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: commitment,
                signers: Vec::new(),
                aggregate_signature: Vec::new(),
            }),
        };
        let certified_fetch_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&certified_fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 50_001)],
        )
        .expect("bind one Commit-authorized Fetch")
        .pop()
        .expect("one Commit Fetch owner");
        let incoming_store_owner = certified_fetch_owner
            .rebind_as_inherited_adapter_effect(&store_effect)
            .expect("carry Commit authority into Store");
        let adopted_store_owner = fixture
            .store_ownership
            .adopt_incumbent_body_stage_for_retry_or_authority(&incoming_store_owner, &store_effect)
            .expect("retain physical Store owner while upgrading authority");
        let upgraded_store = adopted_store_owner
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint upgraded Store binding");
        let upgraded_validate = upgraded_store
            .project_store_validate_successor(&store_effect, &fixture.effect)
            .expect("carry upgraded authority into Validate");
        assert_eq!(
            upgraded_validate.causal_lifecycle_key(),
            &executed.request.causal_lifecycle_key
        );
        assert_ne!(upgraded_validate.candidate_statement(), original_statement);

        let work = fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("authority fixture retains exact Validate row");
        let digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("authority fixture retains one closed Validate")
        };
        let (_store_replay, validate_replay) = certified_pipeline_replay_evidence_for_test(
            tag,
            &fixture.manifest,
            &validate.durable_receipt,
            &upgraded_validate,
        )
        .expect("rebind certified Validate replay to upgraded in-flight authority");
        validate.pending = upgraded_validate;
        validate.replay_evidence = DurableValidateReplayEvidenceV1::certified(validate_replay);
        assert!(validate.validates(digest));
        assert!(work.validates_at(fixture.address));
        let upgraded = format!("{:?}", fixture.registry);
        let Err((error, returned)) = fixture
            .registry
            .reattach_durable_validate_execution(executed)
        else {
            panic!("in-flight authority upgrade must reject unchanged-row CAS")
        };
        assert_eq!(error, DurableValidateExecutionError::InvalidValidateShape);
        assert_eq!(format!("{:?}", fixture.registry), upgraded);
        assert_eq!(
            returned.outcome().rejection_reason(),
            Some("authority-upgrade fixture rejection")
        );
    }

    #[cfg(feature = "bls")]
    fn assert_corrupt_durable_validate_rejected(
        marker: u8,
        corrupt: impl FnOnce(&mut ConcreteLifecycleWork),
    ) {
        let DurableValidateFixture {
            mut registry,
            verified,
            address,
            lease,
            slot,
            ..
        } = durable_validate_fixture(marker);
        let work = registry
            .entries
            .get_mut(&address)
            .expect("corruption fixture retains its closed Validate row");
        corrupt(work);
        assert!(!work.validate_exact());
        let before = format!("{registry:?}");
        assert!(matches!(
            registry.prepare_durable_validate_execution(&lease, slot, &verified),
            Err(DurableValidateExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
        assert_eq!(format!("{registry:?}"), before);
        assert_eq!(registry.len(), 1);
        assert!(registry.entries.contains_key(&address));
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn durable_validate_validation_rejects_every_corrupt_closed_coordinate() {
        assert_corrupt_durable_validate_rejected(0x86, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            validate.address.ordinal = 0;
        });
        assert_corrupt_durable_validate_rejected(0x87, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            let foreign_owner = owner(0xF7, validate.address.ordinal);
            assert_ne!(
                foreign_owner.causal_root(),
                super::super::CausalRoot::new(digest_from_hash(
                    validate.pending.causal_lifecycle_key()
                ))
            );
            validate.address.owner = foreign_owner;
        });

        let mut foreign = durable_validate_fixture(0x88);
        let foreign_work = foreign
            .registry
            .entries
            .remove(&foreign.address)
            .expect("take foreign pending only inside private fixture");
        let ConcreteLifecycleWorkKind::DurableValidateBody(foreign_validate) = foreign_work.kind
        else {
            unreachable!("foreign fixture retains one closed Validate")
        };
        let foreign_pending = foreign_validate.pending;
        assert_corrupt_durable_validate_rejected(0x89, move |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            validate.pending = foreign_pending;
        });

        assert_corrupt_durable_validate_rejected(0x8A, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                unreachable!("corruption fixture retains one Validate effect")
            };
            validate.effect = AdapterEffect::StoreBody {
                tag: *tag,
                round: *round,
                subject: *subject,
            };
        });
        assert_corrupt_durable_validate_rejected(0x8B, |work| {
            work.digest = LifecycleDigest::new([0xDB; 32]);
        });
        assert_corrupt_durable_validate_rejected(0x8C, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
                unreachable!("corruption fixture retains one Validate effect")
            };
            validate.durable_receipt = DurableBodyReceipt::for_test(
                wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"foreign durable Validate context",
                ))),
                *round,
                *subject,
                validate.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_validate_rejected(0x8D, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
                unreachable!("corruption fixture retains one Validate effect")
            };
            let wrong_round = wire::ConsensusRound {
                view: round.view.saturating_add(1),
                ..*round
            };
            validate.durable_receipt = DurableBodyReceipt::for_test(
                round.context_id,
                wrong_round,
                *subject,
                validate.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_validate_rejected(0x8E, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
                unreachable!("corruption fixture retains one Validate effect")
            };
            let wrong_subject = wire::BlockSubject {
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"foreign durable Validate subject",
                )),
                ..*subject
            };
            validate.durable_receipt = DurableBodyReceipt::for_test(
                round.context_id,
                *round,
                wrong_subject,
                validate.expected_manifest_hash,
            );
        });
        assert_corrupt_durable_validate_rejected(0x8F, |work| {
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
                unreachable!("corruption fixture retains one closed Validate")
            };
            validate.expected_manifest_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"altered Validate manifest hash"));
        });
    }

    #[test]
    fn exact_install_borrow_and_take_are_one_shot() {
        let work = concrete(effect(1), 91);
        let digest = work.digest;
        let owner = admitted_owner(&work, 1);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 1, slot).expect("valid address");
        let lease = lease(owner, 1, slot, digest);
        let expected = work.effect().clone();
        let mut registry = ConcreteLifecycleWorkRegistry::default();

        registry
            .install(address, digest, work)
            .expect("install exact work");
        assert_eq!(registry.borrow_for_lease(&lease, slot), Ok(&expected));
        let taken = registry
            .take_for_lease(&lease, slot)
            .expect("take complete exact work");
        assert_eq!(taken.effect(), &expected);
        assert!(taken.validate_exact());
        registry
            .install(address, digest, taken)
            .expect("restore the complete token after a deferred outcome");
        assert_eq!(registry.borrow_for_lease(&lease, slot), Ok(&expected));
        let retired = registry
            .take_for_lease(&lease, slot)
            .expect("terminal execution takes the restored token once");
        assert_eq!(retired.effect(), &expected);
        assert!(matches!(
            registry.take_for_lease(&lease, slot),
            Err(RegistryError::Missing)
        ));
        assert!(registry.is_empty());
    }

    #[test]
    fn certified_fetch_execution_rejects_unclosed_or_inexact_leases_without_mutation() {
        let work = concrete(effect(0x31), 0x31);
        let digest = work.digest();
        let expected = work.effect().clone();
        let owner = admitted_owner(&work, 0x31);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 0x31, slot).expect("valid exact address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, digest, work)
            .expect("install still-pending work");

        let store_lease = lease(owner, 0x31, slot, digest);
        assert!(matches!(
            registry.prepare_certified_fetch_execution(&store_lease, slot),
            Err(CertifiedFetchExecutionError::InvalidLeaseShape)
        ));
        assert!(registry.exactly_contains(address, &expected));

        let exact_fetch_lease = fetch_lease(owner, 0x31, slot, digest);
        assert!(matches!(
            registry.prepare_certified_fetch_execution(&exact_fetch_lease, slot),
            Err(CertifiedFetchExecutionError::WrongWorkKind)
        ));
        assert!(registry.exactly_contains(address, &expected));

        let wrong_digest_lease = fetch_lease(owner, 0x31, slot, LifecycleDigest::new([0xFF; 32]));
        assert!(matches!(
            registry.prepare_certified_fetch_execution(&wrong_digest_lease, slot),
            Err(CertifiedFetchExecutionError::Registry(
                RegistryError::DigestMismatch
            ))
        ));
        assert!(registry.exactly_contains(address, &expected));

        let other_slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 1);
        let mut multi_slot_lease = exact_fetch_lease.clone();
        multi_slot_lease
            .physical_slots
            .insert(other_slot, LifecycleDigest::new([0xEE; 32]));
        assert!(matches!(
            registry.prepare_certified_fetch_execution(&multi_slot_lease, slot),
            Err(CertifiedFetchExecutionError::InvalidLeaseShape)
        ));
        assert!(matches!(
            registry.prepare_certified_fetch_execution(&exact_fetch_lease, other_slot),
            Err(CertifiedFetchExecutionError::InvalidLeaseShape)
        ));
        assert!(registry.exactly_contains(address, &expected));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn installation_unwind_removes_unpublished_work() {
        let work = concrete(effect(0x21), 0x21);
        let digest = work.digest();
        let owner = admitted_owner(&work, 0x21);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 0x21, slot).expect("valid address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ =
                registry.install_before_publication(address, digest, work, || -> Result<(), ()> {
                    panic!("injected admission publication unwind")
                });
        }));
        assert!(unwind.is_err());
        assert!(registry.is_empty());
    }

    #[test]
    fn mismatches_and_duplicates_never_remove_or_overwrite() {
        let first = concrete(effect(2), 92);
        let digest = first.digest;
        let owner = admitted_owner(&first, 2);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 2, slot).expect("valid address");
        let exact_lease = lease(owner, 2, slot, digest);
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, digest, first)
            .expect("install first work");

        let duplicate = concrete(effect(3), 93);
        assert!(matches!(
            registry.install(address, duplicate.digest, duplicate),
            Err((RegistryError::Occupied, _))
        ));
        assert_eq!(registry.len(), 1);

        let wrong_owner = owner(9, 2);
        let wrong_owner_lease = lease(wrong_owner, 2, slot, digest);
        assert!(matches!(
            registry.take_for_lease(&wrong_owner_lease, slot),
            Err(RegistryError::Missing)
        ));
        let wrong_ordinal_lease = lease(owner, 3, slot, digest);
        assert!(matches!(
            registry.take_for_lease(&wrong_ordinal_lease, slot),
            Err(RegistryError::Missing)
        ));
        let wrong_slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 1);
        assert!(matches!(
            registry.take_for_lease(&exact_lease, wrong_slot),
            Err(RegistryError::DigestMismatch)
        ));
        let wrong_digest = LifecycleDigest::new([0xFF; 32]);
        assert!(matches!(
            registry.take_for_lease(&lease(owner, 2, slot, wrong_digest), slot),
            Err(RegistryError::DigestMismatch)
        ));
        assert_eq!(registry.len(), 1);
        assert!(matches!(
            registry.rollback_exact(address, wrong_digest),
            Err(RegistryError::DigestMismatch)
        ));
        assert_eq!(registry.len(), 1);
        registry
            .rollback_exact(address, digest)
            .expect("exact rollback returns work");
        assert!(registry.is_empty());
    }

    #[test]
    fn physical_digest_does_not_alias_distinct_logical_addresses() {
        let first = concrete(effect(4), 94);
        let second = concrete(effect(4), 95);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.causal_root(), second.causal_root());
        let digest = first.digest;
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let shared_owner = admitted_owner(&first, 4);
        let first_address = ConcreteWorkAddress::new(shared_owner, 4, slot).expect("first address");
        let second_address =
            ConcreteWorkAddress::new(shared_owner, 5, slot).expect("second address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(first_address, digest, first)
            .expect("install first logical address");
        registry
            .install(second_address, digest, second)
            .expect("install second logical address");
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn install_rejects_a_foreign_causal_owner_without_consuming_work() {
        let work = concrete(effect(7), 97);
        let digest = work.digest;
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner(0xA7, 7), 7, slot)
            .expect("syntactically valid foreign address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        let returned = registry
            .install(address, digest, work)
            .expect_err("causal owner mismatch must fail closed");
        assert_eq!(returned.0, RegistryError::CausalOwnerMismatch);
        assert!(returned.1.validate_exact());
        assert!(registry.is_empty());
    }

    #[test]
    fn exact_replacement_commits_or_restores_the_incumbent_atomically() {
        let incumbent = concrete(effect_at_generation(0xB1, 7), 0xB7);
        let replacement = concrete(effect_at_generation(0xB2, 7), 0xB7);
        assert_eq!(incumbent.causal_root(), replacement.causal_root());
        assert_ne!(incumbent.digest(), replacement.digest());
        let incumbent_digest = incumbent.digest();
        let replacement_digest = replacement.digest();
        let incumbent_effect = incumbent.effect().clone();
        let replacement_effect = replacement.effect().clone();
        let owner = admitted_owner(&incumbent, 11);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 11, slot).expect("valid address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, incumbent_digest, incumbent)
            .expect("install replacement incumbent");

        let error = registry
            .replace_before_publication(
                address,
                incumbent_digest,
                replacement_digest,
                replacement,
                || Err::<(), _>("queue CAS changed"),
            )
            .expect_err("failed publication must restore the incumbent");
        let RegistryReplacementError::Publication(reason, returned) = error else {
            panic!("exact replacement returned an unexpected error variant")
        };
        assert_eq!(reason, "queue CAS changed");
        assert_eq!(returned.effect(), &replacement_effect);
        assert!(returned.validate_exact());
        assert!(registry.exactly_contains(address, &incumbent_effect));

        let (published, retired) = registry
            .replace_before_publication(
                address,
                incumbent_digest,
                replacement_digest,
                returned,
                || Ok::<_, ()>(0xC0DE_u16),
            )
            .expect("exact publication commits the replacement");
        assert_eq!(published, 0xC0DE);
        assert_eq!(retired.effect(), &incumbent_effect);
        assert!(retired.validate_exact());
        assert!(registry.exactly_contains(address, &replacement_effect));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn replacement_unwind_restores_the_incumbent() {
        let incumbent = concrete(effect_at_generation(0xD1, 9), 0xD9);
        let replacement = concrete(effect_at_generation(0xD2, 9), 0xD9);
        assert_eq!(incumbent.causal_root(), replacement.causal_root());
        let incumbent_digest = incumbent.digest();
        let replacement_digest = replacement.digest();
        let incumbent_effect = incumbent.effect().clone();
        let owner = admitted_owner(&incumbent, 13);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 13, slot).expect("valid address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, incumbent_digest, incumbent)
            .expect("install unwind incumbent");

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = registry.replace_before_publication(
                address,
                incumbent_digest,
                replacement_digest,
                replacement,
                || -> Result<(), ()> { panic!("injected publication unwind") },
            );
        }));
        assert!(unwind.is_err());
        assert!(registry.exactly_contains(address, &incumbent_effect));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn replacement_validation_never_changes_the_incumbent() {
        let incumbent = concrete(effect_at_generation(0xC1, 8), 0xC8);
        let replacement = concrete(effect_at_generation(0xC2, 8), 0xC8);
        let incumbent_digest = incumbent.digest();
        let replacement_digest = replacement.digest();
        let incumbent_effect = incumbent.effect().clone();
        let incumbent_owner = admitted_owner(&incumbent, 12);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(incumbent_owner, 12, slot).expect("valid address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, incumbent_digest, incumbent)
            .expect("install validation incumbent");

        let wrong_digest = LifecycleDigest::new([0xFF; 32]);
        let error = registry
            .replace_before_publication(
                address,
                wrong_digest,
                replacement_digest,
                replacement,
                || -> Result<(), ()> { unreachable!("validation precedes publication") },
            )
            .expect_err("wrong incumbent digest must reject before mutation");
        let RegistryReplacementError::Validation(RegistryError::DigestMismatch, returned) = error
        else {
            panic!("wrong incumbent digest has one typed failure")
        };
        assert_eq!(returned.digest(), replacement_digest);
        assert!(registry.exactly_contains(address, &incumbent_effect));
        assert_eq!(registry.len(), 1);

        let foreign_owner = owner(0xEE, 12);
        let foreign_address =
            ConcreteWorkAddress::new(foreign_owner, 12, slot).expect("syntactic foreign address");
        let error = registry
            .replace_before_publication(
                foreign_address,
                incumbent_digest,
                replacement_digest,
                returned,
                || -> Result<(), ()> { unreachable!("validation precedes publication") },
            )
            .expect_err("foreign address must reject before mutation");
        assert!(matches!(
            error,
            RegistryReplacementError::Validation(RegistryError::CausalOwnerMismatch, _)
        ));
        assert!(registry.exactly_contains(address, &incumbent_effect));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn mismatched_pending_binding_never_becomes_registry_work() {
        let first = effect(5);
        let second = effect(6);
        let tag = match &first {
            AdapterEffect::StoreBody { tag, .. } => *tag,
            _ => unreachable!("registry fixture uses one StoreBody effect"),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&first),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 96)],
        )
        .expect("bind first effect")
        .pop()
        .expect("one first-effect owner");
        let pending = ownership
            .pending_adapter_effect_binding(&first)
            .expect("mint first-effect pending binding");
        let (error, returned_effect, returned_pending) =
            ConcreteLifecycleWork::from_exact(second, pending)
                .expect_err("a foreign effect must return the complete move-only pair");
        assert_eq!(error, RegistryError::UnboundEffect);
        assert!(returned_pending.exactly_binds_adapter_effect(&first));
        assert!(!returned_pending.exactly_binds_adapter_effect(&returned_effect));
        assert!(ConcreteLifecycleWorkRegistry::default().is_empty());
    }

    #[test]
    fn direct_signed_replay_pre_admission_is_closed_exact_and_drop_inert() {
        let tag = EventTag::new(7, 2, Generation::new(1));
        let first_vote = direct_signed_vote(0xD1, 0xD2);
        let broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(first_vote.clone()),
        ));
        let broadcast_pending = direct_signed_pending(&broadcast, tag, 1);
        let registry = ConcreteLifecycleWorkRegistry::default();
        let before = format!("{registry:?}");
        let Ok(broadcast) =
            PreparedDirectSignedReplayPreAdmission::seal_exact(broadcast, broadcast_pending)
        else {
            panic!("exact signed Broadcast seals its pre-admission evidence")
        };
        assert!(broadcast.validates());
        assert!(matches!(
            &broadcast.replay_evidence,
            DirectSignedReplayEvidenceV1::Broadcast(_)
        ));
        drop(broadcast);
        assert_eq!(format!("{registry:?}"), before);

        let second_vote = direct_signed_vote(0xD1, 0xD3);
        let report = AdapterEffect::ReportEquivocation {
            evidence: crate::sumeragi::v2::AdapterEquivocationEvidence::vote_for_test(
                first_vote,
                second_vote,
            ),
        };
        let report_pending = direct_signed_pending(&report, tag, 2);
        let Ok(report) = PreparedDirectSignedReplayPreAdmission::seal_exact(report, report_pending)
        else {
            panic!("exact authenticated conflict seals its pre-admission evidence")
        };
        assert!(report.validates());
        assert!(matches!(
            &report.replay_evidence,
            DirectSignedReplayEvidenceV1::ReportEquivocation(_)
        ));
        drop(report);
        assert_eq!(format!("{registry:?}"), before);

        let unsupported = effect(0xD4);
        let AdapterEffect::StoreBody {
            tag: unsupported_tag,
            ..
        } = &unsupported
        else {
            unreachable!("unsupported fixture is StoreBody")
        };
        let unsupported_pending = direct_signed_pending(&unsupported, *unsupported_tag, 3);
        assert!(
            PreparedDirectSignedReplayPreAdmission::seal_exact(unsupported, unsupported_pending,)
                .is_err()
        );
        assert_eq!(format!("{registry:?}"), before);
    }

    #[test]
    fn live_wal_pre_admission_surface_is_closed_and_has_one_apply_join() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("work registry has one production prefix");
        let token = production
            .split("pub(super) struct PreparedLiveWalReplayPreAdmission<'a>")
            .nth(1)
            .expect("live WAL pre-admission token has one declaration")
            .split(
                "/// Move-only Validate projection sealed under its closed durable Store parent.",
            )
            .next()
            .expect("Store-to-Validate token follows live WAL token");
        for required in [
            "_persisted: SealedLiveWalPersistedEffectV1",
            "LiveWalReplayPreAdmissionOrigin<'a>",
            "PayloadFree",
            "Apply(PreparedValidatedBodyCompletion<'a>)",
            "seal_payload_free(\n        persisted: SealedLiveWalPersistedEffectV1",
            "persisted.exactly_binds_payload_free_pending()",
        ] {
            assert!(
                token.contains(required),
                "live WAL token omitted {required}"
            );
        }
        for required in [
            "pub(super) fn seal_live_wal_apply(",
            "retained_validated_receipt_is_exact()",
            "project_validate_apply_successor",
            "retained_apply_join_is_exact(&persisted)",
        ] {
            assert!(
                production.contains(required),
                "live WAL Apply join omitted {required}"
            );
        }
        for forbidden in [
            "#[derive(Clone",
            "Option<SealedLiveWalPersistedEffectV1>",
            "pub(super) fn effect(",
            "pub(super) fn pending(",
            "pub(super) fn receipt(",
            "pub(super) fn source(",
            "into_parts",
            "fn install(",
            "fn commit(",
        ] {
            assert!(
                !token.contains(forbidden),
                "live WAL token exposed forbidden surface {forbidden}"
            );
        }
        assert_eq!(
            production.matches(".complete_exact_apply(").count(),
            1,
            "only the fixed retained-Validate join supplies an Apply receipt"
        );
        assert_eq!(
            production.matches(".seal_live_wal_apply(").count(),
            0,
            "the inert prerequisite has no production admission caller"
        );
        for outside in [
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!outside.contains("PreparedLiveWalReplayPreAdmission"));
        }
    }

    #[test]
    fn direct_signed_replay_pre_admission_surface_is_move_only_inert_and_unwired() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let token = production
            .split("pub(super) struct PreparedDirectSignedReplayPreAdmission {")
            .nth(1)
            .expect("direct signed pre-admission token has one declaration")
            .split("/// Closed concrete form of one fsynced recovered WAL `Sign` successor.")
            .next()
            .expect("recovered WAL carrier follows direct signed token");
        for required in [
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "replay_evidence: DirectSignedReplayEvidenceV1",
            "Broadcast(SignedBroadcastReplayEvidenceV1)",
            "ReportEquivocation(SignedEquivocationReplayEvidenceV1)",
            "pub(super) fn seal_exact(",
            "SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)",
            "SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)",
            "evidence.exactly_matches_effect(&self.effect, &self.pending)",
            "_effect: AdapterEffect",
            "_pending: PendingRuntimeEffectBinding",
        ] {
            assert!(
                token.contains(required),
                "direct signed pre-admission token omitted {required}"
            );
        }
        let declaration = token
            .split('}')
            .next()
            .expect("direct signed token declaration is bounded");
        assert!(!declaration.contains("Option<"));
        assert!(!declaration.contains("derive(Clone"));
        assert!(!declaration.contains("derive(Debug"));
        for forbidden in [
            "fn new(",
            "fn from_parts(",
            "fn into_parts(",
            "fn effect(",
            "fn pending(",
            "fn replay_evidence(",
            "fn install(",
            "fn commit(",
            "ConcreteLifecycleWorkRegistry",
            ".entries",
            "PendingAdapter {",
        ] {
            assert!(
                !token.contains(forbidden),
                "direct signed pre-admission token acquired forbidden authority {forbidden}"
            );
        }
        assert_eq!(
            production.matches("pub(super) fn seal_exact(").count(),
            1,
            "the token has one exact seal"
        );
        assert_eq!(
            production
                .matches("PreparedDirectSignedReplayPreAdmission::seal_exact(")
                .count(),
            0,
            "the inert token must have no production caller"
        );
        for caller in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_concrete_admission.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let caller = caller
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("caller production prefix is bounded");
            assert!(!caller.contains("PreparedDirectSignedReplayPreAdmission"));
        }
    }

    #[test]
    fn remote_proposal_replay_pre_admission_is_closed_exact_and_unwired() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("work registry has one production prefix");
        let token = production
            .split("pub(super) struct PreparedRemoteProposalFetchReplayPreAdmission {")
            .nth(1)
            .expect("remote Proposal replay token has one declaration")
            .split("/// Closed concrete form of one fsynced recovered WAL `Sign` successor.")
            .next()
            .expect("recovered WAL carrier follows remote Proposal replay tokens");
        for required in [
            "PreparedRemoteProposalStoreReplayPreAdmission",
            "PreparedRemoteProposalStoredReplayPreAdmission",
            "PreparedRemoteProposalValidateReplayPreAdmission",
            "replay_evidence: RemoteProposalFetchReplayEvidenceV1",
            "replay_evidence: RemoteProposalStoreReplayEvidenceV1",
            "replay_evidence: RemoteProposalStoredReplayEvidenceV1",
            "replay_evidence: RemoteProposalValidateReplayEvidenceV1",
            "pub(super) fn seal_exact_fetch(",
            "ownership.exact_remote_proposal_fetch_replay(&effect)",
            "pub(super) fn project_store(",
            ".project_exact_store(&effect, &pending)",
            "pub(super) fn bind_durable_body(",
            ".bind_durable_body(&effect, &durable_receipt)",
            "pub(super) fn project_validate(",
            ".project_exact_validate(",
            "fn into_durable_validate_carrier(",
            "replay_evidence: DurableValidateReplayEvidenceV1::remote_proposal(",
            "_fetch: PreparedRemoteProposalFetchReplayPreAdmission",
            "_store: PreparedRemoteProposalStoreReplayPreAdmission",
            "_stored: PreparedRemoteProposalStoredReplayPreAdmission",
            "_ownership: RuntimeEffectOwnership",
        ] {
            assert!(
                token.contains(required),
                "remote Proposal replay token omitted {required}"
            );
        }
        for declaration in [
            "PreparedRemoteProposalFetchReplayPreAdmission {",
            "PreparedRemoteProposalStoreReplayPreAdmission {",
            "PreparedRemoteProposalStoredReplayPreAdmission {",
            "PreparedRemoteProposalValidateReplayPreAdmission {",
        ] {
            let declaration = token
                .split(declaration)
                .nth(1)
                .expect("remote Proposal token declaration is present")
                .split('}')
                .next()
                .expect("remote Proposal token declaration is bounded");
            assert!(!declaration.contains("Option<"));
            assert!(!declaration.contains("derive(Clone"));
        }
        for forbidden in [
            "Decode",
            "fn from_parts(",
            "fn into_parts(",
            "fn effect(",
            "fn pending(",
            "fn receipt(",
            "fn source(",
            "fn ingress(",
            "fn proposal(",
            "fn install(",
            "fn commit(",
            "ConcreteLifecycleWorkRegistry",
            ".entries",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !token.contains(forbidden),
                "remote Proposal replay token exposed forbidden surface {forbidden}"
            );
        }
        assert_eq!(
            production
                .matches("PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(")
                .count(),
            0,
            "the inert remote Proposal token has no production admission caller"
        );
        for caller in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_concrete_admission.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let caller = caller
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("caller production prefix is bounded");
            assert!(!caller.contains("PreparedRemoteProposalFetchReplayPreAdmission"));
        }
    }

    #[test]
    fn invalid_body_replay_pre_admission_is_closed_exact_and_unwired() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("work registry has one production prefix");
        let token = production
            .split("pub(super) struct PreparedInvalidBodyReportReplayPreAdmission")
            .nth(1)
            .expect("invalid-body replay token has one declaration")
            .split("/// Ownership-preserving failure from the fixed Ready Validate adapter join.")
            .next()
            .expect("Ready Validate preview failure follows invalid-body replay");
        for required in [
            "registry: PreparedReadyDurableValidateExecution",
            "adapter: PreparedInvalidBodyReportAdapterReplay",
            "preview: PreparedReadyDurableValidateAdapterPreview",
            "pub(super) fn seal_invalid_body_report_replay(",
            "ReadyDurableValidateOutcomeKind::Rejected",
            "BodyValidationRejectionIdentity::Rejected",
            "let validate_origin = completion.incumbent.replay_evidence.clone()",
            "adapter.seal_invalid_body_report_replay(",
            "&completion.incumbent.effect",
            "&completion.incumbent.pending",
            "&completion.incumbent.durable_receipt",
            "Err(adapter) =>",
            "preview: Self",
            "fn validates(&self)",
            "pub(super) fn project_for_body_transition(",
            "SealedInvalidBodyReportProjectionPermit",
            ".project_invalid_body_report_candidate(",
            "candidate.replay_authority_is_exact(active_context)",
            "SealedInvalidBodyReportProjection::from_registry",
        ] {
            assert!(
                token.contains(required),
                "invalid-body replay token omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "Decode",
            "Option<InvalidBodyReport",
            "fn into_parts(",
            "fn effect(",
            "fn pending(",
            "fn receipt(",
            "fn certificate(",
            "fn source(",
            "fn install(",
            "fn commit(",
            "fn candidate(",
            "fn report_effect(",
            "projection::admission_request",
            "!= [0; 32]",
            "== [0; 32]",
            "is_zero()",
        ] {
            assert!(
                !token.contains(forbidden),
                "invalid-body replay token exposed forbidden surface {forbidden}"
            );
        }
        assert_eq!(
            production
                .matches("adapter.seal_invalid_body_report_replay(")
                .count(),
            1,
            "only the fixed Ready registry join may invoke the adapter seal"
        );
        for outside in [
            include_str!("v2_lifecycle_ledger.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let outside = outside
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("outside production prefix is bounded");
            assert!(!outside.contains("PreparedInvalidBodyReportReplayPreAdmission"));
            assert!(!outside.contains("InvalidBodyReportReplayEvidenceV1"));
        }
    }

    #[test]
    fn live_validate_sign_join_is_linear_opaque_and_unwired_from_runner() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("work registry has one production prefix");
        let authority = production
            .split("pub(in crate::sumeragi) struct ReadyValidateSignPredecessorAuthority")
            .nth(1)
            .expect("Validate Sign predecessor authority is declared")
            .split("impl<'a> ReadyValidatedAdapterAuthority<'a>")
            .next()
            .expect("validated preview authority follows the Sign authority");
        for required in [
            "_linearity: ReadyValidateSignPredecessorLinearity",
            "impl Drop for ReadyValidateSignPredecessorLinearity",
            "pub(in crate::sumeragi) fn project_successor(\n        self,",
            "project_validate_sign_prepare_successor",
            "project_validate_sign_commit_successor_with_registered_prepare",
        ] {
            assert!(
                authority.contains(required),
                "Validate Sign predecessor authority omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "derive(Copy",
            "fn into_parts(",
            "fn effect(",
            "fn pending(",
            "fn certificate(",
        ] {
            assert!(
                !authority.contains(forbidden),
                "Validate Sign predecessor authority exposed {forbidden}"
            );
        }

        let join = production
            .split("pub(super) fn seal_live_wal_validate_sign(")
            .nth(1)
            .expect("fixed Validate Sign join is declared")
            .split("/// Consume only the exact Ready/rejected report preview")
            .next()
            .expect("invalid-body join follows the live Sign join");
        for required in [
            "registry.validate_sign_predecessor_authority()",
            "adapter.bind_validate_sign_predecessor(predecessor)",
            "adapter.append_live_wal()",
            "ReadyDurableValidateSignPreAdmissionFailure::PreWal",
            "ReadyDurableValidateSignPreAdmissionFailure::Wal",
            "PreparedReadyDurableValidatePersistedSignPreAdmission",
        ] {
            assert!(
                join.contains(required),
                "fixed live Sign join omitted {required}"
            );
        }
        for forbidden in [
            "&AdapterEffect",
            "&PendingRuntimeEffectBinding",
            "&DurableBodyReceipt",
            "QuorumCertificate",
            "LiveWalFrameIdentity",
            "fn into_parts(",
            "fn commit(",
            "fn install(",
        ] {
            assert!(
                !join.contains(forbidden),
                "fixed live Sign join exposed {forbidden}"
            );
        }
        assert_eq!(
            production
                .matches("adapter.bind_validate_sign_predecessor(predecessor)")
                .count(),
            1,
            "only the fixed Ready registry join binds the adapter Sign predecessor"
        );
        for caller in [
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            let caller = caller
                .split("\n#[cfg(test)]\nmod tests {")
                .next()
                .expect("caller production prefix is bounded");
            assert!(!caller.contains("seal_live_wal_validate_sign"));
            assert!(!caller.contains("PreparedReadyDurableValidatePersistedSignPreAdmission"));
        }
    }

    #[test]
    fn sealed_validate_no_successor_branch_inventory_is_exact() {
        for publication in [
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive,
            ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect,
        ] {
            assert_eq!(
                sealed_validate_no_successor_reservation(
                    publication,
                    ReadyDurableValidateOutcomeKind::Validated,
                ),
                Ok(false)
            );
            assert_eq!(
                sealed_validate_no_successor_reservation(
                    publication,
                    ReadyDurableValidateOutcomeKind::Rejected,
                ),
                Err(SealedValidateTerminalProjectionError::InvalidCarrier)
            );
        }
        for publication in [
            ReadyDurableValidateAdapterPublicationKind::RejectedInactive,
            ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
        ] {
            assert_eq!(
                sealed_validate_no_successor_reservation(
                    publication,
                    ReadyDurableValidateOutcomeKind::Rejected,
                ),
                Ok(true)
            );
            assert_eq!(
                sealed_validate_no_successor_reservation(
                    publication,
                    ReadyDurableValidateOutcomeKind::Validated,
                ),
                Err(SealedValidateTerminalProjectionError::InvalidCarrier)
            );
        }
        for publication in [
            ReadyDurableValidateAdapterPublicationKind::ValidatedBusy,
            ReadyDurableValidateAdapterPublicationKind::ValidatedApply,
            ReadyDurableValidateAdapterPublicationKind::ValidatedPersist,
            ReadyDurableValidateAdapterPublicationKind::RejectedBusy,
            ReadyDurableValidateAdapterPublicationKind::RejectedReport,
        ] {
            for outcome in [
                ReadyDurableValidateOutcomeKind::Validated,
                ReadyDurableValidateOutcomeKind::Rejected,
            ] {
                assert_eq!(
                    sealed_validate_no_successor_reservation(publication, outcome),
                    Err(SealedValidateTerminalProjectionError::InvalidBranch)
                );
            }
        }
    }

    #[test]
    fn registry_remains_inert_and_scheduler_free() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("registry source has one production prefix");
        for forbidden in [
            "SchedulerInputs".to_owned(),
            "TurnPlan".to_owned(),
            "ready_index:".to_owned(),
            "high_water:".to_owned(),
            "active_lease:".to_owned(),
            "next_lease:".to_owned(),
            "capacity_used:".to_owned(),
            "observed_generation:".to_owned(),
            "producer_debts:".to_owned(),
            "fn plan(".to_owned(),
            "fn settle_turn(".to_owned(),
            "reserve_one".to_owned(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "registry acquired forbidden scheduler authority: {forbidden}"
            );
        }
        let coordinator = include_str!("v2_lifecycle_coordinator.rs");
        assert_eq!(
            coordinator
                .matches(&["work_registry", "::"].concat())
                .count(),
            1,
            "only the narrow opaque registry authority types may cross the module boundary"
        );
        let export = coordinator
            .split("pub(crate) use work_registry::{")
            .nth(1)
            .expect("coordinator has one narrow registry re-export")
            .split("};")
            .next()
            .expect("registry re-export is bounded");
        assert!(export.contains("PreparedReadyDurableValidateExecution"));
        assert!(export.contains("ReadyDurableValidateOutcomeKind"));
        assert!(export.contains("ReadyValidatedAdapterAuthority"));
        assert!(export.contains("ReadyRejectedAdapterAuthority"));
        assert!(export.contains("RecoveredWalValidateRegistryCut"));
        assert!(export.contains("RecoveredWalValidateRegistryJoinError"));
        assert!(export.contains("AuthenticatedRecoveredWalValidateLifecycleRepair"));
        assert!(export.contains("DurableAuthenticatedRecoveredWalValidateLifecycleRepair"));
        assert!(export.contains("RecoveredWalValidateLedgerPersistError"));
        assert!(export.contains("InstalledRecoveredWalSignRegistryCut"));
        assert!(export.contains("RecoveredWalSignInstallError"));
        assert!(!export.contains("ConcreteLifecycleWorkRegistry"));
        assert!(!export.contains("ReadyDurableValidateExecutionError"));
        assert!(!coordinator.contains("pub(crate) use wal_recovery"));
    }

    #[test]
    fn installed_body_projection_and_recovered_prepare_fixture_keep_authority_closed() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let permit = source
            .split("pub(in crate::sumeragi) struct InstalledBodyCandidateProjectionPermit")
            .nth(1)
            .expect("installed-body projection permit has one declaration")
            .split("impl ConcreteWorkAddress")
            .next()
            .expect("concrete address follows the projection permit");
        for required in [
            "_linearity: InstalledBodyCandidateProjectionLinearity",
            "impl Drop for InstalledBodyCandidateProjectionLinearity",
            "struct SealedBodySuccessorProjectionPermit",
            "_linearity: SealedBodySuccessorProjectionLinearity",
            "impl Drop for SealedBodySuccessorProjectionLinearity",
            "fn new() -> Self",
        ] {
            assert!(
                permit.contains(required),
                "projection permit omitted {required}"
            );
        }
        assert!(!permit.contains("derive(Clone"));
        assert!(!permit.contains("derive(Copy"));

        let fixture = source
            .split("fn install_remote_proposal_validate_completion_for_test")
            .nth(1)
            .expect("remote-Proposal fixture has one registry entrypoint")
            .split("pub(crate) fn recovered_wal_validate_registry_cut_for_test")
            .next()
            .expect("recovered WAL wrapper follows its shared fixture");
        for required in [
            "bind_authenticated_remote_proposal_replay_for_test",
            "exact_remote_proposal_fetch_replay",
            "project_proposal_fetch_store_successor",
            "project_exact_store",
            "bind_durable_body",
            "project_store_validate_successor",
            "project_exact_validate",
            "DurableValidateReplayEvidenceV1::remote_proposal",
            "project_installed_validate_candidate",
        ] {
            assert!(
                fixture.contains(required),
                "recovered Prepare fixture omitted exact origin step {required}"
            );
        }
        for forbidden in [
            "certified_pipeline_replay_evidence_for_test",
            "projection::admission_request(",
            "DurableValidateReplayEvidenceV1::certified",
        ] {
            assert!(
                !fixture.contains(forbidden),
                "recovered Prepare fixture fabricated authority through {forbidden}"
            );
        }

        let replay = include_str!("v2_lifecycle_replay_authority.rs");
        for required in [
            "fn project_installed_store_candidate(",
            "fn project_installed_validate_candidate(",
            "fn project_sealed_store_successor_candidate(",
            "fn project_sealed_validate_successor_candidate(",
            "_permit: InstalledBodyCandidateProjectionPermit",
            "_permit: SealedBodySuccessorProjectionPermit",
            "authority_free_admission_projection(",
        ] {
            assert!(
                replay.contains(required),
                "installed-body replay projection omitted {required}"
            );
        }

        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let sealed_projection = production
            .split("fn sealed_successor_parent")
            .nth(1)
            .expect("sealed successor parent join has one implementation")
            .split("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
            .next()
            .expect("Ready Validate join follows sealed body projection");
        for required in [
            "ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)",
            "work.validates_at(address)",
            "work.digest != digest",
            "completion.address != self._completion_address",
            "store.address != self._store_address",
            "self._durable_body.manifest_hash() != self._expected_manifest_hash",
            "self._store_digest",
            "self._validate_digest",
            "project_sealed_store_successor_candidate",
            "project_sealed_validate_successor_candidate",
            "SealedBodySuccessorProjectionPermit::new()",
            "sealed_successor_candidate_has_exact_geometry",
        ] {
            assert!(
                sealed_projection.contains(required),
                "sealed body successor projection omitted {required}"
            );
        }
        assert_eq!(
            sealed_projection
                .matches("pub(super) fn project_for_body_transition(")
                .count(),
            2
        );
        for forbidden in [
            "projection::admission_request(",
            "candidate.payload =",
            "fn commit(",
            ".insert(",
            ".remove(",
            "for_test",
        ] {
            assert!(
                !sealed_projection.contains(forbidden),
                "sealed body successor projection acquired {forbidden}"
            );
        }
    }

    #[test]
    fn certified_fetch_execution_surface_is_borrow_bound_and_commit_free() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let execution_impl = source
            .split("impl<'a> PreparedCertifiedFetchExecution<'a>")
            .nth(1)
            .expect("execution token has one typed implementation")
            .split("impl<'a> PreparedCertifiedFetchCompletion<'a>")
            .next()
            .expect("completion conversion follows the execution token");
        assert!(execution_impl.contains("pub(super) fn adapter_preview_inputs"));
        assert!(execution_impl.contains("pub(super) fn durable_body_receipt"));
        assert!(execution_impl.contains("pub(super) fn seal_store_successor"));
        assert!(
            !execution_impl.contains("fn commit("),
            "the inert execution tranche must not mutate or publish its parent/child cut"
        );
        assert!(
            !execution_impl.contains("for_test"),
            "the execution token must not acquire a raw test mint"
        );

        let successor_declaration = source
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .nth(1)
            .expect("Store successor has one private declaration")
            .split("pub(super) struct PreparedCertifiedFetchCompletion<'a>")
            .next()
            .expect("completion token follows the Store successor");
        assert!(successor_declaration.contains("&'a mut ConcreteLifecycleWorkRegistry"));
        assert!(successor_declaration.contains("_store_effect: AdapterEffect"));
        assert!(successor_declaration.contains("PendingRuntimeEffectBinding"));
        assert!(successor_declaration.contains("DurableBodyReceipt"));
        assert!(successor_declaration.contains("_expected_manifest_hash"));
        assert!(!successor_declaration.contains("derive(Clone"));
    }

    #[test]
    fn durable_store_execution_surface_is_closed_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = production
            .split("struct DurableStoreBody {")
            .nth(1)
            .expect("durable Store carrier has one declaration")
            .split("impl DurableStoreBody")
            .next()
            .expect("durable Store validation follows its declaration");
        for required in [
            "address: ConcreteWorkAddress",
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                carrier.contains(required),
                "Store carrier omitted {required}"
            );
        }
        assert!(!carrier.contains("derive(Clone"));

        let validation = production
            .split("impl DurableStoreBody {")
            .nth(1)
            .expect("durable Store has one validation implementation")
            .split("struct DurableValidateBody")
            .next()
            .expect("Validate carrier follows Store validation");
        for required in [
            "ConcreteWorkAddress::new",
            "causal_lifecycle_key()",
            "exactly_binds_adapter_effect",
            "exact_effect_identity()",
            "durable_receipt.context_id()",
            "durable_receipt.round()",
            "durable_receipt.subject()",
            "durable_receipt.manifest_hash() == self.expected_manifest_hash",
        ] {
            assert!(
                validation.contains(required),
                "durable Store validation omitted {required}"
            );
        }

        let preparation = production
            .split("pub(super) fn prepare_durable_store_execution(")
            .nth(1)
            .expect("durable Store has one preparation method")
            .split("pub(super) fn prepare_durable_validate_execution(")
            .next()
            .expect("Validate preparation follows Store preparation");
        for required in [
            "store\n            .project_candidate(verified)",
            "durable_validate_body_payload(&store.durable_receipt)",
            "candidate.key != lease.key()",
            "candidate.causal_root != lease.owner().causal_root()",
            "candidate.payload != expected_payload",
            ".physical_geometry",
            ".normalized()",
            "projected_slots != *lease.physical_slots()",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "durable Store preparation omitted {required}"
            );
        }
        assert!(!preparation.contains("projection::admission_request("));
        assert!(!preparation.contains(".insert("));
        assert!(!preparation.contains(".remove("));

        let execution_impl = production
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .nth(1)
            .expect("durable Store token has one implementation")
            .split("impl<'a> PreparedDurableValidateExecution<'a>")
            .next()
            .expect("Validate execution follows Store execution token");
        for required in [
            "pub(super) fn adapter_preview_inputs",
            "pub(super) fn durable_body_receipt",
            "pub(super) fn matches_durable_payload",
            "pub(super) fn expected_manifest_hash",
            "pub(super) fn seal_validate_successor",
            "project_store_validate_successor",
            "candidate_statement()",
            "exact_effect_identity()",
        ] {
            assert!(
                execution_impl.contains(required),
                "durable Store execution omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(
                !execution_impl.contains(forbidden),
                "durable Store token acquired forbidden authority: {forbidden}"
            );
        }

        let validate_token = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .nth(1)
            .expect("Validate successor has one declaration")
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .next()
            .expect("Fetch successor follows Validate token");
        assert!(validate_token.contains("&'a mut ConcreteLifecycleWorkRegistry"));
        assert!(validate_token.contains("_validate_effect: AdapterEffect"));
        assert!(validate_token.contains("_validate_pending: PendingRuntimeEffectBinding"));
        assert!(validate_token.contains("_durable_body: DurableBodyReceipt"));
        assert!(validate_token.contains("_expected_manifest_hash"));
        assert!(!validate_token.contains("derive(Clone"));

        let fetch_execution = production
            .split("impl<'a> PreparedCertifiedFetchExecution<'a>")
            .nth(1)
            .expect("certified Fetch execution has one implementation")
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .next()
            .expect("durable Store execution follows Fetch execution");
        assert!(fetch_execution.contains("HashOf::new(&response.manifest)"));
        assert!(fetch_execution.contains("_expected_manifest_hash: expected_manifest_hash"));
        assert!(
            !fetch_execution.contains("durable_body.manifest_hash()"),
            "parent manifest authority must not be re-read from the body receipt"
        );

        assert_eq!(
            production
                .matches("fn prepare_durable_store_execution(")
                .count(),
            1,
            "the inert Store preflight must have no production caller"
        );
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
        ] {
            assert!(!caller_source.contains("prepare_durable_store_execution"));
        }
    }

    #[test]
    fn durable_validate_execution_surface_is_closed_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = production
            .split("struct DurableValidateBody {")
            .nth(1)
            .expect("durable Validate carrier has one declaration")
            .split("impl DurableValidateBody")
            .next()
            .expect("durable Validate validation follows its declaration");
        for required in [
            "address: ConcreteWorkAddress",
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                carrier.contains(required),
                "Validate carrier omitted {required}"
            );
        }
        assert!(!carrier.contains("derive(Clone"));

        let validation = production
            .split("impl DurableValidateBody {")
            .nth(1)
            .expect("durable Validate has one validation implementation")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work kind follows Validate validation");
        for required in [
            "AdapterEffect::ValidateBody",
            "ConcreteWorkAddress::new",
            "self.address.owner.causal_root()",
            "causal_lifecycle_key()",
            "exactly_binds_adapter_effect",
            "exact_effect_identity()",
            "durable_receipt.context_id()",
            "durable_receipt.round()",
            "durable_receipt.subject()",
            "durable_receipt.manifest_hash() == self.expected_manifest_hash",
        ] {
            assert!(
                validation.contains(required),
                "durable Validate validation omitted {required}"
            );
        }
        for forbidden in ["fn new(", "for_test", "derive(Clone", "fn commit("] {
            assert!(
                !validation.contains(forbidden),
                "durable Validate carrier acquired a raw authority seam: {forbidden}"
            );
        }

        let common_work = production
            .split("impl ConcreteLifecycleWork {")
            .nth(1)
            .expect("concrete work has one implementation")
            .split("pub(super) enum CertifiedFetchCompletionError")
            .next()
            .expect("completion errors follow common concrete-work paths");
        assert_eq!(
            common_work
                .matches("ConcreteLifecycleWorkKind::DurableValidateBody")
                .count(),
            5,
            "Validate carrier must remain exhaustive in validation, address, effect, pending, and generic-adapter rejection paths"
        );
        assert_eq!(
            common_work
                .matches("ConcreteLifecycleWorkKind::DurableRecoveredWalSign")
                .count(),
            5,
            "recovered Sign must remain exhaustive in validation, address, causal-root, effect-borrow, and generic-adapter rejection paths"
        );
        assert_eq!(
            common_work
                .matches("ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch")
                .count(),
            5,
            "recovered Decision Fetch must remain exhaustive in validation, address, causal-root, effect-borrow, and generic-adapter rejection paths"
        );

        assert!(production.contains("pub(super) struct ConcreteLifecycleWork {"));
        assert!(!production.contains("pub(in crate::sumeragi) struct ConcreteLifecycleWork {"));
        let live_sign_work = production
            .split("pub(in crate::sumeragi) struct PreparedLiveValidateSignRegistryWork {")
            .nth(1)
            .and_then(|suffix| {
                suffix
                    .split("pub(super) enum CertifiedFetchCompletionError")
                    .next()
            })
            .expect("opaque live Sign work has one bounded implementation");
        for required in [
            "work: ConcreteLifecycleWork",
            "_permit: LiveValidateSignWorkProjectionPermit",
            "ConcreteLifecycleWork::from_exact(effect, pending)",
            "self.work.validate_exact()",
            "self.work.digest() == digest",
            "self.work.causal_root() == owner.causal_root()",
            "ConcreteLifecycleWorkKind::PendingAdapter",
            "SignRequest::Vote(vote)",
            "wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit",
            "ConcreteWorkAddress::new(owner, ordinal, slot)",
            "PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)",
            "reservation.install_live_sign(self.work)",
        ] {
            assert!(
                live_sign_work.contains(required),
                "opaque live Sign work omitted {required}"
            );
        }
        for forbidden in [
            "pub work:",
            "derive(Clone",
            "into_parts",
            "fn effect(",
            "fn pending(",
            "fn receipt(",
            "fn candidate(",
        ] {
            assert!(
                !live_sign_work.contains(forbidden),
                "opaque live Sign work exposes forbidden surface {forbidden}"
            );
        }

        let preparation = production
            .split("pub(super) fn prepare_durable_validate_execution(")
            .nth(1)
            .expect("durable Validate has one preparation method")
            .split("pub(super) fn borrow_for_lease(")
            .next()
            .expect("generic lease borrow follows Validate preparation");
        for required in [
            "LifecycleWorkClass::Validate",
            "LifecyclePhase::Validate",
            "LifecycleStageKind::ValidateBody",
            "PredecessorScope::Independent",
            "validate\n            .project_candidate(verified)",
            "durable_validate_body_payload(&validate.durable_receipt)",
            "candidate.key != lease.key()",
            "candidate.causal_root != lease.owner().causal_root()",
            "candidate.initial_state != InitialLifecycleState::Ready",
            "candidate.reconstruction_source != lease.owner().causal_root().digest()",
            "candidate.payload != expected_payload",
            "candidate.producer_turn.is_some()",
            ".physical_geometry",
            ".normalized()",
            "projected_slots.len() != 1",
            "projected_slots != *lease.physical_slots()",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "durable Validate preparation omitted {required}"
            );
        }
        assert!(!preparation.contains("projection::admission_request("));
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(
                !preparation.contains(forbidden),
                "durable Validate preparation acquired forbidden authority: {forbidden}"
            );
        }

        let execution_impl = production
            .split("impl<'a> PreparedDurableValidateExecution<'a>")
            .nth(1)
            .expect("durable Validate token has one implementation")
            .split("impl PreparedValidatedBodyCompletion<'_>")
            .next()
            .expect("validated completion follows Validate execution token");
        for required in [
            "pub(super) fn adapter_preview_inputs",
            "pub(super) fn durable_body_receipt",
            "pub(super) fn expected_manifest_hash",
            "pub(super) fn durable_validation_wait_source",
            "pub(super) fn seal_waiting_dispatch",
            "pub(super) fn detach",
            "pub(super) fn bind_validated_receipt",
            "AdapterEffect::ValidateBody",
            "self.durable_validate().expected_manifest_hash",
            "validate_validated_receipt_authority",
            "validated_body_completion_digest",
        ] {
            assert!(
                execution_impl.contains(required),
                "durable Validate execution omitted {required}"
            );
        }
        assert_eq!(
            execution_impl.matches("pub(super) fn ").count(),
            8,
            "Validate token may expose only preview coordinates, the fixed durable-payload equality oracle, durable authorities, sealed wait dispatch, owned detach, and success binding"
        );
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
            "fn new(",
            "durable_body_receipt().manifest_hash()",
        ] {
            assert!(
                !execution_impl.contains(forbidden),
                "durable Validate token acquired forbidden authority: {forbidden}"
            );
        }

        let completion = production
            .split("pub(super) struct PreparedValidatedBodyCompletion<'a>")
            .nth(1)
            .expect("validated completion has one private declaration")
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .next()
            .expect("Store successor follows validated completion declaration");
        for required in [
            "&'a mut ConcreteLifecycleWorkRegistry",
            "incumbent_digest: LifecycleDigest",
            "replacement_digest: LifecycleDigest",
            "validated_receipt: ValidatedBodyReceipt",
        ] {
            assert!(completion.contains(required));
        }
        assert!(!completion.contains("derive(Clone"));

        let completion_impl = production
            .split("impl PreparedValidatedBodyCompletion<'_>")
            .nth(1)
            .expect("validated completion has one implementation")
            .split("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN")
            .next()
            .expect("async Validate handoff follows validated completion");
        for required in [
            "pub(super) const fn adapter_preview_inputs",
            "pub(super) const fn validated_receipt",
            "pub(super) const fn incumbent_digest",
            "pub(super) const fn replacement_digest",
        ] {
            assert!(completion_impl.contains(required));
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(!completion_impl.contains(forbidden));
        }

        let validate_successor = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .nth(1)
            .expect("Store-to-Validate successor has one declaration")
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .next()
            .expect("Fetch successor follows Validate successor");
        for required in [
            "&'a mut ConcreteLifecycleWorkRegistry",
            "_store_address: ConcreteWorkAddress",
            "_validate_effect: AdapterEffect",
            "_validate_digest: LifecycleDigest",
            "_validate_pending: PendingRuntimeEffectBinding",
            "_durable_body: DurableBodyReceipt",
            "_expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                validate_successor.contains(required),
                "Store-to-Validate lineage token omitted {required}"
            );
        }
        assert!(!validate_successor.contains("derive(Clone"));

        assert_eq!(
            production
                .matches("prepare_durable_validate_execution(")
                .count(),
            1,
            "the inert Validate preflight must have no production caller"
        );
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("prepare_durable_validate_execution"));
        }
    }

    #[test]
    fn ready_validate_execution_surface_is_closed_borrow_bound_and_unwired() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let declaration = production
            .split("pub(crate) struct PreparedReadyDurableValidateExecution<'a>")
            .nth(1)
            .expect("Ready Validate token has one declaration")
            .split("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN")
            .next()
            .expect("async handoff follows Ready Validate token");
        assert!(declaration.contains("registry: &'a mut ConcreteLifecycleWorkRegistry"));
        assert!(declaration.contains("address: ConcreteWorkAddress"));
        assert!(declaration.contains("outcome_kind: ReadyDurableValidateOutcomeKind"));
        assert!(declaration.contains("lease: TurnLease"));
        assert!(
            declaration
                .contains("_adapter: PreparedReadyDurableValidateAdapterPublication<'adapter>")
        );
        assert!(!declaration.contains("derive(Clone"));

        let preview_oracles = production
            .split("impl PreparedReadyDurableValidateAdapterPreview<'_, '_>")
            .nth(1)
            .expect("Ready Validate preview has one sealed oracle surface")
            .split("/// Ownership-preserving failure")
            .next()
            .expect("preview failure follows its oracle surface");
        for required in [
            "project_no_successor_for_body_transition",
            "self._registry.matches_exact_lease(lease)",
            "self._adapter.kind()",
            "SealedValidateNoSuccessorProjectionPermit",
            "sealed_validate_no_successor_reservation(",
            "durable_validate_body_payload(&completion.incumbent.durable_receipt)",
            "SealedValidateNoSuccessorProjection::from_registry",
        ] {
            assert!(
                preview_oracles.contains(required),
                "Ready Validate preview omitted sealed oracle {required}"
            );
        }
        for forbidden in [
            "into_parts",
            "-> &DurableBodyReceipt",
            "-> Option<&DurableBodyReceipt>",
            "fn durable_receipt(",
            "fn receipt(",
            "matches_exact_successor_effect",
            "publication_kind",
            "projection::admission_request",
            "CandidateAdmission",
        ] {
            assert!(
                !preview_oracles.contains(forbidden),
                "Ready Validate preview exposed body authority {forbidden}"
            );
        }

        assert_eq!(
            production
                .matches("pub(super) fn prepare_ready_durable_validate_execution(")
                .count(),
            1,
            "the exact Ready completion has one registry entrypoint"
        );
        let preparation = production
            .split("pub(super) fn prepare_ready_durable_validate_execution(")
            .nth(1)
            .expect("Ready Validate preflight exists")
            .split("pub(super) fn reattach_durable_validate_execution(")
            .next()
            .expect("async reattachment follows Ready preflight");
        for required in [
            "LifecycleWorkClass::Validate",
            "LifecyclePhase::Validate",
            "LifecycleStageKind::ValidateBody",
            "PredecessorScope::Independent",
            "validated_lease_address(lease, slot)",
            "ConcreteLifecycleWorkKind::DurableValidateCompletion",
            "completion.validates(work.digest)",
            "candidate_statement.context_id()",
            "candidate_statement.proposal_round()",
            "candidate_statement.subject()",
            "completion.incumbent.expected_manifest_hash",
            "BodyValidationRejectionIdentity::Rejected",
            "validate_validated_receipt_authority",
            "output_reservation()",
            "CapacityClass::Consensus",
            ".incumbent\n            .project_candidate(verified)",
            "durable_validate_body_payload(&completion.incumbent.durable_receipt)",
            "candidate.key != lease.key()",
            "candidate.payload != expected_payload",
            "projected_slots != incumbent_slots",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "Ready Validate preflight omitted {required}"
            );
        }
        assert!(!preparation.contains("projection::admission_request("));
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "rejection_reason",
            "EffectWorkId",
            "BodyValidationTask",
            "SchedulerRank",
            "TurnPlan",
        ] {
            assert!(
                !preparation.contains(forbidden),
                "Ready Validate preflight acquired forbidden authority {forbidden}"
            );
        }

        let fixed_join = production
            .split_once("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
            .expect("Ready Validate fixed join begins")
            .1
            .split_once("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_END")
            .expect("Ready Validate fixed join ends")
            .0;
        for required in [
            "pub(crate) const fn outcome_kind",
            "fn validated_authority",
            "fn rejected_authority",
            "pub(super) fn prepare_adapter_preview",
            "adapter.prepare_sealed_ready_durable_validate_succeeded(authority)",
            "adapter.prepare_sealed_ready_durable_validate_failed(authority)",
            "adapter_preview.preflight_publication()",
            "receipt.durable().manifest_hash()",
            "completion.incumbent.expected_manifest_hash",
            "BodyValidationRejectionIdentity::Rejected",
            "validate_validated_receipt_authority",
        ] {
            assert!(
                fixed_join.contains(required),
                "Ready Validate fixed join omitted {required}"
            );
        }
        for forbidden in [
            "with_validated_preview",
            "with_rejected_preview",
            "FnOnce",
            "-> Option<R>",
            "rejection_reason",
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "pub(crate) fn validated_receipt",
            "pub(crate) fn durable_body_receipt",
            "for_test",
        ] {
            assert!(
                !fixed_join.contains(forbidden),
                "Ready Validate fixed join exposed forbidden authority {forbidden}"
            );
        }

        let recovered_detach = production
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_BEGIN")
            .expect("recovered WAL Validate detach begins")
            .1
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_END")
            .expect("recovered WAL Validate detach ends")
            .0;
        for required in [
            "into_recovered_wal_validate_registry_cut",
            "ReadyDurableValidateOutcomeKind::Validated",
            "self.completion().is_none()",
            "self.registry.entries.remove(&address)",
            "work: Some(work)",
        ] {
            assert!(
                recovered_detach.contains(required),
                "recovered WAL Validate detach omitted {required}"
            );
        }
        for forbidden in ["into_parts", "Clone", "pub(super) fn new(", "for_test"] {
            assert!(
                !recovered_detach.contains(forbidden),
                "recovered WAL Validate detach exposed forbidden authority {forbidden}"
            );
        }

        let live_publication = production
            .split("pub(super) fn prepare_registry_publication(")
            .nth(1)
            .and_then(|suffix| suffix.split("/// Ownership-preserving failure").next())
            .expect("live Validate-to-Sign registry publication has one bounded surface");
        for required in [
            "prepare_registry_work(LiveValidateSignWorkProjectionPermit::new())",
            "ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)",
            "adapter.registry_work_matches(",
            "registry.into_recovered_wal_validate_registry_cut()",
            ".into_live_validate_sign_reservation()",
            "reservation.bind_exact_child(child_address, child_digest)",
            "PreparedLiveValidateSignRegistryPublication",
            "publish_after_ledger_fsync",
            ".install_registry_and_commit_adapter(self.reservation)",
        ] {
            assert!(
                live_publication.contains(required),
                "live Validate-to-Sign registry publication omitted {required}"
            );
        }
        for forbidden in [
            "into_parts",
            "fn effect(",
            "fn pending(",
            "fn receipt(",
            "fn candidate(",
            "persist_durable_projection",
            "publish_status(",
        ] {
            assert!(
                !live_publication.contains(forbidden),
                "live Validate-to-Sign registry publication exposes {forbidden}"
            );
        }

        let recovered_join = production
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN")
            .expect("recovered WAL Validate join begins")
            .1
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END")
            .expect("recovered WAL Validate join ends")
            .0;
        for required in [
            "pub(crate) fn join_recovered_vote",
            "completion.outcome.validated_receipt()",
            "receipt.execution_commitment() == recovered_commitment",
            "pending.project_recovered_wal_vote_successor(&effect, recovered)",
            "DetachedValidateReplayEvidenceV1::Retained(replay_evidence)",
            "authenticate_recovered_wal_vote_lifecycle_from_durable_body(",
            "completion.restore(effect, pending)",
            "self.registry.take()",
            "RecoveredWalValidateRegistryReservation",
        ] {
            assert!(
                recovered_join.contains(required),
                "recovered WAL Validate join omitted {required}"
            );
        }
        for forbidden in [
            "into_parts",
            "pub(crate) fn effect(",
            "pub(crate) fn pending(",
            "fresh_for_test",
            "RuntimeEffectOwnership",
        ] {
            assert!(
                !recovered_join.contains(forbidden),
                "recovered WAL Validate join exposed forbidden authority {forbidden}"
            );
        }

        let recovered_fsync = production
            .split_once("// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_BEGIN")
            .expect("recovered WAL Validate ledger fsync begins")
            .1
            .split_once("// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_END")
            .expect("recovered WAL Validate ledger fsync ends")
            .0;
        for required in [
            "pub(crate) struct DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
            "pub(crate) struct RecoveredWalValidateLedgerPersistError<'registry>",
            "AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
            "RecoveredWalValidateRegistryReservation<'registry>",
            "fn ledger_parent_core_identity_is_exact(",
            "parent.owner() == self.validation.address.owner",
            "parent.ordinal() == self.validation.address.ordinal",
            "fn projected_child_address(",
            "bind_child_if_vacant(child_address, child_digest)",
            "pub(super) fn persist_in_opened_ledger(",
            "opened.stage_authenticated_wal_vote_repair(&self.repair)",
            "store.persist_authenticated_wal_vote_repair(opened, repair)",
            "DurableAuthenticatedWalVoteLifecycleRepair",
            "PostFsync",
        ] {
            assert!(
                recovered_fsync.contains(required),
                "recovered WAL Validate fsync splice omitted {required}"
            );
        }
        for forbidden in [
            "into_parts",
            "pub(crate) fn effect(",
            "pub(crate) fn pending(",
            "pub(crate) fn receipt(",
            "FnOnce",
            "RuntimeEffectOwnership",
            "PendingRuntimeEffectBinding",
        ] {
            assert!(
                !recovered_fsync.contains(forbidden),
                "recovered WAL Validate fsync splice exposed forbidden authority {forbidden}"
            );
        }

        let recovered_install = production
            .split_once("// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_BEGIN")
            .expect("recovered WAL Sign registry install begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_END")
            .expect("recovered WAL Sign registry install ends")
            .0;
        for required in [
            "pub(super) fn install_recovered_sign(",
            "self.post_fsync_authority_is_exact(store)",
            "PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)",
            ".all(|address| address.owner != child.owner)",
            "store.revalidates_durable_authenticated_wal_vote_repair(",
            "ConcreteLifecycleWorkKind::DurableRecoveredWalSign(",
            "std::collections::btree_map::Entry::Vacant(entry)",
            "entry.insert(work);",
            "pub(crate) struct InstalledRecoveredWalSignRegistryCut<'registry>",
            "pub(crate) struct RecoveredWalSignInstallError<'registry>",
            "fn installed_entry_is_exact(",
            "self.registry.entries.contains_key(&self.parent_address)",
            ".filter(|address| address.owner == self.child_address.owner)",
            "sign.validates_in_store(",
        ] {
            assert!(
                recovered_install.contains(required),
                "recovered WAL Sign install omitted {required}"
            );
        }
        for forbidden in [
            "into_parts",
            "into_pair",
            "pub(crate) fn effect(",
            "pub(crate) fn pending(",
            "pub(crate) fn receipt(",
            "PendingRuntimeEffectBinding",
            "RuntimeEffectOwnership",
            "DurableWalVoteLedgerRepairReceipt {",
            "DetachedRecoveredValidateCompletion {",
            "FnOnce",
            "LifecycleCoordinator",
            "publish_status(",
            ".remove(",
        ] {
            assert!(
                !recovered_install.contains(forbidden),
                "recovered WAL Sign install exposed forbidden authority {forbidden}"
            );
        }
        let after_insert = recovered_install
            .split_once("entry.insert(work);")
            .expect("recovered Sign has one insertion")
            .1
            .split_once("    }")
            .expect("install method ends after insertion")
            .0;
        for forbidden in ["return Err", "?", "if ", "match ", "debug_assert"] {
            assert!(
                !after_insert.contains(forbidden),
                "post-insert recovered Sign path acquired fallible check {forbidden}"
            );
        }

        let carrier_inventory = production
            .split("struct DurableRecoveredWalSignWork")
            .nth(1)
            .expect("closed recovered Sign carrier exists")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work-kind inventory follows recovered Sign carrier");
        for required in [
            "repair: DurableAuthenticatedWalVoteLifecycleRepair",
            "validation: DetachedRecoveredValidateCompletion",
            "fn validates_digest(",
            "fn validates_in_store(",
        ] {
            assert!(
                carrier_inventory.contains(required),
                "closed recovered Sign carrier omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "into_parts",
            "into_pair",
            "PendingRuntimeEffectBinding",
        ] {
            assert!(
                !carrier_inventory.contains(forbidden),
                "closed recovered Sign carrier exposes {forbidden}"
            );
        }
        let work_kind_inventory = production
            .split("enum ConcreteLifecycleWorkKind")
            .nth(1)
            .expect("concrete work kind has one inventory")
            .split("/// One move-only concrete effect")
            .next()
            .expect("concrete work follows its kind inventory");
        assert_eq!(
            work_kind_inventory
                .matches("DurableRecoveredWalSign(DurableRecoveredWalSignWork)")
                .count(),
            1,
            "the durable recovered phase-vote handoff owns exactly one closed work variant"
        );

        let wal_recovery = include_str!("v2_lifecycle_wal_recovery.rs");
        let child_effect_borrow = wal_recovery
            .split("pub(super) const fn installed_child_effect(")
            .nth(1)
            .expect("durable WAL repair exposes one narrow child-effect borrow")
            .split("    }")
            .next()
            .expect("child-effect borrow is bounded");
        assert!(child_effect_borrow.contains("self.repair.projection.installed_child_effect()"));
        for forbidden in ["pending", "into_", "clone", "receipt"] {
            assert!(
                !child_effect_borrow.contains(forbidden),
                "child-effect borrow exposed forbidden {forbidden}"
            );
        }
        assert_eq!(
            production.matches(".installed_child_effect()").count(),
            1,
            "only the closed concrete carrier may borrow the durable child effect"
        );

        let ledger_source = include_str!("v2_lifecycle_ledger.rs");
        let frame_revalidation = ledger_source
            .split("pub(super) fn revalidates_durable_authenticated_wal_vote_repair(")
            .nth(1)
            .expect("ledger exposes one narrow durable repair revalidation")
            .split("    /// Atomically replace the ledger")
            .next()
            .expect("ledger revalidation ends before persistence");
        for required in [
            "let Ok(loaded) = self.load()",
            "durable.belongs_to_loaded(self, &loaded)",
            "loaded.stage_authenticated_wal_vote_repair(durable.repair())",
            "!changed",
            "observed_child_ordinal == durable.child_ordinal()",
            "staged == loaded",
        ] {
            assert!(
                frame_revalidation.contains(required),
                "same-frame recovered Sign preflight omitted {required}"
            );
        }
        assert_eq!(
            frame_revalidation.matches("self.load()").count(),
            1,
            "receipt hash and repaired-pair shape must share one loaded frame"
        );

        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("prepare_ready_durable_validate_execution"));
            assert!(!caller_source.contains("installed_child_effect"));
        }
    }

    #[test]
    fn recovered_wal_sign_open_is_opaque_precommit_checked_and_runner_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let open = production
            .split_once("// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_BEGIN")
            .expect("recovered Sign coordinator open begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_END")
            .expect("recovered Sign coordinator open ends")
            .0;
        for required in [
            "pub(super) struct AuthenticatedRecoveredWalSignProjection",
            "parent: CandidateAdmission",
            "child: CandidateAdmission",
            "parent_address: ConcreteWorkAddress",
            "child_address: ConcreteWorkAddress",
            "fn repaired_pair_is_exact(",
            "record.replay_matches_candidate(&self.child)",
            "parent.replay_matches_candidate(&self.parent)",
            "parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))",
            "parent.continuation()",
            "fn insert_repaired_child_from_record(",
            "record.owner() != self.child_address.owner",
            "record.ordinal() != self.child_address.ordinal",
            "fn splice_candidates(",
            "(Some(parent), None) if parent == &self.parent",
            "(None, Some(child)) if child == &self.child",
            "pub(crate) struct OpenedRecoveredWalSignLifecycleCut<'registry>",
            "pub(crate) struct RecoveredWalSignLifecycleOpenError<'registry>",
            "LifecycleCoordinator::prepare_with_authority_borrowed(",
            "self.prepared_join_is_exact(&prepared, &recovery, &projection)",
            "prepared.commit(payload_store, &recovery)",
            "self.opened_join_is_exact(&coordinator, &recovery, &projection)",
            "PostCommitMismatch",
        ] {
            assert!(
                open.contains(required),
                "recovered Sign open omitted {required}"
            );
        }
        for forbidden in [
            "pub parent:",
            "pub child:",
            "fn new(",
            "into_parts",
            "pub(crate) fn effect(",
            "pub(crate) fn pending(",
            "pub(crate) fn receipt(",
            "publish_status(",
            "RuntimeEffectOwnership",
        ] {
            assert!(
                !open.contains(forbidden),
                "recovered Sign open exposed forbidden surface {forbidden}"
            );
        }
        let precommit = open
            .find("self.prepared_join_is_exact(&prepared, &recovery, &projection)")
            .expect("precommit exact join exists");
        let commit = open
            .find("prepared.commit(payload_store, &recovery)")
            .expect("durable open commit exists");
        let postcommit = open
            .find("self.opened_join_is_exact(&coordinator, &recovery, &projection)")
            .expect("postcommit exact join exists");
        assert!(precommit < commit && commit < postcommit);

        for seed in [
            "seed_parent_candidate_for_test",
            "seed_child_candidate_for_test",
            "seed_both_candidates_for_test",
            "seed_parent_recovery_for_test",
            "seed_child_recovery_for_test",
            "seed_both_recovery_for_test",
        ] {
            let offset = open.find(seed).unwrap_or_else(|| panic!("missing {seed}"));
            let prefix = &open[offset.saturating_sub(180)..offset];
            assert!(
                prefix.contains("#[cfg(test)]"),
                "fixture seed {seed} must remain test-only"
            );
        }
        let projection_impl = open
            .split_once("impl AuthenticatedRecoveredWalSignProjection")
            .expect("opaque installed projection impl exists")
            .1
            .split_once("/// Sealed coordinator-open result")
            .expect("opaque installed projection impl ends")
            .0;
        for seed in [
            "seed_parent_candidate_for_test",
            "seed_child_candidate_for_test",
            "seed_both_candidates_for_test",
        ] {
            assert!(
                projection_impl.contains(seed),
                "fixture seed {seed} must require the opaque installed projection"
            );
        }
        for seed in [
            "seed_parent_recovery_for_test",
            "seed_child_recovery_for_test",
            "seed_both_recovery_for_test",
        ] {
            let offset = open.find(seed).unwrap_or_else(|| panic!("missing {seed}"));
            let method = &open[offset
                ..offset
                    + open[offset..]
                        .find("\n    }\n")
                        .unwrap_or_else(|| panic!("fixture seed {seed} has no method end"))];
            assert!(
                method.contains("self.authenticated_projection()"),
                "fixture seed {seed} must mint its opaque projection from the installed cut"
            );
            let signature = method
                .split_once('{')
                .expect("fixture seed has a function body")
                .0;
            assert!(
                !signature.contains("AuthenticatedRecoveredWalSignProjection"),
                "fixture seed {seed} must not accept a caller-supplied projection"
            );
        }

        let open_source = include_str!("v2_lifecycle_open.rs");
        let splice = open_source
            .split_once("// RECOVERED_WAL_SIGN_RECOVERY_SPLICE_BEGIN")
            .expect("opaque recovery splice begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_RECOVERY_SPLICE_END")
            .expect("opaque recovery splice ends")
            .0;
        assert!(splice.contains("projection: &AuthenticatedRecoveredWalSignProjection"));
        for forbidden in [
            "parent: &CandidateAdmission",
            "child: &CandidateAdmission",
            "CandidateAdmission) ->",
            "into_parts",
        ] {
            assert!(
                !splice.contains(forbidden),
                "recovery splice accepts forbidden caller material {forbidden}"
            );
        }
        let borrowed = open_source
            .split_once("// RECOVERED_WAL_SIGN_BORROWED_OPEN_BEGIN")
            .expect("borrowed recovery open begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_BORROWED_OPEN_END")
            .expect("borrowed recovery open ends")
            .0;
        assert!(borrowed.contains("prepare_with_authority_borrowed("));
        assert!(borrowed.contains("PreparedLifecycleCoordinatorOpen"));

        for runner_source in [
            include_str!("v2_runner.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_effects.rs"),
        ] {
            assert!(!runner_source.contains("open_coordinator_from_verified"));
            assert!(!runner_source.contains("OpenedRecoveredWalSignLifecycleCut"));
        }
    }

    #[test]
    fn durable_validate_async_handoff_surface_is_move_only_scheduler_free_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN")
            .expect("detached Validate declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_END")
            .expect("detached Validate declarations end")
            .0;
        for required in [
            "struct DetachedDurableValidateExecution",
            "address: ConcreteWorkAddress",
            "incumbent_digest: LifecycleDigest",
            "tag: EventTag",
            "round: wire::ConsensusRound",
            "subject: wire::BlockSubject",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
            "causal_lifecycle_key: Hash",
            "candidate_statement: Option<RuntimeCandidateSemanticStatement>",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct ExecutedDurableValidateExecution",
            "request: DetachedDurableValidateExecution",
            "outcome: DurableBodyValidationOutcome",
            "struct PreparedDurableValidateCompletion<'a>",
            "&'a mut ConcreteLifecycleWorkRegistry",
        ] {
            assert!(
                declarations.contains(required),
                "detached Validate declarations omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "detached Validate declarations acquired forbidden scheduler surface: {forbidden}"
            );
        }

        let implementation = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN")
            .expect("detached Validate implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_END")
            .expect("detached Validate implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 0);
        assert_eq!(implementation.matches("fn execute").count(), 1);
        assert_eq!(
            implementation
                .matches("execute_durable_validation(")
                .count(),
            1
        );
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
            "into_parts",
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "detached Validate implementation acquired forbidden authority: {forbidden}"
            );
        }

        let reattachment = production
            .split("pub(super) fn reattach_durable_validate_execution(")
            .nth(1)
            .expect("detached Validate has one reattachment method")
            .split("pub(super) fn borrow_for_lease(")
            .next()
            .expect("generic borrow follows detached Validate reattachment");
        for required in [
            "ConcreteWorkAddress::new",
            "work.validates_at(request.address)",
            "work.digest != request.incumbent_digest",
            "DurableValidateBody(validate)",
            "exactly_binds_adapter_effect",
            "causal_lifecycle_key() != &request.causal_lifecycle_key",
            "candidate_statement() != request.candidate_statement",
            "executed.outcome.durable_body() != &request.durable_receipt",
            "validate_validated_receipt_authority(validate, receipt)?",
            "return Err((error, executed))",
        ] {
            assert!(
                reattachment.contains(required),
                "detached Validate reattachment omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !reattachment.contains(forbidden),
                "detached Validate reattachment acquired forbidden mutation: {forbidden}"
            );
        }

        assert_eq!(production.matches("pub(super) fn detach(").count(), 1);
        assert_eq!(
            production
                .matches("pub(super) fn reattach_durable_validate_execution(")
                .count(),
            1
        );
        assert_eq!(production.matches(".detach()").count(), 1);
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("DetachedDurableValidateExecution"));
            assert!(!caller_source.contains("reattach_durable_validate_execution"));
        }
    }

    #[test]
    fn durable_validate_wait_dispatch_is_move_only_single_entry_and_unwired() {
        let registry_source = include_str!("v2_lifecycle_work_registry.rs");
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_BEGIN")
            .expect("wait-dispatch declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_END")
            .expect("wait-dispatch declarations end")
            .0;
        for required in [
            "struct DurableValidateWakeAuthority",
            "wait_token: WaitToken",
            "struct DurableValidateDispatch",
            "request: DetachedDurableValidateExecution",
            "struct ExecutedDurableValidateDispatch",
            "executed: ExecutedDurableValidateExecution",
        ] {
            assert!(
                declarations.contains(required),
                "wait-dispatch declaration omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "wait-dispatch declaration acquired legacy authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_BEGIN")
            .expect("wait-dispatch implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_END")
            .expect("wait-dispatch implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 1);
        assert!(implementation.contains("request.execute(body_store, validator)"));
        assert!(implementation.contains("Err((error, Self { request, wake }))"));
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "fn commit(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "wait-dispatch execution acquired forbidden authority: {forbidden}"
            );
        }
        assert_eq!(
            registry_production.matches("pub(super) fn execute").count(),
            1,
            "the outer dispatch must be the sole externally visible validation execution path"
        );
        assert_eq!(
            registry_production
                .matches("projection::durable_validation_wait_source(")
                .count(),
            1,
            "only the sealed registry preflight may call the raw wait projection"
        );

        let concrete_source = include_str!("v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn begin_durable_validate_dispatch(")
                .count(),
            1
        );
        let entrypoint = concrete_production
            .split("pub(super) fn begin_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one dispatch entrypoint")
            .split("/// Atomically publish one exact executable Validate result across the")
            .next()
            .expect("Validate completion follows dispatch entrypoint");
        for required in [
            "claimed_durable_validate_record_is_exact",
            "prepare_durable_validate_execution",
            "prepared.matches_durable_payload(metadata.payload)",
            "durable_validation_wait_source",
            "observed_generation",
            "observed_generation == u64::MAX",
            "AliasedWaitSource",
            "stage_durable_transaction",
            "TurnOutcome::Blocked(wait_token)",
            "staged_durable_validate_wait_is_exact",
            "seal_waiting_dispatch(wait_token)",
            "DurableValidateDispatchError, TurnLease",
            "*self = next",
        ] {
            assert!(
                entrypoint.contains(required),
                "dispatch entrypoint omitted {required}"
            );
        }
        let staging = entrypoint
            .find("stage_durable_transaction")
            .expect("entrypoint stages coordinator state");
        let sealing = entrypoint
            .find("seal_waiting_dispatch")
            .expect("entrypoint seals its dispatch");
        let publication = entrypoint
            .find("*self = next")
            .expect("entrypoint publishes its staged coordinator");
        assert!(staging < sealing && sealing < publication);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "checked_add(",
            "LeaseId(",
            "SchedulerRank::new",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "dispatch entrypoint acquired forbidden authority: {forbidden}"
            );
        }

        let claimed_helper = concrete_production
            .split("fn claimed_durable_validate_record_is_exact(")
            .nth(1)
            .expect("claimed Validate exactness helper exists")
            .split("fn staged_durable_validate_wait_is_exact(")
            .next()
            .expect("staged wait helper follows claimed exactness");
        for required in [
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
            "durable_validate_payload_is_exact(record.key, metadata.payload)",
        ] {
            assert!(
                claimed_helper.contains(required),
                "claimed Validate exactness omitted reverse identity check {required}"
            );
        }
        let staged_helper = concrete_production
            .split("fn staged_durable_validate_wait_is_exact(")
            .nth(1)
            .expect("staged Validate wait helper exists")
            .split("fn concrete_work_location(")
            .next()
            .expect("concrete location helper follows staged wait");
        for required in [
            "next.episode_authority == current.episode_authority",
            "next.ledger_store.is_some() == current.ledger_store.is_some()",
            "next.active_lease.is_none()",
            "next.observed_generation == expected_observed",
        ] {
            assert!(
                staged_helper.contains(required),
                "staged Validate wait omitted exact projection check {required}"
            );
        }

        let projection_source = include_str!("v2_lifecycle_projection.rs");
        let projection = projection_source
            .split("pub(super) fn durable_validation_wait_source(")
            .nth(1)
            .expect("durable validation wait projection exists")
            .split("pub(super) fn reducer_fence_wait_source")
            .next()
            .expect("reducer-fence projection follows durable validation");
        for required in [
            "DURABLE_VALIDATION_WAIT_SOURCE_DOMAIN",
            "owner.causal_root().digest()",
            "owner.first_admission_ordinal()",
            "incumbent_digest",
            "causal_lifecycle_key",
            "candidate_statement",
            "durable_frame_hash",
            "expected_manifest_hash",
            "lifecycle_key",
            "lifecycle_stage",
        ] {
            assert!(
                projection.contains(required),
                "durable validation wait projection omitted {required}"
            );
        }

        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("begin_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateDispatch"));
        }
    }

    #[test]
    fn durable_validate_volatile_completion_is_atomic_move_only_and_unwired() {
        let registry_source = include_str!("v2_lifecycle_work_registry.rs");
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = registry_production
            .split("struct DurableValidateCompletion {")
            .nth(1)
            .expect("Validate completion carrier has one declaration")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work-kind inventory follows Validate completion carrier");
        for required in [
            "address: ConcreteWorkAddress",
            "incumbent: DurableValidateBody",
            "incumbent_digest: LifecycleDigest",
            "outcome: DurableBodyValidationOutcome",
            "self.incumbent.validates(self.incumbent_digest)",
            "self.address.owner.causal_root()",
            "exactly_binds_adapter_effect",
            "self.outcome.durable_body() == &self.incumbent.durable_receipt",
            "self.incumbent.durable_receipt.manifest_hash()",
            "self.incumbent.expected_manifest_hash",
            "validate_validated_receipt_authority(&self.incumbent, receipt)",
            "durable_validate_completion_digest(",
            "installed_digest != self.incumbent_digest",
        ] {
            assert!(
                carrier.contains(required),
                "Validate completion carrier omitted {required}"
            );
        }
        for forbidden in ["derive(Clone", "fn new(", "into_parts"] {
            assert!(
                !carrier.contains(forbidden),
                "Validate completion carrier acquired raw or remintable authority: {forbidden}"
            );
        }

        let rejected_digest = registry_production
            .split("fn rejected_body_completion_digest(")
            .nth(1)
            .expect("rejected completion has one digest helper")
            .split("fn durable_validate_outcome_kind(")
            .next()
            .expect("outcome classification follows rejected digest");
        assert!(rejected_digest.contains("identity.canonical_code()"));
        assert!(!rejected_digest.contains("reason"));
        let validated_authority = registry_production
            .split("fn validate_validated_receipt_authority(")
            .nth(1)
            .expect("validated receipt has one shared authority helper")
            .split("fn validated_body_completion_digest(")
            .next()
            .expect("validated digest follows shared authority helper");
        for required in [
            "validated_receipt.durable() != &validate.durable_receipt",
            "validated_receipt.execution_commitment().validate().is_err()",
            "validate.pending.candidate_statement()",
            "statement.context_id() != round.context_id",
            "statement.proposal_round() != *round",
            "statement.subject() != Some(*subject)",
            ".execution_commitment()",
            "DurableValidateExecutionError::ConflictingValidationCommitment",
        ] {
            assert!(
                validated_authority.contains(required),
                "shared validated authority helper omitted {required}"
            );
        }
        assert_eq!(
            registry_production
                .matches("validate_validated_receipt_authority(")
                .count(),
            8,
            "carrier validation, classification, binding, reattachment, Ready preflight, recovery, and fixed adapter join must share one helper"
        );

        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_BEGIN")
            .expect("volatile completion declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_END")
            .expect("volatile completion declarations end")
            .0;
        for required in [
            "struct DurableValidateCompletionAuthority",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct PublishedValidated",
            "struct PublishedRejected",
            "struct DeferredDurableValidateDispatch",
            "dispatch: ExecutedDurableValidateDispatch",
            "enum DurableValidateCompletionPublication",
            "#[allow(variant_size_differences, clippy::large_enum_variant)]",
            "struct PreparedExecutedDurableValidateCompletion<'a>",
            "struct StagedDurableValidateCompletion<'a>",
            "request: Option<DetachedDurableValidateExecution>",
            "wake: Option<DurableValidateWakeAuthority>",
        ] {
            assert!(
                declarations.contains(required),
                "volatile completion declarations omitted {required}"
            );
        }
        for move_only in [
            "pub(super) struct DeferredDurableValidateDispatch",
            "pub(super) struct PreparedExecutedDurableValidateCompletion<'a>",
            "pub(super) struct StagedDurableValidateCompletion<'a>",
        ] {
            let declaration = declarations
                .split(move_only)
                .next()
                .expect("move-only declaration prefix exists")
                .rsplit("#[derive(")
                .next()
                .expect("derive prefix is inspectable");
            assert!(
                !declaration.contains("Clone"),
                "{move_only} must remain move-only"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "TurnPlan",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "volatile completion declarations acquired legacy scheduler authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_BEGIN")
            .expect("volatile completion implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_END")
            .expect("volatile completion implementation ends")
            .0;
        for required in [
            "pub(super) fn stage_executable_carrier",
            "ConcreteLifecycleWorkKind::DurableValidateBody(incumbent)",
            "ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)",
            "impl Drop for StagedDurableValidateCompletion<'_>",
            "drop(self.restore())",
            "pub(super) fn missing_reference",
        ] {
            assert!(
                implementation.contains(required),
                "volatile completion implementation omitted {required}"
            );
        }
        assert_eq!(implementation.matches("pub(super) fn commit(").count(), 1);
        let commit = implementation
            .split("pub(super) fn commit(mut self)")
            .nth(1)
            .expect("staged completion has one infallible commit")
            .split("impl Drop for StagedDurableValidateCompletion")
            .next()
            .expect("guard Drop follows commit");
        assert!(commit.contains("self.armed = false;"));
        assert!(commit.contains("self.publication"));
        for forbidden in [
            ".get(", ".insert(", ".remove(", "expect(", "assert", "panic!", "?;", "Result<",
        ] {
            assert!(
                !commit.contains(forbidden),
                "post-swap guard commit acquired a fallible operation: {forbidden}"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "replace_before_publication",
            "enqueue_",
            "persist_durable_projection",
            "into_parts",
            "pub(super) fn new(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "volatile completion implementation acquired forbidden authority: {forbidden}"
            );
        }

        let concrete_source = include_str!("v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn complete_durable_validate_dispatch(")
                .count(),
            1,
            "there must be one sealed coordinator completion entrypoint"
        );
        assert_eq!(
            concrete_production
                .matches("prepare_executed_durable_validate_completion(dispatch)")
                .count(),
            1,
            "only the coordinator entrypoint may reattach a full dispatch"
        );
        let entrypoint = concrete_production
            .split("pub(super) fn complete_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one completion entrypoint")
            .split("/// Atomically admit and register one exact adapter effect.")
            .next()
            .expect("generic admission follows completion entrypoint");
        for required in [
            "prepare_executed_durable_validate_completion(dispatch)",
            "waiting_durable_validate_record_is_exact",
            "prepared.defer_merge_sidecar()",
            "authority.ready_event()",
            "stage_durable_transaction()",
            "publish_ready(ready_event)",
            "staged_durable_validate_ready_is_exact",
            "prepared.stage_executable_carrier()?",
            "core::mem::swap(self, &mut next);\n        let published = staged_registry.commit();",
        ] {
            assert!(
                entrypoint.contains(required),
                "completion entrypoint omitted {required}"
            );
        }
        let coordinator_stage = entrypoint
            .find("stage_durable_transaction()")
            .expect("completion stages a coordinator copy");
        let registry_stage = entrypoint
            .find("prepared.stage_executable_carrier()?")
            .expect("completion stages the exact registry carrier");
        let coordinator_swap = entrypoint
            .find("core::mem::swap(self, &mut next)")
            .expect("completion swaps the checked coordinator copy");
        let registry_commit = entrypoint
            .find("staged_registry.commit()")
            .expect("completion infallibly disarms the registry guard");
        assert!(coordinator_stage < registry_stage);
        assert!(registry_stage < coordinator_swap);
        assert!(coordinator_swap < registry_commit);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "enqueue_",
            "persist_durable_projection",
            "ledger_store.",
            "replace_before_publication",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "completion entrypoint acquired forbidden durable or scheduler machinery: {forbidden}"
            );
        }

        let waiting_exact = concrete_production
            .split("fn waiting_durable_validate_record_is_exact(")
            .nth(1)
            .expect("waiting Validate exactness helper exists")
            .split("fn staged_durable_validate_ready_is_exact(")
            .next()
            .expect("staged Ready helper follows waiting exactness");
        for required in [
            "record.key == authority.lifecycle_key()",
            "record.stage == authority.lifecycle_stage()",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
            "durable_validate_payload_is_exact(record.key, metadata.payload)",
            "authority.matches_durable_payload(metadata.payload)",
        ] {
            assert!(
                waiting_exact.contains(required),
                "waiting completion exactness omitted {required}"
            );
        }

        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("complete_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateCompletionPublication"));
        }
    }

    #[test]
    fn certified_fetch_dequeue_commit_requires_the_durable_token() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let preflight_declaration = production
            .split("pub(super) struct PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one declaration")
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .next()
            .expect("durable token follows selector preflight");
        assert!(
            preflight_declaration
                .contains("replay_origin: AuthenticatedCertifiedFetchReplayOriginV1")
        );
        assert!(!preflight_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(!preflight_declaration.contains("derive(Clone"));

        let durable_declaration = production
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("durable completion token has one declaration")
            .split("pub(super) enum RegistryPublicationError")
            .next()
            .expect("registry publication error follows durable token");
        assert!(durable_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(durable_declaration.contains("replay_evidence: CertifiedFetchReplayEvidenceV1"));
        assert!(!durable_declaration.contains("derive(Clone"));

        let preflight_impl = production
            .split("impl<'a> PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one implementation")
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .next()
            .expect("durable implementation follows selector preflight");
        assert!(preflight_impl.contains("pub(super) fn bind_durable_body_receipt"));
        assert!(!preflight_impl.contains("fn commit_after_exact_dequeue("));
        assert!(!preflight_impl.contains(".remove("));
        assert!(!preflight_impl.contains(".insert("));

        let durable_impl = production
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .nth(1)
            .expect("durable completion has one implementation")
            .split("fn ingress_identity_matches_round")
            .next()
            .expect("response helpers follow durable completion");
        assert!(durable_impl.contains("fn commit_after_exact_dequeue("));
        assert_eq!(
            production.matches("fn commit_after_exact_dequeue(").count(),
            1,
            "only the receipt-bound token may own the post-CAS commit"
        );

        let installed_completion = production
            .split("struct CertifiedFetchCompletion {")
            .nth(1)
            .expect("installed completion has one declaration")
            .split("impl CertifiedFetchCompletion")
            .next()
            .expect("installed completion validation follows its declaration");
        assert!(installed_completion.contains("durable_receipt: DurableBodyReceipt"));
        assert!(installed_completion.contains("replay_evidence: CertifiedFetchReplayEvidenceV1"));
        assert!(installed_completion.contains(".project_durable_ready_fetch("));
        assert!(!installed_completion.contains("CertifiedFetchDequeuedResponse"));

        let durable_binding = production
            .split("fn durable_receipt_matches_fetch(")
            .nth(1)
            .expect("durable response binding has one helper")
            .split("fn exact_selected_response_matches(")
            .next()
            .expect("exact dequeue validation follows durable binding");
        for required in [
            "receipt.request_hash()",
            "receipt.response_hash()",
            "durable_body.context_id()",
            "durable_body.round()",
            "durable_body.subject()",
            "durable_body.manifest_hash()",
            "fetch_effect_matches_manifest",
        ] {
            assert!(
                durable_binding.contains(required),
                "durable Fetch binding omitted {required}"
            );
        }
    }

    #[test]
    fn certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let fetch = production
            .split("struct CertifiedFetchCompletion {")
            .nth(1)
            .expect("certified Fetch completion has one declaration")
            .split("/// Closed durable form of one admitted `StoreBody` effect.")
            .next()
            .expect("Store carrier follows certified Fetch completion");
        for required in [
            "replay_evidence: CertifiedFetchReplayEvidenceV1",
            "durable_receipt: DurableBodyReceipt",
            ".project_durable_ready_fetch(",
        ] {
            assert!(
                fetch.contains(required),
                "certified Fetch carrier omitted {required}"
            );
        }
        assert!(!fetch.contains("CertifiedFetchDequeuedResponse"));

        let store = production
            .split("struct DurableStoreBody {")
            .nth(1)
            .expect("durable Store has one declaration")
            .split("/// Closed durable form of one admitted `ValidateBody` effect.")
            .next()
            .expect("Validate carrier follows Store");
        for required in [
            "replay_evidence: CertifiedStoreReplayEvidenceV1",
            ".exactly_matches_store(&self.effect, &self.durable_receipt)",
        ] {
            assert!(store.contains(required), "Store carrier omitted {required}");
        }

        let validate = production
            .split("struct DurableValidateBody {")
            .nth(1)
            .expect("durable Validate has one declaration")
            .split("/// Same-address closed result of one completed durable body validation.")
            .next()
            .expect("Validate completion follows its carrier");
        for required in [
            "replay_evidence: DurableValidateReplayEvidenceV1",
            ".exactly_matches_validate_pending(",
            "&self.effect,\n                &self.durable_receipt,\n                &self.pending",
        ] {
            assert!(
                validate.contains(required),
                "Validate carrier omitted {required}"
            );
        }
        let completion = production
            .split("struct DurableValidateCompletion {")
            .nth(1)
            .expect("durable Validate completion has one declaration")
            .split("impl DurableValidateCompletion")
            .next()
            .expect("Validate completion validation follows its declaration");
        assert!(completion.contains("incumbent: DurableValidateBody"));

        let fetch_successor = production
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a> {")
            .nth(1)
            .expect("Fetch-to-Store successor has one declaration")
            .split("/// Borrow-bound registry conversion prepared")
            .next()
            .expect("certified Fetch completion token follows its successor");
        assert!(fetch_successor.contains("_replay_evidence: CertifiedStoreReplayEvidenceV1"));
        let validate_successor = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a> {")
            .nth(1)
            .expect("Store-to-Validate successor has one declaration")
            .split("/// Move-only Store-successor projection")
            .next()
            .expect("Fetch successor follows Validate successor");
        assert!(validate_successor.contains("_replay_evidence: CertifiedValidateReplayEvidenceV1"));

        let fetch_projection = production
            .split("pub(super) fn seal_store_successor(")
            .nth(1)
            .expect("Fetch-to-Store projection has one implementation")
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .next()
            .expect("Store execution follows Fetch projection");
        assert!(fetch_projection.contains("completion.replay_evidence.project_store("));
        assert!(fetch_projection.contains("_replay_evidence: replay_evidence"));
        let validate_projection = production
            .split("pub(super) fn seal_validate_successor(")
            .nth(1)
            .expect("Store-to-Validate projection has one implementation")
            .split("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
            .next()
            .expect("Ready Validate join follows Store projection");
        assert!(validate_projection.contains("store.replay_evidence.project_validate("));
        assert!(validate_projection.contains("&validate_pending"));
        assert!(validate_projection.contains("_replay_evidence: replay_evidence"));

        let detached = production
            .split("struct DetachedRecoveredValidateCompletion {")
            .nth(1)
            .expect("recovered Validate completion has one detached declaration")
            .split("pub(crate) struct AuthenticatedRecoveredWalValidateLifecycleRepair")
            .next()
            .expect("authenticated repair follows detached evidence");
        for required in [
            "replay_evidence: DetachedValidateReplayEvidenceV1",
            "#[allow(variant_size_differences, clippy::large_enum_variant)]",
            "Retained(DurableValidateReplayEvidenceV1)",
            "RecoveredBodyMarker(DurableBodyReceipt)",
            "Self::Retained(evidence) => evidence.exactly_matches_durable_body(receipt)",
            "Self::RecoveredBodyMarker(recovered) => recovered == receipt",
        ] {
            assert!(
                detached.contains(required),
                "detached Validate replay evidence omitted {required}"
            );
        }
        assert!(
            !detached.contains("=> true"),
            "detached recovery must not use a truth-sentinel provenance bypass"
        );

        let recovered_join = production
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN")
            .expect("recovered Validate join begins")
            .1
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END")
            .expect("recovered Validate join ends")
            .0;
        assert!(
            production.contains(
                "replay_evidence: DetachedValidateReplayEvidenceV1::RecoveredBodyMarker("
            )
        );
        for required in [
            "replay_evidence: DetachedValidateReplayEvidenceV1::Retained(replay_evidence)",
            "let DurableValidateBody {",
            "replay_evidence,",
            "completion.restore(effect, pending)",
        ] {
            assert!(
                recovered_join.contains(required),
                "recovered Validate join dropped {required}"
            );
        }
        let restore = production
            .split("impl DetachedRecoveredValidateCompletion {")
            .nth(1)
            .expect("detached Validate has one restore implementation")
            .split("/// Ownership-preserving failure")
            .next()
            .expect("recovered join error follows detached restore");
        assert!(restore.contains(
            "let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) = self.replay_evidence"
        ));
        assert!(restore.contains("replay_evidence,"));
    }
}
