//! Move-only candidate custody through round-local validation-marker persistence.
//!
//! This is the retention boundary for the future resource-admitted validator.
//! It does not supply an admission policy or enable the current scalar service.
//! TODO: wire the live validator only when its concrete reservation can fund
//! original execution, detached journals and all publication overlap.

use crate::sumeragi::v2_body_store::{
    BodyValidationError, DurableBodyReceipt, V2BodyStoreInstanceIdentity, ValidatedBodyReceipt,
};
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};

mod sealed {
    pub trait Owner {}
}

/// A complete original candidate, never an erased allocation or scalar receipt.
pub(in crate::sumeragi) trait RetainedValidationOwner:
    sealed::Owner + Send + 'static
{
    /// Compare only the original frozen context and canonical proposal bytes.
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool;
    /// The original execution-prefix commitment, without executing again.
    fn commitment(&self) -> wire::ExecutionCommitment;
}

impl<A> sealed::Owner for crate::state::PreparedCarrierJournals<A> {}
impl<A: Send + 'static> RetainedValidationOwner for crate::state::PreparedCarrierJournals<A> {
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
        self.matches_validation_candidate(context, body)
    }
    fn commitment(&self) -> wire::ExecutionCommitment {
        self.execution_prefix_commitment()
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
        commitment: wire::ExecutionCommitment,
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
                commitment,
                payload: Box::new(73),
                drops,
            }
        }
        pub(in crate::sumeragi) fn allocation(&self) -> *const u64 {
            std::ptr::from_ref(self.payload.as_ref())
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
        fn commitment(&self) -> wire::ExecutionCommitment {
            self.commitment
        }
    }
    impl<P: CarrierValidator> RetainedBodyValidationService<P> {
        pub(in crate::sumeragi) fn owner_for_test(
            &self,
            subject: wire::BlockSubject,
        ) -> Option<&P::Owner> {
            self.candidates
                .iter()
                .find(|row| row.subject == subject)
                .and_then(|row| row.owner.as_ref())
        }
        pub(in crate::sumeragi) fn marker_counts_for_test(&self) -> (usize, usize) {
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
pub(in crate::sumeragi) trait CarrierValidator {
    /// Full detached carrier with its concrete admission owner still attached.
    type Owner: RetainedValidationOwner;
    /// Typed deterministic or local refusal from the actual producer.
    type Error: BodyValidationError;
    /// Execute and detach once, after the adapter has reserved descriptor slots.
    /// Any locally refused staged execution must stay in this producer for its
    /// next call; returning an error must never discard work and reexecute it.
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error>;
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
pub(in crate::sumeragi) struct RetainedBodyValidationService<P: CarrierValidator> {
    validator: P,
    identity: V2BodyStoreInstanceIdentity,
    candidates: Vec<Candidate<P::Owner>>,
    markers: Vec<Marker>,
    limit: usize,
}

impl<P: CarrierValidator> RetainedBodyValidationService<P> {
    /// Only BodyStore supplies the original instance identity and its bound.
    pub(in crate::sumeragi) fn new(
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

    pub(in crate::sumeragi) fn matches_store(
        &self,
        identity: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.identity.same_instance(identity)
    }

    /// Install before fsync. Failure after this point leaves the exact owner and
    /// pending occurrence here; a prior confirmed occurrence is never overwritten.
    pub(in crate::sumeragi) fn prepare_marker(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
        durable: &DurableBodyReceipt,
        requires_existing_owner: bool,
    ) -> Result<Result<wire::ExecutionCommitment, P::Error>, CarrierCustodyError> {
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
                    Err(error) => return Ok(Err(error)),
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
        let commitment = owner.commitment();
        if marker.is_none() {
            self.markers.push(Marker {
                durable: durable.clone(),
                confirmed: None,
            });
        }
        Ok(Ok(commitment))
    }

    /// Record only the receipt returned after the original marker's fsync.
    pub(in crate::sumeragi) fn confirm(
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
        if owner.commitment() != receipt.execution_commitment()
            || marker.confirmed.as_ref().is_some_and(|old| old != receipt)
        {
            return Err(CarrierCustodyError::Identity);
        }
        marker.confirmed = Some(receipt.clone());
        Ok(())
    }

    /// Select this exact confirmed occurrence; later failed marker writes do not
    /// prevent its use. The borrowing cut prevents concurrent replacement/removal.
    pub(in crate::sumeragi) fn select(
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
pub(in crate::sumeragi) struct SelectedValidationCarrier<'a, P: CarrierValidator> {
    service: &'a mut RetainedBodyValidationService<P>,
    index: usize,
    owner: Option<P::Owner>,
}

impl<P: CarrierValidator> SelectedValidationCarrier<'_, P> {
    /// This callback must run the consuming publisher, which still requires its
    /// own exact Decision/source/storage authority. Custody grants none of it.
    /// A local refusal must return the same complete owner. Panic is fail-stop;
    /// the occupied row remains a tombstone and cannot trigger reexecution.
    pub(in crate::sumeragi) fn try_consume<R, E>(
        mut self,
        publish: impl FnOnce(P::Owner) -> Result<R, (P::Owner, E)>,
    ) -> Result<R, E> {
        let owner = self
            .owner
            .take()
            .expect("live selection retains its original owner");
        match publish(owner) {
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
