//! Joint acquisition of the original carrier journals, without publishing State.
//!
//! Every refusal returns the complete decided carrier after releasing all State
//! and Kura fences and component writers. Busy waits belong to the failed owner;
//! the caller awaits only after this synchronous attempt has returned.

use super::super::runtime_journals::{PreparedRuntimeJournals, RuntimePublicationError};
use super::{super::DetachedCarrierComponents, DecisionBoundCarrierJournals};
use crate::kura::{
    KuraPublicationCleanup, KuraPublicationLease, KuraPublicationPreparationError,
    KuraWsvCheckpointReceipt,
};
use crate::publication_lock::PublicationGuard;
use crate::state::carrier_preparation::queue_retirement::{
    CarrierQueueRetirement, CarrierQueueRetirementError, ReleasedCarrierQueue,
};
use crate::state::{
    State,
    block_hashes_publication::PreparedBlockHashes,
    storage_transactions::PreparedDetachedTransactionsBlock,
    world_journals::publication::{PreparedWorld, WorldPublicationError},
};
use crate::sumeragi::v2_apply::carrier_queue_retirement::OriginalCarrierQueue;
use std::convert::Infallible;

#[path = "participant_preparation.rs"]
mod participant_preparation;
use participant_preparation::CarrierPreparation;

/// Exact local acquisition refusal; this never invalidates a consensus decision.
pub(in crate::state::carrier_preparation::journals) enum CarrierPhysicalPreparationError {
    /// Equal bytes or paths cannot replace the captured original Kura owner.
    ForeignKura,
    /// The target State or block header differs from the captured original geometry.
    ForeignTarget,
    /// Original service Queue identity, pending work, or recovery prevents publication.
    Queue(CarrierQueueRetirementError),
    /// The original Kura is busy or requires storage repair before acquisition.
    Kura(KuraPublicationPreparationError),
    /// The retained durable checkpoint/finality no longer matches its original owner.
    Checkpoint(crate::kura::Error),
    /// The retained execution or Native source differs from exact durable evidence.
    Source(super::super::super::execution_prefix::CarrierSourceAuthenticationError),
    /// The original provider capture does not join this exact durable carrier.
    Provider(crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1),
    /// The original reputation capture does not join this exact durable carrier.
    Reputation(crate::query::reputation_finalized::ReputationFinalizedArchiveError),
    /// The original execution witness could not be persisted or reauthenticated.
    ExecutionWitness(crate::kura::Error),
    /// Retained archive persistence failed under the original joint lease.
    Archive(super::archive_publication::CarrierArchivePublicationError),
    /// The named original State fence must release before another attempt.
    Fence {
        /// State lock which prevented acquisition.
        field: &'static str,
        /// Observation captured before probing that actual lock.
        wait: concread::release::ReleaseWait,
    },
    /// Hash or membership storage could not retain its exact original writer.
    Component {
        /// Original storage family which refused acquisition.
        field: &'static str,
        /// Busy, changed or poisoned physical owner.
        cause: mv::PublicationPreparationError<Infallible>,
    },
    /// One of the four original runtime cells refused acquisition.
    Runtime(RuntimePublicationError<Infallible>),
    /// One of the complete World inventory's original writers refused acquisition.
    World(WorldPublicationError<Infallible>),
}

