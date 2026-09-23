//! Move-only candidate custody through round-local validation-marker persistence.
//!
//! The production Native validator supplies the finite local shell reservation.
//! This layer retains the exact source, execution and publication phase across
//! marker retries; it grants no consensus authority from allocation credits.

use crate::sumeragi::v2_body_store::{
    BodyValidationError, DurableBodyReceipt, LocalValidationRefusal, V2BodyStoreInstanceIdentity,
    ValidatedBodyReceipt,
};
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};
use mv::allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, InsufficientReservation,
};
use std::alloc::Layout;

mod sealed {
    pub trait Owner {}
}

/// An original candidate phase, never an erased allocation or scalar receipt.
pub(crate) trait RetainedValidationOwner: sealed::Owner + Send + 'static {
    /// Compare only the original frozen context and canonical proposal bytes.
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool;
    /// The original prefix only after all captures complete, without reexecution.
    fn ready_commitment(&self) -> Option<wire::ExecutionCommitment>;
}

impl<A> sealed::Owner for crate::state::RetainedCarrier<A> {}
impl<A: Send + 'static> RetainedValidationOwner for crate::state::RetainedCarrier<A> {
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
        self.matches_validation_candidate(context, body)
    }
    fn ready_commitment(&self) -> Option<wire::ExecutionCommitment> {
        self.ready_commitment()
    }
}

impl sealed::Owner for super::native_validation::NativeValidationCandidate {}

#[cfg(test)]
pub(in crate::sumeragi) mod test_support {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    /// Stand-in only for the custody state machine, never production admission.
    pub(in crate::sumeragi) struct TrackedOwner {
        context: wire::HeightContext,
        wire_hash: iroha_crypto::Hash,
        commitment: Option<wire::ExecutionCommitment>,
        payload: Box<u64>,
        drops: Arc<AtomicUsize>,
    }
    impl TrackedOwner {
        pub(in crate::sumeragi) fn new(
            context: &wire::HeightContext,
            body: &SignedBlock,
            commitment: wire::ExecutionCommitment,
            drops: Arc<AtomicUsize>,
        ) -> Self {
            Self {
                context: context.clone(),
                wire_hash: body.canonical_proposal_wire_hash().unwrap(),
                commitment: Some(commitment),
                payload: Box::new(73),
                drops,
            }
        }
        pub(in crate::sumeragi) fn allocation(&self) -> *const u64 {
            std::ptr::from_ref(self.payload.as_ref())
        }
        pub(in crate::sumeragi) fn into_incomplete(mut self) -> Self {
            self.commitment = None;
            self
        }
    }
    impl Drop for TrackedOwner {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl sealed::Owner for TrackedOwner {}
    impl RetainedValidationOwner for TrackedOwner {
        fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
            &self.context == context
                && body
                    .canonical_proposal_wire_hash()
                    .is_ok_and(|hash| hash == self.wire_hash)
        }
        fn ready_commitment(&self) -> Option<wire::ExecutionCommitment> {
            self.commitment
        }
    }
    impl<P: CarrierValidator> RetainedBodyValidationService<P> {
        pub(crate) fn owner_for_test(&self, subject: wire::BlockSubject) -> Option<&P::Owner> {
            self.candidates
                .iter()
                .find(|row| row.subject == subject)
                .and_then(|row| row.owner.as_ref())
        }
        pub(crate) fn marker_counts_for_test(&self) -> (usize, usize) {
            (
                self.markers
                    .iter()
                    .filter(|row| row.confirmed.is_none())
                    .count(),
                self.markers
                    .iter()
                    .filter(|row| row.confirmed.is_some())
                    .count(),
            )
        }
        pub(crate) fn descriptor_allocation_bytes_for_test(&self) -> usize {
            self.candidates.capacity() * std::mem::size_of::<Candidate<P::Owner>>()
                + self.markers.capacity() * std::mem::size_of::<Marker>()
        }
    }
}

/// Adapter implemented by the validator that owns the real reservation policy.
pub(crate) trait CarrierValidator {
    /// Full detached carrier with its concrete admission owner still attached.
    type Owner: RetainedValidationOwner;
    /// Typed deterministic or local refusal from the actual producer.
    type Error: BodyValidationError;
    /// Execute and detach once, after the adapter has reserved descriptor slots.
    /// Return any detached unfinished capture as an owner, not an error: the
    /// existing candidate slot retains it before capture completion is attempted.
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error>;
    /// Advance only the original retained phase, or return it with its local refusal.
    /// Source recovery may finish the first execution; executed phases must never rerun
    /// that execution or turn a local readiness failure into a proposal rejection.
    fn resume(
        &mut self,
        owner: Self::Owner,
    ) -> Result<Self::Owner, (Self::Owner, LocalValidationRefusal)>;
}

