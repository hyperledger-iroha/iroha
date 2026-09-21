//! Joint acquisition of the original carrier journals, without publishing State.
//!
//! Every refusal returns the complete decided carrier after releasing all State
//! and Kura fences and component writers. Busy waits belong to the failed owner;
//! the caller awaits only after this synchronous attempt has returned.

use super::super::runtime_journals::{PreparedRuntimeJournals, RuntimePublicationError};
use super::{super::DetachedCarrierComponents, DecisionBoundCarrierJournals};
use crate::kura::{
    KuraPublicationLease, KuraPublicationPreparationError, KuraWsvCheckpointReceipt,
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

/// Exact local acquisition refusal; this never invalidates a consensus decision.
pub(in crate::state::carrier_preparation::journals) enum CarrierPhysicalPreparationError<E> {
    /// Complete installation capacity was refused before any physical acquisition.
    Admission(E),
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
    /// Retained archive persistence failed before the final joint lease.
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

impl<E: std::fmt::Debug> std::fmt::Debug for CarrierPhysicalPreparationError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => f.debug_tuple("Admission").field(error).finish(),
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
struct SourceAuthenticatedCarrier<'target, Admission, BindingAdmission> {
    decision: DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    >,
    kura: KuraPublicationLease<'target>,
}

impl<'target, Admission, BindingAdmission>
    SourceAuthenticatedCarrier<'target, Admission, BindingAdmission>
{
    fn try_new<E>(
        decision: DecisionBoundCarrierJournals<
            Admission,
            BindingAdmission,
            DetachedCarrierComponents,
            KuraWsvCheckpointReceipt,
        >,
        kura: KuraPublicationLease<'target>,
    ) -> Result<
        Self,
        (
            DecisionBoundCarrierJournals<
                Admission,
                BindingAdmission,
                DetachedCarrierComponents,
                KuraWsvCheckpointReceipt,
            >,
            CarrierPhysicalPreparationError<E>,
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
    ) -> DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    > {
        let Self { decision, kura } = self;
        drop(kura);
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
    fn try_acquire<E>(target: &'target State) -> Result<Self, CarrierPhysicalPreparationError<E>> {
        let acquire = |field, lock: &'target crate::publication_lock::PublicationMutex| {
            lock.try_lock_or_wait()
                .map_err(|wait| CarrierPhysicalPreparationError::Fence { field, wait })
        };
        let commit = acquire("state_commit_lock", &target.state_commit_lock)?;
        let lifecycle = acquire("lane_lifecycle_lock", &target.lane_lifecycle_lock)?;
        let write = acquire("state_write_lock", &target.state_write_lock)?;
        Ok(Self {
            _write: write,
            _lifecycle: lifecycle,
            _commit: commit,
        })
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
    _kura: [concread::release::DeferredRelease; 4],
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
/// The full carrier owns this group before its capture and binding reservations.
pub(in crate::state::carrier_preparation::journals) struct AcquiredCarrierComponents<'target> {
    original: Option<AcquiredCarrierParticipants<'target>>,
}

/// Original participants remain one owned unit until publication or joint abort.
pub(in crate::state::carrier_preparation::journals) struct AcquiredCarrierParticipants<'target> {
    world: PreparedWorld<'target, (), ()>,
    runtime: PreparedRuntimeJournals<'target, (), ()>,
    transactions: PreparedDetachedTransactionsBlock<'target, ()>,
    block_hashes: PreparedBlockHashes<'target, ()>,
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
        let fences;
        let Self {
            world,
            runtime,
            transactions,
            block_hashes,
            _fences: original_fences,
        } = self;
        fences = original_fences;
        let (world, world_retirement) = world.abort();
        let (runtime, runtime_retirement) = runtime.abort();
        let (transactions, transactions_retirement) = transactions.abort();
        let (block_hashes, block_hashes_retirement) = block_hashes.abort();
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
    BindingAdmission,
    Installation,
> {
    // The writers below belong to this exact State, never a caller-supplied replacement.
    target: &'target State,
    decision: DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        AcquiredCarrierComponents<'target>,
        KuraWsvCheckpointReceipt,
    >,
    // Drops after original values, all writers, State/Kura fences and prior admissions.
    installation: Installation,
}

impl<Admission, BindingAdmission>
    DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    >
{
    /// Admit installation, join the original Kura, then acquire Kura and State.
    /// Every physical probe is nonblocking and no source permission is minted.
    ///
    /// The required callback covers all component staging/COW copies, retained
    /// readers, publication identities, durable body/finality/checkpoint decoding
    /// and verification, both retained archive anchors and their result-bearing
    /// body authentication, original witness and archive persistence, final
    /// witness reauthentication, and acquisition/abort bookkeeping. There
    /// is no implicit production capacity policy. A failed attempt returns the
    /// exact block, verified artifact, journals, effects and prior reservations.
    pub(in crate::state::carrier_preparation::journals) fn try_prepare_physical<
        'target,
        Installation,
        E,
    >(
        self,
        target: &'target State,
        queue_source: Option<&OriginalCarrierQueue<'target>>,
        admit: impl FnOnce(&Self, &State) -> Result<Installation, E>,
    ) -> Result<
        PhysicallyPreparedCarrier<'target, Admission, BindingAdmission, Installation>,
        (Self, CarrierPhysicalPreparationError<E>),
    > {
        // Shadow the original after declaring installation: even an unwind in
        // an early probe drops every retained original before its reservation.
        let installation;
        let mut original = self;
        installation = match admit(&original, target) {
            Ok(guard) => guard,
            Err(error) => {
                return Err((original, CarrierPhysicalPreparationError::Admission(error)));
            }
        };
        if !target.matches_kura_instance(&original.journals.kura) {
            drop(installation);
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
            drop(installation);
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
                drop(installation);
                return Err((original, CarrierPhysicalPreparationError::Queue(error)));
            }
        }
        // Reject substituted execution or archive custody before any derived
        // persistence can modify its durable namespace. Release the temporary
        // lease before the continuations enter their own storage APIs; the final
        // lease below must authenticate all these same owners again.
        let kura = match target.kura.try_publication_lease() {
            Ok(lease) => lease,
            Err(error) => {
                drop(installation);
                return Err((original, CarrierPhysicalPreparationError::Kura(error)));
            }
        };
        original = match SourceAuthenticatedCarrier::try_new(original, kura) {
            Ok(owner) => owner.release(),
            Err((original, error)) => {
                drop(installation);
                return Err((original, error));
            }
        };
        if let Err(error) = original.publish_execution_witness() {
            use super::execution_witness_publication::CarrierExecutionWitnessPublicationError;
            let error = match error {
                CarrierExecutionWitnessPublicationError::Kura(error) => {
                    CarrierPhysicalPreparationError::Kura(error)
                }
                CarrierExecutionWitnessPublicationError::Checkpoint(error) => {
                    CarrierPhysicalPreparationError::Checkpoint(error)
                }
                CarrierExecutionWitnessPublicationError::Witness(error) => {
                    CarrierPhysicalPreparationError::ExecutionWitness(error)
                }
            };
            drop(installation);
            return Err((original, error));
        }
        if let Err(error) = original.publish_archives() {
            use super::archive_publication::CarrierArchivePublicationError;
            let error = match error {
                CarrierArchivePublicationError::Kura(error) => {
                    CarrierPhysicalPreparationError::Kura(error)
                }
                CarrierArchivePublicationError::Checkpoint(error) => {
                    CarrierPhysicalPreparationError::Checkpoint(error)
                }
                error => CarrierPhysicalPreparationError::Archive(error),
            };
            drop(installation);
            return Err((original, error));
        }
        // Borrow the exact target's Arc, never a self-referential field inside
        // the retained carrier. Identity equality above joins that same owner.
        let kura = match target.kura.try_publication_lease() {
            Ok(lease) => lease,
            Err(error) => {
                drop(installation);
                return Err((original, CarrierPhysicalPreparationError::Kura(error)));
            }
        };
        // Join the original source and checkpoint under all four Kura fences
        // before any State probe. The complete owner carries this authentication
        // only while that same lease remains held.
        let authenticated = match SourceAuthenticatedCarrier::try_new(original, kura) {
            Ok(owner) => owner,
            Err((original, error)) => {
                drop(installation);
                return Err((original, error));
            }
        };
        if let Err(error) = authenticated
            .kura
            .reauthenticate_execution_witness(authenticated.decision.finality.artifact())
        {
            let original = authenticated.release();
            drop(installation);
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
                    drop(installation);
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
            Err(error) => {
                drop(queue_observer);
                let original = authenticated.release();
                drop(installation);
                return Err((original, error));
            }
        };
        let queue = match queue_observer {
            Some(observer) => {
                let source = queue_source.expect("original service source remains borrowed");
                let acquired = observer
                    .try_into_cut()
                    .map_err(|error| CarrierQueueRetirementError::Busy {
                        field: error.field,
                        wait: error.wait,
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
                    Err(error) => {
                        drop(state);
                        let original = authenticated.release();
                        drop(installation);
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
        let binding_admission;
        let Self {
            checkpoint,
            finality,
            committed_event,
            journals,
            _binding_admission: original_binding_admission,
        } = original;
        binding_admission = original_binding_admission;
        let prepared = journals.try_map_components(|original| {
            let DetachedCarrierComponents {
                world,
                runtime,
                transactions,
                block_hashes,
            } = original;
            // Private execution has already released its hash writer. Acquire the
            // exact hash predecessor first, then every remaining component.
            let block_hashes = match block_hashes
                .try_prepare_publication(&target.block_hashes, |_, _| Ok::<_, Infallible>(()))
            {
                Ok(prepared) => prepared,
                Err((block_hashes, cause)) => {
                    drop(fences.release_for_completion());
                    return Err((
                        DetachedCarrierComponents {
                            world,
                            runtime,
                            transactions,
                            block_hashes,
                        },
                        CarrierPhysicalPreparationError::Component {
                            field: "block_hashes",
                            cause,
                        },
                    ));
                }
            };
            let transactions = match transactions
                .try_prepare_publication(&target.transactions, |_, _| Ok::<_, Infallible>(()))
            {
                Ok(prepared) => prepared,
                Err((transactions, cause)) => {
                    let (block_hashes, block_hashes_retirement) = block_hashes.abort();
                    drop(fences.release_for_completion());
                    drop(block_hashes_retirement);
                    return Err((
                        DetachedCarrierComponents {
                            world,
                            runtime,
                            transactions,
                            block_hashes,
                        },
                        CarrierPhysicalPreparationError::Component {
                            field: "transactions",
                            cause,
                        },
                    ));
                }
            };
            let runtime =
                match runtime.try_prepare_publication(target, |_, _| Ok::<_, Infallible>(())) {
                    Ok(prepared) => prepared,
                    Err((runtime, error, runtime_retirement)) => {
                        let (transactions, transactions_retirement) = transactions.abort();
                        let (block_hashes, block_hashes_retirement) = block_hashes.abort();
                        drop(fences.release_for_completion());
                        drop((
                            runtime_retirement,
                            transactions_retirement,
                            block_hashes_retirement,
                        ));
                        return Err((
                            DetachedCarrierComponents {
                                world,
                                runtime,
                                transactions,
                                block_hashes,
                            },
                            CarrierPhysicalPreparationError::Runtime(error),
                        ));
                    }
                };
            let world = match world
                .try_prepare_publication(&target.world, |_, _| Ok::<_, Infallible>(()))
            {
                Ok(prepared) => prepared,
                Err((world, error, world_retirement)) => {
                    let (runtime, runtime_retirement) = runtime.abort();
                    let (transactions, transactions_retirement) = transactions.abort();
                    let (block_hashes, block_hashes_retirement) = block_hashes.abort();
                    drop(fences.release_for_completion());
                    drop((
                        world_retirement,
                        runtime_retirement,
                        transactions_retirement,
                        block_hashes_retirement,
                    ));
                    return Err((
                        DetachedCarrierComponents {
                            world,
                            runtime,
                            transactions,
                            block_hashes,
                        },
                        CarrierPhysicalPreparationError::World(error),
                    ));
                }
            };
            Ok(AcquiredCarrierComponents {
                original: Some(AcquiredCarrierParticipants {
                    world,
                    runtime,
                    transactions,
                    block_hashes,
                    _fences: fences,
                }),
            })
        });
        macro_rules! retain {
            ($journals:expr) => {
                DecisionBoundCarrierJournals {
                    checkpoint,
                    finality,
                    committed_event,
                    journals: $journals,
                    _binding_admission: binding_admission,
                }
            };
        }
        match prepared {
            Ok(journals) => Ok(PhysicallyPreparedCarrier {
                target,
                decision: retain!(journals),
                installation,
            }),
            Err((journals, error)) => {
                // All partially acquired writers, State fences and Kura lease are gone.
                drop(installation);
                Err((retain!(journals), error))
            }
        }
    }
}

impl<Admission, BindingAdmission, Installation>
    PhysicallyPreparedCarrier<'_, Admission, BindingAdmission, Installation>
{
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
    ) -> DecisionBoundCarrierJournals<
        Admission,
        BindingAdmission,
        DetachedCarrierComponents,
        KuraWsvCheckpointReceipt,
    > {
        let installation;
        let binding_admission;
        let Self {
            target: _,
            decision,
            installation: original_installation,
        } = self;
        installation = original_installation;
        let DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
            _binding_admission: original_binding_admission,
        } = decision;
        binding_admission = original_binding_admission;
        let journals = journals.try_map_components(|components| {
            Ok::<_, (AcquiredCarrierComponents<'_>, Infallible)>(components.abort())
        });
        let journals = match journals {
            Ok(journals) => journals,
            Err((_, never)) => match never {},
        };
        drop(installation);
        DecisionBoundCarrierJournals {
            checkpoint,
            finality,
            committed_event,
            journals,
            _binding_admission: binding_admission,
        }
    }
}

#[path = "publication.rs"]
mod publication;
pub(crate) use publication::PublishedNativeApply;

#[cfg(test)]
#[path = "physical_publication_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "governance_fixture.rs"]
mod governance_fixture;
#[cfg(test)]
pub(crate) use governance_fixture::publish_governance_fixture;