impl std::fmt::Debug for CarrierPhysicalPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignKura => f.write_str("ForeignKura"),
            Self::ForeignTarget => f.write_str("ForeignTarget"),
            Self::Queue(error) => f.debug_tuple("Queue").field(error).finish(),
            Self::Kura(error) => f.debug_tuple("Kura").field(error).finish(),
            Self::Checkpoint(error) => f.debug_tuple("Checkpoint").field(error).finish(),
            Self::Source(error) => f.debug_tuple("Source").field(error).finish(),
            Self::Provider(error) => f.debug_tuple("Provider").field(error).finish(),
            Self::Reputation(error) => f.debug_tuple("Reputation").field(error).finish(),
            Self::ExecutionWitness(error) => {
                f.debug_tuple("ExecutionWitness").field(error).finish()
            }
            Self::Archive(error) => f.debug_tuple("Archive").field(error).finish(),
            Self::Fence { field, wait } => f
                .debug_struct("Fence")
                .field("field", field)
                .field("wait", wait)
                .finish(),
            Self::Component { field, cause } => f
                .debug_struct("Component")
                .field("field", field)
                .field("cause", cause)
                .finish(),
            Self::Runtime(error) => f.debug_tuple("Runtime").field(error).finish(),
            Self::World(error) => f.debug_tuple("World").field(error).finish(),
        }
    }
}

/// The whole original decided execution joined to durable sources under its lease.
/// No standalone proof escapes: releasing the lease returns only the original
/// unauthenticated decision, so every later attempt must reauthenticate it.
struct SourceAuthenticatedCarrier<'target, Admission> {
    // Unlock the complete physical boundary before original values/admission
    // can run cleanup callbacks during abandonment or unwind.
    kura: KuraPublicationLease<'target>,
    decision: DecisionBoundCarrierJournals<
        Admission,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    >,
}

impl<'target, Admission> SourceAuthenticatedCarrier<'target, Admission> {
    fn try_new(
        decision: DecisionBoundCarrierJournals<
            Admission,
            DetachedCarrierComponents,
            KuraWsvCheckpointReceipt,
        >,
        kura: KuraPublicationLease<'target>,
    ) -> Result<
        Self,
        (
            DecisionBoundCarrierJournals<
                Admission,
                DetachedCarrierComponents,
                KuraWsvCheckpointReceipt,
            >,
            CarrierPhysicalPreparationError,
        ),
    > {
        let owner = Self { decision, kura };
        let result = (|| {
            let original = &owner.decision;
            owner
                .kura
                .reauthenticate_checkpoint(
                    &original.checkpoint,
                    original.finality.artifact(),
                    original.journals.checkpoint,
                )
                .map_err(CarrierPhysicalPreparationError::Checkpoint)?;
            original
                .journals
                .source_prefix
                .authenticate_durable_carrier(
                    original.block(),
                    &original.journals.context,
                    &original.journals.execution_prefix,
                    &owner.kura,
                )
                .map_err(CarrierPhysicalPreparationError::Source)?;
            // Standalone archive readers would reacquire Kura and deadlock.
            // Join both retained anchors under this exact source boundary.
            if let Some(capture) = original.journals.provider_capture.as_ref() {
                capture
                    .reauthenticate_under_publication_lease(
                        &owner.kura,
                        original.checkpoint.finality_receipt(),
                    )
                    .map_err(CarrierPhysicalPreparationError::Provider)?;
            }
            if let Some(capture) = original.journals.reputation_capture.as_ref() {
                capture
                    .reauthenticate_under_publication_lease(
                        &owner.kura,
                        original.checkpoint.finality_receipt(),
                    )
                    .map_err(CarrierPhysicalPreparationError::Reputation)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(owner),
            Err(error) => Err((owner.release(), error)),
        }
    }

    fn release(
        self,
    ) -> DecisionBoundCarrierJournals<Admission, DetachedCarrierComponents, KuraWsvCheckpointReceipt>
    {
        let Self { decision, kura } = self;
        drop(kura.release_deferred());
        decision
    }
}

/// Original State fences, acquired without waiting and released after writers.
struct StateFences<'target> {
    _write: PublicationGuard<'target>,
    _lifecycle: PublicationGuard<'target>,
    _commit: PublicationGuard<'target>,
}

impl<'target> StateFences<'target> {
    fn try_acquire(
        target: &'target State,
    ) -> Result<
        Self,
        (
            CarrierPhysicalPreparationError,
            [Option<concread::release::DeferredRelease>; 3],
        ),
    > {
        let acquire = |field, lock: &'target crate::publication_lock::PublicationMutex| {
            lock.try_lock_or_wait()
                .map_err(|wait| CarrierPhysicalPreparationError::Fence { field, wait })
        };
        let commit = acquire("state_commit_lock", &target.state_commit_lock)
            .map_err(|error| (error, [None, None, None]))?;
        let lifecycle = match acquire("lane_lifecycle_lock", &target.lane_lifecycle_lock) {
            Ok(guard) => guard,
            Err(error) => return Err((error, [None, None, Some(commit.release_deferred())])),
        };
        let write = match acquire("state_write_lock", &target.state_write_lock) {
            Ok(guard) => guard,
            Err(error) => {
                let lifecycle = lifecycle.release_deferred();
                let commit = commit.release_deferred();
                return Err((error, [None, Some(lifecycle), Some(commit)]));
            }
        };
        Ok(Self {
            _write: write,
            _lifecycle: lifecycle,
            _commit: commit,
        })
    }

