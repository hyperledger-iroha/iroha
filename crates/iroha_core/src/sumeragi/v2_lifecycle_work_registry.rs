//! Scheduler-free registry for exact concrete lifecycle work.
//!
//! The logical coordinator retains only authenticated slot digests. This
//! module keeps the corresponding process-local effect values in a separate,
//! deterministic map so planning never makes the coordinator own physical
//! bytes or service handles.
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
        RecoveredLifecycleNextWalVoteCandidateProjectionV1, RemoteProposalFetchReplayEvidenceV1,
        RemoteProposalStoreReplayEvidenceV1, RemoteProposalStoredReplayEvidenceV1,
        RemoteProposalValidateReplayEvidenceV1, SealedLiveWalPersistedEffectV1,
        SignedBroadcastReplayEvidenceV1, SignedEquivocationReplayEvidenceV1,
        exact_direct_signed_admission_authority,
    },
    schema::DurablePayloadReference,
    selector::{CertifiedFetchCompletionAuthority, CertifiedFetchDequeuedResponse},
    wal_recovery::{
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection, AuthenticatedWalVoteLifecycleRepair,
        DurableAuthenticatedWalVoteLifecycleRepair, DurableRecoveredWalControlSignCarrierV1,
        DurableRecoveredWalDecisionFetchCarrierV1, RecoveredDecisionFetchStoreProjectionV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        RecoveredLifecycleSignedBroadcastOutputAuthorityV1,
        RecoveredLifecycleSignedBroadcastProjectionV1, RecoveredWalVoteLifecycleRepairError,
        authenticate_recovered_wal_vote_lifecycle_from_durable_body,
        authenticate_recovered_wal_vote_lifecycle_from_ledger_parent,
    },
};
use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    peer::PeerId,
};
use norito::codec::Encode;
use std::{collections::BTreeMap, fmt, path::Path, sync::Arc};
use thiserror::Error;
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
#[cfg(test)]
use crate::sumeragi::v2_runtime::bind_adapter_effect_batch_ownership;
use crate::sumeragi::{
    FairV2IngressDequeueDisposition, InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterEffect, PreparedInvalidBodyReportAdapterReplay,
        PreparedReadyDurableValidateAdapterPublication, PreparedReadyDurableValidatePersistedSign,
        ProductionLifecycleAdapterStartupV1, ReadyDurableValidateAdapterPublicationKind,
        ReadyDurableValidateSignWalError, RecoveredDecisionApplyRegistryCarrierV1,
        RecoveredDecisionApplyStagedStorageV1, RecoveredWalVoteSign,
        RegisteredPrepareValidateSignCapability, SignRequest, SumeragiV2Adapter,
        VerifiedHeightContext,
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
/// One-shot authority to split a staged recovered Decision into its cold
/// adapter and one dedicated Apply carrier inside the exact registry install.
pub(in crate::sumeragi) struct RecoveredDecisionApplyRegistryProjectionPermit {
    _linearity: RecoveredDecisionApplyRegistryProjectionLinearity,
}
/// One-shot proof that an exact claimed recovered Apply owns worker completion.
///
/// Only the concrete registry can mint this permit after joining the active
/// lease, installed carrier, and in-flight dispatch key. The adapter therefore
/// never accepts a detached worker receipt or finality artifact.
pub(in crate::sumeragi) struct RecoveredDecisionApplyCompletionProjectionPermit {
    _linearity: RecoveredDecisionApplyCompletionProjectionLinearity,
}
struct RecoveredDecisionApplyCompletionProjectionLinearity;
impl Drop for RecoveredDecisionApplyCompletionProjectionLinearity {
    fn drop(&mut self) {}
}
impl RecoveredDecisionApplyCompletionProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: RecoveredDecisionApplyCompletionProjectionLinearity,
        }
    }
}
struct RecoveredDecisionApplyRegistryProjectionLinearity;
impl Drop for RecoveredDecisionApplyRegistryProjectionLinearity {
    fn drop(&mut self) {}
}
impl RecoveredDecisionApplyRegistryProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: RecoveredDecisionApplyRegistryProjectionLinearity,
        }
    }
}
/// One-shot authority to split a durably published combined Sign successor.
///
/// Construction is private to the concrete registry. WAL recovery accepts it
/// only by move, so Broadcast and next-Sign executable authority cannot be
/// separated before the exact LedgerV1 fsync and registry replacement tail.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1 {
    _linearity: RecoveredLifecycleBroadcastAndSignRegistryCommitLinearityV1,
}
#[cfg_attr(not(test), allow(dead_code))]
struct RecoveredLifecycleBroadcastAndSignRegistryCommitLinearityV1;
impl Drop for RecoveredLifecycleBroadcastAndSignRegistryCommitLinearityV1 {
    fn drop(&mut self) {}
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleBroadcastAndSignRegistryCommitLinearityV1,
        }
    }
}
/// Logical address of one exact concrete-work slot.
///
/// Digest-only indexing is intentionally forbidden: two logical body stages
/// may retain the same physical carrier while inheriting different authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ConcreteWorkAddress {
    pub(super) owner: OwnerId,
    pub(super) ordinal: u128,
    pub(super) slot: PhysicalSlotId,
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
/// Copyable queue key for one exact recovered Decision Apply dispatch.
///
/// The key is process-local ownership metadata, never a runtime effect work id.
/// It binds the immutable lifecycle context, logical owner, ordinal, physical
/// slot, and installed carrier digest used by the dedicated worker queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::sumeragi) struct RecoveredDecisionApplyDispatchKeyV1 {
    context: LifecycleDigest,
    height: u64,
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    digest: LifecycleDigest,
}
impl RecoveredDecisionApplyDispatchKeyV1 {
    const fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> Self {
        Self {
            context: context.id(),
            height: context.height(),
            owner: address.owner,
            ordinal: address.ordinal,
            slot: address.slot,
            digest,
        }
    }
    /// Return the immutable actor-global lifecycle ordinal.
    pub(in crate::sumeragi) const fn lifecycle_ordinal(self) -> u128 {
        self.ordinal
    }
    /// Build a deterministic exact queue key for worker ownership tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(ordinal: u128, discriminator: u8) -> Self {
        let context = LifecycleDigest::new([discriminator; 32]);
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context,
            height: 1,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
        }
    }
    /// Build an exact queue key bound to one real fixture height context.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_height_context_test(
        context: &wire::HeightContext,
        ordinal: u128,
        discriminator: u8,
    ) -> Self {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context: LifecycleDigest::new(context_id),
            height: context.height,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
        }
    }
    /// Recheck the exact wire height context owning this queue position.
    pub(in crate::sumeragi) fn matches_height_context(self, context: &wire::HeightContext) -> bool {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        self.context == LifecycleDigest::new(context_id) && self.height == context.height
    }
    /// Recheck the immutable context and digest retained by a closed carrier.
    pub(in crate::sumeragi) fn matches_carrier(
        self,
        context: LifecycleContext,
        digest: LifecycleDigest,
    ) -> bool {
        self.context == context.id() && self.height == context.height() && self.digest == digest
    }
    /// Return whether this key still names the exact installed carrier.
    fn matches(
        self,
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> bool {
        self.context == context.id()
            && self.height == context.height()
            && self.owner == address.owner
            && self.ordinal == address.ordinal
            && self.slot == address.slot
            && self.digest == digest
    }
}
/// Move-only authority for one exact recovered Decision Apply worker dispatch.
///
/// Only the concrete registry can mint this identity after joining a claimed
/// lease to its unchanged closed carrier. The worker may project only its
/// copyable queue key; no effect, pending binding, receipt, or candidate parts
/// are exposed.
#[must_use = "a recovered Decision Apply dispatch must enter the dedicated worker"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyDispatchIdentityV1 {
    key: RecoveredDecisionApplyDispatchKeyV1,
    _linearity: RecoveredDecisionApplyDispatchLinearity,
}
struct RecoveredDecisionApplyDispatchLinearity;
impl Drop for RecoveredDecisionApplyDispatchLinearity {
    fn drop(&mut self) {}
}
impl RecoveredDecisionApplyDispatchIdentityV1 {
    fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> Self {
        Self {
            key: RecoveredDecisionApplyDispatchKeyV1::new(context, address, digest),
            _linearity: RecoveredDecisionApplyDispatchLinearity,
        }
    }
    /// Return the closed copyable worker-queue key.
    pub(in crate::sumeragi) const fn key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        self.key
    }
    /// Recheck the immutable context and digest retained by the closed carrier.
    pub(in crate::sumeragi) fn matches_carrier(
        &self,
        context: LifecycleContext,
        digest: LifecycleDigest,
    ) -> bool {
        self.key.matches_carrier(context, digest)
    }
    /// Recheck the exact wire height context selected by the storage worker.
    pub(in crate::sumeragi) fn matches_height_context(
        &self,
        context: &wire::HeightContext,
    ) -> bool {
        self.key.matches_height_context(context)
    }
    /// Recheck this identity against one exact installed registry location.
    fn matches(
        &self,
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> bool {
        self.key.matches(context, address, digest)
    }
}
/// Closed semantic class of one lifecycle-owned recovered signing command.
///
/// The class is part of the dedicated queue key even though the installed
/// address and effect digest are already distinct. This prevents a corrupted
/// registry or test mutation from aliasing phase-vote, proposal, and timeout
/// signing work under one physical queue owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::sumeragi) enum RecoveredLifecycleSignClassV1 {
    /// Recovered Validate continuation which must sign its exact phase vote.
    PhaseVote,
    /// Standalone recovered leader proposal signing work.
    ControlProposal,
    /// Standalone recovered timeout-vote signing work.
    ControlTimeout,
}
impl RecoveredLifecycleSignClassV1 {
    const fn matches_request(self, request: &SignRequest) -> bool {
        matches!(
            (self, request),
            (Self::PhaseVote, SignRequest::Vote(_))
                | (Self::ControlProposal, SignRequest::Proposal(_))
                | (Self::ControlTimeout, SignRequest::TimeoutVote(_))
        )
    }
}
/// Copyable queue key for one exact lifecycle-owned recovered Sign dispatch.
///
/// This process-local identity is deliberately independent from
/// [`EffectWorkId`]. It binds the immutable height, logical owner, physical
/// slot, exact installed effect digest, and semantic Sign class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::sumeragi) struct RecoveredLifecycleSignDispatchKeyV1 {
    context: LifecycleDigest,
    height: u64,
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    digest: LifecycleDigest,
    class: RecoveredLifecycleSignClassV1,
}
impl RecoveredLifecycleSignDispatchKeyV1 {
    const fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        class: RecoveredLifecycleSignClassV1,
    ) -> Self {
        Self {
            context: context.id(),
            height: context.height(),
            owner: address.owner,
            ordinal: address.ordinal,
            slot: address.slot,
            digest,
            class,
        }
    }
    /// Return the immutable actor-global lifecycle ordinal.
    pub(in crate::sumeragi) const fn lifecycle_ordinal(self) -> u128 {
        self.ordinal
    }
    /// Recheck the exact wire height context owning this queue position.
    pub(in crate::sumeragi) fn matches_height_context(self, context: &wire::HeightContext) -> bool {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        self.context == LifecycleDigest::new(context_id) && self.height == context.height
    }
    fn matches(
        self,
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        class: RecoveredLifecycleSignClassV1,
    ) -> bool {
        self.context == context.id()
            && self.height == context.height()
            && self.owner == address.owner
            && self.ordinal == address.ordinal
            && self.slot == address.slot
            && self.digest == digest
            && self.class == class
    }
    /// Build an exact class-sensitive queue key for worker ownership tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(
        ordinal: u128,
        discriminator: u8,
        class: RecoveredLifecycleSignClassV1,
    ) -> Self {
        let context = LifecycleDigest::new([discriminator; 32]);
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context,
            height: 1,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
            class,
        }
    }

    /// Build an exact class-sensitive queue key for one real fixture context.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_height_context_test(
        context: &wire::HeightContext,
        ordinal: u128,
        discriminator: u8,
        class: RecoveredLifecycleSignClassV1,
    ) -> Self {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context: LifecycleDigest::new(context_id),
            height: context.height,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
            class,
        }
    }
}
/// Move-only registry proof for one exact recovered Sign worker dispatch.
///
/// Only the registry can mint this value after joining the current claimed
/// lease to its unchanged sealed carrier. The worker receives the identity
/// only as part of a fixed task projection; no adapter effect, pending binding,
/// runtime owner, or generic work identifier is exposed.
#[must_use = "a recovered Sign dispatch must enter the dedicated worker"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignDispatchIdentityV1 {
    key: RecoveredLifecycleSignDispatchKeyV1,
    _linearity: RecoveredLifecycleSignDispatchLinearity,
}
/// Copyable process-local identity for one recovered Decision Fetch request.
///
/// This key is deliberately independent from [`EffectWorkId`]. It joins the
/// immutable height, logical owner, physical slot, and exact installed Fetch
/// digest retained by the closed WAL carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::sumeragi) struct RecoveredDecisionFetchDispatchKeyV1 {
    context: LifecycleDigest,
    height: u64,
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    digest: LifecycleDigest,
}
impl RecoveredDecisionFetchDispatchKeyV1 {
    const fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> Self {
        Self {
            context: context.id(),
            height: context.height(),
            owner: address.owner,
            ordinal: address.ordinal,
            slot: address.slot,
            digest,
        }
    }
    /// Return the immutable actor-global lifecycle ordinal.
    pub(in crate::sumeragi) const fn lifecycle_ordinal(self) -> u128 {
        self.ordinal
    }
    /// Build a deterministic exact queue key for worker ownership tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn for_test(ordinal: u128, discriminator: u8) -> Self {
        let context = LifecycleDigest::new([discriminator; 32]);
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context,
            height: 1,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
        }
    }
    /// Build a deterministic queue key bound to one real fixture height context.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_height_context_test(
        context: &wire::HeightContext,
        ordinal: u128,
        discriminator: u8,
    ) -> Self {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        let causal_root =
            super::CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
        Self {
            context: LifecycleDigest::new(context_id),
            height: context.height,
            owner: OwnerId::new(causal_root, ordinal),
            ordinal,
            slot: PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
            digest: LifecycleDigest::new([discriminator.wrapping_add(2); 32]),
        }
    }
    /// Recheck the exact immutable height context owning this request.
    pub(in crate::sumeragi) fn matches_height_context(self, context: &wire::HeightContext) -> bool {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        self.context == LifecycleDigest::new(context_id) && self.height == context.height
    }
    fn matches(
        self,
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> bool {
        self.context == context.id()
            && self.height == context.height()
            && self.owner == address.owner
            && self.ordinal == address.ordinal
            && self.slot == address.slot
            && self.digest == digest
    }
}
/// Move-only registry proof for one exact recovered Decision Fetch request.
///
/// Only an exact installed carrier can mint this identity. The carrier-derived
/// request authority consumes it directly; no generic runtime owner, pending
/// binding, adapter effect, or work identifier crosses this boundary.
#[must_use = "a recovered Decision Fetch identity must remain in its sealed request authority"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchDispatchIdentityV1 {
    key: RecoveredDecisionFetchDispatchKeyV1,
    _linearity: RecoveredDecisionFetchDispatchLinearityV1,
}
struct RecoveredDecisionFetchDispatchLinearityV1;
impl Drop for RecoveredDecisionFetchDispatchLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredDecisionFetchDispatchIdentityV1 {
    fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> Self {
        Self {
            key: RecoveredDecisionFetchDispatchKeyV1::new(context, address, digest),
            _linearity: RecoveredDecisionFetchDispatchLinearityV1,
        }
    }
    /// Return the closed copyable request/response owner key.
    pub(in crate::sumeragi) const fn key(&self) -> RecoveredDecisionFetchDispatchKeyV1 {
        self.key
    }
    /// Recheck carrier-derived request coordinates against the installed digest.
    pub(in crate::sumeragi) fn authorizes_request(
        &self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        sources: &[iroha_data_model::peer::PeerId],
        certificate: &wire::QuorumCertificate,
    ) -> bool {
        if self.key.height != tag.height()
            || self.key.height != round.height
            || certificate.phase != wire::GlobalPhase::Commit
            || certificate.proposal_round != round
            || certificate.subject != subject
            || sources.is_empty()
        {
            return false;
        }
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(round.context_id.0.as_ref());
        self.key.context == LifecycleDigest::new(context_id)
            && crate::sumeragi::v2_runtime::adapter_effect_matches_lifecycle_digest(
                &AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    manifest: None,
                    certified_sources: sources.to_vec(),
                    certificate: Some(certificate.clone()),
                },
                self.key.digest.as_bytes(),
            )
    }
}
struct RecoveredLifecycleSignDispatchLinearity;
impl Drop for RecoveredLifecycleSignDispatchLinearity {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleSignDispatchIdentityV1 {
    fn new(
        context: LifecycleContext,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        class: RecoveredLifecycleSignClassV1,
    ) -> Self {
        Self {
            key: RecoveredLifecycleSignDispatchKeyV1::new(context, address, digest, class),
            _linearity: RecoveredLifecycleSignDispatchLinearity,
        }
    }
    /// Return the closed copyable worker-queue key.
    pub(in crate::sumeragi) const fn key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.key
    }
    /// Recheck a carrier-derived tag/request before sealing the worker task.
    pub(in crate::sumeragi) fn authorizes_request(
        &self,
        tag: EventTag,
        request: &SignRequest,
    ) -> bool {
        let round = match request {
            SignRequest::Proposal(proposal) => proposal.round,
            SignRequest::Vote(vote) => vote.round,
            SignRequest::TimeoutVote(vote) => vote.round,
        };
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(round.context_id.0.as_ref());
        self.key.height == tag.height()
            && self.key.height == round.height
            && self.key.context == LifecycleDigest::new(context_id)
            && self.key.class.matches_request(request)
            && crate::sumeragi::v2_runtime::adapter_effect_matches_lifecycle_digest(
                &AdapterEffect::Sign {
                    tag,
                    request: request.clone(),
                },
                self.key.digest.as_bytes(),
            )
    }
    /// Mint one exact identity through production effect hashing for worker tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        ordinal: u128,
        tag: EventTag,
        request: &SignRequest,
        class: RecoveredLifecycleSignClassV1,
    ) -> Option<Self> {
        let round = match request {
            SignRequest::Proposal(proposal) => proposal.round,
            SignRequest::Vote(vote) => vote.round,
            SignRequest::TimeoutVote(vote) => vote.round,
        };
        if ordinal == 0 || round.height != tag.height() || !class.matches_request(request) {
            return None;
        }
        let effect = AdapterEffect::Sign {
            tag,
            request: request.clone(),
        };
        let digest = LifecycleDigest::new(
            *crate::sumeragi::v2_runtime::adapter_effect_identity_for_test(&effect).as_ref(),
        );
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(round.context_id.0.as_ref());
        let context = LifecycleContext::new(LifecycleDigest::new(context_id), round.height);
        let address = ConcreteWorkAddress::new(
            OwnerId::new(super::CausalRoot::new(digest), ordinal),
            ordinal,
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
        )?;
        Some(Self::new(context, address, digest, class))
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
    dispatch_key: Option<RecoveredLifecycleSignDispatchKeyV1>,
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
    dispatch_key: Option<RecoveredLifecycleSignDispatchKeyV1>,
}
/// Closed concrete carrier for one standalone WAL-owned follow-on Vote Sign.
///
/// This row is admitted atomically beside the signed Broadcast which caused
/// it, but it retains its own WAL causal owner and validated-body authority.
/// No effect, request, pending binding, replay envelope, or body receipt is
/// exposed through the registry.
#[cfg_attr(not(test), allow(dead_code))]
struct DurableRecoveredLifecycleNextWalVoteSignWork {
    projection: RecoveredLifecycleNextWalVoteCandidateProjectionV1,
    verified: VerifiedHeightContext,
    address: ConcreteWorkAddress,
    dispatch_key: Option<RecoveredLifecycleSignDispatchKeyV1>,
}
/// Exact recovered Sign parent retained beneath its durable Broadcast child.
///
/// Keeping the complete parent carrier closes both live replacement and cold
/// restart over the same WAL/replay authority. No effect, pending binding, or
/// signature bytes can be extracted through this discriminator.
enum DurableRecoveredLifecycleSignParentV1 {
    PhaseVote(DurableRecoveredWalSignWork),
    NextWalVote(DurableRecoveredLifecycleNextWalVoteSignWork),
    Control(DurableRecoveredWalControlSignWork),
}
impl DurableRecoveredLifecycleSignParentV1 {
    fn dispatch_key(&self) -> Option<RecoveredLifecycleSignDispatchKeyV1> {
        match self {
            Self::PhaseVote(parent) => parent.dispatch_key,
            Self::NextWalVote(parent) => parent.dispatch_key,
            Self::Control(parent) => parent.dispatch_key,
        }
    }
    fn validates_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        match self {
            Self::PhaseVote(parent) => parent.repair.matches_signed_broadcast(verified, broadcast),
            Self::NextWalVote(parent) => {
                broadcast.validates_from_next_wal_vote(verified, &parent.projection)
            }
            Self::Control(parent) => parent.carrier.matches_signed_broadcast(verified, broadcast),
        }
    }
    fn causal_root(&self) -> super::CausalRoot {
        match self {
            Self::PhaseVote(parent) => parent.repair.repair().child().causal_root,
            Self::NextWalVote(parent) => parent.address.owner.causal_root(),
            Self::Control(parent) => parent.address.owner.causal_root(),
        }
    }
}
/// Closed concrete carrier for an fsynced recovered Sign-to-Broadcast edge.
///
/// The signed child remains inseparable from its original recovered WAL Sign
/// and the verified roster which authenticated the signature. Generic runtime
/// effect ownership cannot observe or execute this row.
struct DurableRecoveredLifecycleSignedBroadcastWork {
    parent: DurableRecoveredLifecycleSignParentV1,
    broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    verified: VerifiedHeightContext,
    address: ConcreteWorkAddress,
    paired_next_sign: Option<(ConcreteWorkAddress, LifecycleDigest)>,
}
impl fmt::Debug for DurableRecoveredLifecycleSignedBroadcastWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredLifecycleSignedBroadcastWork")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}
impl DurableRecoveredLifecycleSignedBroadcastWork {
    fn pairs_exact_next_sign(
        &self,
        next_address: ConcreteWorkAddress,
        next_digest: LifecycleDigest,
    ) -> bool {
        self.paired_next_sign == Some((next_address, next_digest))
    }

