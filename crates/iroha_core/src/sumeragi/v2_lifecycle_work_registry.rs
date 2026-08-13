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
use thiserror::Error;

#[cfg(test)]
use super::{AdmissionRequest, CausalRoot, schema::DurableBodyFrameReference};
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
        RecoveredLifecycleNextWalVoteCandidateProjectionV1, RemoteProposalFetchReplayEvidenceV1,
        RemoteProposalStoreReplayEvidenceV1, RemoteProposalStoredReplayEvidenceV1,
        RemoteProposalValidateReplayEvidenceV1, SealedLiveWalPersistedEffectV1,
        SignedBroadcastReplayEvidenceV1, SignedEquivocationReplayEvidenceV1,
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

#[cfg(test)]
use crate::sumeragi::v2_runtime::bind_adapter_effect_batch_ownership;

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

    const fn matches(
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

    const fn matches(
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
            ConcreteLifecycleWorkKind::PendingAdapter { effect, pending } => {
                Some((effect, pending))
            }
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
    authority: PersistedRecoveredWalLifecycleAuthority<'registry>,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum PersistedRecoveredWalLifecycleAuthority<'registry> {
    Sign(DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>),
    SignedBroadcast(DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>),
    SignedBroadcastAndNextVote {
        repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    },
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
        verified: &VerifiedHeightContext,
        repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ) -> Result<
        PersistedRecoveredWalValidateLedger<'registry>,
        ExactStoreRecoveredWalPersistError<'registry>,
    > {
        let Self { store, opened } = self;
        if opened
            .recovered_phase_signed_broadcast_ordinals(&repair.repair)
            .is_some()
        {
            return match repair
                .authenticate_signed_broadcast_in_opened_ledger(verified, &store, &opened)
            {
                Ok(authority) => Ok(PersistedRecoveredWalValidateLedger {
                    store,
                    repaired: opened,
                    authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(authority),
                }),
                Err(error) => Err(ExactStoreRecoveredWalPersistError {
                    _ledger: Self { store, opened },
                    error,
                }),
            };
        }
        match repair.persist_in_opened_ledger(&store, &opened) {
            Ok((repaired, repair, _changed)) => Ok(PersistedRecoveredWalValidateLedger {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::Sign(repair),
            }),
            Err(error) => Err(ExactStoreRecoveredWalPersistError {
                _ledger: Self { store, opened },
                error,
            }),
        }
    }
}

impl<'registry> PersistedRecoveredWalValidateLedger<'registry> {
    /// Advance the cold adapter through either the exact single Broadcast or
    /// its adjacent WAL-backed Commit-Sign pair.
    ///
    /// Pair recognition is frame-bound and transaction-local; unrelated later
    /// rows do not change it. The body-store join and adapter replay happen
    /// before the authority variant changes, and the exact store is reloaded
    /// once more before this method releases the prepared startup.
    pub(in crate::sumeragi) fn prepare_cold_adapter_startup(
        self,
        verified: &VerifiedHeightContext,
        startup: crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
        body_store: &V2BodyStore,
    ) -> Result<
        (
            crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1,
            Self,
        ),
        &'static str,
    > {
        let Self {
            store,
            repaired,
            authority,
        } = self;
        let repair = match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                return Ok((
                    startup,
                    Self {
                        store,
                        repaired,
                        authority: PersistedRecoveredWalLifecycleAuthority::Sign(repair),
                    },
                ));
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => repair,
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote { .. } => {
                return Err("recovered phase cold adapter pair was prepared twice");
            }
        };
        let (observed_broadcast, validate_ordinal, sign_ordinal, broadcast_ordinal) = repaired
            .authenticate_recovered_phase_signed_broadcast(verified, &repair.repair)
            .map_err(|_| "recovered phase Broadcast changed before cold adapter preparation")?;
        if !observed_broadcast.exactly_matches(&repair.broadcast) {
            return Err("recovered phase Broadcast projection changed after ledger authentication");
        }
        let mut matching = repaired
            .recovered_lifecycle_signed_broadcast_and_sign_pairs()
            .map_err(|_| "recovered phase Broadcast-and-Sign pair classification failed")?
            .into_iter()
            .filter(|pair| {
                pair.parent()
                    == super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                        validate_ordinal,
                    }
                    && pair.parent_ordinal() == sign_ordinal
                    && pair.broadcast_ordinal() == broadcast_ordinal
            });
        let pair_hint = matching.next();
        if matching.next().is_some() {
            return Err("recovered phase Broadcast matched multiple durable successor pairs");
        }
        let Some(pair_hint) = pair_hint else {
            let adapter_authority = repair
                .repair
                .project_cold_adapter_authority(verified, &repair.broadcast)
                .ok_or("recovered phase Broadcast cannot replay the exact cold adapter")?;
            let startup = startup
                .advance_recovered_lifecycle_signed_broadcast(verified, adapter_authority)?;
            return Ok((
                startup,
                Self {
                    store,
                    repaired,
                    authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair),
                },
            ));
        };

        let mut preview = repair.repair.prepare_cold_signed_broadcast_and_sign(
            verified,
            startup,
            &repair.broadcast,
        )?;
        let body = body_store
            .authenticate_recovered_lifecycle_next_vote_body(&mut preview)
            .map_err(|_| "recovered phase next Vote lost its exact body-store authority")?;
        let seal = preview
            .seal_recovered_lifecycle_next_wal_vote(body)
            .map_err(|_| "recovered phase next Vote lost its WAL/body seal")?;
        let (startup, mut combined) = repair
            .repair
            .project_authenticated_cold_signed_broadcast_and_sign(verified, seal)
            .ok_or("recovered phase cold pair changed its WAL/body authority")?;
        let pair = repaired
            .authenticate_recovered_phase_signed_broadcast_and_sign(
                verified,
                &repair.repair,
                &combined,
            )
            .map_err(|_| "recovered phase cold pair changed its exact durable rows")?;
        if pair != pair_hint {
            return Err("recovered phase cold pair changed after executable projection");
        }
        let adapter_authority = combined
            .project_cold_adapter_replay_authority(verified)
            .ok_or("recovered phase cold pair cannot advance the exact adapter")?;
        let startup = startup
            .advance_recovered_lifecycle_signed_broadcast_and_sign(verified, adapter_authority)?;
        if !store.revalidates_recovered_phase_signed_broadcast_and_sign(
            verified,
            &repair.repair,
            &combined,
            &pair,
        ) {
            return Err("recovered phase cold pair changed after adapter advance");
        }
        Ok((
            startup,
            Self {
                store,
                repaired,
                authority: PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote {
                    repair,
                    combined,
                    pair,
                },
            },
        ))
    }

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
            authority,
        } = self;
        let installed = match authority {
            PersistedRecoveredWalLifecycleAuthority::Sign(repair) => {
                repair.install_recovered_sign(&store)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcast(repair) => {
                repair.install_recovered_broadcast(&store)
            }
            PersistedRecoveredWalLifecycleAuthority::SignedBroadcastAndNextVote {
                repair,
                combined,
                pair,
            } => repair.install_recovered_broadcast_and_next_vote(&store, combined, pair),
        };
        match installed {
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
        let recovery = if let Some((broadcast, next_sign, pair)) =
            installed.phase_broadcast_and_next_vote_projection()
        {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                pair,
                broadcast,
                next_sign,
                fetches,
            )
        } else if let Some(broadcast) = installed.phase_broadcast_projection() {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                broadcast,
                fetches,
            )
        } else {
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_sign_and_durable_fetch_startup(
                repaired,
                serve_payloads,
                body_store,
                &projection,
                fetches,
            )
        };
        let (recovery, fetches) = recovery.map_err(|error| {
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

/// Frame-bound recovered Validate→Sign→Broadcast authority.
///
/// The durable vote repair, body-revalidated Validate completion, exact child
/// projection, and exclusive registry reservation remain inseparable. Cold
/// recovery installs only the Broadcast address while retaining the complete
/// Validate/Sign authority underneath it.
#[must_use = "a durable recovered signed Broadcast still needs registry installation"]
struct DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry> {
    repair: DurableAuthenticatedWalVoteLifecycleRepair,
    validation: DetachedRecoveredValidateCompletion,
    reservation: RecoveredWalValidateRegistryReservation<'registry>,
    broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    verified: VerifiedHeightContext,
    sign_address: ConcreteWorkAddress,
    broadcast_address: ConcreteWorkAddress,
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
    next_sign: Option<(ConcreteWorkAddress, LifecycleDigest)>,
    pair: Option<super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1>,
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
    next_sign: Option<(ConcreteWorkAddress, LifecycleDigest)>,
    pair: Option<super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1>,
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

/// Exclusive installed view of one recovered Decision Apply.
///
/// The closed carrier retains the original WAL Fetch and every body-backed
/// successor. This cut exposes no address, candidate, effect, pending binding,
/// or registry parts; it can only finish the exact prospective coordinator
/// publication assembled from the same four-row ledger lineage.
#[must_use = "the installed recovered Decision Apply must complete startup"]
pub(super) struct InstalledRecoveredDecisionApplyRegistryCut<'registry> {
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
    BroadcastProjection {
        _projection: AuthenticatedRecoveredWalControlProjection,
        _broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    },
    BroadcastCarrier {
        _parent: DurableRecoveredWalControlSignCarrierV1,
        _broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
    },
    BroadcastAndSignProjection {
        _projection: AuthenticatedRecoveredWalControlProjection,
        _combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
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
            RecoveredWalControlSignInstallFailure::BroadcastProjection { .. } => {
                "recovered control Broadcast failed exact registry preflight"
            }
            RecoveredWalControlSignInstallFailure::BroadcastCarrier { .. } => {
                "recovered control Broadcast carrier disagrees with durable storage"
            }
            RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection { .. } => {
                "recovered control Broadcast-and-Sign failed exact registry preflight"
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
    StoreProjection {
        _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        _store: RecoveredDecisionFetchStoreProjectionV1,
    },
    StoreCarrier {
        _fetch: DurableRecoveredWalDecisionFetchCarrierV1,
        _store: RecoveredDecisionFetchStoreProjectionV1,
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
            RecoveredWalDecisionFetchInstallFailure::StoreProjection { .. } => {
                "recovered Decision Store failed exact registry preflight"
            }
            RecoveredWalDecisionFetchInstallFailure::StoreCarrier { .. } => {
                "recovered Decision Store carrier disagrees with durable storage"
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

/// Fail-stop diagnostic for a rejected recovered Decision Apply install.
#[must_use = "failed recovered Decision Apply installation requires restart"]
pub(super) struct RecoveredDecisionApplyInstallError {
    reason: &'static str,
    _authority: RecoveredDecisionApplyInstallAuthority,
}

#[allow(variant_size_differences)]
enum RecoveredDecisionApplyInstallAuthority {
    Projection {
        _projection: RecoveredDecisionApplyStagedStorageV1,
        _effects: Vec<AdapterEffect>,
    },
    Carrier {
        _adapter: ProductionLifecycleAdapterStartupV1,
        _carrier: RecoveredDecisionApplyRegistryCarrierV1,
    },
}

impl RecoveredDecisionApplyInstallError {
    fn projection(
        reason: &'static str,
        projection: RecoveredDecisionApplyStagedStorageV1,
        effects: Vec<AdapterEffect>,
    ) -> Self {
        Self {
            reason,
            _authority: RecoveredDecisionApplyInstallAuthority::Projection {
                _projection: projection,
                _effects: effects,
            },
        }
    }

    fn carrier(
        reason: &'static str,
        adapter: ProductionLifecycleAdapterStartupV1,
        carrier: RecoveredDecisionApplyRegistryCarrierV1,
    ) -> Self {
        Self {
            reason,
            _authority: RecoveredDecisionApplyInstallAuthority::Carrier {
                _adapter: adapter,
                _carrier: carrier,
            },
        }
    }

    /// Return the stable non-authorizing failure classification.
    pub(super) const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Fail-stop diagnostic from the installed Decision-Apply coordinator join.
#[must_use = "failed recovered Decision Apply lifecycle open requires restart"]
pub(super) struct RecoveredDecisionApplyLifecycleOpenError {
    reason: &'static str,
}

impl RecoveredDecisionApplyLifecycleOpenError {
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
    SignedBroadcast {
        _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
    },
    SignedBroadcastAndNextVote {
        _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        _combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
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
            RecoveredWalSignInstallFailure::SignedBroadcast { .. } => {
                "fsynced recovered phase Broadcast failed exact registry preflight"
            }
            RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote { .. } => {
                "fsynced recovered phase Broadcast-and-Sign pair failed exact registry preflight"
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

    /// Bind the already-fsynced three-row vote lineage to its exact store.
    #[allow(clippy::result_large_err)]
    fn authenticate_signed_broadcast_in_opened_ledger(
        self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        opened: &super::ledger::LifecycleLedgerV1,
    ) -> Result<
        DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry>,
        RecoveredWalValidateLedgerPersistError<'registry>,
    > {
        if !self.concrete_pair_and_validation_are_exact()
            || !self.ledger_parent_core_identity_is_exact(opened)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        let (broadcast, parent_ordinal, sign_ordinal, broadcast_ordinal) = match opened
            .authenticate_recovered_phase_signed_broadcast_repair(verified, &self.repair)
        {
            Ok(projection) => projection,
            Err(error) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                        _error: error,
                        _authority: self,
                    },
                });
            }
        };
        let (physical, universe, consumed) =
            match self.repair.child().physical_geometry.normalized() {
                Ok(geometry) => geometry,
                Err(error) => {
                    return Err(RecoveredWalValidateLedgerPersistError {
                        failure: RecoveredWalValidateLedgerPersistFailure::Stage {
                            _error: super::ledger::LifecycleLedgerError::InvalidLedger(format!(
                                "recovered signed Broadcast Sign geometry is invalid: {error}"
                            )),
                            _authority: self,
                        },
                    });
                }
            };
        let Some((&sign_slot, &sign_digest)) = physical.first_key_value() else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        };
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let (Some(sign_address), Some(broadcast_address)) = (
            ConcreteWorkAddress::new(self.validation.address.owner, sign_ordinal, sign_slot),
            ConcreteWorkAddress::new(
                self.validation.address.owner,
                broadcast_ordinal,
                broadcast_slot,
            ),
        ) else {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        };
        if parent_ordinal != self.validation.address.ordinal
            || sign_slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            || physical.len() != 1
            || universe.len() != 1
            || consumed != universe
            || physical.get(&sign_slot) != Some(&sign_digest)
            || !broadcast.validates_at(verified, broadcast_address, broadcast.digest())
            || self
                .reservation
                .registry
                .entries
                .keys()
                .any(|address| address.owner == self.validation.address.owner)
        {
            return Err(RecoveredWalValidateLedgerPersistError {
                failure: RecoveredWalValidateLedgerPersistFailure::InvalidAuthority {
                    _authority: self,
                },
            });
        }
        let Self {
            repair,
            validation,
            reservation,
        } = self;
        let repair = match store.authenticate_wal_vote_repair_for_signed_broadcast(opened, repair) {
            Ok(repair) => repair,
            Err((error, repair)) => {
                return Err(RecoveredWalValidateLedgerPersistError {
                    failure: RecoveredWalValidateLedgerPersistFailure::Stage {
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
        assert_eq!(repair.child_ordinal(), sign_ordinal);
        Ok(
            DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                repair,
                validation,
                reservation,
                broadcast,
                verified: verified.clone(),
                sign_address,
                broadcast_address,
            },
        )
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
                dispatch_key: None,
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
            next_sign: None,
            pair: None,
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

impl<'registry> DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair<'registry> {
    /// Install the exact live Broadcast while retaining its complete phase-vote parent.
    #[allow(clippy::result_large_err)]
    fn install_recovered_broadcast(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let Self {
            repair,
            validation,
            reservation,
            broadcast,
            verified,
            sign_address,
            broadcast_address,
        } = self;
        let Ok(ledger) = store.load() else {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::SignedBroadcast {
                    _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                        repair,
                        validation,
                        reservation,
                        broadcast,
                        verified,
                        sign_address,
                        broadcast_address,
                    },
                },
            });
        };
        let exact = detached_recovered_validation_is_exact(repair.repair(), &validation)
            && repair.belongs_to_loaded(store, &ledger)
            && ledger
                .authenticate_recovered_phase_signed_broadcast(&verified, &repair)
                .is_ok_and(|(recovered, parent, sign, child)| {
                    parent == validation.address.ordinal
                        && sign == sign_address.ordinal
                        && child == broadcast_address.ordinal
                        && recovered.exactly_matches(&broadcast)
                })
            && reservation.parent_address == validation.address
            && reservation
                .registry
                .entries
                .keys()
                .all(|address| address.owner != broadcast_address.owner);
        if !exact {
            return Err(RecoveredWalSignInstallError {
                failure: RecoveredWalSignInstallFailure::SignedBroadcast {
                    _repair: DurableAuthenticatedRecoveredWalSignedBroadcastLifecycleRepair {
                        repair,
                        validation,
                        reservation,
                        broadcast,
                        verified,
                        sign_address,
                        broadcast_address,
                    },
                },
            });
        }
        let digest = broadcast.digest();
        let parent = DurableRecoveredWalSignWork {
            repair,
            validation,
            dispatch_key: None,
        };
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::PhaseVote(parent),
                    broadcast,
                    verified,
                    address: broadcast_address,
                    paired_next_sign: None,
                },
            ),
        };
        assert!(work.validates_at(broadcast_address));
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        let previous = registry.entries.insert(broadcast_address, work);
        assert!(previous.is_none());
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address: broadcast_address,
            child_digest: digest,
            next_sign: None,
            pair: None,
        })
    }

    /// Install one exact phase Prepare-Broadcast plus its independent Commit Sign.
    ///
    /// The loaded LedgerV1 frame, detached Validate completion, WAL repair,
    /// combined executable projection, and both fresh child rows are joined
    /// before either process-local carrier is inserted. The Broadcast retains
    /// the historical phase parent and an explicit link to the independently
    /// owned next Sign, which remains undispatched.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    fn install_recovered_broadcast_and_next_vote(
        self,
        store: &super::ledger::LifecycleLedgerStoreV1,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> Result<
        InstalledRecoveredWalSignRegistryCut<'registry>,
        RecoveredWalSignInstallError<'registry>,
    > {
        let fail = |repair, combined| RecoveredWalSignInstallError {
            failure: RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote {
                _repair: repair,
                _combined: combined,
            },
        };
        let Ok(ledger) = store.load() else {
            return Err(fail(self, combined));
        };
        let record_at = |ordinal| {
            ledger
                .records()
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index))
        };
        let Some(broadcast_record) = record_at(pair.broadcast_ordinal()) else {
            return Err(fail(self, combined));
        };
        let Some(next_sign_record) = record_at(pair.next_sign_ordinal()) else {
            return Err(fail(self, combined));
        };
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let next_sign_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(broadcast_address) = ConcreteWorkAddress::new(
            broadcast_record.owner(),
            broadcast_record.ordinal(),
            broadcast_slot,
        ) else {
            return Err(fail(self, combined));
        };
        let Some(next_sign_address) = ConcreteWorkAddress::new(
            next_sign_record.owner(),
            next_sign_record.ordinal(),
            next_sign_slot,
        ) else {
            return Err(fail(self, combined));
        };
        let exact = pair.parent()
            == super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                validate_ordinal: self.validation.address.ordinal,
            }
            && pair.parent_ordinal() == self.sign_address.ordinal
            && pair.broadcast_ordinal() == self.broadcast_address.ordinal
            && detached_recovered_validation_is_exact(self.repair.repair(), &self.validation)
            && self.repair.belongs_to_loaded(store, &ledger)
            && ledger
                .authenticate_recovered_phase_signed_broadcast_and_sign(
                    &self.verified,
                    &self.repair,
                    &combined,
                )
                .is_ok_and(|observed| observed == pair)
            && self.reservation.parent_address == self.validation.address
            && self.reservation.registry.entries.is_empty()
            && combined.broadcast_exactly_matches(&self.broadcast)
            && combined.exactly_matches_fresh_records(
                ledger.context(),
                broadcast_record,
                next_sign_record,
            )
            && broadcast_address == self.broadcast_address
            && broadcast_address.owner == self.sign_address.owner
            && next_sign_address.owner != self.sign_address.owner;
        if !exact {
            return Err(fail(self, combined));
        }

        let Self {
            repair,
            validation,
            reservation,
            broadcast: _,
            verified,
            sign_address: _,
            broadcast_address,
        } = self;
        let (broadcast, next_sign) = combined.into_registry_children(
            RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new(),
        );
        let broadcast_digest = broadcast.digest();
        let next_sign_digest = next_sign.digest();
        let parent = DurableRecoveredWalSignWork {
            repair,
            validation,
            dispatch_key: None,
        };
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::PhaseVote(parent),
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((next_sign_address, next_sign_digest)),
                },
            ),
        };
        let next_sign_work = ConcreteLifecycleWork {
            digest: next_sign_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: next_sign,
                    verified,
                    address: next_sign_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(next_sign_work.validates_at(next_sign_address));
        let RecoveredWalValidateRegistryReservation {
            registry,
            parent_address,
            child: _,
        } = reservation;
        assert!(
            registry
                .entries
                .insert(broadcast_address, broadcast_work)
                .is_none()
        );
        assert!(
            registry
                .entries
                .insert(next_sign_address, next_sign_work)
                .is_none()
        );
        Ok(InstalledRecoveredWalSignRegistryCut {
            registry,
            parent_address,
            child_address: broadcast_address,
            child_digest: broadcast_digest,
            next_sign: Some((next_sign_address, next_sign_digest)),
            pair: Some(pair),
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl InstalledRecoveredWalSignRegistryCut<'_> {
    fn phase_broadcast_projection(&self) -> Option<&RecoveredLifecycleSignedBroadcastProjectionV1> {
        if self.next_sign.is_some() || self.pair.is_some() {
            return None;
        }
        let work = self.registry.entries.get(&self.child_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        matches!(
            &broadcast.parent,
            DurableRecoveredLifecycleSignParentV1::PhaseVote(_)
        )
        .then_some(&broadcast.broadcast)
    }

    fn phase_broadcast_and_next_vote_projection(
        &self,
    ) -> Option<(
        &RecoveredLifecycleSignedBroadcastProjectionV1,
        &RecoveredLifecycleNextWalVoteCandidateProjectionV1,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        let broadcast_work = self.registry.entries.get(&self.child_address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.child_digest
            && next_sign_work.digest == next_sign_digest
            && pair.broadcast_ordinal() == self.child_address.ordinal
            && pair.next_sign_ordinal() == next_sign_address.ordinal
            && matches!(
                pair.parent(),
                super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                    validate_ordinal
                } if validate_ordinal == self.parent_address.ordinal
            )
            && broadcast.paired_next_sign == Some((next_sign_address, next_sign_digest))
            && matches!(
                &broadcast.parent,
                DurableRecoveredLifecycleSignParentV1::PhaseVote(_)
            ))
        .then_some((&broadcast.broadcast, &next_sign.projection, pair))
    }

    fn installed_entry_is_exact(&self, store: &super::ledger::LifecycleLedgerStoreV1) -> bool {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self.parent_address == self.child_address
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
            RecoveredWalSignInstallFailure::SignedBroadcast { .. }
            | RecoveredWalSignInstallFailure::SignedBroadcastAndNextVote { .. } => return false,
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

    /// Prove the same repaired parent with an Advanced Sign and live Broadcast.
    pub(super) fn signed_broadcast_chain_is_exact(
        &self,
        context: LifecycleContext,
        records: &[super::ledger::LifecycleLedgerRecordV1],
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        let Some(validate_edge) = self.continuation_edge() else {
            return false;
        };
        let Some(parent) = records
            .iter()
            .find(|record| record.ordinal() == self.parent_address.ordinal)
        else {
            return false;
        };
        let Some(sign) = records
            .iter()
            .find(|record| record.ordinal() == self.child_address.ordinal)
        else {
            return false;
        };
        let expected_broadcast_edge = match (self.child.key.phase(), self.child.stage.kind()) {
            (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote) => {
                super::schema::DurableContinuationEdge::SignPrepareToBroadcast
            }
            (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote) => {
                super::schema::DurableContinuationEdge::SignCommitToBroadcast
            }
            _ => return false,
        };
        let Some((observed_edge, broadcast_ordinal)) = sign
            .continuation()
            .and_then(|continuation| continuation.successor_parts())
        else {
            return false;
        };
        let Some(broadcast_record) = records
            .iter()
            .find(|record| record.ordinal() == broadcast_ordinal)
        else {
            return false;
        };
        self.belongs_to_context(context)
            && observed_edge == expected_broadcast_edge
            && parent.key() == Some(self.parent.key)
            && parent.owner() == self.parent_address.owner
            && parent.work_class() == Some(self.parent.work_class)
            && parent.stage() == Some(self.parent.stage)
            && parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && parent.reconstruction_source() == self.parent.reconstruction_source
            && parent.durable_payload() == Some(self.parent.payload)
            && parent.replay_matches_candidate(&self.parent)
            && parent.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    validate_edge,
                    self.child_address.ordinal,
                ))
            && sign.key() == Some(self.child.key)
            && sign.owner() == self.child_address.owner
            && sign.work_class() == Some(self.child.work_class)
            && sign.stage() == Some(self.child.stage)
            && sign.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && sign.reconstruction_source() == self.child.reconstruction_source
            && sign.durable_payload() == Some(self.child.payload)
            && sign.replay_matches_candidate(&self.child)
            && broadcast.exactly_matches_record(broadcast_record, sign.owner())
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
    pub(super) serve_payloads:
        crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,
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

    fn structurally_exact_phase_broadcast(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredLifecycleSignedBroadcastWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self.parent_address == self.child_address
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
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == self.child_digest
            && work.validates_at(self.child_address)
            && broadcast.validates_phase_in_store(store))
        .then_some(broadcast)
    }

    fn structurally_exact_phase_broadcast_and_next_vote(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<(
        &DurableRecoveredLifecycleSignedBroadcastWork,
        &DurableRecoveredLifecycleNextWalVoteSignWork,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        if self.parent_address == self.child_address
            || self.child_address.owner == next_sign_address.owner
            || self.registry.entries.contains_key(&self.parent_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.child_address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == next_sign_address.owner)
                .count()
                != 1
            || !store
                .load()
                .is_ok_and(|ledger| pair.exactly_matches_ledger(&ledger))
        {
            return None;
        }
        let broadcast_work = self.registry.entries.get(&self.child_address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.child_digest
            && next_sign_work.digest == next_sign_digest
            && broadcast_work.validates_at(self.child_address)
            && next_sign_work.validates_at(next_sign_address)
            && broadcast.paired_next_sign == Some((next_sign_address, next_sign_digest))
            && broadcast.validates_phase_in_store(store)
            && matches!(
                pair.parent(),
                super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::PhasePrepare {
                    validate_ordinal
                } if validate_ordinal == self.parent_address.ordinal
            )
            && pair.broadcast_ordinal() == self.child_address.ordinal
            && pair.next_sign_ordinal() == next_sign_address.ordinal)
            .then_some((broadcast, next_sign, pair))
    }

    fn authenticated_projection(&self) -> Option<AuthenticatedRecoveredWalSignProjection> {
        let (repair, parent_address, sign_ordinal) = if let Some(sign) =
            self.structurally_exact_sign()
        {
            (
                sign.repair.repair(),
                self.parent_address,
                self.child_address.ordinal,
            )
        } else {
            let work = self.registry.entries.get(&self.child_address)?;
            let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
                &work.kind
            else {
                return None;
            };
            let DurableRecoveredLifecycleSignParentV1::PhaseVote(parent) = &broadcast.parent else {
                return None;
            };
            (
                parent.repair.repair(),
                parent.validation.address,
                parent.repair.child_ordinal(),
            )
        };
        let child_address = ConcreteWorkAddress::new(
            self.child_address.owner,
            sign_ordinal,
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0),
        )?;
        Some(AuthenticatedRecoveredWalSignProjection {
            parent: repair.parent().clone(),
            child: repair.child().clone(),
            parent_address,
            child_address,
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

    fn coordinator_broadcast_is_exact(&self, coordinator: &LifecycleCoordinator) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.structurally_exact_phase_broadcast(store)
            .is_some_and(|broadcast| {
                broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    coordinator,
                )
            })
    }

    fn coordinator_broadcast_and_next_vote_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(store) = coordinator.ledger_store.as_ref() else {
            return false;
        };
        self.structurally_exact_phase_broadcast_and_next_vote(store)
            .is_some_and(|(broadcast, next_sign, _pair)| {
                broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    coordinator,
                ) && self.next_sign.is_some_and(|(address, digest)| {
                    next_sign.matches_current_ready_record(address, digest, coordinator)
                })
            })
    }

    fn recovery_is_exact(
        &self,
        recovery: &mut AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        if let Some((broadcast, next_sign, pair)) = self.phase_broadcast_and_next_vote_projection()
        {
            recovery.owns_recovered_phase_broadcast_and_next_sign(pair, broadcast, next_sign)
        } else if let Some(broadcast) = self.phase_broadcast_projection() {
            recovery.owns_recovered_phase_broadcast(projection, broadcast)
        } else {
            recovery.splice_recovered_wal_sign(projection)
                && recovery.owns_recovered_wal_sign(projection)
        }
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
        projection: &AuthenticatedRecoveredWalSignProjection,
    ) -> bool {
        let sign_is_exact = recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(prepared.store())
            && self.coordinator_is_exact(prepared.coordinator(), projection);
        let broadcast_is_exact = self
            .structurally_exact_phase_broadcast(prepared.store())
            .is_some_and(|broadcast| {
                recovery.owns_recovered_phase_broadcast(projection, &broadcast.broadcast)
                    && broadcast.matches_current_ready_record(
                        self.child_address,
                        self.child_digest,
                        prepared.coordinator(),
                    )
            });
        let pair_is_exact = self
            .structurally_exact_phase_broadcast_and_next_vote(prepared.store())
            .is_some_and(|(broadcast, next_sign, pair)| {
                recovery.owns_recovered_phase_broadcast_and_next_sign(
                    pair,
                    &broadcast.broadcast,
                    &next_sign.projection,
                ) && broadcast.matches_current_ready_record(
                    self.child_address,
                    self.child_digest,
                    prepared.coordinator(),
                ) && self.next_sign.is_some_and(|(address, digest)| {
                    next_sign.matches_current_ready_record(address, digest, prepared.coordinator())
                })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact phase pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.child_address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.child_address)
                    } else {
                        RecoveredWalRegistrySlotV1::PhaseVote(self.child_address)
                    },
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
        let sign_is_exact = recovery.owns_recovered_wal_sign(projection)
            && self.installed_entry_is_exact(store)
            && self.coordinator_is_exact(coordinator, projection);
        let broadcast_is_exact =
            self.structurally_exact_phase_broadcast(store)
                .is_some_and(|broadcast| {
                    recovery.owns_recovered_phase_broadcast(projection, &broadcast.broadcast)
                        && self.coordinator_broadcast_is_exact(coordinator)
                });
        let pair_is_exact = self
            .structurally_exact_phase_broadcast_and_next_vote(store)
            .is_some_and(|(broadcast, next_sign, pair)| {
                recovery.owns_recovered_phase_broadcast_and_next_sign(
                    pair,
                    &broadcast.broadcast,
                    &next_sign.projection,
                ) && self.coordinator_broadcast_and_next_vote_is_exact(coordinator)
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact phase pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.child_address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.child_address)
                    } else {
                        RecoveredWalRegistrySlotV1::PhaseVote(self.child_address)
                    },
                )
    }
}