    /// Release every acquired State fence while retaining all original callbacks.
    fn release_deferred(self) -> [concread::release::DeferredRelease; 3] {
        [
            self._write.release_deferred(),
            self._lifecycle.release_deferred(),
            self._commit.release_deferred(),
        ]
    }
}

/// Physical State ownership drops before its enclosing original Kura boundary.
struct CarrierFences<'target> {
    _state: StateFences<'target>,
    _queue: Option<CarrierQueueRetirement<'target>>,
    _kura: KuraPublicationLease<'target>,
}

/// Apply serialization with cleanup from already unlocked physical fences.
/// Commit unlocks first on ordinary drop and unwind, before any retained callback.
struct CompletionFences<'target> {
    _commit: PublicationGuard<'target>,
    _state: [concread::release::DeferredRelease; 2],
    _queue: Option<ReleasedCarrierQueue>,
    _kura: KuraPublicationCleanup,
}

impl<'target> CarrierFences<'target> {
    /// Keep serialization of Apply while releasing every physical writer/fence
    /// needed by derived persistence and cache readers after visibility changes.
    fn release_for_completion(self) -> CompletionFences<'target> {
        let Self {
            _state: state,
            _queue: queue,
            _kura: kura,
        } = self;
        let StateFences {
            _write: write,
            _lifecycle: lifecycle,
            _commit: commit,
        } = state;
        let state = [write.release_deferred(), lifecycle.release_deferred()];
        let queue = queue.map(CarrierQueueRetirement::release_deferred);
        let kura = kura.release_deferred();
        CompletionFences {
            _commit: commit,
            _state: state,
            _queue: queue,
            _kura: kura,
        }
    }
}

/// All original component writers and State/Kura fences, with no independent
/// authority to publish outside the complete carrier consumer.
/// The full carrier releases these writers before refunding its shell reservation.
pub(in crate::state::carrier_preparation::journals) struct AcquiredCarrierComponents<'target> {
    original: Option<AcquiredCarrierParticipants<'target>>,
}

/// Original participants remain one owned unit until publication or joint abort.
pub(in crate::state::carrier_preparation::journals) struct AcquiredCarrierParticipants<'target> {
    world: PreparedWorld<'target, (), ()>,
    runtime: PreparedRuntimeJournals<'target, (), ()>,
    transactions: PreparedDetachedTransactionsBlock<'target, ()>,
    block_hashes: PreparedBlockHashes<'target, ()>,
    effect_locks: crate::state::effect_publication::StateEffectLocks<'target>,
    _fences: CarrierFences<'target>,
}

impl<'target> AcquiredCarrierComponents<'target> {
    fn into_original(mut self) -> AcquiredCarrierParticipants<'target> {
        self.original.take().expect("original carrier participants")
    }

    fn abort(self) -> DetachedCarrierComponents {
        self.into_original().abort()
    }
}

impl<'target> std::ops::Deref for AcquiredCarrierComponents<'target> {
    type Target = AcquiredCarrierParticipants<'target>;
    fn deref(&self) -> &Self::Target {
        self.original
            .as_ref()
            .expect("original carrier participants")
    }
}

