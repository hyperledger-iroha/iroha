//! Sealed executor join for exact fair-ingress selector debt.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_crypto::HashOf;
use iroha_data_model::block::consensus_v2 as wire;

use super::{
    LifecycleCoordinator,
    ingress_position::{
        FairIngressQueueCut, FairIngressQueuePositions, PendingFairIngressIdentity,
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
        CertifiedFetchCompletionError, CertifiedFetchReplacementLocation,
        ConcreteLifecycleWorkRegistry, PreparedCertifiedFetchCompletion,
    },
};
use crate::sumeragi::{
    FairV2Ingress, FairV2IngressClass, FairV2IngressQueueGateVerdict, FairV2IngressSourceClass,
    InboundBlockMessage,
    message::BlockMessage,
    v2_effects::{
        CertifiedResponsePriorityCandidate, CertifiedResponsePriorityProbe, EffectTransportError,
        V2EffectExecutor,
    },
    v2_runtime::SerializedV2Runtime,
    v2_transport::V2TransportError,
};

#[cfg(test)]
use super::{
    CapacityClass, OwnerId, PhysicalSlotId,
    work_registry::{ConcreteLifecycleWork, ConcreteWorkAddress},
};
#[cfg(test)]
use crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding;

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
    /// The complete occurrence key set or cardinality was not representable.
    InvalidCensus,
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
    location: CertifiedFetchReplacementLocation,
}

