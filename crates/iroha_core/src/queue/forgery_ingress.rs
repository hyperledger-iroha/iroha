//! One original Queue's native SCCP ingress: cheap source checks, state dedup,
//! charged LRU4096, 8/s burst32, then exactly one native pairing (§4.9).
//!
//! Cache membership is local denial-of-service state, never recorded evidence or
//! execution authority. The cache key binds the aggregate signature, so a submission
//! with a refused signature never shadows the genuine certificate of the same
//! `(g, m, signers)`. A key is remembered only once a pairing token is taken, and
//! forgotten again after a local refusal: neither a rate refusal nor local resource
//! exhaustion is a verdict on the evidence, so neither may deny a later genuine retry.
//!
//! The cache and the bucket throttle remote submissions only. A distributed flood of
//! fabricated evidence passes the cheap checks and could keep the shared bucket empty;
//! the node's own keeper, which verifies each certificate before it submits, therefore
//! skips both and still pays the full pairing ([`SubmissionOrigin::LocalKeeper`]).

use std::time::Instant;

use super::{Error, Queue};
use crate::{
    smartcontracts::isi::sccp::{
        admission::{SccpAdmissionKeysV1, SccpExemptClassV1},
        forgery::{self, ForgeryVerificationError},
    },
    state::{StateReadOnly, StateView, sccp_forgery_custody::ForgeryStorageError},
};
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError};
use iroha_data_model::{
    isi::sccp::SubmitSccpForgeryEvidenceV1,
    transaction::{Executable, SignedTransaction},
};

const CACHE_CAPACITY: usize = 4_096;
const TOKEN_NS: u64 = 125_000_000;
const BURST_NS: u64 = 32 * TOKEN_NS;

/// Typed native ingress refusal. This local result never certifies an offence.
#[derive(Debug, thiserror::Error)]
#[allow(
    variant_size_differences,
    reason = "retain original resource refusal owners inline"
)]
pub enum ForgeryIngressError {
    /// Ordered claim/generation/window/mismatch or native pairing refusal.
    #[error(transparent)]
    Verification(#[from] ForgeryVerificationError),
    /// The original permanent-offence query could not complete.
    #[error(transparent)]
    Offence(#[from] ForgeryStorageError),
    /// Original finite allocation pool refused the cache or scratch.
    #[error(transparent)]
    Allocation(#[from] AllocationRefusal),
    /// Physical cache backing allocation failed after original admission.
    #[error("SCCP ingress cache allocation unavailable ({bytes} bytes)")]
    Allocator {
        /// Exact requested backing bytes.
        bytes: usize,
    },
    /// A Queue cannot inherit a second State's finite allocation pool.
    #[error("SCCP ingress cache belongs to a different original State pool")]
    PoolMismatch,
    /// No selected slashable G6 member is new and recorded evidence names the generation.
    #[error("SCCP evidence duplicate")]
    Duplicate,
    /// The exact (generation, message, signer bitmap, signature) was already seen.
    #[error("SCCP forgery submission already seen")]
    Seen,
    /// The node-wide pairing token bucket has no token.
    #[error("SCCP forgery pairing rate limit")]
    RateLimited,
}

impl ForgeryIngressError {
    /// Whether this is local availability/custody failure, not bad evidence.
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            Self::Allocation(_)
                | Self::Allocator { .. }
                | Self::PoolMismatch
                | Self::Offence(_)
                | Self::Verification(
                    ForgeryVerificationError::Unavailable(_)
                        | ForgeryVerificationError::UnsupportedScope
                        | ForgeryVerificationError::Allocation(_)
                        | ForgeryVerificationError::PhysicalAllocation(_)
                        | ForgeryVerificationError::Cancelled
                        | ForgeryVerificationError::Deadline
                )
        )
    }

    /// Whether node-local repeat/rate state refused this attempt.
    #[must_use]
    pub const fn is_throttled(&self) -> bool {
        matches!(self, Self::Seen | Self::RateLimited)
    }

    fn into_queue(self) -> Error {
        match self {
            Self::Allocation(original @ AllocationRefusal::Capacity { .. })
            | Self::Verification(ForgeryVerificationError::Allocation(
                original @ AllocationRefusal::Capacity { .. },
            ))
            | Self::Offence(ForgeryStorageError::Storage(
                mv::storage::AdmittedStorageError::Allocation(
                    original @ AllocationRefusal::Capacity { .. },
                ),
            )) => Error::Deferred(original.into()),
            other => Error::SccpForgeryIngress(other),
        }
    }
}

/// Who submits a transaction to this node's queue. Only the forgery ingress reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubmissionOrigin {
    /// Torii, P2P gossip and every other source outside this node: the repeat cache
    /// and the node-wide pairing bucket apply.
    Remote,
    /// This node's own SCCP keeper (`Queue::push_local_keeper_with_state`). Its bounded
    /// watch paces it and it verifies each certificate first, so its evidence skips the
    /// cache and bucket that remote floods share. Every other check and the pairing stay.
    LocalKeeper,
}