impl InstalledRecoveredWalControlSignRegistryCut<'_> {
    fn exact_control_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredWalControlSignWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self
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

    fn exact_control_broadcast_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredLifecycleSignedBroadcastWork> {
        if self.next_sign.is_some()
            || self.pair.is_some()
            || self
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
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && broadcast.validates_control_in_store(store))
        .then_some(broadcast)
    }

    fn exact_control_broadcast_and_next_vote_work(
        &self,
        store: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<(
        &DurableRecoveredLifecycleSignedBroadcastWork,
        &DurableRecoveredLifecycleNextWalVoteSignWork,
        &super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    )> {
        let (next_sign_address, next_sign_digest) = self.next_sign?;
        let pair = self.pair.as_ref()?;
        if pair.parent()
            != super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
            || pair.broadcast_ordinal() != self.address.ordinal
            || pair.next_sign_ordinal() != next_sign_address.ordinal
            || self.address.owner == next_sign_address.owner
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == next_sign_address.owner)
                .count()
                != 1
            || !store
                .load()
                .is_ok_and(|ledger| pair.exactly_matches_ledger(&ledger))
        {
            return None;
        }
        let broadcast_work = self.registry.entries.get(&self.address)?;
        let next_sign_work = self.registry.entries.get(&next_sign_address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &broadcast_work.kind
        else {
            return None;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_sign_work.kind
        else {
            return None;
        };
        (broadcast_work.digest == self.digest
            && next_sign_work.digest == next_sign_digest
            && broadcast_work.validates_at(self.address)
            && next_sign_work.validates_at(next_sign_address)
            && broadcast.validates_control_in_store(store))
        .then_some((broadcast, next_sign, pair))
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let sign_is_exact = self
            .exact_control_work(prepared.store())
            .is_some_and(|sign| {
                sign.carrier.owns_recovery(recovery)
                    && sign.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let broadcast_is_exact = self
            .exact_control_broadcast_work(prepared.store())
            .is_some_and(|broadcast| {
                broadcast.owns_control_recovery(recovery)
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let pair_is_exact = self
            .exact_control_broadcast_and_next_vote_work(prepared.store())
            .is_some_and(|(broadcast, next_sign, pair)| {
                broadcast.owns_control_recovery(recovery)
                    && recovery.owns_recovered_control_broadcast_and_next_sign(
                        pair,
                        &broadcast.broadcast,
                        &next_sign.projection,
                    )
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
                    && self.next_sign.is_some_and(|(address, digest)| {
                        next_sign.matches_current_ready_record(
                            address,
                            digest,
                            prepared.coordinator(),
                        )
                    })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::ControlSign(self.address)
                    },
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
        let sign_is_exact = self.exact_control_work(store).is_some_and(|sign| {
            sign.carrier.owns_recovery(recovery)
                && sign.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        let broadcast_is_exact =
            self.exact_control_broadcast_work(store)
                .is_some_and(|broadcast| {
                    broadcast.owns_control_recovery(recovery)
                        && broadcast.matches_current_ready_record(
                            self.address,
                            self.digest,
                            coordinator,
                        )
                });
        let pair_is_exact = self
            .exact_control_broadcast_and_next_vote_work(store)
            .is_some_and(|(broadcast, next_sign, pair)| {
                broadcast.owns_control_recovery(recovery)
                    && recovery.owns_recovered_control_broadcast_and_next_sign(
                        pair,
                        &broadcast.broadcast,
                        &next_sign.projection,
                    )
                    && broadcast.matches_current_ready_record(
                        self.address,
                        self.digest,
                        coordinator,
                    )
                    && self.next_sign.is_some_and(|(address, digest)| {
                        next_sign.matches_current_ready_record(address, digest, coordinator)
                    })
            });
        usize::from(sign_is_exact) + usize::from(broadcast_is_exact) + usize::from(pair_is_exact)
            == 1
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if pair_is_exact {
                        let (next_sign, _) = self
                            .next_sign
                            .expect("exact pair retains its next Sign address");
                        RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                            broadcast: self.address,
                            next_sign,
                        }
                    } else if broadcast_is_exact {
                        RecoveredWalRegistrySlotV1::SignedBroadcast(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::ControlSign(self.address)
                    },
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
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredWalControlSignLifecycleOpenError,
    > {
        let exact_carriers = usize::from(self.exact_control_work(&store).is_some())
            + usize::from(self.exact_control_broadcast_work(&store).is_some())
            + usize::from(
                self.exact_control_broadcast_and_next_vote_work(&store)
                    .is_some(),
            );
        if exact_carriers != 1 {
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
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
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
        Ok((coordinator, recovery))
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

    fn exact_decision_store_work(
        &self,
        ledger: &super::ledger::LifecycleLedgerStoreV1,
    ) -> Option<&DurableRecoveredDecisionStoreWork> {
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
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && store.validates_in_store(self.address, self.digest, ledger))
        .then_some(store)
    }

    fn prepared_join_is_exact(
        &self,
        prepared: &PreparedLifecycleCoordinatorOpen,
        recovery: &AuthenticatedLifecycleRecoveryCut,
    ) -> bool {
        let fetch_is_exact = self
            .exact_decision_fetch_work(prepared.store())
            .is_some_and(|fetch| {
                fetch.carrier.owns_recovery(recovery)
                    && fetch.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        let store_is_exact = self
            .exact_decision_store_work(prepared.store())
            .is_some_and(|store| {
                store.owns_recovery(recovery)
                    && store.matches_current_ready_record(
                        self.address,
                        self.digest,
                        prepared.coordinator(),
                    )
            });
        (fetch_is_exact ^ store_is_exact)
            && self
                .registry
                .exactly_covers_recovered_ready_fetches_with_extra(
                    prepared.coordinator(),
                    if store_is_exact {
                        RecoveredWalRegistrySlotV1::DecisionStore(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::DecisionFetch(self.address)
                    },
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
        let fetch_is_exact = self.exact_decision_fetch_work(store).is_some_and(|fetch| {
            fetch.carrier.owns_recovery(recovery)
                && fetch.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        let store_is_exact = self.exact_decision_store_work(store).is_some_and(|store| {
            store.owns_recovery(recovery)
                && store.matches_current_ready_record(self.address, self.digest, coordinator)
        });
        (fetch_is_exact ^ store_is_exact)
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    if store_is_exact {
                        RecoveredWalRegistrySlotV1::DecisionStore(self.address)
                    } else {
                        RecoveredWalRegistrySlotV1::DecisionFetch(self.address)
                    },
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
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredWalDecisionFetchLifecycleOpenError,
    > {
        if self.exact_decision_fetch_work(&store).is_none()
            && self.exact_decision_store_work(&store).is_none()
        {
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
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
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
        Ok((coordinator, recovery))
    }
}

impl InstalledRecoveredDecisionApplyRegistryCut<'_> {
    fn exact_apply_work(&self) -> Option<&DurableRecoveredDecisionApplyWork> {
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
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            return None;
        };
        (work.digest == self.digest
            && work.validates_at(self.address)
            && apply.validates_at(self.address, self.digest))
        .then_some(apply)
    }

    fn prepared_join_is_exact(&self, prepared: &PreparedLifecycleCoordinatorOpen) -> bool {
        self.exact_apply_work().is_some_and(|apply| {
            apply.matches_current_ready_record(self.address, self.digest, prepared.coordinator())
        }) && self
            .registry
            .exactly_covers_recovered_ready_fetches_with_extra(
                prepared.coordinator(),
                RecoveredWalRegistrySlotV1::DecisionApply(self.address),
            )
    }

    fn opened_join_is_exact(&self, coordinator: &LifecycleCoordinator) -> bool {
        coordinator.ledger_store.is_some()
            && self.exact_apply_work().is_some_and(|apply| {
                apply.matches_current_ready_record(self.address, self.digest, coordinator)
            })
            && self
                .registry
                .exactly_covers_recovered_ready_work_with_extra(
                    coordinator,
                    RecoveredWalRegistrySlotV1::DecisionApply(self.address),
                )
    }

    /// Install every unrelated durable Ready-Fetch beside the sole Apply carrier.
    pub(super) fn install_fetches(
        &mut self,
        fetches: PreparedDurableCertifiedFetchStartupV1,
    ) -> Result<(), RecoveredDecisionApplyLifecycleOpenError> {
        fetches
            .install_alongside_recovered_wal_authority(&mut *self.registry)
            .map_err(|_fetches| {
                RecoveredDecisionApplyLifecycleOpenError::new(
                    "recovered Decision Apply and Ready-Fetch carriers conflict",
                )
            })
    }
}

impl<'registry> InstalledRecoveredDecisionApplyRegistryCut<'registry> {
    /// Publish the exact prospective four-row successor and finish startup.
    ///
    /// Coordinator reconstruction, payload-store authentication, the complete
    /// prepublication registry census, and the exact predecessor/successor
    /// ledger pair are already retained by `prepared`. After its single exact
    /// successor fsync, only infallible coordinator/registry ownership moves
    /// remain.
    #[allow(clippy::result_large_err)]
    pub(super) fn open_with_prepared_successor(
        self,
        prepared: PreparedLifecycleCoordinatorOpen,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<
        (LifecycleCoordinator, AuthenticatedLifecycleRecoveryCut),
        RecoveredDecisionApplyLifecycleOpenError,
    > {
        if !self.prepared_join_is_exact(&prepared) {
            return Err(RecoveredDecisionApplyLifecycleOpenError::new(
                "prepared recovered Decision Apply registry/coordinator census changed",
            ));
        }
        let coordinator = prepared
            .commit_with_registry(&mut *self.registry, payload_store, &mut recovery)
            .map_err(|_error| {
                RecoveredDecisionApplyLifecycleOpenError::new(
                    "recovered Decision Apply exact successor publication failed",
                )
            })?;
        assert!(
            self.opened_join_is_exact(&coordinator),
            "preflighted recovered Decision Apply publication must finish with the exact opened census"
        );
        Ok((coordinator, recovery))
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
        let recovery_is_exact = self.recovery_is_exact(&mut recovery, &projection);
        if !recovery_is_exact {
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
        let coordinator = match prepared.commit_with_registry(
            &mut *self.registry,
            payload_store,
            &mut recovery,
        ) {
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
        let recovery_is_exact = self.recovery_is_exact(&mut recovery, &projection);
        if !recovery_is_exact {
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
        let coordinator = match prepared.commit_with_registry(
            &mut *self.registry,
            payload_store,
            &mut recovery,
        ) {
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
        Ok(RecoveredWalProductionOwnerOpenV1 {
            coordinator,
            verified,
            serve_payloads: recovery.into_serve_payloads(),
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

impl ConcreteLifecycleWorkRegistry {
    /// Bind one fsynced recovered body to the exact claimed WAL Fetch carrier.
    pub(super) fn prepare_recovered_decision_fetch_store_adapter_authority(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        key: RecoveredDecisionFetchDispatchKeyV1,
        body: crate::sumeragi::v2_body_store::RecoveredDecisionFetchStoreBodyAuthorityV1,
    ) -> Result<
        super::RecoveredDecisionFetchStoreAdapterAuthorityV1,
        RecoveredDecisionFetchStorePreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        if lease.physical_slots().len() != 1 {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let work = self
            .entries
            .get(&address)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        };
        if work.digest != digest
            || !work.validates_at(address)
            || fetch.dispatch_key != Some(key)
            || !key.matches(coordinator.active_context, address, digest)
            || !fetch.matches_claimed_record(address, digest, coordinator, lease)
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        fetch
            .carrier
            .project_store_adapter_authority(body)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidBody)
    }

    /// Seal the reducer-derived recovered Store child under the claimed Fetch.
    pub(super) fn prepare_recovered_decision_fetch_store_successor<'registry, 'adapter>(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredDecisionFetchDispatchKeyV1,
        adapter: crate::sumeragi::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'adapter>,
    ) -> Result<
        PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>,
        RecoveredDecisionFetchStorePreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        if lease.physical_slots().len() != 1 {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let fetch_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let fetch = self
            .entries
            .get(&fetch_address)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &fetch.kind else {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        };
        if fetch.dispatch_key != Some(key)
            || !key.matches(coordinator.active_context, fetch_address, digest)
            || !fetch.matches_claimed_record(fetch_address, digest, coordinator, lease)
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::InvalidFetchCarrier);
        }
        let store = fetch
            .carrier
            .project_store_successor(verified, adapter.body_authority(), adapter.store_effect())
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::InvalidStoreProjection)?;
        let store_ordinal = coordinator
            .high_water
            .checked_add(1)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision)?;
        let store_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let store_address = ConcreteWorkAddress::new(lease.owner(), store_ordinal, store_slot)
            .ok_or(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision)?;
        if self.entries.contains_key(&store_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
            || !store.validates_at(coordinator.active_context, store_address, store.digest())
        {
            return Err(RecoveredDecisionFetchStorePreparationErrorV1::ChildCollision);
        }
        Ok(PreparedRecoveredDecisionFetchStoreSuccessor {
            registry: self,
            fetch_address,
            store_address,
            store,
            adapter,
        })
    }
}

impl<'registry, 'adapter> PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter> {
    /// Project the exact child while retaining registry and adapter borrows.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        if self.fetch_address.owner != lease.owner()
            || self.fetch_address.ordinal != lease.ordinal()
            || self.store_address.owner != lease.owner()
        {
            return Err(SealedBodySuccessorProjectionError::ForeignParent);
        }
        self.store
            .candidate_for_transition(verified)
            .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)
    }

    /// Replace the exact WAL Fetch carrier with its dedicated Store carrier.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'adapter> {
        let Self {
            registry,
            fetch_address,
            store_address,
            store,
            adapter,
        } = self;
        let fetch = registry
            .entries
            .remove(&fetch_address)
            .expect("published recovered Store retains its exact Fetch carrier");
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = fetch.kind else {
            panic!("published recovered Store cannot replace another carrier class")
        };
        assert!(fetch.dispatch_key.is_some());
        let context = store.context();
        let digest = store.digest();
        let replacement = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(
                DurableRecoveredDecisionStoreWork {
                    fetch: fetch.carrier,
                    store,
                    context,
                    address: store_address,
                },
            ),
        };
        assert!(replacement.validates_at(store_address));
        assert!(
            registry
                .entries
                .insert(store_address, replacement)
                .is_none()
        );
        adapter
    }
}

impl ConcreteLifecycleWorkRegistry {
    /// Seal the adapter-authenticated Broadcast child under one claimed recovered Sign.
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_successor<'registry, 'adapter>(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredLifecycleSignDispatchKeyV1,
        adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter>,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastPreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        if lease.physical_slots().len() != 1
            || adapter.dispatch_key() != key
            || adapter.shape()
                != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::Broadcast
        {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier);
        }
        let sign_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        let projection_authority = adapter.project_registry_broadcast_authority();
        let sign = self
            .entries
            .get(&sign_address)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier)?;
        if sign.digest != digest || !sign.validates_at(sign_address) {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidSignCarrier);
        }
        let (projected_key, broadcast) = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(sign_address, digest, coordinator, lease) =>
            {
                sign.repair
                    .project_authenticated_signed_broadcast(verified, projection_authority)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.matches_current_claimed_record(
                        sign_address,
                        digest,
                        coordinator,
                        lease,
                    ) =>
            {
                super::wal_recovery::project_recovered_next_wal_vote_signed_broadcast(
                    &sign.projection,
                    verified,
                    projection_authority,
                )
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                if sign.dispatch_key == Some(key)
                    && sign.carrier.matches_claimed_record(coordinator, lease) =>
            {
                sign.carrier
                    .project_authenticated_signed_broadcast(verified, projection_authority)
            }
            _ => None,
        }
        .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidBroadcastProjection)?;
        if projected_key != key {
            return Err(
                RecoveredLifecycleSignBroadcastPreparationErrorV1::InvalidBroadcastProjection,
            );
        }
        let broadcast_ordinal = coordinator
            .high_water
            .checked_add(1)
            .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision)?;
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let broadcast_address =
            ConcreteWorkAddress::new(lease.owner(), broadcast_ordinal, broadcast_slot)
                .ok_or(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision)?;
        if self.entries.contains_key(&broadcast_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
            || !broadcast.validates_at(verified, broadcast_address, broadcast.digest())
        {
            return Err(RecoveredLifecycleSignBroadcastPreparationErrorV1::ChildCollision);
        }
        Ok(PreparedRecoveredLifecycleSignBroadcastSuccessor {
            registry: self,
            sign_address,
            broadcast_address,
            broadcast,
            verified: verified.clone(),
            adapter,
        })
    }

    /// Seal the exact Broadcast-and-next-WAL-Sign pair under one claimed Sign.
    ///
    /// Body authority has already crossed the launched service/executor census,
    /// but remains opaque here. The adapter consumes it only after the exact
    /// installed parent, dispatch key, lease, and two-child reducer shape have
    /// all rejoined; no registry entry changes before LedgerV1 publication.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<
        'registry,
        'adapter,
    >(
        &'registry mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
        key: RecoveredLifecycleSignDispatchKeyV1,
        mut adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<
            'adapter,
        >,
        body: crate::sumeragi::v2::RecoveredLifecycleNextVoteBodyAuthorityV1,
    ) -> Result<
        PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1,
    > {
        let (&slot, &digest) = lease
            .physical_slots()
            .first_key_value()
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        if lease.physical_slots().len() != 1
            || adapter.dispatch_key() != key
            || adapter.shape()
                != crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
            || coordinator.high_water.checked_add(2).is_none()
        {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let sign_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        let sign = self
            .entries
            .get(&sign_address)
            .ok_or(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier)?;
        if sign.digest != digest
            || !sign.validates_at(sign_address)
            || self
                .entries
                .keys()
                .filter(|address| address.owner == lease.owner())
                .count()
                != 1
        {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let parent_is_exact = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(sign_address, digest, coordinator, lease)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_current_claimed_record(sign_address, digest, coordinator, lease)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.dispatch_key == Some(key)
                    && sign.carrier.matches_claimed_record(coordinator, lease)
            }
            _ => false,
        };
        if !parent_is_exact {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidSignCarrier,
            );
        }
        let projection_authority = adapter.project_broadcast_and_sign_authority(body).map_err(
            |_| RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
        )?;
        let (projected_key, successor) = match &sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => sign
                .repair
                .project_authenticated_signed_broadcast_and_sign(verified, projection_authority),
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                super::wal_recovery::project_recovered_next_wal_vote_signed_broadcast_and_sign(
                    &sign.projection,
                    verified,
                    projection_authority,
                )
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => sign
                .carrier
                .project_authenticated_signed_broadcast_and_sign(verified, projection_authority),
            _ => None,
        }
        .ok_or(
            RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
        )?;
        if projected_key != key {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        }
        Ok(PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {
            registry: self,
            sign_address,
            successor,
            verified: verified.clone(),
            adapter,
        })
    }
}

impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter> {
    /// Project the exact Broadcast candidate while retaining every owner.
    pub(super) fn project_for_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        if self.sign_address.owner != lease.owner()
            || self.sign_address.ordinal != lease.ordinal()
            || self.broadcast_address.owner != lease.owner()
            || self.verified.context() != verified.context()
            || self.verified.proofs_of_possession() != verified.proofs_of_possession()
            || !self.broadcast.validates_at(
                verified,
                self.broadcast_address,
                self.broadcast.digest(),
            )
        {
            return Err(SealedBodySuccessorProjectionError::ForeignParent);
        }
        Ok(self.broadcast.candidate().clone())
    }

    /// Replace the exact recovered Sign carrier with its durable Broadcast child.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter> {
        let Self {
            registry,
            sign_address,
            broadcast_address,
            broadcast,
            verified,
            adapter,
        } = self;
        let sign = registry
            .entries
            .remove(&sign_address)
            .expect("published recovered Broadcast retains its exact Sign carrier");
        let parent = match sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::PhaseVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::NextWalVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::Control(sign)
            }
            _ => panic!("published recovered Broadcast cannot replace another carrier class"),
        };
        assert!(parent.dispatch_key().is_some());
        let digest = broadcast.digest();
        let replacement = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent,
                    broadcast,
                    verified,
                    address: broadcast_address,
                    paired_next_sign: None,
                },
            ),
        };
        assert!(replacement.validates_at(broadcast_address));
        assert!(
            registry
                .entries
                .insert(broadcast_address, replacement)
                .is_none()
        );
        adapter
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry, 'adapter>
    PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>
{
    /// Classify the sole assertion-only adapter commit mode before fsync.
    pub(super) fn publication_is_vote(&self) -> Option<bool> {
        match (
            self.adapter.is_vote_broadcast_and_sign(),
            self.adapter.is_authorized_proposal_broadcast_and_sign(),
        ) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            (true, true) | (false, false) => None,
        }
    }

    /// Clone the two inert admissions only under the transition-private permit.
    pub(super) fn project_transition_candidates(
        &self,
        permit: super::body_pipeline_transition::RecoveredLifecycleBroadcastAndSignTransitionProjectionPermitV1,
    ) -> (CandidateAdmission, CandidateAdmission) {
        self.successor.project_transition_candidates(permit)
    }

    /// Bind both process-local child addresses to the exact staged rows.
    pub(super) fn bind_staged_children(
        self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
        next_sign_ordinal: u128,
    ) -> Result<
        BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>,
        RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1,
    > {
        if !self.successor.matches_staged_ready_children(
            &self.verified,
            coordinator,
            broadcast_ordinal,
            next_sign_ordinal,
        ) {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        }
        let (Some(broadcast_record), Some(next_sign_record)) = (
            coordinator.records.get(&broadcast_ordinal),
            coordinator.records.get(&next_sign_ordinal),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        let (Some((&broadcast_slot, _)), Some((&next_sign_slot, _))) = (
            broadcast_record.physical_slots.first_key_value(),
            next_sign_record.physical_slots.first_key_value(),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        let (Some(broadcast_address), Some(next_sign_address)) = (
            ConcreteWorkAddress::new(broadcast_record.owner, broadcast_ordinal, broadcast_slot),
            ConcreteWorkAddress::new(next_sign_record.owner, next_sign_ordinal, next_sign_slot),
        ) else {
            return Err(
                RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::InvalidCombinedProjection,
            );
        };
        if broadcast_address.owner != self.sign_address.owner
            || next_sign_address.owner == self.sign_address.owner
            || self.registry.entries.contains_key(&broadcast_address)
            || self.registry.entries.contains_key(&next_sign_address)
            || self
                .registry
                .entries
                .keys()
                .filter(|address| address.owner == self.sign_address.owner)
                .count()
                != 1
            || self
                .registry
                .entries
                .keys()
                .any(|address| address.owner == next_sign_address.owner)
        {
            return Err(RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1::ChildCollision);
        }
        Ok(BoundRecoveredLifecycleSignBroadcastAndSignSuccessor {
            registry: self.registry,
            sign_address: self.sign_address,
            broadcast_address,
            next_sign_address,
            successor: self.successor,
            verified: self.verified,
            adapter: self.adapter,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'registry, 'adapter>
    BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter>
{
    /// Replace the exact recovered Sign with both durably published children.
    ///
    /// The combined projection separates only in this assertion-only tail.
    /// Broadcast retains the original claimed Sign carrier; the follow-on WAL
    /// Vote becomes a distinct undispatched Sign carrier at its staged owner.
    pub(super) fn commit_after_publication(
        self,
    ) -> crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter> {
        let Self {
            registry,
            sign_address,
            broadcast_address,
            next_sign_address,
            successor,
            verified,
            adapter,
        } = self;
        let sign = registry
            .entries
            .remove(&sign_address)
            .expect("published combined successor retains its exact Sign parent");
        let parent = match sign.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::PhaseVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::NextWalVote(sign)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                DurableRecoveredLifecycleSignParentV1::Control(sign)
            }
            _ => panic!("published combined successor cannot replace another carrier class"),
        };
        assert!(parent.dispatch_key().is_some());
        let (broadcast, next_sign) = successor.into_registry_children(
            RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new(),
        );
        let broadcast_digest = broadcast.digest();
        let next_sign_digest = next_sign.digest();
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent,
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((next_sign_address, next_sign_digest)),
                },
            ),
        };
        let next_sign_work = ConcreteLifecycleWork {
            digest: next_sign_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: next_sign,
                    verified,
                    address: next_sign_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(next_sign_work.validates_at(next_sign_address));
        assert!(
            registry
                .entries
                .insert(broadcast_address, broadcast_work)
                .is_none()
        );
        assert!(
            registry
                .entries
                .insert(next_sign_address, next_sign_work)
                .is_none()
        );
        adapter
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
    include!("tests/v2_lifecycle_work_registry_validate_dispatch_cases.rs");
    include!("tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs");
    include!("tests/v2_lifecycle_work_registry_durable_store_and_validate_cases.rs");
    include!("tests/v2_lifecycle_work_registry_exact_registry_cases.rs");
    include!("tests/v2_lifecycle_work_registry_recovery_surface_cases.rs");
    include!("tests/v2_lifecycle_work_registry_replay_evidence_cases.rs");
}
