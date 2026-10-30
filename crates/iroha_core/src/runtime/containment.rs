//! Emergency containment of runtime definitions (`specs/live_runtime_upgrades.md` §7.1).
//!
//! Full Parliament cannot act at the next block, so it delegates containment to the committed
//! runtime emergency panel (`runtime_emergency_panel_v1`). Enough fresh endorsements from one
//! panel close admission of definitions, or keep only their completion, from the block after the
//! authorization. Containment only narrows the committed selection: it never activates, rebinds,
//! migrates, moves value, deletes storage or fully disables, and it never restricts the
//! governance and lifecycle interfaces an incident is repaired through.
//!
//! Containment is a lease and opens a ratification case. Ordinary governance ratifies, repairs
//! or lifts it with change sets: proposing or approving a change set that addresses a contained
//! definition records review progress, and once the change sets of a height take over every
//! restriction the containment held, the case is resolved. A renewal of the same incident with
//! fresh endorsements may take the lease over once half of it has passed, and only with review
//! progress since the containment it renews; otherwise the containment expires at block start
//! and restores every restriction it still holds. A change set approved before the incident
//! names the selection containment replaced, so it no longer applies and cannot undo it.

use std::collections::BTreeSet;

use iroha_data_model::{
    isi::{error::InstructionExecutionError as Error, runtime_upgrade::ContainRuntimeV1},
    parameter::runtime_emergency_panel::RuntimeEmergencyPanelV1,
    runtime::{
        definition::{
            DefinitionDescriptorV1, DefinitionId, RestrictionEntryV1, RestrictionV1,
            RootSelectionV1,
        },
        lifecycle::{
            ChangeSetId, ChangeSetV1, ContainedDefinitionV1, ContainmentKindV1,
            ContainmentRecordV1, ContainmentReviewV1, ContainmentStatusV1, RestrictionKindV1,
            RootScopeV1, SelectionTransitionV1,
        },
    },
};
use mv::storage::StorageReadOnly;

/// Whether containment must leave the interface bound to `wire_id` alone: governance, parameters
/// and the runtime lifecycle stay available to repair an incident.
fn protected(wire_id: &str) -> bool {
    wire_id.starts_with("iroha.instruction.v1::governance::")
        || wire_id.starts_with("iroha.runtime.")
        || wire_id == "iroha.set_parameter"
}

/// The committed runtime emergency panel of the universal root.
///
/// # Errors
/// No panel is committed or it is malformed; there is no default (§7.1).
pub(crate) fn committed_panel(
    parameters: &iroha_data_model::parameter::Parameters,
) -> Result<RuntimeEmergencyPanelV1, Error> {
    let custom = parameters
        .custom()
        .get(RuntimeEmergencyPanelV1::parameter_id())
        .ok_or_else(|| {
            Error::InvariantViolation(
                "no runtime emergency panel is committed; no containment is admitted".into(),
            )
        })?;
    RuntimeEmergencyPanelV1::from_custom_parameter(custom)
        .map_err(|error| Error::InvariantViolation(error.to_string().into()))
}

/// The restriction `selection` holds for `definition`.
fn restriction_of(selection: &RootSelectionV1, definition: &DefinitionId) -> Option<RestrictionV1> {
    selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(definition))
        .ok()
        .map(|index| selection.restrictions[index].restriction)
}

/// Set or clear the restriction of `definition`, keeping restrictions sorted.
fn set_restriction(
    selection: &mut RootSelectionV1,
    definition: DefinitionId,
    restriction: Option<RestrictionV1>,
) {
    match (
        selection
            .restrictions
            .binary_search_by(|entry| entry.definition.cmp(&definition)),
        restriction,
    ) {
        (Ok(index), Some(restriction)) => selection.restrictions[index].restriction = restriction,
        (Ok(index), None) => {
            selection.restrictions.remove(index);
        }
        (Err(index), Some(restriction)) => selection.restrictions.insert(
            index,
            RestrictionEntryV1 {
                definition,
                restriction,
            },
        ),
        (Err(_), None) => {}
    }
}

/// How much a restriction stops, and from which height.
const fn strength(restriction: RestrictionV1) -> (u8, u64) {
    match restriction {
        RestrictionV1::Deprecated(from) => (0, from),
        RestrictionV1::AdmissionClosed(from) => (1, from),
        RestrictionV1::CompletionOnly(from) => (2, from),
        RestrictionV1::Disabled(from) => (3, from),
    }
}