/// The LRU key of one submission: the `(g, m, signers)` triple of spec §4.9 and the
/// SHA-256 of its aggregate signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressKey {
    generation: u64,
    message: [u8; 32],
    signers: u32,
    signature: [u8; 32],
}

/// The inline Queue mutex owns this exact charged fixed backing until Queue drop.
/// No route, peer, `StateView`, queue clear or tip change resets it.
pub(super) struct ForgeryIngressState {
    seen: ChargedBuffer<IngressKey>,
    last_refill: Instant,
    credit_ns: u64,
}

impl ForgeryIngressState {
    fn new(budget: &AllocationBudget, now: Instant) -> Result<Self, ForgeryIngressError> {
        let seen = ChargedBuffer::new(CACHE_CAPACITY, budget).map_err(|error| match error {
            ChargedBufferError::Admission(original) => ForgeryIngressError::Allocation(original),
            ChargedBufferError::Allocator { requested_bytes } => ForgeryIngressError::Allocator {
                bytes: requested_bytes,
            },
        })?;
        Ok(Self {
            seen,
            last_refill: now,
            credit_ns: BURST_NS,
        })
    }

    fn observe(&mut self, key: IngressKey, now: Instant) -> Result<(), ForgeryIngressError> {
        if let Some(index) = self.position(&key) {
            // Touching a repeat is an LRU access, but never consumes a pairing token.
            self.seen.as_mut_slice()[index..].rotate_left(1);
            return Err(ForgeryIngressError::Seen);
        }
        // Take the token BEFORE remembering the key: a rate refusal is no verdict on the
        // evidence, so it stays unseen and a later retry reaches the pairing.
        // Monotonic time prevents wall-clock rollback from minting tokens.
        let elapsed = now.saturating_duration_since(self.last_refill);
        if now >= self.last_refill {
            self.last_refill = now;
        }
        let refill = u64::try_from(elapsed.as_nanos())
            .unwrap_or(u64::MAX)
            .min(BURST_NS);
        self.credit_ns = self.credit_ns.saturating_add(refill).min(BURST_NS);
        if self.credit_ns < TOKEN_NS {
            return Err(ForgeryIngressError::RateLimited);
        }
        self.credit_ns -= TOKEN_NS;
        // Remember the key before the pairing, so concurrent exact repeats share this one
        // pairing. A refused signature stays seen under its own digest only.
        if self.seen.as_slice().len() == CACHE_CAPACITY {
            self.seen.as_mut_slice().rotate_left(1);
            self.seen.as_mut_slice()[CACHE_CAPACITY - 1] = key;
        } else {
            self.seen.push_reserved(key);
        }
        Ok(())
    }

    /// Forget `key` after a local refusal that was no verdict on the evidence.
    fn forget(&mut self, key: &IngressKey) {
        if let Some(index) = self.position(key) {
            self.seen.as_mut_slice()[index..].rotate_left(1);
            self.seen.pop();
        }
    }

