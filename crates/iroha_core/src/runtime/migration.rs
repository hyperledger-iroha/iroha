//! The kernel's migration engine (`specs/live_runtime_upgrades.md` §5.4).
//!
//! A migration moves the rows of old storage into successor storage while the chain keeps
//! committing. Three governed change sets drive it, and every step between them is bounded,
//! atomic and certified in the block that performs it:
//!
//! 1. **Start.** A change set installs the successor storage and activates the migration
//!    definition. Its record in `world.runtime_migrations` starts `Running`. At the start of
//!    every block one stage copies a bounded number of old rows after the committed cursor into
//!    the successor ([`run_stages`]). The old storage stays authoritative. At the end of every
//!    block the write barrier mirrors the old rows that block touched at or before the cursor
//!    ([`barrier`]), whichever code wrote them, so the staged part of the successor is exact.
//!    When no old row lies after the cursor the record is `Complete`; the barrier keeps holding.
//! 2. **Cutover.** A change set restricts the migration to completion only, together with the
//!    transitions that move the release's readers and writers to the successor. It activates
//!    only once the record is `Complete`. From its height the successor is authoritative, the
//!    barrier is lifted, and each block one stage drains a bounded number of old rows. A write
//!    that still reaches the old storage stops the drain (`StaleWrite`).
//! 3. **Retirement.** A change set retires the old storage and deactivates the migration. It
//!    activates only once the record is `Drained`, so the storage that leaves the commitment is
//!    empty.
//!
//! Governance suspends a running migration by closing its admission and resumes it by lifting
//! the restriction; the cursor and the barrier stay in place meanwhile. A stage that fails
//! faults the migration: no further stage runs, the barrier keeps holding, and a change set
//! that deactivates it and activates a replacement over the same storage and cursor hands the
//! cursor over.
//!
//! The engine's State is storage of the `iroha.runtime.lifecycle.v1` addition. A root that has
//! not activated it admits no migration and no storage retirement, and holds no record.

use iroha_data_model::runtime::{
    definition::{
        DefinitionDescriptorV1, DefinitionId, DefinitionLookup, MigrationDescriptorV1,
        RestrictionV1, RootSelectionV1,
    },
    lifecycle::{ChangeSetV1, RestrictionKindV1, SelectionTransitionV1},
    obligation::{MigrationRecordV1, MigrationStatusV1},
};
use mv::storage::StorageReadOnly as _;

use super::{catalog::additions::RUNTIME_LIFECYCLE, release_catalog};
use crate::state::{WorldBlock, WorldTransaction};

mod scratch;
pub mod storage;

/// A migration result preserving original local refusal independently of a completed rejection.
/// This local type is never serialized into migration records.
pub type MigrationResult<T> = Result<T, crate::execution_attempt::ExecutionAttemptError<String>>;

/// Original operation failure at an existing World/MV child boundary.
#[derive(Debug, thiserror::Error)]
pub(crate) enum MigrationBoundaryError {
    #[error(transparent)]
    Execution(#[from] crate::execution_attempt::ExecutionAttemptError<String>),
    #[error(transparent)]
    Storage(#[from] crate::state::StateStorageAdmissionError),
}
pub(crate) type MigrationBoundaryResult<T> = Result<T, MigrationBoundaryError>;

/// The actual block-start context, shared by each independently disposable World child.
/// Migration callbacks never receive this context or the World itself.
pub(crate) struct MigrationContext<'context, 'world> {
    pub(crate) lane_config: &'context iroha_config::parameters::actual::LaneConfig,
    pub(crate) current_slot: u64,
    pub(crate) lane_map: &'context std::collections::BTreeMap<
        iroha_model_base::topology::DataSpaceId,
        iroha_model_base::topology::LaneId,
    >,
    pub(crate) quantity_candidate:
        &'context mut crate::state::fastpq_quantity_capture::QuantityCandidateArchive,
    #[cfg(feature = "telemetry")]
    pub(crate) telemetry: Option<&'world crate::telemetry::StateTelemetry>,
    pub(crate) world_lifetime: core::marker::PhantomData<&'world ()>,
}
impl<'world> MigrationContext<'_, 'world> {
    fn child<'block>(
        &self,
        world: &'block mut WorldBlock<'world>,
    ) -> Result<Box<WorldTransaction<'block, 'world>>, crate::state::StateStorageAdmissionError>
    {
        world.try_transaction_with_axt_lane_map(
            #[cfg(feature = "telemetry")]
            self.telemetry,
            self.lane_config.clone(),
            self.current_slot,
            self.lane_map.clone(),
        )
    }
}