impl Drop for AcquiredCarrierComponents<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            // Joint abort unlocks every participant and enclosing fence before
            // destroying any returned journal or invoking its original callbacks.
            drop(original.abort());
        }
    }
}

impl AcquiredCarrierParticipants<'_> {
    fn abort(self) -> DetachedCarrierComponents {
        // Reverse local drop order also keeps fences behind all components if
        // abort bookkeeping unwinds before the explicit release below.
        let mut effect_cleanup;
        let fences;
        let Self {
            world,
            runtime,
            transactions,
            block_hashes,
            effect_locks: original_effect_locks,
            _fences: original_fences,
        } = self;
        fences = original_fences;
        effect_cleanup = original_effect_locks;
        let mut effect_locks = effect_cleanup.physical_scope();
        let (world, world_retirement) = world.abort();
        let (runtime, runtime_retirement) = runtime.abort();
        let (transactions, transactions_retirement) = transactions.abort();
        let (block_hashes, block_hashes_retirement) = block_hashes.abort();
        effect_locks.release_writers();
        drop(fences.release_for_completion());
        drop((
            world_retirement,
            runtime_retirement,
            transactions_retirement,
            block_hashes_retirement,
        ));
        DetachedCarrierComponents {
            world,
            runtime,
            transactions,
            block_hashes,
        }
    }
}

/// A complete decided carrier holding every original storage writer together.
///
/// The private terminal consumer completes retained geometry while keeping its
/// original service Queue retirement cut through visibility. TODO: join complete
/// participant durability and production resource admission.
/// Acquiring these writers neither advances State visibility nor grants finality,
/// retirement or Kura permission. No physical guard may cross an async wait.
#[must_use = "keep the complete carrier until authorized publication or abort"]
pub(in crate::state::carrier_preparation::journals) struct PhysicallyPreparedCarrier<
    'target,
    Admission,
> {
    // The writers below belong to this exact State, never a caller-supplied replacement.
    target: &'target State,
    decision: DecisionBoundCarrierJournals<
        Admission,
        AcquiredCarrierComponents<'target>,
        KuraWsvCheckpointReceipt,
    >,
}

impl<Admission>
    DecisionBoundCarrierJournals<Admission, DetachedCarrierComponents, KuraWsvCheckpointReceipt>
{
    /// Join the original Kura and acquire every State writer without waiting.
    /// The original capture reservation retains the named shell allocations.
    /// Durable decoding and component COW use standard allocation; this method
    /// makes no claim of complete process-memory admission. Every refusal returns
    /// the original detached decision after releasing its physical writers.
    pub(in crate::state::carrier_preparation::journals) fn try_prepare_physical<'target>(
        self,
        target: &'target State,
        queue_source: Option<&OriginalCarrierQueue<'target>>,
    ) -> Result<
        PhysicallyPreparedCarrier<'target, Admission>,
        (Self, CarrierPhysicalPreparationError),
    > {
        let original = self;
        if !target.matches_kura_instance(&original.journals.kura) {
            return Err((original, CarrierPhysicalPreparationError::ForeignKura));
        }
        // The original geometry pins the State identity as well as its header.
        // Reject another State sharing this Kura before any durable side effect;
        // physical predecessor and terminal geometry checks still follow below.
        if !original
            .journals
            .geometry
            .matches_publication_target(target, original.block().header())
        {
            return Err((original, CarrierPhysicalPreparationError::ForeignTarget));
        }
        if original.journals.geometry.requires_queue_custody() {
            let refusal = match queue_source {
                None => Some(CarrierQueueRetirementError::Missing),
                Some(source) if !source.belongs_to(target) => {
                    Some(CarrierQueueRetirementError::ForeignState)
                }
                Some(_) => None,
            };
            if let Some(error) = refusal {
                return Err((original, CarrierPhysicalPreparationError::Queue(error)));
            }
        }
        // Retain the same original Kura fences from source authentication through
        // witness/archive persistence and State acquisition. Releasing between
        // phases lets queued readers repeatedly preempt the next try-only probe.
        let kura = match target.kura.try_publication_lease() {
            Ok(lease) => lease,
            Err(error) => {
                return Err((original, CarrierPhysicalPreparationError::Kura(error)));
            }
        };
        let authenticated = SourceAuthenticatedCarrier::try_new(original, kura)?;
        authenticated.try_prepare(target, queue_source)
    }
}

