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
            CausalRoot::new(LifecycleDigest::new([discriminator.wrapping_add(1); 32]));
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.validates_digest(self.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.validates_digest(self.digest)
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
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                    sign.validates_at(address, self.digest)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                    fetch.validates_at(address, self.digest)
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.address.owner.causal_root()
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.address.owner.causal_root()
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
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
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_) => None,
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_) => None,
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

include!("v2_lifecycle_work_registry_recovered_wal.rs");
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
