//! Scheduler-free registry for exact concrete lifecycle work.
//!
//! The logical coordinator retains only authenticated slot digests. This
//! module keeps the corresponding process-local effect values in a separate,
//! deterministic map so planning never makes the coordinator own physical
//! bytes or service handles.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire},
    peer::PeerId,
};
use norito::codec::Encode;

use super::{
    AdmissionRequest, InitialLifecycleState, LifecycleContext, LifecycleDigest, LifecycleKey,
    LifecyclePhase, LifecycleStage, LifecycleStageKind, LifecycleWorkClass, OwnerId,
    PhysicalReplacement, PhysicalSlot, PhysicalSlotId, PredecessorScope, ReadyEvent, TurnLease,
    WaitSource, WaitToken,
    ingress_position::PendingFairIngressIdentity,
    projection::{self, AdapterEffectAdmissionError, certified_fetch_lifecycle_key},
    schema::DurablePayloadReference,
    selector::CertifiedFetchCompletionAuthority,
};
use crate::sumeragi::{
    InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterEffect, PreparedReadyDurableValidateAdapterPublication, SumeragiV2Adapter,
        VerifiedHeightContext,
    },
    v2_body_store::{
        BodyValidationError, BodyValidationRejectionIdentity, DurableBodyReceipt,
        DurableBodyValidationOutcome, DurableCertifiedFetchBodyReceipt, V2BodyStore,
        V2BodyStoreError, ValidatedBodyReceipt,
    },
    v2_core::EventTag,
    v2_runtime::{PendingRuntimeEffectBinding, RuntimeCandidateSemanticStatement},
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
            }
    }

    /// Derive the coordinator causal root from the sealed pending key.
    pub(super) fn causal_root(&self) -> super::CausalRoot {
        super::CausalRoot::new(digest_from_hash(
            self.pending_binding().causal_lifecycle_key(),
        ))
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
        }
    }

    const fn pending_binding(&self) -> &PendingRuntimeEffectBinding {
        match &self.kind {
            ConcreteLifecycleWorkKind::PendingAdapter { pending, .. } => pending,
            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                &completion.incumbent_pending
            }
            ConcreteLifecycleWorkKind::DurableStoreBody(store) => &store.pending,
            ConcreteLifecycleWorkKind::DurableValidateBody(validate) => &validate.pending,
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                &completion.incumbent.pending
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

/// Move-only Validate projection sealed under its closed durable Store parent.
///
/// No field can be extracted and no consuming installation exists in this
/// tranche. A future composite transaction must join this token with the
/// coordinator transition and adapter preview before publishing either side.
///
/// TODO: Add the sole consuming Store-to-Validate transaction only when its
/// registry, coordinator, durable-catalog, and adapter cuts publish together.
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
}

/// Move-only Store-successor projection sealed under its closed Fetch parent.
///
/// The projected pending binding never escapes this token. In particular,
/// callers cannot clone or install it independently of the still-borrowed
/// completion. A future parent-to-child transaction may add the sole consuming
/// commit once the real queue-CAS result is available.
///
/// TODO: Add that consuming commit only with a typed output from the real
/// checked-dequeue witness; never add a constructor from raw response parts.
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
    location: CertifiedFetchReplacementLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    response_round: wire::ConsensusRound,
    response_subject: wire::BlockSubject,
    response_manifest_hash: HashOf<wire::PayloadManifest>,
    authenticated_responder: PeerId,
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
    location: CertifiedFetchReplacementLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    durable_receipt: DurableCertifiedFetchBodyReceipt,
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
#[derive(Debug, Default)]
pub(super) struct ConcreteLifecycleWorkRegistry {
    entries: BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
}

impl ConcreteLifecycleWorkRegistry {
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
        location: CertifiedFetchReplacementLocation,
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
            || location.incumbent_digest == location.replacement_digest
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
        if location.replacement_digest != ingress_identity.digest() {
            return Err(CertifiedFetchCompletionError::ReplacementDigestMismatch);
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
        let Some((incumbent_effect, incumbent_pending)) = incumbent.pending_adapter_pair() else {
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
        let BlockMessage::V2(message) = completion.dequeued.inbound.message() else {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        };
        if !completion.validates(work.digest)
            || message.validate_version().is_err()
            || !matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            )
        {
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

        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        let request =
            projection::admission_request(active_context, verified, &store.effect, &store.pending)
                .map_err(DurableStoreExecutionError::Projection)?;
        let AdmissionRequest::Candidate(candidate) = request else {
            return Err(DurableStoreExecutionError::InvalidProjection);
        };
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
            || candidate.payload != DurablePayloadReference::None
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

        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        let request = projection::admission_request(
            active_context,
            verified,
            &validate.effect,
            &validate.pending,
        )
        .map_err(DurableValidateExecutionError::Projection)?;
        let AdmissionRequest::Candidate(candidate) = request else {
            return Err(DurableValidateExecutionError::InvalidProjection);
        };
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
            || candidate.payload != DurablePayloadReference::None
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

        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        let request = projection::admission_request(
            active_context,
            verified,
            &completion.incumbent.effect,
            &completion.incumbent.pending,
        )
        .map_err(ReadyDurableValidateExecutionError::Projection)?;
        let AdmissionRequest::Candidate(candidate) = request else {
            return Err(ReadyDurableValidateExecutionError::InvalidProjection);
        };
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
            || candidate.payload != DurablePayloadReference::None
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
        let authority = DurableValidateCompletionAuthority {
            address: request.address,
            incumbent_digest: request.incumbent_digest,
            replacement_digest,
            wait_token: dispatch.wake.wait_token,
            outcome_kind,
            lifecycle_key: request.lifecycle_key,
            lifecycle_stage: request.lifecycle_stage,
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
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
        let AdapterEffect::FetchBody { tag, .. } = &completion.incumbent_effect else {
            unreachable!("prepared certified-Fetch completion retains its Fetch effect")
        };
        let BlockMessage::V2(message) = completion.dequeued.inbound.message() else {
            unreachable!("prepared certified-Fetch completion retains a v2 response")
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
        else {
            unreachable!("prepared certified-Fetch completion retains its response payload")
        };
        (*tag, &response.manifest)
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
        completion.durable_receipt.durable_body()
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
        let (store_effect, store_pending, store_digest, durable_body, expected_manifest_hash) = {
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
            let durable_body = completion.durable_receipt.durable_body().clone();
            let BlockMessage::V2(message) = completion.dequeued.inbound.message() else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
            else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            let expected_manifest_hash = HashOf::new(&response.manifest);
            (
                successor.clone(),
                store_pending,
                store_digest,
                durable_body,
                expected_manifest_hash,
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
            (
                successor.clone(),
                validate_pending,
                validate_digest,
                store.durable_receipt.clone(),
                store.expected_manifest_hash,
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
        })
    }
}

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
impl PreparedValidatedBodyCompletion<'_> {
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

        Ok(PreparedDurableCertifiedFetchCompletion {
            registry: self.registry,
            location: self.location,
            ingress_identity: self.ingress_identity,
            request_hash: self.request_hash,
            response_hash: self.response_hash,
            authenticated_responder: self.authenticated_responder,
            durable_receipt,
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
    let BlockMessage::V2(message) = dequeued.inbound.message() else {
        return false;
    };
    if message.validate_version().is_err() {
        return false;
    }
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload else {
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

fn digest_from_hash(hash: &iroha_crypto::Hash) -> LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    LifecycleDigest::new(bytes)
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
}