impl<'target, Admission> SourceAuthenticatedCarrier<'target, Admission> {
    /// Continue under the original lease; no intermediate phase reacquires Kura.
    fn try_prepare(
        mut self,
        target: &'target State,
        queue_source: Option<&OriginalCarrierQueue<'target>>,
    ) -> Result<
        PhysicallyPreparedCarrier<'target, Admission>,
        (
            DecisionBoundCarrierJournals<
                Admission,
                DetachedCarrierComponents,
                KuraWsvCheckpointReceipt,
            >,
            CarrierPhysicalPreparationError,
        ),
    > {
        if let Err(error) = self.decision.publish_execution_witness(&self.kura) {
            use super::execution_witness_publication::CarrierExecutionWitnessPublicationError;
            let error = match error {
                CarrierExecutionWitnessPublicationError::Checkpoint(error) => {
                    CarrierPhysicalPreparationError::Checkpoint(error)
                }
                CarrierExecutionWitnessPublicationError::Witness(error) => {
                    CarrierPhysicalPreparationError::ExecutionWitness(error)
                }
            };
            return Err((self.release(), error));
        }
        if let Err(error) = self.decision.publish_archives(&self.kura) {
            use super::archive_publication::CarrierArchivePublicationError;
            let error = match error {
                CarrierArchivePublicationError::Checkpoint(error) => {
                    CarrierPhysicalPreparationError::Checkpoint(error)
                }
                error => CarrierPhysicalPreparationError::Archive(error),
            };
            return Err((self.release(), error));
        }
        let authenticated = self;
        if let Err(error) = authenticated
            .kura
            .reauthenticate_execution_witness(authenticated.decision.finality.artifact())
        {
            let original = authenticated.release();

            return Err((
                original,
                CarrierPhysicalPreparationError::ExecutionWitness(error),
            ));
        }
        // Queue transition ownership precedes lifecycle; all later probes are
        // try-only because ordinary ingress may own a State view before Queue.
        let queue_observer = if authenticated
            .decision
            .journals
            .geometry
            .requires_queue_custody()
        {
            let source = queue_source.expect("required original source checked before persistence");
            match source.try_observe() {
                Ok(observer) => Some(observer),
                Err(wait) => {
                    let original = authenticated.release();

                    return Err((
                        original,
                        CarrierPhysicalPreparationError::Queue(CarrierQueueRetirementError::Busy {
                            field: "lane_reservation_transition_lock",
                            wait,
                        }),
                    ));
                }
            }
        } else {
            None
        };
        let state = match StateFences::try_acquire(target) {
            Ok(fences) => fences,
            Err((error, state_retirement)) => {
                let queue_retirement = queue_observer.map(|observer| observer.release_deferred());
                let SourceAuthenticatedCarrier {
                    decision: original,
                    kura,
                } = authenticated;
                let kura_retirement = kura.release_deferred();
                drop((state_retirement, queue_retirement, kura_retirement));

                return Err((original, error));
            }
        };
        let queue = match queue_observer {
            Some(observer) => {
                let source = queue_source.expect("original service source remains borrowed");
                let acquired = observer
                    .try_into_cut()
                    .map_err(|(error, cleanup)| {
                        (
                            CarrierQueueRetirementError::Busy {
                                field: error.field,
                                wait: error.wait,
                            },
                            cleanup,
                        )
                    })
                    .and_then(|cut| {
                        CarrierQueueRetirement::try_new(
                            target,
                            &authenticated.decision.journals.geometry,
                            authenticated.decision.block().header(),
                            source,
                            cut,
                        )
                    });
                match acquired {
                    Ok(cut) => Some(cut),
                    Err((error, queue_retirement)) => {
                        let state_retirement = state.release_deferred();
                        let SourceAuthenticatedCarrier {
                            decision: original,
                            kura,
                        } = authenticated;
                        let kura_retirement = kura.release_deferred();
                        drop((queue_retirement, state_retirement, kura_retirement));

                        return Err((original, CarrierPhysicalPreparationError::Queue(error)));
                    }
                }
            }
            None => None,
        };
        let SourceAuthenticatedCarrier {
            decision: original,
            kura,
        } = authenticated;
        let fences = CarrierFences {
            _state: state,
            _queue: queue,
            _kura: kura,
        };

        let DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
        } = original;