    fn is_unpaired(&self) -> bool {
        self.paired_next_sign.is_none()
    }

    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address
            && self.address.owner.causal_root() == self.parent.causal_root()
            && self.paired_next_sign.is_none_or(|(next, _)| {
                self.address.ordinal.checked_add(1) == Some(next.ordinal)
                    && self.address.owner != next.owner
                    && next.slot
                        == PhysicalSlotId::for_capacity(
                            LifecycleWorkClass::SignVote.capacity_class(),
                            0,
                        )
            })
            && self
                .parent
                .validates_broadcast(&self.verified, &self.broadcast)
            && self
                .broadcast
                .validates_at(&self.verified, address, installed_digest)
    }
    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.broadcast.matches_current_ready_record(
                coordinator.active_context,
                address,
                installed_digest,
                coordinator,
            )
    }
    fn matches_current_finalization_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.broadcast.matches_current_finalization_record(
                coordinator.active_context,
                address,
                installed_digest,
                coordinator,
            )
    }
    fn project_claimed_output_authority(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Option<RecoveredLifecycleSignedBroadcastOutputAuthorityV1> {
        (lease.output_reservation().is_none()
            && self.validates_at(address, installed_digest)
            && self.broadcast.matches_current_claimed_record(
                coordinator.active_context,
                address,
                installed_digest,
                coordinator,
                lease,
            ))
        .then(|| self.broadcast.project_output_authority(&self.verified))
        .flatten()
    }
    fn validates_control_in_store(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        let DurableRecoveredLifecycleSignParentV1::Control(parent) = &self.parent else {
            return false;
        };
        parent.carrier.validates_signed_broadcast_in_store(
            &self.verified,
            &self.broadcast,
            store,
            self.address.ordinal,
        )
    }
    fn validates_phase_in_store(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        let DurableRecoveredLifecycleSignParentV1::PhaseVote(parent) = &self.parent else {
            return false;
        };
        let Ok(ledger) = store.load() else {
            return false;
        };
        parent.repair.belongs_to_loaded(store, &ledger)
            && ledger
                .authenticate_recovered_phase_signed_broadcast(&self.verified, &parent.repair)
                .is_ok_and(
                    |(broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal)| {
                        parent_ordinal == parent.validation.address.ordinal
                            && sign_ordinal == parent.repair.child_ordinal()
                            && broadcast_ordinal == self.address.ordinal
                            && broadcast.exactly_matches(&self.broadcast)
                    },
                )
    }

    /// Rejoin the retained historical Sign parent to this exact live Broadcast row.
    fn validates_in_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        match &self.parent {
            DurableRecoveredLifecycleSignParentV1::PhaseVote(parent) => ledger
                .authenticate_recovered_phase_signed_broadcast(&self.verified, &parent.repair)
                .is_ok_and(
                    |(broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal)| {
                        parent_ordinal == parent.validation.address.ordinal
                            && sign_ordinal == parent.repair.child_ordinal()
                            && broadcast_ordinal == self.address.ordinal
                            && broadcast.exactly_matches(&self.broadcast)
                    },
                ),
            DurableRecoveredLifecycleSignParentV1::Control(parent) => {
                parent.carrier.validates_signed_broadcast_in_ledger(
                    &self.verified,
                    &self.broadcast,
                    ledger,
                    self.address.ordinal,
                )
            }
            DurableRecoveredLifecycleSignParentV1::NextWalVote(parent) => {
                let Some(parent_record) = ledger
                    .records()
                    .iter()
                    .find(|record| record.ordinal() == parent.address.ordinal)
                else {
                    return false;
                };
                let Some(broadcast_record) = ledger
                    .records()
                    .iter()
                    .find(|record| record.ordinal() == self.address.ordinal)
                else {
                    return false;
                };
                parent.projection.exactly_matches_advanced_broadcast_parent(
                    ledger.context(),
                    parent_record,
                    self.address.ordinal,
                ) && self
                    .broadcast
                    .exactly_matches_record(broadcast_record, parent.address.owner)
                    && ledger
                        .records()
                        .iter()
                        .filter(|record| record.owner() == parent.address.owner)
                        .count()
                        == 2
            }
        }
    }

    fn owns_control_recovery(&self, recovery: &AuthenticatedLifecycleRecoveryCut) -> bool {
        let DurableRecoveredLifecycleSignParentV1::Control(parent) = &self.parent else {
            return false;
        };
        parent
            .carrier
            .owns_signed_broadcast_recovery(recovery, &self.broadcast)
    }
    fn owns_phase_recovery(&self, recovery: &AuthenticatedLifecycleRecoveryCut) -> bool {
        let DurableRecoveredLifecycleSignParentV1::PhaseVote(parent) = &self.parent else {
            return false;
        };
        recovery.owns_recovered_phase_broadcast(
            &AuthenticatedRecoveredWalSignProjection {
                parent: parent.repair.repair().parent().clone(),
                child: parent.repair.repair().child().clone(),
                parent_address: parent.validation.address,
                child_address: ConcreteWorkAddress::new(
                    self.address.owner,
                    parent.repair.child_ordinal(),
                    PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
                )
                .expect("durable recovered Sign ordinal retains a nonzero exact address"),
            },
            &self.broadcast,
        )
    }
}
impl fmt::Debug for DurableRecoveredWalControlSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalControlSignWork")
            .field("dispatched", &self.dispatch_key.is_some())
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
impl fmt::Debug for DurableRecoveredLifecycleNextWalVoteSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredLifecycleNextWalVoteSignWork")
            .field("address", &self.address)
            .field("dispatched", &self.dispatch_key.is_some())
            .finish_non_exhaustive()
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl DurableRecoveredLifecycleNextWalVoteSignWork {
    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address
            && self
                .projection
                .validates_at(&self.verified, address, installed_digest)
    }

    fn validates_in_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == self.address.ordinal)
            .is_some_and(|record| {
                self.projection
                    .exactly_matches_fresh_record(ledger.context(), record)
                    && ledger
                        .records()
                        .iter()
                        .filter(|candidate| candidate.owner() == self.address.owner)
                        .count()
                        == 1
            })
    }

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.projection.matches_current_ready_record(
                &self.verified,
                address,
                installed_digest,
                coordinator,
            )
    }
    fn matches_current_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.projection.matches_current_claimed_record(
                &self.verified,
                address,
                installed_digest,
                coordinator,
                lease,
            )
    }
    fn project_task(
        &self,
        identity: RecoveredLifecycleSignDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1> {
        self.projection
            .project_recovered_lifecycle_sign_task(&self.verified, identity)
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
    dispatch_key: Option<RecoveredDecisionFetchDispatchKeyV1>,
}
/// Dedicated Store carrier which permanently retains its recovered WAL Fetch lineage.
///
/// This is not ordinary pending adapter work or an ordinary certified Store.
/// The original payload-free Fetch carrier and body-backed Store projection
/// remain inseparable across live execution and cold restart.
struct DurableRecoveredDecisionStoreWork {
    fetch: DurableRecoveredWalDecisionFetchCarrierV1,
    store: RecoveredDecisionFetchStoreProjectionV1,
    context: LifecycleContext,
    address: ConcreteWorkAddress,
}
impl fmt::Debug for DurableRecoveredDecisionStoreWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredDecisionStoreWork")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}
impl DurableRecoveredDecisionStoreWork {
    fn validates_at(&self, address: ConcreteWorkAddress, digest: LifecycleDigest) -> bool {
        self.address == address
            && self.fetch.is_exact()
            && self.address.owner.causal_root() == self.fetch.causal_root()
            && self.store.validates_at(self.context, address, digest)
    }
    fn validates_in_store(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        ledger: &super::ledger::LifecycleLedgerStoreV1,
    ) -> bool {
        self.validates_at(address, digest)
            && self
                .fetch
                .validates_recovered_store_in_store(&self.store, ledger)
    }
    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.validates_at(address, digest)
            && self
                .store
                .matches_current_ready_record(self.context, address, digest, coordinator)
    }
    fn owns_recovery(&self, recovery: &AuthenticatedLifecycleRecoveryCut) -> bool {
        self.fetch.owns_store_recovery(&self.store, recovery)
    }
}
/// Closed concrete carrier for the sole live Apply in a recovered Decision body chain.
///
/// The carrier retains the original WAL Fetch, all three body successors, and
/// the final pending binding. It has no generic adapter-effect extraction path.
struct DurableRecoveredDecisionApplyWork {
    carrier: RecoveredDecisionApplyRegistryCarrierV1,
    address: ConcreteWorkAddress,
    dispatch_key: Option<RecoveredDecisionApplyDispatchKeyV1>,
}
impl fmt::Debug for DurableRecoveredDecisionApplyWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredDecisionApplyWork")
            .field("address", &self.address)
            .field("dispatched", &self.dispatch_key.is_some())
            .finish_non_exhaustive()
    }
}
impl DurableRecoveredDecisionApplyWork {
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        self.carrier.installed_digest() == installed_digest
            && self.carrier.lineage().is_exact(self.carrier.context())
    }
    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address
            && self.address.slot
                == PhysicalSlotId::for_capacity(LifecycleWorkClass::Apply.capacity_class(), 0)
            && self.validates_digest(installed_digest)
    }
    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
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
            super::PhysicalGeometry::new([PhysicalSlot::new(slot, digest)], [slot]),
            None,
        );
        self.validates_at(address, installed_digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == self.carrier.context()
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Apply
            && record.state == super::LifecycleState::Ready
            && slot == address.slot
            && digest == installed_digest
            && metadata.matches_admission(&candidate)
            && self.carrier.exactly_matches_candidate(&candidate)
            && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && coordinator.ready_index.contains(&record.ordinal)
    }
    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
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
            super::PhysicalGeometry::new([PhysicalSlot::new(slot, digest)], [slot]),
            None,
        );
        self.validates_at(address, installed_digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == self.carrier.context()
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.key == lease.key()
            && record.owner == lease.owner()
            && record.work_class == LifecycleWorkClass::Apply
            && record.work_class == lease.work_class()
            && record.stage == lease.stage()
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.ordinal() == address.ordinal
            && lease.physical_slots() == &record.physical_slots
            && slot == address.slot
            && digest == installed_digest
            && metadata.matches_admission(&candidate)
            && self.carrier.exactly_matches_candidate(&candidate)
            && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && !coordinator.ready_index.contains(&record.ordinal)
    }
}
/// Closed service demand authenticated for one Ready recovered Decision Apply.
///
/// The classifier has exactly one first-release outcome: execution must enter
/// the bounded height-local I/O worker before the coordinator may claim the
/// Apply. Keeping this as a typed outcome prevents callers from supplying an
/// unbound boolean capacity hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredDecisionApplyDemand {
    /// Reserve one bounded I/O command position before claiming the Apply.
    BoundedIo,
}
/// Opaque proof that one Ready row is the exact recovered Decision Apply carrier.
///
/// Construction is private to the concrete registry classifier. The retained
/// carrier, body receipt, effect, pending binding, address, and digest never
/// leave the registry; the scheduler can inspect only the typed service demand
/// and an opaque key for reserving the exact worker position.
#[must_use = "a Ready recovered Decision Apply attestation must enter scheduler classification"]
pub(super) struct ReadyRecoveredDecisionApplyAttestation {
    demand: ReadyRecoveredDecisionApplyDemand,
    dispatch_key: RecoveredDecisionApplyDispatchKeyV1,
    _seal: ReadyRecoveredDecisionApplyAttestationSeal,
}
struct ReadyRecoveredDecisionApplyAttestationSeal;
impl Drop for ReadyRecoveredDecisionApplyAttestationSeal {
    fn drop(&mut self) {}
}
impl ReadyRecoveredDecisionApplyAttestation {
    /// Return the sole typed service demand without exposing carrier parts.
    pub(super) const fn demand(&self) -> ReadyRecoveredDecisionApplyDemand {
        self.demand
    }
    /// Return the queue key derived from the exact Ready carrier location.
    pub(super) const fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        self.dispatch_key
    }
    /// Recheck that this attestation still belongs to the exact Ready row.
    pub(super) fn matches_ready_record(&self, record: &super::LifecycleRecord) -> bool {
        record.state == super::LifecycleState::Ready
            && record.work_class == LifecycleWorkClass::Apply
            && record.key.phase() == LifecyclePhase::Apply
            && record.stage.kind() == LifecycleStageKind::ApplyDecision
            && record.stage.predecessor_scope() == PredecessorScope::Independent
            && record.physical_slots.len() == 1
            && record
                .physical_slots
                .first_key_value()
                .and_then(|(&slot, &digest)| {
                    ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                        .map(|address| (address, digest))
                })
                .is_some_and(|(address, digest)| {
                    self.dispatch_key.context == record.key.context()
                        && self.dispatch_key.height == record.key.round().height()
                        && self.dispatch_key.owner == address.owner
                        && self.dispatch_key.ordinal == address.ordinal
                        && self.dispatch_key.slot == address.slot
                        && self.dispatch_key.digest == digest
                })
    }
}
/// Closed service demand authenticated for one Ready recovered Sign carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredLifecycleSignDemandV1 {
    /// Reserve one dedicated bounded Consensus command before claiming it.
    BoundedIo,
}
/// Opaque proof that one Ready row is an exact recovered Sign carrier.
///
/// Only its class-sensitive queue key and typed I/O demand are observable.
/// Exact `AdapterEffect::Sign`, pending ownership, tag, and request remain
/// sealed until the registry projects the claimed carrier directly to worker
/// task ownership.
#[must_use = "a Ready recovered Sign attestation must enter scheduler classification"]
pub(super) struct ReadyRecoveredLifecycleSignAttestationV1 {
    demand: ReadyRecoveredLifecycleSignDemandV1,
    dispatch_key: RecoveredLifecycleSignDispatchKeyV1,
    _seal: ReadyRecoveredLifecycleSignAttestationSealV1,
}
struct ReadyRecoveredLifecycleSignAttestationSealV1;
impl Drop for ReadyRecoveredLifecycleSignAttestationSealV1 {
    fn drop(&mut self) {}
}
impl ReadyRecoveredLifecycleSignAttestationV1 {
    /// Return the sole typed service demand without exposing carrier parts.
    pub(super) const fn demand(&self) -> ReadyRecoveredLifecycleSignDemandV1 {
        self.demand
    }
    /// Return the class-sensitive dedicated queue key.
    pub(super) const fn dispatch_key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.dispatch_key
    }
    /// Recheck this attestation against the exact unchanged Ready row.
    pub(super) fn matches_ready_record(&self, record: &super::LifecycleRecord) -> bool {
        let expected_class = match record.work_class {
            LifecycleWorkClass::SignVote => RecoveredLifecycleSignClassV1::PhaseVote,
            LifecycleWorkClass::SignProposal => RecoveredLifecycleSignClassV1::ControlProposal,
            LifecycleWorkClass::SignTimeout => RecoveredLifecycleSignClassV1::ControlTimeout,
            _ => return false,
        };
        record.state == super::LifecycleState::Ready
            && record.physical_slots.len() == 1
            && record
                .physical_slots
                .first_key_value()
                .and_then(|(&slot, &digest)| {
                    ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                        .map(|address| (address, digest))
                })
                .is_some_and(|(address, digest)| {
                    self.dispatch_key.matches(
                        LifecycleContext::new(record.key.context(), record.key.round().height()),
                        address,
                        digest,
                        expected_class,
                    )
                })
    }
    /// Mint the same row-bound capacity attestation for focused scheduler tests.
    #[cfg(test)]
    pub(super) fn for_test(record: &super::LifecycleRecord) -> Option<Self> {
        let class = match record.work_class {
            LifecycleWorkClass::SignVote => RecoveredLifecycleSignClassV1::PhaseVote,
            LifecycleWorkClass::SignProposal => RecoveredLifecycleSignClassV1::ControlProposal,
            LifecycleWorkClass::SignTimeout => RecoveredLifecycleSignClassV1::ControlTimeout,
            _ => return None,
        };
        let (&slot, &digest) = record.physical_slots.first_key_value()?;
        let address = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)?;
        let attestation = Self {
            demand: ReadyRecoveredLifecycleSignDemandV1::BoundedIo,
            dispatch_key: RecoveredLifecycleSignDispatchKeyV1::new(
                LifecycleContext::new(record.key.context(), record.key.round().height()),
                address,
                digest,
                class,
            ),
            _seal: ReadyRecoveredLifecycleSignAttestationSealV1,
        };
        attestation
            .matches_ready_record(record)
            .then_some(attestation)
    }
}
/// Closed failure while attesting one Ready recovered Sign carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredLifecycleSignAttestationErrorV1 {
    /// The logical row, durable metadata, or reverse index is not exact and Ready.
    InvalidCoordinatorIndex,
    /// The process-local address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The exact address contains another concrete carrier class.
    WrongWorkKind,
    /// The recovered Sign carrier no longer matches its immutable logical row.
    InvalidCarrier,
}
/// Closed failure while projecting one claimed recovered Sign dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredLifecycleSignDispatchProjectionErrorV1 {
    /// The active lease does not name one exact claimed Sign row and slot.
    InvalidLease,
    /// The process-local address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The exact address contains another concrete carrier class.
    WrongWorkKind,
    /// The closed carrier no longer matches the claimed row and exact effect.
    InvalidCarrier,
    /// The exact carrier already owns a queued, active, or pending completion.
    AlreadyDispatched,
}
enum PreparedRecoveredLifecycleSignCarrier<'registry> {
    PhaseVote(&'registry mut DurableRecoveredWalSignWork),
    NextWalVote(&'registry mut DurableRecoveredLifecycleNextWalVoteSignWork),
    Control(&'registry mut DurableRecoveredWalControlSignWork),
}
/// Borrow-bound one-shot projection of a claimed recovered Sign.
///
/// Dropping this before publication leaves the registry carrier unarmed. The
/// dedicated capacity reservation consumes it while holding the queue cut and
/// performs the only infallible carrier-key/FIFO commit.
#[must_use = "a prepared recovered Sign dispatch must enter its reserved queue"]
pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignDispatch<'registry> {
    carrier: PreparedRecoveredLifecycleSignCarrier<'registry>,
    task: Option<crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1>,
    key: RecoveredLifecycleSignDispatchKeyV1,
}
impl PreparedRecoveredLifecycleSignDispatch<'_> {
    /// Return the immutable queue key without releasing Sign material.
    pub(in crate::sumeragi) const fn dispatch_key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.key
    }
    /// Arm the exact carrier and release its task under the reserved queue cut.
    pub(in crate::sumeragi) fn commit_for_worker(
        mut self,
    ) -> crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1 {
        let dispatch_key = match &mut self.carrier {
            PreparedRecoveredLifecycleSignCarrier::PhaseVote(work) => &mut work.dispatch_key,
            PreparedRecoveredLifecycleSignCarrier::NextWalVote(work) => &mut work.dispatch_key,
            PreparedRecoveredLifecycleSignCarrier::Control(work) => &mut work.dispatch_key,
        };
        assert!(
            dispatch_key.is_none(),
            "prepared recovered Sign remains the sole dispatch owner"
        );
        *dispatch_key = Some(self.key);
        self.task
            .take()
            .expect("prepared recovered Sign retains its exact worker task")
    }
}
/// Closed service demand authenticated for one Ready recovered Decision Fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredDecisionFetchDemandV1 {
    /// Reserve one exact-output fanout and one vacant executor owner before claim.
    ExactOutputAndExecutor,
}
/// Opaque proof and move-only request authority for one Ready recovered Decision Fetch.
///
/// The request authority retains the exact tag, CommitQC, round, subject, and
/// frozen archive sequence. It exposes only its dedicated dispatch key until
/// consumed by the fixed signing/authentication path in the executor module.
#[must_use = "a Ready recovered Decision Fetch must enter exact request dispatch"]
pub(super) struct ReadyRecoveredDecisionFetchAttestationV1 {
    demand: ReadyRecoveredDecisionFetchDemandV1,
    dispatch_key: RecoveredDecisionFetchDispatchKeyV1,
    request: Option<crate::sumeragi::v2_worker::RecoveredDecisionFetchRequestAuthorityV1>,
    _seal: ReadyRecoveredDecisionFetchAttestationSealV1,
}
struct ReadyRecoveredDecisionFetchAttestationSealV1;
impl Drop for ReadyRecoveredDecisionFetchAttestationSealV1 {
    fn drop(&mut self) {}
}
impl ReadyRecoveredDecisionFetchAttestationV1 {
    /// Return the fixed service demand without exposing request material.
    pub(super) const fn demand(&self) -> ReadyRecoveredDecisionFetchDemandV1 {
        self.demand
    }
    /// Return the exact dedicated request/response owner key.
    pub(super) const fn dispatch_key(&self) -> RecoveredDecisionFetchDispatchKeyV1 {
        self.dispatch_key
    }
    /// Recheck this proof against the unchanged Ready Fetch row.
    pub(super) fn matches_ready_record(&self, record: &super::LifecycleRecord) -> bool {
        record.state == super::LifecycleState::Ready
            && record.work_class == LifecycleWorkClass::Fetch
            && record.key.phase() == LifecyclePhase::Fetch
            && record.stage.kind() == LifecycleStageKind::FetchBody
            && record.stage.predecessor_scope() == PredecessorScope::Independent
            && record.physical_slots.len() == 1
            && record
                .physical_slots
                .first_key_value()
                .and_then(|(&slot, &digest)| {
                    ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                        .map(|address| (address, digest))
                })
                .is_some_and(|(address, digest)| {
                    self.dispatch_key.matches(
                        LifecycleContext::new(record.key.context(), record.key.round().height()),
                        address,
                        digest,
                    )
                })
    }
    /// Consume the sole carrier-derived request authority.
    pub(super) fn take_request_authority(
        &mut self,
    ) -> crate::sumeragi::v2_worker::RecoveredDecisionFetchRequestAuthorityV1 {
        self.request
            .take()
            .expect("Ready recovered Decision Fetch retains one request authority")
    }
}
/// Closed failure while attesting one Ready recovered Decision Fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredDecisionFetchAttestationErrorV1 {
    /// The logical row, durable metadata, or reverse index is not exact and Ready.
    InvalidCoordinatorIndex,
    /// The process-local address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The exact address contains another closed carrier class.
    WrongWorkKind,
    /// The closed carrier or its exact request authority is inconsistent.
    InvalidCarrier,
}
/// Failure while joining a claimed recovered Decision Fetch back to its carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredDecisionFetchDispatchProjectionErrorV1 {
    /// The sole active lease is not the exact recovered Fetch row.
    InvalidLease,
    /// The installed registry address or digest is absent or corrupt.
    Registry(RegistryError),
    /// Another closed carrier occupies the exact address.
    WrongWorkKind,
    /// The carrier no longer matches the claimed row.
    InvalidCarrier,
    /// The carrier already owns a live request/response lifecycle.
    AlreadyDispatched,
}
/// Borrow-bound one-shot arming token for a claimed recovered Decision Fetch.
///
/// Dropping this token leaves the carrier unarmed. The exclusive executor
/// registration reservation consumes it only after every request/output
/// preflight has succeeded.
#[must_use = "a prepared recovered Decision Fetch must enter its executor owner"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchDispatchV1<'registry> {
    work: &'registry mut DurableRecoveredWalDecisionFetchWork,
    key: RecoveredDecisionFetchDispatchKeyV1,
}
impl PreparedRecoveredDecisionFetchDispatchV1<'_> {
    /// Return the exact dispatch key without releasing registry ownership.
    pub(in crate::sumeragi) const fn dispatch_key(&self) -> RecoveredDecisionFetchDispatchKeyV1 {
        self.key
    }
    /// Arm the exact carrier after the executor and output reservations preflight.
    pub(in crate::sumeragi) fn commit_for_executor(self) -> RecoveredDecisionFetchDispatchKeyV1 {
        assert!(
            self.work.dispatch_key.is_none(),
            "prepared recovered Decision Fetch remains the sole dispatch owner"
        );
        self.work.dispatch_key = Some(self.key);
        self.key
    }
}
/// Closed failure while attesting one Ready recovered Decision Apply carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredDecisionApplyAttestationError {
    /// The logical row, durable metadata, or reverse index is not exact and Ready.
    InvalidCoordinatorIndex,
    /// The process-local address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The exact address contains another closed carrier class.
    WrongWorkKind,
    /// The recovered Apply carrier no longer matches its immutable logical row.
    InvalidCarrier,
}
/// Closed failure while projecting one claimed recovered Decision Apply dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredDecisionApplyDispatchProjectionError {
    /// The active lease does not name one exact claimed Apply row and slot.
    InvalidLease,
    /// The process-local address or installed digest is absent or corrupt.
    Registry(RegistryError),
    /// The exact address contains another concrete carrier class.
    WrongWorkKind,
    /// The closed recovered Apply carrier no longer matches the claimed row.
    InvalidCarrier,
    /// The exact carrier already owns a queued, active, or completion-pending dispatch.
    AlreadyDispatched,
}
/// Borrow-bound one-shot projection of a claimed recovered Decision Apply.
///
/// Dropping this value before queue publication leaves the closed registry
/// carrier unchanged. The dedicated worker reservation consumes it while
/// holding the queue cut; only that infallible commit arms the carrier's
/// in-flight key and releases the exact worker task.
#[must_use = "a prepared recovered Decision Apply dispatch must enter its reserved queue"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionApplyDispatch<'registry> {
    work: &'registry mut DurableRecoveredDecisionApplyWork,
    task: Option<crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1>,
    key: RecoveredDecisionApplyDispatchKeyV1,
}
/// Exact claimed Apply carrier and completion authority before LedgerV1.
///
/// This token retains no mutable registry borrow. The current carrier is
/// revalidated again by the publication method immediately before fsync.
#[must_use = "recovered Apply completion has not reached its durable terminal"]
pub(super) struct PreparedRecoveredDecisionApplyTerminalTransitionV1 {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    dispatch_key: RecoveredDecisionApplyDispatchKeyV1,
    _linearity: RecoveredDecisionApplyTerminalTransitionLinearity,
}
struct RecoveredDecisionApplyTerminalTransitionLinearity;
impl Drop for RecoveredDecisionApplyTerminalTransitionLinearity {
    fn drop(&mut self) {}
}
/// Failure from the recovered Apply carrier-before-Ledger publication cut.
pub(super) enum RecoveredDecisionApplyTerminalPublicationError<E> {
    /// Current or staged coordinator/registry state failed exact preflight.
    Preflight(PreparedRecoveredDecisionApplyTerminalTransitionV1),
    /// LedgerV1 publication failed while the incumbent carrier stayed installed.
    Publication(E, PreparedRecoveredDecisionApplyTerminalTransitionV1),
}
impl PreparedRecoveredDecisionApplyDispatch<'_> {
    /// Return the immutable queue key without releasing the worker task.
    pub(in crate::sumeragi) const fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        self.key
    }
    /// Arm the exact carrier and release its task under the reserved queue cut.
    pub(in crate::sumeragi) fn commit_for_worker(
        mut self,
    ) -> crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1 {
        assert!(
            self.work.dispatch_key.is_none(),
            "prepared recovered Decision Apply remains the sole dispatch owner"
        );
        self.work.dispatch_key = Some(self.key);
        self.task
            .take()
            .expect("prepared recovered Decision Apply retains its worker task")
    }
}
impl fmt::Debug for DurableRecoveredWalDecisionFetchWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalDecisionFetchWork")
            .field("dispatched", &self.dispatch_key.is_some())
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
    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        self.validates_at(address, installed_digest)
            && self.carrier.matches_claimed_record(coordinator, lease)
    }
}
impl fmt::Debug for DurableRecoveredWalSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredWalSignWork")
            .field("child_ordinal", &self.repair.child_ordinal())
            .field("parent_address", &self.validation.address)
            .field("dispatched", &self.dispatch_key.is_some())
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
    /// Rejoin the live Sign child to its exact terminal Validate parent.
    ///
    /// Startup authenticated this pair against LedgerV1, but dispatch must
    /// repeat the join against the current coordinator so a missing or
    /// mutated parent cannot reserve worker capacity. The retained validation
    /// completion supplies the original parent address and body provenance;
    /// no effect or continuation parts escape this carrier.
    fn matches_current_terminal_parent(&self, coordinator: &LifecycleCoordinator) -> bool {
        let repair = self.repair.repair();
        let parent = repair.parent();
        let parent_address = self.validation.address;
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&parent_address.ordinal),
            coordinator.durable_records.get(&parent_address.ordinal),
        ) else {
            return false;
        };
        detached_recovered_validation_is_exact(repair, &self.validation)
            && coordinator.active_context.id() == parent.key.context()
            && coordinator.active_context.height() == parent.key.round().height()
            && record.key == parent.key
            && record.owner == parent_address.owner
            && record.owner.causal_root() == parent.causal_root
            && record.ordinal == parent_address.ordinal
            && record.work_class == LifecycleWorkClass::Validate
            && record.stage == parent.stage
            && record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
            && record.physical_slots.is_empty()
            && metadata.matches_admission(parent)
            && metadata.continuation
                == super::schema::DurableContinuation::successor(
                    repair.edge(),
                    self.repair.child_ordinal(),
                )
            && coordinator.key_index.get(&parent.key) == Some(&parent_address.ordinal)
            && coordinator.owner_index.get(&parent.causal_root) == Some(&parent_address.owner)
            && !coordinator.ready_index.contains(&parent_address.ordinal)
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
            && self.matches_current_terminal_parent(coordinator)
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
            && metadata.continuation == super::schema::DurableContinuation::None
            && coordinator.key_index.get(&candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&candidate.causal_root) == Some(&record.owner)
            && coordinator.ready_index.contains(&address.ordinal)
    }
    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        let candidate = self.repair.repair().child();
        let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
            return false;
        };
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at(address, installed_digest)
            && self.matches_current_terminal_parent(coordinator)
            && coordinator.fault.is_none()
            && coordinator.active_context.id() == candidate.key.context()
            && coordinator.active_context.height() == candidate.key.round().height()
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.key == candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::SignVote
            && record.stage == candidate.stage
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.key() == record.key
            && lease.owner() == record.owner
            && lease.ordinal() == record.ordinal
            && lease.work_class() == record.work_class
            && lease.stage() == record.stage
            && lease.physical_slots() == &record.physical_slots
            && record.physical_slots == physical
            && record.episode.slot_universe == universe
            && record.episode.consumed_slots == consumed
            && physical.get(&address.slot) == Some(&installed_digest)
            && metadata.matches_admission(candidate)
            && metadata.continuation == super::schema::DurableContinuation::None
            && coordinator.key_index.get(&candidate.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&candidate.causal_root) == Some(&record.owner)
            && !coordinator.ready_index.contains(&address.ordinal)
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
        replay_authority: LifecycleReplayAuthorityV1,
    },
    CertifiedFetchCompletion(CertifiedFetchCompletion),
    DurableStoreBody(DurableStoreBody),
    DurableValidateBody(DurableValidateBody),
    DurableValidateCompletion(DurableValidateCompletion),
    DurableRecoveredWalSign(DurableRecoveredWalSignWork),
    DurableRecoveredLifecycleNextWalVoteSign(DurableRecoveredLifecycleNextWalVoteSignWork),
    DurableRecoveredWalControlSign(DurableRecoveredWalControlSignWork),
    DurableRecoveredLifecycleSignedBroadcast(DurableRecoveredLifecycleSignedBroadcastWork),
    DurableRecoveredWalDecisionFetch(DurableRecoveredWalDecisionFetchWork),
    DurableRecoveredDecisionStore(DurableRecoveredDecisionStoreWork),
    DurableRecoveredDecisionApply(DurableRecoveredDecisionApplyWork),
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
        let Some(replay_authority) = exact_direct_signed_admission_authority(&effect, &pending)
        else {
            return Err((RegistryError::CorruptWork, effect, pending));
        };
        Self::from_authorized_exact(effect, pending, replay_authority)
    }

    /// Seal one exact effect, pending binding, and already-authenticated replay envelope.
    fn from_authorized_exact(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        replay_authority: LifecycleReplayAuthorityV1,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        if !pending.exactly_binds_adapter_effect(&effect) {
            return Err((RegistryError::UnboundEffect, effect, pending));
        }
        if exact_direct_signed_admission_authority(&effect, &pending)
            .is_some_and(|direct| direct != replay_authority)
        {
            return Err((RegistryError::CorruptWork, effect, pending));
        }
        let digest = digest_from_hash(pending.exact_effect_identity());
        Ok(Self {
            digest,
            kind: ConcreteLifecycleWorkKind::PendingAdapter {
                effect,
                pending,
                replay_authority,
            },
        })
    }

    /// Construct an inert unsupported-effect carrier for registry-only tests.
    #[cfg(test)]
    pub(super) fn from_inert_fixture_for_test(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        let replay_authority = super::replay_authority::exact_record_fixture(
            LifecycleContext::new(LifecycleDigest::new([0xE1; 32]), 1),
            LifecycleStageKind::StoreBody,
            0xE1,
        )
        .authority;
        Self::from_authorized_exact(effect, pending, replay_authority)
    }

    /// Revalidate the sealed binding and its derived physical digest.
    pub(super) fn validate_exact(&self) -> bool {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect,
                pending,
                replay_authority,
            } => {
                pending.exactly_binds_adapter_effect(effect)
                    && self.digest == digest_from_hash(pending.exact_effect_identity())
                    && exact_direct_signed_admission_authority(effect, pending)
                        .is_none_or(|direct| &direct == replay_authority)
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
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                sign.validates_at(sign.address, self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.validates_digest(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                broadcast.validates_at(broadcast.address, self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.validates_digest(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) => {
                store.validates_at(store.address, self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                apply.validates_digest(self.digest)
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
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                    sign.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                    sign.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                    broadcast.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                    fetch.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) => {
                    store.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                    apply.validates_at(address, self.digest)
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
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                sign.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                broadcast.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) => {
                store.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                apply.address.owner.causal_root()
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
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect, pending, ..
        } = self.kind
        else {
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
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_)
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
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect, pending, ..
            } => Some((effect, pending)),
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_) => None,
            ConcreteLifecycleWorkKind::DurableStoreBody(_) => None,
            ConcreteLifecycleWorkKind::DurableValidateBody(_) => None,
            ConcreteLifecycleWorkKind::DurableValidateCompletion(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_) => None,
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
        verified: &VerifiedHeightContext,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
    ) -> bool {
        self.entries.len() == 2
            && self.preflights_registry(registry)
            && registry.exactly_covers_all_live_work(verified, current)
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
        replay_authority: LifecycleReplayAuthorityV1,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        ConcreteLifecycleWork::from_authorized_exact(effect, pending, replay_authority)
            .map(|work| Self { work })
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
include!("v2_lifecycle_work_registry_recovered_wal.rs");
include!("v2_lifecycle_work_registry_validate_recovery.rs");
include!("v2_lifecycle_work_registry_validate_execution.rs");
#[cfg(test)]
mod tests {
    include!("tests/v2_lifecycle_work_registry_00.rs");
    include!("tests/v2_lifecycle_work_registry_01.rs");
    include!("tests/v2_lifecycle_work_registry_02.rs");
    include!("tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs");
    include!("tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs");
    include!("tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs");
    include!("tests/v2_lifecycle_work_registry_exact_registry_cases.rs");
    include!("tests/v2_lifecycle_work_registry_recovery_surface_cases.rs");
    include!("tests/v2_lifecycle_work_registry_replay_evidence_cases.rs");
}