/// Whether `previous` already stops at least what `imposed` would from its height.
fn already_restricted(previous: Option<RestrictionV1>, imposed: RestrictionV1) -> bool {
    previous.is_some_and(|previous| {
        let (previous_rank, previous_from) = strength(previous);
        let (imposed_rank, imposed_from) = strength(imposed);
        previous_rank >= imposed_rank && previous_from <= imposed_from
    })
}

/// Check that containment may restrict `target` under `selection`.
fn check_target<S>(
    selection: &RootSelectionV1,
    definitions: &S,
    target: &DefinitionId,
) -> Result<(), Error>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    let refused = |reason: &str| {
        Error::InvariantViolation(format!("containment of {target}: {reason}").into())
    };
    if selection.active.binary_search(target).is_err() {
        return Err(refused("the definition is not active"));
    }
    let bindings: Vec<&str> = match definitions.get(target) {
        Some(DefinitionDescriptorV1::Interface(_)) => selection
            .bindings
            .iter()
            .filter(|binding| binding.interface == *target)
            .map(|binding| binding.wire_id.as_str())
            .collect(),
        Some(DefinitionDescriptorV1::Implementation(_)) => selection
            .bindings
            .iter()
            .filter(|binding| binding.implementation == *target)
            .map(|binding| binding.wire_id.as_str())
            .collect(),
        Some(_) => return Err(refused("only interfaces and implementations are contained")),
        None => return Err(refused("the definition is not published")),
    };
    if bindings.into_iter().any(protected) {
        return Err(refused(
            "governance and lifecycle interfaces stay available to repair an incident",
        ));
    }
    Ok(())
}

/// Authorize a containment endorsed by the committed panel (§7.1).
///
/// # Errors
/// No panel is committed or its term ended, the containment names another network or a data
/// space root, its lease is outside the panel's bounds, its endorsements fall short of the
/// threshold in both panels, it is already recorded, a target may not be contained, every target
/// is already restricted at least as much, or a renewal does not match the containment it
/// renews or comes before renewal opens.
pub(crate) fn contain(
    state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    instruction: ContainRuntimeV1,
) -> Result<(), Error> {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeContained, RuntimeUpgradeEvent,
    };
    let ContainRuntimeV1 {
        containment,
        endorsements,
    } = instruction;
    let invalid = |reason: &str| Error::InvariantViolation(format!("containment: {reason}").into());
    let panel = committed_panel(crate::state::WorldReadOnly::parameters(
        &state_transaction.world,
    ))?;
    let height = state_transaction._curr_block.height().get();
    let now_ms = super::change_set::block_ms(state_transaction);
    if containment.network != state_transaction.network_id {
        return Err(invalid("it belongs to another network"));
    }
    if containment.root != RootScopeV1::Universal {
        // TODO(live_runtime_upgrades.md §7.1, §11): a permissioned data space's admins contain
        // its own selection; this release implements the universal root only.
        return Err(invalid("this release contains the universal root only"));
    }
    if now_ms >= panel.term_until_ms {
        return Err(invalid("the panel's term has ended"));
    }
    if containment.lease_until_ms < now_ms.saturating_add(panel.min_lease_ms)
        || containment.lease_until_ms > now_ms.saturating_add(panel.max_lease_ms)
    {
        return Err(invalid("its lease is outside the panel's bounds"));
    }
    let threshold =
        usize::try_from(panel.threshold).map_err(|_| invalid("the threshold exceeds usize"))?;
    if containment
        .check_endorsements(&panel.primary, threshold, now_ms, &endorsements)
        .is_err()
    {
        containment
            .check_endorsements(&panel.backup, threshold, now_ms, &endorsements)
            .map_err(|error| invalid(&error.to_string()))?;
    }
    let id = containment
        .id()
        .map_err(|error| invalid(&error.to_string()))?;
    if state_transaction
        .world
        .runtime_containments
        .get(&id)
        .is_some()
    {
        return Err(invalid("it is already recorded"));
    }
    let effective_height = height.saturating_add(1);
    let imposed = match containment.kind {
        ContainmentKindV1::AdmissionClosed => RestrictionKindV1::AdmissionClosed,
        ContainmentKindV1::CompletionOnly => RestrictionKindV1::CompletionOnly,
    }
    .at(effective_height);
    let renewed = state_transaction
        .world
        .runtime_containments
        .iter()
        .find(|(_, record)| {
            record.status == ContainmentStatusV1::Active
                && record.containment.incident == containment.incident
        })
        .map(|(renewed_id, record)| (*renewed_id, record.clone()));
    let mut selection = state_transaction.world.runtime_selection.get().clone();
    let contained = if let Some((renewed_id, mut renewed)) = renewed {
        if renewed.containment.targets != containment.targets
            || renewed.containment.kind != containment.kind
        {
            return Err(invalid(
                "a renewal contains the same definitions in the same way",
            ));
        }
        if now_ms < renewed.renewal_opens_ms() {
            return Err(invalid("renewal opens halfway through the lease"));
        }
        if !renewed
            .review
            .is_some_and(|review| review.height >= renewed.authorized_height)
        {
            return Err(invalid(
                "a renewal needs review progress: governance has proposed or approved no change set \
                 for the incident since the containment it renews",
            ));
        }
        let held: Vec<ContainedDefinitionV1> = renewed
            .contained
            .iter()
            .filter(|entry| restriction_of(&selection, &entry.definition) == Some(entry.imposed))
            .copied()
            .collect();
        if held.is_empty() {
            return Err(invalid(
                "governance already took over every contained definition",
            ));
        }
        renewed.status = ContainmentStatusV1::Renewed(height);
        let review = renewed.review;
        state_transaction
            .world
            .runtime_containments
            .insert(renewed_id, renewed);
        (held, review)
    } else {
        let mut contained = Vec::new();
        for target in &containment.targets {
            check_target(
                &selection,
                &state_transaction.world.runtime_definitions,
                target,
            )?;
            let previous = restriction_of(&selection, target);
            if already_restricted(previous, imposed) {
                continue;
            }
            set_restriction(&mut selection, *target, Some(imposed));
            contained.push(ContainedDefinitionV1 {
                definition: *target,
                previous,
                imposed,
            });
        }
        if contained.is_empty() {
            return Err(invalid(
                "every target is already restricted at least as much",
            ));
        }
        *state_transaction.world.runtime_selection.get_mut() = selection;
        (contained, None)
    };
    let (contained, review) = contained;
    let incident = containment.incident;
    state_transaction.world.runtime_containments.insert(
        id,
        ContainmentRecordV1 {
            containment,
            authorized_height: height,
            authorized_ms: now_ms,
            contained,
            status: ContainmentStatusV1::Active,
            review,
        },
    );
    state_transaction
        .world
        .emit_events(Some(RuntimeUpgradeEvent::Contained(
            RuntimeUpgradeContained {
                containment: id,
                incident,
                effective_height,
            },
        )));
    Ok(())
}