        let prepared = journals.try_map_components(|original| {
            let mut preparation = CarrierPreparation::new(original, target, fences);
            match preparation.try_prepare() {
                Ok(()) => Ok(preparation.into_prepared()),
                Err(error) => {
                    let original = preparation.recover_original();
                    drop(preparation);
                    Err((original, error))
                }
            }
        });
        macro_rules! retain {
            ($journals:expr) => {
                DecisionBoundCarrierJournals {
                    checkpoint,
                    finality,
                    committed_event,
                    journals: $journals,
                }
            };
        }
        match prepared {
            Ok(journals) => Ok(PhysicallyPreparedCarrier {
                target,
                decision: retain!(journals),
            }),
            Err((journals, error)) => {
                // All partially acquired writers, State fences and Kura lease are gone.

                Err((retain!(journals), error))
            }
        }
    }
}

impl<Admission> PhysicallyPreparedCarrier<'_, Admission> {
    /// Complete the original storage transition under its retained Queue custody. The terminal
    /// publisher checks exact source and lifecycle authority before calling this.
    /// No retry state or replacement descriptor is introduced here; local refusal
    /// is returned to that publisher, which releases all writers with the owner.
    fn try_complete_geometry(&mut self) -> Result<bool, crate::state::LaneLifecycleError> {
        let journals = &mut self.decision.journals;
        if !journals.geometry.requires_storage_transition() {
            return Ok(false);
        }
        let mut backend = self
            .target
            .tiered_backend
            .try_lock_or_wait()
            .map_err(|wait| crate::state::LaneLifecycleError::PublicationBusy {
                field: "tiered_backend",
                wait,
            })?;
        journals
            .geometry
            .prepare_under(&backend, &journals.components._fences._kura)?;
        let completed = journals.geometry.complete_under(
            self.target,
            journals.effects.header,
            &mut backend,
            &journals.components._fences._kura,
            journals.components._fences._queue.as_ref(),
        )?;
        Ok(completed.updated_da_mapping().is_some())
    }

    /// Release all physical ownership and return the complete original decision.
    pub(in crate::state::carrier_preparation::journals) fn abort(
        self,
    ) -> DecisionBoundCarrierJournals<Admission, DetachedCarrierComponents, KuraWsvCheckpointReceipt>
    {
        let Self {
            target: _,
            decision,
        } = self;

        let DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
        } = decision;

        let journals = journals.try_map_components(|components| {
            Ok::<_, (AcquiredCarrierComponents<'_>, Infallible)>(components.abort())
        });
        let journals = match journals {
            Ok(journals) => journals,
            Err((_, never)) => match never {},
        };

        DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
        }
    }
}

#[path = "publication.rs"]
pub(super) mod publication;
pub(crate) use publication::{PublishedCarrier, PublishedNativeApply};

#[cfg(test)]
#[path = "physical_publication_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "governance_fixture.rs"]
mod governance_fixture;
#[cfg(test)]
pub(crate) use governance_fixture::publish_governance_fixture;