/// Capture refusal is separate from the producer's deterministic error channel.
pub(crate) enum CarrierMarkerPreparation<E> {
    /// Original capture is complete and can authorize a durable marker write.
    Ready(wire::ExecutionCommitment),
    /// The original candidate remains installed without a validation marker.
    Deferred(LocalValidationRefusal),
    /// A classified producer error returned before a detached owner existed.
    ValidationError(E),
}

/// A local custody failure cannot become a deterministic rejection marker.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CarrierCustodyError {
    /// The original open store or candidate no longer matches.
    #[error("retained carrier belongs to another store or candidate")]
    Identity,
    /// A scalar cached receipt cannot stand in for the missing executed owner.
    #[error("validation marker has no original retained carrier")]
    MissingOwner,
    /// Existing per-height descriptor capacity was exhausted before execution.
    #[error("retained carrier descriptor capacity exhausted")]
    Capacity,
    /// Host allocation failed before any new candidate execution.
    #[error("cannot reserve retained carrier descriptors: {0}")]
    Allocation(#[from] std::collections::TryReserveError),
    /// The exact descriptor layouts could not be admitted before allocation.
    #[error("retained carrier descriptor byte admission: {0}")]
    DescriptorAdmission(#[from] AllocationRefusal),
    /// Original prepaid credits did not cover their requested allocation split.
    #[error("retained carrier descriptor reservation: {0}")]
    DescriptorReservation(#[from] InsufficientReservation),
    /// Selection requires a confirmed exact receipt, never a pending write.
    #[error("retained carrier has no matching confirmed validation receipt")]
    Unconfirmed,
    /// A producer cannot authorize a marker for an unfinished capture phase.
    #[error("retained carrier archive capture is incomplete")]
    IncompleteCapture,
}

struct Candidate<O> {
    subject: wire::BlockSubject,
    owner: Option<O>,
}

struct Marker {
    durable: DurableBodyReceipt,
    confirmed: Option<ValidatedBodyReceipt>,
}

/// Descriptor storage admitted by count and exact requested allocation bytes.
/// Nested candidate payload capacity remains inside each original owner.
pub(crate) struct RetainedBodyValidationService<P: CarrierValidator> {
    candidates: Vec<Candidate<P::Owner>>,
    markers: Vec<Marker>,
    identity: V2BodyStoreInstanceIdentity,
    limit: usize,
    // Neither vector escapes this owner. Charges drop only after both vectors
    // and every retained candidate have actually been destroyed.
    _descriptor_admission: [AllocationCharge; 2],
    // Field drop order keeps the original service and its resource policy alive
    // until every retained payload and descriptor allocation has been released.
    validator: P,
}

impl<P: CarrierValidator> RetainedBodyValidationService<P> {
    fn descriptor_layouts(limit: usize) -> Result<[Layout; 2], AllocationRefusal> {
        Ok([
            Layout::array::<Candidate<P::Owner>>(limit)
                .map_err(|_| AllocationRefusal::DemandOverflow)?,
            Layout::array::<Marker>(limit).map_err(|_| AllocationRefusal::DemandOverflow)?,
        ])
    }

    /// Plan both actual vector allocations without allocating or executing.
    /// This funds inline owner storage, not its separately retained payloads.
    pub(crate) fn descriptor_bytes(limit: usize) -> Result<usize, AllocationRefusal> {
        Self::descriptor_layouts(limit)?
            .into_iter()
            .try_fold(0, |total: usize, layout| {
                total
                    .checked_add(layout.size())
                    .ok_or(AllocationRefusal::DemandOverflow)
            })
    }

    /// Only BodyStore supplies the original instance identity and its bound.
    /// The explicit shared pool admits both vectors before either is allocated.
    pub(crate) fn new(
        validator: P,
        identity: V2BodyStoreInstanceIdentity,
        limit: usize,
        budget: &AllocationBudget,
    ) -> Result<Self, CarrierCustodyError> {
        let layouts = Self::descriptor_layouts(limit)?;
        let mut reservation = budget.try_reserve_layouts(layouts)?;
        let descriptor_admission = [
            reservation.try_split(layouts[0])?,
            reservation.try_split(layouts[1])?,
        ];
        drop(reservation);
        let mut candidates = Vec::new();
        candidates.try_reserve_exact(limit)?;
        let mut markers = Vec::new();
        markers.try_reserve_exact(limit)?;
        Ok(Self {
            validator,
            identity,
            candidates,
            markers,
            limit,
            _descriptor_admission: descriptor_admission,
        })
    }

    pub(crate) fn matches_store(&self, identity: &V2BodyStoreInstanceIdentity) -> bool {
        self.identity.same_instance(identity)
    }

    /// Refuse unavailable descriptor custody before BodyStore reads or decodes
    /// another candidate. This fresh observation reserves nothing and grants no
    /// marker authority; preparation rechecks capacity after authenticating the
    /// body. Existing occurrences remain serviceable at the configured limit.
    pub(crate) fn preflight_marker(
        &self,
        durable: &DurableBodyReceipt,
    ) -> Result<(), CarrierCustodyError> {
        let candidate = self
            .candidates
            .iter()
            .find(|row| row.subject == durable.subject());
        if candidate.is_some_and(|row| row.owner.is_none()) {
            return Err(CarrierCustodyError::MissingOwner);
        }
        if (!self.markers.iter().any(|row| row.durable == *durable)
            && self.markers.len() == self.limit)
            || (candidate.is_none() && self.candidates.len() == self.limit)
        {
            return Err(CarrierCustodyError::Capacity);
        }
        Ok(())
    }

    /// Install before capture retry or fsync. Incomplete capture retains only its
    /// candidate; a complete capture can add a pending marker occurrence. A prior
    /// confirmed occurrence is never overwritten.
    pub(crate) fn prepare_marker(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
        durable: &DurableBodyReceipt,
        requires_existing_owner: bool,
    ) -> Result<CarrierMarkerPreparation<P::Error>, CarrierCustodyError> {
        let existing = self
            .candidates
            .iter()
            .position(|row| row.subject == durable.subject());
        let marker = self.markers.iter().position(|row| row.durable == *durable);
        if marker.is_none() && self.markers.len() == self.limit {
            return Err(CarrierCustodyError::Capacity);
        }
        let index = match existing {
            Some(index) => index,
            None => {
                if requires_existing_owner {
                    return Err(CarrierCustodyError::MissingOwner);
                }
                if self.candidates.len() == self.limit {
                    return Err(CarrierCustodyError::Capacity);
                }
                // Occupy the already allocated descriptor before execution.
                // Unwind must leave a subject tombstone, just as consuming
                // capture and publication do, rather than permit reexecution.
                let index = self.candidates.len();
                self.candidates.push(Candidate {
                    subject: durable.subject(),
                    owner: None,
                });
                let owner = match self.validator.prepare(context, body) {
                    Ok(owner) => owner,
                    Err(error) => {
                        // The producer's explicit error contract guarantees no
                        // detached owner exists. Only this branch can release
                        // the vacant descriptor for a later admission attempt.
                        let vacant = self
                            .candidates
                            .pop()
                            .expect("reserved candidate descriptor");
                        debug_assert_eq!(vacant.subject, durable.subject());
                        debug_assert!(vacant.owner.is_none());
                        return Ok(CarrierMarkerPreparation::ValidationError(error));
                    }
                };
                self.candidates[index].owner = Some(owner);
                index
            }
        };
        let owner = self.candidates[index]
            .owner
            .as_ref()
            .ok_or(CarrierCustodyError::MissingOwner)?;
        if !owner.matches_candidate(context, body) {
            return Err(CarrierCustodyError::Identity);
        }
        let commitment = match owner.ready_commitment() {
            Some(commitment) => commitment,
            None => {
                if let Some(refusal) = self.resume_candidate(index, context, body)? {
                    return Ok(CarrierMarkerPreparation::Deferred(refusal));
                }
                self.candidates[index]
                    .owner
                    .as_ref()
                    .ok_or(CarrierCustodyError::MissingOwner)?
                    .ready_commitment()
                    .ok_or(CarrierCustodyError::IncompleteCapture)?
            }
        };
        if marker.is_none() {
            self.markers.push(Marker {
                durable: durable.clone(),
                confirmed: None,
            });
        }
        Ok(CarrierMarkerPreparation::Ready(commitment))
    }

    // Only unfinished capture enters this consuming frame. Ready marker/cache
    // paths borrow their original owner without moving it through a resume Result.
    fn resume_candidate(
        &mut self,
        index: usize,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Option<LocalValidationRefusal>, CarrierCustodyError> {
        let owner = self.candidates[index]
            .owner
            .take()
            .ok_or(CarrierCustodyError::MissingOwner)?;
        // Resume consumes the same phase; ordinary refusal restores it before
        // any outward error or wake can escape. Panic remains fail-stop, with an
        // occupied subject tombstone that cannot authorize fresh execution.
        let refusal = match self.validator.resume(owner) {
            Ok(owner) => {
                self.candidates[index].owner = Some(owner);
                None
            }
            Err((owner, refusal)) => {
                self.candidates[index].owner = Some(owner);
                Some(refusal)
            }
        };
        let owner = self.candidates[index]
            .owner
            .as_ref()
            .expect("capture completion restored its current original phase");
        if !owner.matches_candidate(context, body) {
            return Err(CarrierCustodyError::Identity);
        }
        Ok(refusal)
    }

    /// Record only the receipt returned after the original marker's fsync.
    pub(crate) fn confirm(
        &mut self,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<(), CarrierCustodyError> {
        let marker = self
            .markers
            .iter_mut()
            .find(|row| row.durable == *receipt.durable())
            .ok_or(CarrierCustodyError::Identity)?;
        let owner = self
            .candidates
            .iter()
            .find(|row| row.subject == receipt.durable().subject())
            .and_then(|row| row.owner.as_ref())
            .ok_or(CarrierCustodyError::MissingOwner)?;
        if owner.ready_commitment() != Some(receipt.execution_commitment())
            || marker.confirmed.as_ref().is_some_and(|old| old != receipt)
        {
            return Err(CarrierCustodyError::Identity);
        }
        marker.confirmed = Some(receipt.clone());
        Ok(())
    }

    /// Select this exact confirmed occurrence; later failed marker writes do not
    /// prevent its use. The borrowing cut prevents concurrent replacement/removal.
    pub(crate) fn select(
        &mut self,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<SelectedValidationCarrier<'_, P>, CarrierCustodyError> {
        if !self
            .markers
            .iter()
            .any(|row| row.confirmed.as_ref() == Some(receipt))
        {
            return Err(CarrierCustodyError::Unconfirmed);
        }
        let index = self
            .candidates
            .iter()
            .position(|row| row.subject == receipt.durable().subject())
            .ok_or(CarrierCustodyError::MissingOwner)?;
        let owner = self.candidates[index]
            .owner
            .take()
            .ok_or(CarrierCustodyError::MissingOwner)?;
        Ok(SelectedValidationCarrier {
            service: self,
            index,
            owner: Some(owner),
        })
    }
}

/// Affine candidate selection. Drop/ordinary refusal restores the original owner.
#[must_use = "selected candidate must be consumed by publication or restored"]
pub(crate) struct SelectedValidationCarrier<'a, P: CarrierValidator> {
    service: &'a mut RetainedBodyValidationService<P>,
    index: usize,
    owner: Option<P::Owner>,
}

impl<P: CarrierValidator> SelectedValidationCarrier<'_, P> {
    /// This callback must run the consuming publisher, which still requires its
    /// own exact Decision/source/storage authority. Custody grants none of it.
    /// A local refusal must return the same complete owner in its current phase,
    /// including a decision or checkpoint attached during this callback. Drop
    /// restores that phase into its original slot without rebuilding execution.
    /// The callback borrows this service's original producer only for this call;
    /// publication dependencies must come from that producer. Guards borrowing
    /// it must finish inside the callback and cannot escape in its result.
    /// Panic is fail-stop;
    /// the occupied row remains a tombstone and cannot trigger reexecution.
    pub(crate) fn try_consume<R, E>(
        mut self,
        publish: impl FnOnce(&P, P::Owner) -> Result<R, (P::Owner, E)>,
    ) -> Result<R, E> {
        let owner = self
            .owner
            .take()
            .expect("live selection retains its original owner");
        match publish(&self.service.validator, owner) {
            Ok(value) => {
                // Keep the bounded subject tombstone until this height retires.
                // An earlier unvalidated round can arrive after consumption;
                // scalar marker reuse must never cause it to execute again.
                let subject = self.service.candidates[self.index].subject;
                self.service
                    .markers
                    .retain(|row| row.durable.subject() != subject);
                Ok(value)
            }
            Err((owner, error)) => {
                self.owner = Some(owner);
                Err(error)
            }
        }
    }
}

impl<P: CarrierValidator> Drop for SelectedValidationCarrier<'_, P> {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            debug_assert!(self.service.candidates[self.index].owner.is_none());
            self.service.candidates[self.index].owner = Some(owner);
        }
    }
}

impl RetainedBodyValidationService<super::native_validation::OwnedNativeCarrierValidator> {
    /// Join a verified response to the exact original candidate waiting for that source.
    pub(crate) fn complete_native_source(
        &mut self,
        subject: wire::BlockSubject,
        request: &crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
        response: &crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyResponse,
    ) -> Result<super::native_validation::NativeSourceRecoveryCompletion, LocalValidationRefusal>
    {
        let owner = self
            .candidates
            .iter_mut()
            .find(|row| row.subject == subject)
            .and_then(|row| row.owner.as_mut())
            .ok_or_else(|| {
                LocalValidationRefusal::RecoveryRequired(
                    "Native source response has no original retained candidate".into(),
                )
            })?;
        owner.complete_native_source(subject, request, response)
    }
}
