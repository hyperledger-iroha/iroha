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
use crate::state::{
    State,
    block_hashes_publication::PreparedBlockHashes,
    storage_transactions::PreparedDetachedTransactionsBlock,
    world_journals::publication::{PreparedWorld, WorldPublicationError},
};
use std::convert::Infallible;

/// Exact local acquisition refusal; this never invalidates a consensus decision.
pub(in crate::state::carrier_preparation::journals) enum CarrierPhysicalPreparationError<E> {
    /// Complete installation capacity was refused before any physical acquisition.
    Admission(E),
    /// Equal bytes or paths cannot replace the captured original Kura owner.
    ForeignKura,
    /// The original Kura is busy or requires storage repair before acquisition.
    Kura(KuraPublicationPreparationError),
    /// The retained durable checkpoint/finality no longer matches its original owner.
    Checkpoint(crate::kura::Error),
    /// The original execution witness could not be persisted or reauthenticated.
    ExecutionWitness(crate::kura::Error),
    /// Retained archive persistence failed before the final joint lease.
    Archive(super::archive_publication::CarrierArchivePublicationError),
    /// The named original State fence must release before another attempt.
    Fence {
        /// State lock which prevented acquisition.
        field: &'static str,
        /// Observation captured before probing that actual lock.
        wait: mv::ReleaseWait,
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
            Self::Kura(error) => f.debug_tuple("Kura").field(error).finish(),
            Self::Checkpoint(error) => f.debug_tuple("Checkpoint").field(error).finish(),
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
    _kura: KuraPublicationLease<'target>,
}

impl<'target> CarrierFences<'target> {
    /// Keep serialization of Apply while releasing every physical writer/fence
    /// needed by derived persistence and cache readers after visibility changes.
    fn release_for_completion(self) -> PublicationGuard<'target> {
        let Self {
            _state: state,
            _kura: kura,
        } = self;
        let StateFences {
            _write: write,
            _lifecycle: lifecycle,
            _commit: commit,
        } = state;
        drop(write);
        drop(lifecycle);
        drop(kura);
        commit
    }
}

/// All original component writers and State/Kura fences, with no independent
/// authority to publish outside the complete carrier consumer.
/// The full carrier owns this group before its capture and binding reservations.
pub(in crate::state::carrier_preparation::journals) struct AcquiredCarrierComponents<'target> {
    world: PreparedWorld<'target, (), ()>,
    runtime: PreparedRuntimeJournals<'target, (), ()>,
    transactions: PreparedDetachedTransactionsBlock<'target, ()>,
    block_hashes: PreparedBlockHashes<'target, ()>,
    _fences: CarrierFences<'target>,
}

impl AcquiredCarrierComponents<'_> {
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
        let world = world.abort();
        let runtime = runtime.abort();
        let transactions = transactions.abort();
        let block_hashes = block_hashes.abort();
        drop(fences);
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
/// The private terminal consumer refuses outstanding geometry and participant
/// durability work. TODO: complete those owners and production resource admission.
/// Acquiring these writers neither advances State visibility nor grants finality,
/// retirement or Kura permission. No physical guard may cross an async wait.
#[must_use = "keep the complete carrier until authorized publication or abort"]
pub(in crate::state::carrier_preparation::journals) struct PhysicallyPreparedCarrier<
    'target,
    Admission,
    BindingAdmission,
    Installation,
> {
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
    /// and verification, original execution-witness staging/promotion and final
    /// proof authentication, and acquisition/abort bookkeeping. There
    /// is no implicit production capacity policy. A failed attempt returns the
    /// exact block, verified artifact, journals, effects and prior reservations.
    pub(in crate::state::carrier_preparation::journals) fn try_prepare_physical<
        'target,
        Installation,
        E,
    >(
        self,
        target: &'target State,
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
        // Authenticate under the original four Kura fences, before any State
        // probe. Equal bytes from another Kura or a replaced checkpoint cannot
        // grant publication. A storage refusal is not a lock-release dependency.
        if let Err(error) = kura.reauthenticate_checkpoint(
            &original.checkpoint,
            original.finality.artifact(),
            original.journals.checkpoint,
        ) {
            drop(kura);
            drop(installation);
            return Err((original, CarrierPhysicalPreparationError::Checkpoint(error)));
        }
        if let Err(error) = kura.reauthenticate_execution_witness(original.finality.artifact()) {
            drop(kura);
            drop(installation);
            return Err((
                original,
                CarrierPhysicalPreparationError::ExecutionWitness(error),
            ));
        }
        let state = match StateFences::try_acquire(target) {
            Ok(fences) => fences,
            Err(error) => {
                drop(kura);
                drop(installation);
                return Err((original, error));
            }
        };
        let fences = CarrierFences {
            _state: state,
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
            // Validation takes a hash read snapshot before World writers. Probe
            // its writer first, so a retained snapshot cannot create a cycle.
            let block_hashes = match block_hashes
                .try_prepare_publication(&target.block_hashes, |_, _| Ok::<_, Infallible>(()))
            {
                Ok(prepared) => prepared,
                Err((block_hashes, cause)) => {
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
                    let block_hashes = block_hashes.abort();
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
                    Err((runtime, error)) => {
                        let transactions = transactions.abort();
                        let block_hashes = block_hashes.abort();
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
                Err((world, error)) => {
                    let runtime = runtime.abort();
                    let transactions = transactions.abort();
                    let block_hashes = block_hashes.abort();
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
                world,
                runtime,
                transactions,
                block_hashes,
                _fences: fences,
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

#[cfg(test)]
#[path = "physical_publication_tests.rs"]
mod tests;