/// Whether `selection` installs the kernel's lifecycle State: the storage of the
/// `iroha.runtime.lifecycle.v1` addition.
///
/// A root admits a migration, a storage retirement and the committed checkpoint records only
/// once it does. Every validator then runs a release with the engine, because the addition
/// activated with every validator's support, so the rule that admits them is the same on every
/// executing node.
#[must_use]
pub fn lifecycle_installed(selection: &RootSelectionV1) -> bool {
    release_catalog().is_ok_and(|catalog| {
        RUNTIME_LIFECYCLE.storage.iter().all(|storage| {
            catalog
                .world_storage
                .get(storage.field())
                .is_some_and(|id| selection.installed_storage.binary_search(id).is_ok())
        })
    })
}

fn migration<'a>(
    lookup: &'a impl DefinitionLookup,
    id: &DefinitionId,
) -> Option<&'a MigrationDescriptorV1> {
    match lookup.definition(id) {
        Some(DefinitionDescriptorV1::Migration(descriptor)) => Some(descriptor),
        _ => None,
    }
}

/// The migrations `selection` activates, with their descriptors.
fn active_migrations<'a>(
    selection: &'a RootSelectionV1,
    lookup: &'a impl DefinitionLookup,
) -> impl Iterator<Item = (DefinitionId, &'a MigrationDescriptorV1)> {
    selection
        .active
        .iter()
        .filter_map(move |id| migration(lookup, id).map(|descriptor| (*id, descriptor)))
}

/// Why `change_set` asks for a migration effect this root cannot perform, if it does: the
/// structural rules of a migration's change sets, which depend only on the change set, the
/// selections and the published definitions.
///
/// `committed` is the selection the root executes when the rule is evaluated. `predecessor` is
/// the selection the change set meets and `successor` the one it produces; `lookup` resolves the
/// published definitions and those the change set publishes. `identity_cutover` says the change
/// set is the release's byte-preserving replacement of storage definitions, whose retirements
/// move no row.
///
/// - Activating a migration or retiring storage needs the lifecycle State in `committed`.
/// - A migration is activated over old storage the predecessor installs, and no other migration
///   the successor activates shares storage with it.
/// - Storage is retired only together with the deactivation of an active migration whose old
///   storage it is, and that deactivation retires all of the migration's old storage.
/// - A migration is otherwise deactivated only together with the activation of a replacement
///   over the same old storage, successor storage and cursor.
pub(super) fn unsupported_transition(
    change_set: &ChangeSetV1,
    identity_cutover: bool,
    committed: &RootSelectionV1,
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    lookup: &impl DefinitionLookup,
) -> Option<String> {
    let activated: Vec<(DefinitionId, &MigrationDescriptorV1)> = change_set
        .transitions
        .iter()
        .filter_map(|transition| match transition {
            SelectionTransitionV1::Activate(id) => {
                migration(lookup, id).map(|descriptor| (*id, descriptor))
            }
            _ => None,
        })
        .collect();
    let deactivated: Vec<(DefinitionId, &MigrationDescriptorV1)> = change_set
        .transitions
        .iter()
        .filter_map(|transition| match transition {
            SelectionTransitionV1::Deactivate(id) => {
                migration(lookup, id).map(|descriptor| (*id, descriptor))
            }
            _ => None,
        })
        .collect();
    // An identity cutover replaces storage definitions over the same fields and moves no row:
    // its retirements belong to no migration cursor.
    let retired: Vec<DefinitionId> = change_set
        .transitions
        .iter()
        .filter_map(|transition| match transition {
            SelectionTransitionV1::RetireStorage(storage) if !identity_cutover => Some(*storage),
            _ => None,
        })
        .collect();
    if activated.is_empty() && deactivated.is_empty() && retired.is_empty() {
        return None;
    }
    if !lifecycle_installed(committed) {
        // The operations a root without the engine refuses, as before the engine existed.
        return Some(
            if retired.is_empty() {
                "runtime transition requires unimplemented migration execution"
            } else {
                "runtime transition requires unimplemented storage migration cutover"
            }
            .to_owned(),
        );
    }
    for (id, descriptor) in &activated {
        if let Some(missing) = descriptor.from.iter().find(|storage| {
            predecessor
                .installed_storage
                .binary_search(storage)
                .is_err()
        }) {
            return Some(format!(
                "migration {id} reads storage {missing}, which the root does not install"
            ));
        }
        let shared = active_migrations(successor, lookup).any(|(other, active)| {
            other != *id
                && descriptor
                    .from
                    .iter()
                    .chain(&descriptor.to)
                    .any(|storage| active.from.contains(storage) || active.to.contains(storage))
        });
        if shared {
            return Some(format!(
                "migration {id} shares storage with another active migration"
            ));
        }
    }
    for storage in &retired {
        let owned = deactivated
            .iter()
            .any(|(_, descriptor)| descriptor.from.contains(storage));
        if !owned {
            return Some(format!(
                "storage {storage} is retired outside the retirement of its migration"
            ));
        }
    }
    for (id, descriptor) in &deactivated {
        if predecessor.active.binary_search(id).is_err() {
            continue;
        }
        let retires_all = descriptor
            .from
            .iter()
            .all(|storage| retired.contains(storage));
        let retires_any = descriptor
            .from
            .iter()
            .any(|storage| retired.contains(storage));
        let replaced = activated.iter().any(|(_, replacement)| {
            replacement.from == descriptor.from
                && replacement.to == descriptor.to
                && replacement.cursor == descriptor.cursor
        });
        if retires_any != retires_all || retires_all == replaced {
            return Some(format!(
                "migration {id} is deactivated only by retiring all its old storage or by a \
                 replacement over the same storage and cursor"
            ));
        }
    }
    None
}