/// The interface and implementation `selection` binds to `wire_id`.
fn bound_to<'a>(
    selection: &'a RootSelectionV1,
    wire_id: &'a str,
) -> impl Iterator<Item = DefinitionId> + 'a {
    selection
        .bindings
        .iter()
        .filter(move |binding| binding.wire_id == wire_id)
        .flat_map(|binding| [binding.interface, binding.implementation])
}

/// The definitions `change_set` addresses under `selection` (§7.1, "Review"): those its
/// transitions activate, deactivate, restrict or lift, and the interface and implementation
/// behind each wire ID it binds or unbinds, before and after.
fn addressed(change_set: &ChangeSetV1, selection: &RootSelectionV1) -> BTreeSet<DefinitionId> {
    let mut addressed = BTreeSet::new();
    for transition in &change_set.transitions {
        match transition {
            SelectionTransitionV1::Activate(definition)
            | SelectionTransitionV1::Deactivate(definition)
            | SelectionTransitionV1::Lift(definition)
            | SelectionTransitionV1::Restrict { definition, .. } => {
                addressed.insert(*definition);
            }
            SelectionTransitionV1::Bind(binding) => {
                addressed.extend([binding.interface, binding.implementation]);
                addressed.extend(bound_to(selection, &binding.wire_id));
            }
            SelectionTransitionV1::Unbind(wire_id) => {
                addressed.extend(bound_to(selection, wire_id));
            }
            SelectionTransitionV1::InstallStorage(_)
            | SelectionTransitionV1::RetireStorage(_)
            | SelectionTransitionV1::SetKernel(_)
            | SelectionTransitionV1::SetCommitmentProfile(_)
            | SelectionTransitionV1::SetServicesProfile(_)
            | SelectionTransitionV1::RetainCompletion(_)
            | SelectionTransitionV1::ReleaseCompletion(_) => {}
        }
    }
    addressed
}

