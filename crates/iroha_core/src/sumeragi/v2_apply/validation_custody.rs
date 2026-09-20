//! Move-only candidate custody through round-local validation-marker persistence.
//!
//! This is the retention boundary for the future resource-admitted validator.
//! It does not supply an admission policy or enable the current scalar service.
//! TODO: wire the live validator only when its concrete reservation can fund
//! original execution, detached journals and all publication overlap.

use crate::sumeragi::v2_body_store::{
    BodyValidationError, DurableBodyReceipt, LocalValidationRefusal, V2BodyStoreInstanceIdentity,
    ValidatedBodyReceipt,
};
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};

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

impl<A, B> sealed::Owner for crate::state::RetainedCarrier<A, B> {}
impl<A: Send + 'static, B: Send + 'static> RetainedValidationOwner
    for crate::state::RetainedCarrier<A, B>
{
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
        self.matches_validation_candidate(context, body)
    }
    fn ready_commitment(&self) -> Option<wire::ExecutionCommitment> {
        self.ready_commitment()
    }
}

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
    /// Complete only the original capture, or return the same current phase with
    /// its local refusal. This operation never executes or rejects the proposal.
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

/// Descriptor-bounded service storage. Payload capacity remains inside each owner.
pub(crate) struct RetainedBodyValidationService<P: CarrierValidator> {
    validator: P,
    identity: V2BodyStoreInstanceIdentity,
    candidates: Vec<Candidate<P::Owner>>,
    markers: Vec<Marker>,
    limit: usize,
}

impl<P: CarrierValidator> RetainedBodyValidationService<P> {
    /// Only BodyStore supplies the original instance identity and its bound.
    pub(crate) fn new(
        validator: P,
        identity: V2BodyStoreInstanceIdentity,
        limit: usize,
    ) -> Result<Self, CarrierCustodyError> {
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
        })
    }

    pub(crate) fn matches_store(&self, identity: &V2BodyStoreInstanceIdentity) -> bool {
        self.identity.same_instance(identity)
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
                let owner = match self.validator.prepare(context, body) {
                    Ok(owner) => owner,
                    Err(error) => return Ok(CarrierMarkerPreparation::ValidationError(error)),
                };
                self.candidates.push(Candidate {
                    subject: durable.subject(),
                    owner: Some(owner),
                });
                self.candidates.len() - 1
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