/// Why `change_set`, applied to `predecessor`, may not activate now, if it may not: the rules
/// that depend on how far each migration has come (§5.4).
///
/// - The cutover, restricting a migration to completion only, needs every old row staged.
/// - The retirement needs the old storage drained.
/// - A replacement takes over only a migration that has not cut over.
/// - A migration activates once: a definition with a record does not start again.
pub(super) fn blocked(
    world: &WorldTransaction<'_, '_>,
    change_set: &ChangeSetV1,
    predecessor: &RootSelectionV1,
    lookup: &impl DefinitionLookup,
) -> Option<String> {
    let status = |id: &DefinitionId| world.runtime_migrations.get(id).map(|record| record.status);
    let retires = |descriptor: &MigrationDescriptorV1| {
        change_set.transitions.iter().any(|transition| {
            matches!(transition, SelectionTransitionV1::RetireStorage(storage)
                if descriptor.from.contains(storage))
        })
    };
    for transition in &change_set.transitions {
        match transition {
            SelectionTransitionV1::Activate(id) if migration(lookup, id).is_some() => {
                if status(id).is_some() {
                    return Some(format!("migration {id} already ran on this root"));
                }
            }
            SelectionTransitionV1::Restrict {
                definition,
                restriction: RestrictionKindV1::CompletionOnly,
            } if migration(lookup, definition).is_some() => {
                if !matches!(status(definition), Some(MigrationStatusV1::Complete(_))) {
                    return Some(format!(
                        "migration {definition} cuts over only once every old row is staged"
                    ));
                }
            }
            SelectionTransitionV1::Deactivate(id) => {
                let Some(descriptor) = migration(lookup, id) else {
                    continue;
                };
                if predecessor.active.binary_search(id).is_err() {
                    continue;
                }
                let allowed = match (retires(descriptor), status(id)) {
                    (true, Some(MigrationStatusV1::Drained(_))) => true,
                    (true, _) => false,
                    (
                        false,
                        Some(
                            MigrationStatusV1::Running
                            | MigrationStatusV1::Complete(_)
                            | MigrationStatusV1::Faulted(_),
                        ),
                    ) => true,
                    (false, _) => false,
                };
                if !allowed {
                    return Some(format!(
                        "migration {id} is retired only once its old storage is drained, and \
                         replaced only before its cutover"
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

/// Record what an activated `change_set` did to the root's migrations at `height`: a newly
/// activated migration starts, or takes over the cursor of the migration it replaces.
pub(super) fn activated(
    world: &mut WorldTransaction<'_, '_>,
    change_set: &ChangeSetV1,
    height: u64,
) {
    let started: Vec<(DefinitionId, MigrationDescriptorV1)> = change_set
        .transitions
        .iter()
        .filter_map(|transition| match transition {
            SelectionTransitionV1::Activate(id) => {
                world
                    .runtime_definitions
                    .get(id)
                    .and_then(|descriptor| match descriptor {
                        DefinitionDescriptorV1::Migration(descriptor) => {
                            Some((*id, descriptor.clone()))
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
        .collect();
    for (id, descriptor) in started {
        // The migration this one replaces: deactivated by the same change set over the same
        // storage and cursor. The structural rules allow at most one.
        let replaced = change_set
            .transitions
            .iter()
            .find_map(|transition| match transition {
                SelectionTransitionV1::Deactivate(old) => {
                    match world.runtime_definitions.get(old) {
                        Some(DefinitionDescriptorV1::Migration(old_descriptor))
                            if old_descriptor.from == descriptor.from
                                && old_descriptor.to == descriptor.to
                                && old_descriptor.cursor == descriptor.cursor =>
                        {
                            Some(*old)
                        }
                        _ => None,
                    }
                }
                _ => None,
            });
        let record = match replaced.and_then(|old| {
            world
                .runtime_migrations
                .get(&old)
                .cloned()
                .map(|record| (old, record))
        }) {
            Some((old, mut record)) => {
                let taken_over = record.taken_over();
                record.status = MigrationStatusV1::Replaced(height);
                world.runtime_migrations.insert(old, record);
                taken_over
            }
            None => MigrationRecordV1::started(height),
        };
        world.runtime_migrations.insert(id, record);
    }
}

/// Run each migration in its own existing World child at the original block-start cut.
/// A successful child applies once; deterministic stage rejection discards its writes before
/// a separate child records `Faulted`. Any local refusal aborts the enclosing State start.
/// Earlier successful children are still unpublished and disappear with that original block.
pub(crate) fn run_stages<'world>(
    world: &mut WorldBlock<'world>,
    height: u64,
    context: &mut MigrationContext<'_, 'world>,
) -> MigrationBoundaryResult<()> {
    use crate::execution_attempt::ExecutionAttemptError;
    if world.runtime_migrations.iter().next().is_none() {
        return Ok(());
    }
    let catalog = release_catalog().map_err(|error| {
        MigrationBoundaryError::Execution(ExecutionAttemptError::Rejected(error.to_string()))
    })?;
    let due = scratch::stage_census(world, height)?;
    for entry in due.as_slice() {
        let (id, restriction) = (entry.id, entry.restriction);
        let Some(migration) = catalog.migration(&id).copied() else {
            continue;
        };
        // Only the current row becomes a persistence candidate. Earlier callbacks
        // hold capabilities for their declared tables, never these engine records.
        // TODO: retain native table/payload charges with this record and its cursor;
        // the funded ID census does not fund the eventual persisted record graph.
        let mut record = world
            .runtime_migrations
            .get(&id)
            .expect("selected engine-owned migration record remains present")
            .clone();
        if !matches!(
            (record.status, restriction),
            (MigrationStatusV1::Running, None)
                | (
                    MigrationStatusV1::Complete(_),
                    Some(RestrictionV1::CompletionOnly(_))
                )
                | (
                    MigrationStatusV1::CutOver(_),
                    Some(RestrictionV1::CompletionOnly(_))
                )
        ) {
            continue;
        }
        let mut child = context.child(world)?;
        match (record.status, restriction) {
            (MigrationStatusV1::Running, None) => {
                match migration
                    .code
                    .stage(&mut child, &record.cursor, migration.rows_per_stage)
                {
                    Ok(outcome) => {
                        record.cursor = outcome.cursor;
                        record.rows = record.rows.saturating_add(u64::from(outcome.rows));
                        record.stages = record.stages.saturating_add(1);
                        if outcome.done {
                            record.status = MigrationStatusV1::Complete(height);
                        }
                    }
                    Err(ExecutionAttemptError::Deferred(original)) => {
                        return Err(ExecutionAttemptError::Deferred(original).into());
                    }
                    Err(ExecutionAttemptError::Rejected(reason)) => {
                        // Restore exact original roots/touches before recording the terminal
                        // failure. No inverse writes or copied journal reconstruct the source.
                        drop(child);
                        iroha_logger::error!(migration = %id, %reason, "migration stage faulted");
                        record.status = MigrationStatusV1::Faulted(height);
                        let mut fault = context.child(world)?;
                        fault.runtime_migrations.insert(id, record);
                        context.quantity_candidate.apply_migration_child(fault);
                        continue;
                    }
                }
            }
            (MigrationStatusV1::Complete(_), Some(RestrictionV1::CompletionOnly(_))) => {
                record.status = MigrationStatusV1::CutOver(height);
            }
            (MigrationStatusV1::CutOver(_), Some(RestrictionV1::CompletionOnly(_))) => {
                let outcome = migration.code.drain(&mut child, migration.rows_per_stage)?;
                record.stages = record.stages.saturating_add(1);
                if outcome.empty {
                    record.status = MigrationStatusV1::Drained(height);
                }
            }
            _ => unreachable!("checked migration stage eligibility"),
        }
        child.runtime_migrations.insert(id, record);
        context.quantity_candidate.apply_migration_child(child);
    }
    Ok(())
}

/// Keep every staged successor exact at the end of the block at `height` (§5.4): mirror the old
/// rows the block touched at or before each cursor while the migration's barrier holds, and stop
/// the drain of a migration whose old storage the block still wrote after its cutover.
///
/// It reads the block's own journal of touched rows, so it covers every writer of the old
/// storage: instructions, hooks and the block's tail writes alike. It runs as the last World
/// write before the block's World commitment is folded. A root that holds no migration pays one
/// emptiness check. Any error prevents publication of this entire original block; no
/// incomplete barrier is recorded as success.
pub(crate) fn barrier(world: &mut WorldBlock<'_>, height: u64) -> MigrationBoundaryResult<()> {
    resume_barrier(world, height, &mut BarrierProgress::default())
}

/// Private progress of the same completed World tail. The enclosing publisher
/// retains this cursor; callers cannot supply or modify a completed prefix.
#[derive(Default)]
pub(crate) struct BarrierProgress {
    completed_through: Option<DefinitionId>,
}

/// Resume only the first unfinished migration in the unchanged original World.
/// An individual failed callback rolls back its existing successor child; prior
/// successful siblings remain private and are never executed a second time.
pub(crate) fn resume_barrier(
    world: &mut WorldBlock<'_>,
    height: u64,
    progress: &mut BarrierProgress,
) -> MigrationBoundaryResult<()> {
    if world.runtime_migrations.iter().next().is_none() {
        return Ok(());
    }
    let catalog = release_catalog().map_err(|error| {
        MigrationBoundaryError::Execution(
            crate::execution_attempt::ExecutionAttemptError::Rejected(error.to_string()),
        )
    })?;
    let held = scratch::barrier_census(world, progress.completed_through)?;
    for entry in held.entries() {
        let id = entry.id;
        let Some(migration) = catalog.migration(&id).copied() else {
            progress.completed_through = Some(id);
            continue;
        };
        if entry.barrier_holds {
            migration.code.barrier(world, held.cursor(entry))?;
        } else if matches!(
            entry.status,
            MigrationStatusV1::CutOver(_) | MigrationStatusV1::Drained(_)
        ) && migration.code.stale(world)
        {
            iroha_logger::error!(
                migration = %id,
                "a write reached the old storage after the cutover; the drain stops"
            );
            // The original map owns this persistent mutation. Its node and
            // nested record cloning remain a separate physical funding boundary.
            world
                .runtime_migrations
                .get_mut(&id)
                .expect("selected engine-owned migration record remains present")
                .status = MigrationStatusV1::StaleWrite(height);
        }
        progress.completed_through = Some(id);
    }
    Ok(())
}

/// The change set of one step of the migration `module` on the authenticated root of `world`
/// ([`super::ReleaseCatalog::migration_step`]); its root's governance authorizes scheduling.
///
/// # Errors
/// `module` is not a migration of this release, or the step does not apply to the committed
/// selection.
pub fn draft_step(
    world: &impl crate::state::WorldReadOnly,
    network: iroha_data_model::NetworkId,
    module: &str,
    step: super::catalog::MigrationStep,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, super::CatalogError> {
    let change = release_catalog()?.migration_step(
        module,
        step,
        world.runtime_selection(),
        &super::change_set::PublishedDefinitions(world.runtime_definitions()),
        network,
        effective_height,
        qualification,
    )?;
    super::draft_scope::bind(world, network, change).map_err(|error| {
        super::CatalogError::Addition {
            module: module.into(),
            reason: error.to_string(),
        }
    })
}

#[cfg(test)]
mod tests;