    fn position(&self, key: &IngressKey) -> Option<usize> {
        self.seen.as_slice().iter().position(|seen| seen == key)
    }
}

impl Queue {
    fn observe_forgery(
        &self,
        key: IngressKey,
        budget: &AllocationBudget,
        now: Instant,
    ) -> Result<(), ForgeryIngressError> {
        // A failed allocation may refund synchronously. Keep original refund callbacks
        // deferred until this short cache guard releases, including every error path.
        budget.with_deferred_refund_notifications(|_| {
            let mut state = self.forgery_ingress.lock();
            if state.is_none() {
                *state = Some(ForgeryIngressState::new(budget, now)?);
            }
            let Some(state) = state.as_mut() else {
                return Err(ForgeryIngressError::PoolMismatch);
            };
            if !state.seen.belongs_to(budget) {
                return Err(ForgeryIngressError::PoolMismatch);
            }
            state.observe(key, now)
        })
    }

    /// Forget `key` after its pairing met a local refusal (§4.9 ingress).
    fn forget_forgery(&self, key: &IngressKey) {
        if let Some(state) = self.forgery_ingress.lock().as_mut() {
            state.forget(key);
        }
    }

    pub(super) fn admit_forgery(
        &self,
        view: &StateView<'_>,
        transaction: &SignedTransaction,
        origin: SubmissionOrigin,
    ) -> Result<SccpAdmissionKeysV1, Error> {
        self.admit_forgery_inner(view, transaction, origin)
            .map_err(ForgeryIngressError::into_queue)
    }

    fn admit_forgery_inner(
        &self,
        view: &StateView<'_>,
        transaction: &SignedTransaction,
        origin: SubmissionOrigin,
    ) -> Result<SccpAdmissionKeysV1, ForgeryIngressError> {
        let payload = transaction.payload();
        let evidence = match &payload.instructions {
            Executable::Instructions(instructions) => match instructions.as_ref() {
                [only] => only.as_any().downcast_ref::<SubmitSccpForgeryEvidenceV1>(),
                _ => None,
            },
            _ => None,
        }
        .ok_or(ForgeryVerificationError::NotEvidence)?;
        // Typed instruction arrays have exact lengths. prepare_committed checks parsed
        // claim, original Global source, generation/root, E0 and mismatch, in that order.
        let precheck = forgery::prepare_committed(view, evidence, payload.creation_time_ms)?;
        let budget = view.execution_budget();
        // Query exact original G6 tuples through the shared permanent-state reader and the
        // shared step-7 predicate. It funds every nested AccountId query copy; no writes.
        if forgery::new_offenders(
            view.world(),
            precheck.generation(),
            precheck.qc().signers,
            &budget,
        )?
        .is_none()
        {
            return Err(ForgeryIngressError::Duplicate);
        }
        let key = IngressKey {
            generation: precheck.generation().generation,
            message: *precheck.message(),
            signers: precheck.qc().signers,
            signature: iroha_crypto::sha256(evidence.aggregate_signature),
        };
        // Forgers can flood remote ingress to keep the shared bucket empty; the node's own
        // keeper must not queue behind them, so only remote submissions are throttled.
        let throttled = origin == SubmissionOrigin::Remote;
        if throttled {
            self.observe_forgery(key, &budget, Instant::now())?;
        }
        // No cache mutex spans cryptography; all observers share its preceding atomic
        // LRU/token decision. No invented read-worker deadline or replacement pool.
        if let Err(error) = forgery::verify_prechecked(&precheck, &budget) {
            let error = ForgeryIngressError::from(error);
            if throttled && error.is_local() {
                self.forget_forgery(&key);
            }
            return Err(error);
        }
        let parts = (key.generation, key.message, key.signers);
        Ok(SccpAdmissionKeysV1::new(SccpExemptClassV1::ForgeryEvidence)
            .with_exclusive(&parts)
            .with_content(&parts))
    }
}

#[cfg(test)]
mod tests;