/// Record review progress (§7.1, "Review"): governance proposed or approved, at `height`, the
/// change set `id`, which addresses a definition an active containment restricted. A renewal
/// needs progress recorded since the containment it renews.
pub(crate) fn record_review(
    world: &mut crate::state::WorldTransaction<'_, '_>,
    id: ChangeSetId,
    change_set: &ChangeSetV1,
    height: u64,
) {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeContainmentReviewed, RuntimeUpgradeEvent,
    };
    let addressed = addressed(change_set, world.runtime_selection.get());
    let reviewed: Vec<[u8; 32]> = world
        .runtime_containments
        .iter()
        .filter(|(_, record)| {
            record.status == ContainmentStatusV1::Active
                && record
                    .contained
                    .iter()
                    .any(|entry| addressed.contains(&entry.definition))
        })
        .map(|(containment, _)| *containment)
        .collect();
    for containment in reviewed {
        let Some(mut record) = world.runtime_containments.get(&containment).cloned() else {
            continue;
        };
        record.review = Some(ContainmentReviewV1 {
            change_set: id,
            height,
        });
        world.runtime_containments.insert(containment, record);
        world.emit_events(Some(RuntimeUpgradeEvent::ContainmentReviewed(
            RuntimeUpgradeContainmentReviewed {
                containment,
                change_set: id,
            },
        )));
    }
}

/// Whether `record` still holds a restriction it imposed under `selection`.
fn holds(record: &ContainmentRecordV1, selection: &RootSelectionV1) -> bool {
    record
        .contained
        .iter()
        .any(|entry| restriction_of(selection, &entry.definition) == Some(entry.imposed))
}

/// The active containments that hold at least one restriction they imposed.
pub(crate) fn holding(world: &crate::state::WorldTransaction<'_, '_>) -> Vec<[u8; 32]> {
    let selection = world.runtime_selection.get();
    world
        .runtime_containments
        .iter()
        .filter(|(_, record)| {
            record.status == ContainmentStatusV1::Active && holds(record, selection)
        })
        .map(|(containment, _)| *containment)
        .collect()
}

/// Resolve each containment of `holding_before` that, after the change sets of `height`
/// activated, holds nothing any more: ordinary governance ratified, repaired or lifted it
/// (§7.1, "Review").
pub(crate) fn resolve_taken_over(
    world: &mut crate::state::WorldTransaction<'_, '_>,
    height: u64,
    holding_before: &[[u8; 32]],
) {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeContainmentResolved, RuntimeUpgradeEvent,
    };
    for containment in holding_before {
        let Some(mut record) = world.runtime_containments.get(containment).cloned() else {
            continue;
        };
        if holds(&record, world.runtime_selection.get()) {
            continue;
        }
        record.status = ContainmentStatusV1::Resolved(height);
        world.runtime_containments.insert(*containment, record);
        world.emit_events(Some(RuntimeUpgradeEvent::ContainmentResolved(
            RuntimeUpgradeContainmentResolved {
                containment: *containment,
                height,
            },
        )));
    }
}

/// Expire every active containment whose lease ended by the block's committed time `now_ms`,
/// restoring each restriction it still holds (§7.1, "Expiry").
pub(crate) fn expire_due(
    wtx: &mut crate::state::WorldTransaction<'_, '_>,
    height: u64,
    now_ms: u64,
) {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeContainmentExpired, RuntimeUpgradeEvent,
    };
    let due: Vec<[u8; 32]> = wtx
        .runtime_containments
        .iter()
        .filter(|(_, record)| {
            record.status == ContainmentStatusV1::Active
                && record.containment.lease_until_ms <= now_ms
        })
        .map(|(id, _)| *id)
        .collect();
    for id in due {
        let Some(mut record) = wtx.runtime_containments.get(&id).cloned() else {
            continue;
        };
        let mut selection = wtx.runtime_selection.get().clone();
        let mut restored = false;
        for entry in &record.contained {
            if restriction_of(&selection, &entry.definition) == Some(entry.imposed) {
                set_restriction(&mut selection, entry.definition, entry.previous);
                restored = true;
            }
        }
        if restored {
            *wtx.runtime_selection.get_mut() = selection;
        }
        record.status = ContainmentStatusV1::Expired(height);
        wtx.runtime_containments.insert(id, record);
        wtx.emit_events(Some(RuntimeUpgradeEvent::ContainmentExpired(
            RuntimeUpgradeContainmentExpired {
                containment: id,
                height,
            },
        )));
    }
}

#[cfg(test)]
mod tests;