impl PreparedCertifiedFetchReadyMutation<'_> {
    #[cfg(test)]
    fn target_for_test(&self) -> &LifecycleCoordinator {
        self.target
    }

    fn commit(self) {
        *self.target = self.next;
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
    candidate: Box<CertifiedResponsePriorityCandidate>,
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
/// This value is deliberately not a `SchedulerInputs` mint. Returning it drops
/// the queue service guard and executor borrow, so the future composite factory
/// must reacquire its output permit, revalidate and commit the exact queue
/// occurrence, then atomically claim the response family and reserve runtime
/// and service capacity. Before entering the sealed queue mutation tail it
/// must consume this whole value and release every retained family `Arc` after
/// its final candidate probe; checked dequeue requires exclusive ownership of
/// the selected envelope. Those mutating seams remain intentionally unavailable.
#[must_use = "the prepared selector still requires the unavailable atomic transaction"]
pub(crate) struct PreparedLifecycleIngressSelector {
    context: LifecycleContext,
    request_fence_active: bool,
    queue_witness: PreparedFairIngressQueueWitness,
    verdicts: BTreeMap<u64, LifecycleIngressOccurrenceVerdict>,
    priority_owners: BTreeSet<u64>,
    claimed_response_families:
        BTreeMap<HashOf<wire::CertifiedBodyRequest>, PreparedClaimedResponseFamily>,
    selector_debt: u64,
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

    /// Derive a sealed wake authority only when the queue-selected occurrence
    /// is the unique authenticated winner of its exact response family.
    fn selected_certified_fetch_ready_authority(
        &self,
    ) -> Result<CertifiedFetchReadyAuthority, CertifiedFetchReadyPublicationError> {
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

        let candidate = prepared.candidate.as_ref();
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
            || candidate.request_hash() != *request_hash
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
            request_hash: *request_hash,
            key,
            causal_root: pending_effect_causal_root(binding),
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
            candidate_pending: prepared.candidate.pending_effect_binding(),
        })
    }

    /// Prepare the concrete same-slot conversion while every registry byte is
    /// still untouched and the selected response candidate is available for
    /// exact equality checks.
    ///
    /// This method intentionally neither consumes the selector nor exposes a
    /// queue commit. A future output-permitted composite transaction must first
    /// persist the authenticated body, then recapture a fresh selector/queue
    /// cut, use this preflight while the retained family `Arc` is available,
    /// and bind the result to that sealed durability receipt. Only the resulting
    /// receipt-bound registry token may consume the later exact checked-dequeue
    /// result.
    #[allow(dead_code)]
    fn prepare_selected_certified_fetch_completion<'a>(
        &self,
        registry: &'a mut ConcreteLifecycleWorkRegistry,
        location: CertifiedFetchReplacementLocation,
    ) -> Result<PreparedCertifiedFetchCompletion<'a>, CertifiedFetchCompletionPreparationError>
    {
        let authority = self
            .selected_certified_fetch_completion_authority()
            .map_err(CertifiedFetchCompletionPreparationError::ReadyAuthority)?;
        if location.owner().causal_root() != authority.causal_root()
            || location.replacement_digest() != authority.ingress_identity().digest()
        {
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
        let location = CertifiedFetchReplacementLocation::new(
            owner,
            ordinal,
            slot,
            incumbent_digest,
            ready.ingress_identity.digest(),
        )
        .ok_or_else(|| "exact Fetch replacement location was invalid".to_owned())?;
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        registry
            .install(address, incumbent_digest, incumbent)
            .map_err(|(error, _)| format!("exact Fetch registry install rejected: {error:?}"))?;
        let prepared = self
            .prepare_selected_certified_fetch_completion(&mut registry, location)
            .map_err(|error| format!("sealed Fetch registry preflight rejected: {error:?}"))?;
        drop(prepared);
        if registry.len() != 1 || !registry.exactly_contains(address, &expected_effect) {
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

impl LifecycleCoordinator {
    /// Publish one exact certified response into the original waiting Fetch
    /// record without consuming the prepared selector or changing queue,
    /// response-claim, concrete-registry, runtime, or service ownership. The
    /// logical row does stage the response's same-slot physical digest, which
    /// is why this method remains private until the registry swap joins it.
    ///
    /// This inert seam deliberately borrows the full preparation. The future
    /// output-permitted transaction must still consume it while committing the
    /// family claim, capacity reservations, and exact queue dequeue together.
    // TODO: Fold this publication into the unavailable output-permitted
    // response-claim/runtime/service/queue-CAS transaction.
    #[allow(dead_code)]
    fn publish_selected_certified_fetch_ready(
        &mut self,
        prepared: &PreparedLifecycleIngressSelector,
    ) -> Result<CertifiedFetchReadyPublication, CertifiedFetchReadyPublicationError> {
        let authority = prepared.selected_certified_fetch_ready_authority()?;
        self.publish_certified_fetch_ready_authority(authority)
    }

    fn publish_certified_fetch_ready_authority(
        &mut self,
        authority: CertifiedFetchReadyAuthority,
    ) -> Result<CertifiedFetchReadyPublication, CertifiedFetchReadyPublicationError> {
        match self.prepare_certified_fetch_ready_authority(authority)? {
            PreparedCertifiedFetchReadyTransition::Mutation(prepared) => {
                prepared.commit();
                Ok(CertifiedFetchReadyPublication::Published)
            }
            PreparedCertifiedFetchReadyTransition::Stutter(publication) => Ok(publication),
        }
    }

    fn prepare_certified_fetch_ready_authority(
        &mut self,
        authority: CertifiedFetchReadyAuthority,
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
                if installed_digest != authority.ingress_identity.digest() {
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
        let replacement_digest = authority.ingress_identity.digest();
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
        let location = CertifiedFetchReplacementLocation::new(
            owner,
            ordinal,
            slot,
            incumbent_digest,
            replacement_digest,
        )
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

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Prepare one opaque complete selector census from an exact queue target.
    ///
    /// This is the sole crate-visible mint. It deliberately returns an inert,
    /// borrow-free token rather than rank authority; the future synchronous
    /// output-permitted transaction must consume and revalidate that token.
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

    /// Classify every exact pre-cut fair-ingress occurrence without mutation.
    ///
    /// The queue's service guard remains held while queue state is released for
    /// authentication and body reconstruction. Producer appends may proceed;
    /// the final read-only queue recheck rejects any pre-cut mutation. Returning
    /// the borrow-free preparation releases that guard without removing a row.
    /// This tranche never claims a response, reserves runtime capacity, or
    /// publishes selector debt as scheduler authority.
    // TODO: Consume this preparation only from the future synchronous factory
    // which holds the consensus-output permit and can atomically validate and
    // remove the exact queue occurrence while committing response ownership.
    pub(super) fn capture_lifecycle_ingress_selector(
        &self,
        cut: FairIngressQueueCut<'_>,
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
        let mut verdicts = BTreeMap::new();
        let mut response_candidates = BTreeMap::new();
        for occurrence in cut.selector_occurrences() {
            let ordinal = occurrence.physical_admission_ordinal();
            let mut verdict = LifecycleIngressOccurrenceVerdict::NOT_PRIORITY;
            if occurrence.queue_gate() != FairV2IngressQueueGateVerdict::Blocked {
                let inbound = occurrence.inbound();
                if let BlockMessage::V2(message) = inbound.message() {
                    let drainable = occurrence.is_obsolete()
                        || message.validate_version().is_err()
                        || self.can_admit_network_message_with_ingress_ownership(
                            message,
                            inbound.ingress_ownership().ok_or(
                                LifecycleIngressSelectorError::InvalidOccurrenceIdentity {
                                    ordinal,
                                },
                            )?,
                        );
                    let physical_completion = matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::PayloadChunk(_)
                            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    );
                    let certified_response = matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    );
                    let request_fenced_completion = drainable
                        && request_fence_active
                        && lifecycle_ingress_resource_is_untrusted(
                            occurrence.source_class(),
                            certified_response,
                        )
                        && occurrence.class() == FairV2IngressClass::TransportCompletion
                        && physical_completion;
                    if drainable
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
                                                candidate,
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
            if !self
                .revalidate_certified_response_priority_candidate(
                    &prepared.candidate,
                    response,
                    authenticated_responder,
                )
                .map_err(|error| LifecycleIngressSelectorError::ExecutorAuthority {
                    ordinal: Some(prepared.ingress_identity.physical_admission_ordinal()),
                    error: Box::new(error),
                })?
            {
                return Err(LifecycleIngressSelectorError::CandidateRevalidationDrift {
                    ordinal: prepared.ingress_identity.physical_admission_ordinal(),
                });
            }
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
    use iroha_crypto::Hash;

    use super::super::schema::{
        AdmissionDecision, AdmissionRequest, CandidateAdmission, CapacityClass, CapacityGeometry,
        CoordinatorFault, DurablePayloadReference, InitialLifecycleState, LeaseId, LifecycleStage,
        LifecycleStageKind, OwnerId, PhysicalGeometry, PhysicalSlot, PhysicalSlotId,
        PredecessorScope, SchedulerInputs, SchedulerReadyInputs, TerminalOutcome, TurnOutcome,
        TurnPlan, WaitToken,
    };
    use super::*;

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
            InitialLifecycleState::Waiting(wait),
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
        let slot_universe = coordinator.records[&ordinal].episode.slot_universe.clone();
        let consumed_slots = coordinator.records[&ordinal].episode.consumed_slots.clone();
        assert_ne!(incumbent_digest, authority.ingress_identity.digest());

        let before = coordinator.clone();
        let PreparedCertifiedFetchReadyTransition::Mutation(prepared) = coordinator
            .prepare_certified_fetch_ready_authority(authority)
            .expect("exact response prepares one logical replacement")
        else {
            panic!("waiting Fetch preparation cannot stutter")
        };
        assert_eq!(prepared.location.owner(), owner);
        assert_eq!(prepared.location.ordinal(), ordinal);
        assert_eq!(prepared.location.slot(), slot);
        assert_eq!(prepared.location.incumbent_digest(), incumbent_digest);
        assert_eq!(
            prepared.location.replacement_digest(),
            authority.ingress_identity.digest()
        );
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
            authority.ingress_identity.digest()
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
        let (&slot, &incumbent_digest) = coordinator.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("one exact Fetch slot");
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
        let (&slot, &incumbent_digest) = coordinator.records[&ordinal]
            .physical_slots
            .first_key_value()
            .expect("one exact Fetch slot");

        let mut same_digest = authority;
        same_digest.ingress_identity = PendingFairIngressIdentity::for_test(
            context,
            incumbent_digest,
            authority.ingress_identity.physical_admission_ordinal(),
        );
        let mut trial = coordinator.clone();
        assert_eq!(
            trial.publish_certified_fetch_ready_authority(same_digest),
            Err(CertifiedFetchReadyPublicationError::InvalidPhysicalReplacement)
        );
        assert_rejection_preserved(&coordinator, &trial);

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
    fn inert_selector_source_contains_no_claim_or_completion_commit() {
        let source = include_str!("v2_lifecycle_selector.rs");
        let forbidden = [
            [".claim_", "authenticated_response("].concat(),
            [".plan_", "fetch_completion("].concat(),
            [".commit_", "fetch_completion("].concat(),
            [".accept_", "certified_body_response("].concat(),
            ["commit_after_exact_", "dequeue"].concat(),
        ];
        for forbidden in &forbidden {
            assert!(
                !source.contains(forbidden),
                "inert selector unexpectedly contains mutating seam {forbidden}"
            );
        }
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
