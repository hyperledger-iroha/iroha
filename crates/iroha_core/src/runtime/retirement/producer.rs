//! Selected permanent history, floor checks and irreversible native implementation exclusion.

use std::collections::BTreeSet;

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::InstructionExecutionError as IsiError, runtime_retirement::RetireRuntimeDefinitionV1,
    },
    runtime::{
        checkpoint::RuntimeBoundaryV1,
        definition::{
            DefinitionDescriptorV1, DefinitionId, DefinitionLookup, ImplementationCodeV1,
            RestrictionV1, RootSelectionV1,
        },
        lifecycle::{ChangeSetV1, RestrictionKindV1, SelectionTransitionV1},
        retirement::{
            MAX_RETIREMENT_DEFINITIONS, RuntimeRetirementReceiptV1, RuntimeRetirementRootV1,
            RuntimeRetirementStateV1, SERVICE, SERVICE_CLOSURE,
        },
    },
};
use mv::storage::StorageReadOnly as _;

use crate::runtime::{change_set::PublishedDefinitions, obligations::LiveObligations};
use crate::{
    smartcontracts::Execute,
    state::{
        StateReadOnly, StateStorageAdmissionError, StateTransaction, WorldReadOnly,
        WorldTransaction,
    },
};

fn invalid(message: impl std::fmt::Display) -> IsiError {
    IsiError::InvariantViolation(format!("runtime retirement: {message}").into())
}

pub(crate) fn enabled(selection: &RootSelectionV1) -> Result<bool, String> {
    crate::runtime::release_catalog()
        .map_err(|error| error.to_string())?
        .service_enabled(
            selection.services_profile,
            SERVICE,
            Hash::new(SERVICE_CLOSURE).into(),
        )
        .map_err(|error| error.to_string())
}

fn disabled(selection: &RootSelectionV1, id: DefinitionId) -> bool {
    selection.restrictions.iter().any(|restriction| {
        restriction.definition == id
            && matches!(restriction.restriction, RestrictionV1::Disabled(_))
    })
}

fn excluded(selection: &RootSelectionV1, id: DefinitionId) -> bool {
    !selection.active.contains(&id) || disabled(selection, id)
}

fn selected_needed(
    selection: &RootSelectionV1,
    lookup: &impl DefinitionLookup,
) -> Result<BTreeSet<DefinitionId>, String> {
    let mut pending: Vec<_> = selection
        .active
        .iter()
        .chain(&selection.installed_storage)
        .copied()
        .chain([
            selection.kernel,
            selection.services_profile,
            selection.commitment_profile,
        ])
        .collect();
    let mut needed = BTreeSet::new();
    while let Some(id) = pending.pop() {
        // Retired descriptors stay metadata; a rejected call does not execute its old handler.
        if disabled(selection, id) || !needed.insert(id) {
            continue;
        }
        let descriptor = lookup
            .definition(&id)
            .ok_or_else(|| format!("unpublished definition {id}"))?;
        pending.extend(
            descriptor
                .typed_dependencies()
                .into_iter()
                .map(|(_, id)| id),
        );
        if let DefinitionDescriptorV1::Interface(interface) = descriptor
            && let Some(binding) = selection
                .bindings
                .iter()
                .find(|binding| binding.wire_id == interface.wire_id)
        {
            pending.push(binding.implementation);
        }
    }
    Ok(needed)
}

/// Restored history must retain its producer and exact permanent exclusions.
pub(crate) fn validate_persistence(world: &impl WorldReadOnly) -> Result<(), String> {
    let selected = enabled(world.runtime_selection())?;
    let rows = world.runtime_retirements();
    if rows.iter().count() > MAX_RETIREMENT_DEFINITIONS {
        return Err("runtime retirement history exceeds capacity".into());
    }
    if !selected {
        return if rows.iter().next().is_some() {
            Err("unselected runtime retirement history is nonempty".into())
        } else {
            Ok(())
        };
    }
    let lookup = PublishedDefinitions(world.runtime_definitions());
    for namespace in ["world.runtime_retirements", "world.runtime_boundaries"] {
        if !world.runtime_selection().installed_storage.iter().any(|id| {
            matches!(lookup.definition(id), Some(DefinitionDescriptorV1::Storage(storage)) if storage.namespace == namespace)
        }) {
            return Err(format!("selected retirement producer lacks {namespace}"));
        }
    }
    let needed = selected_needed(world.runtime_selection(), &lookup)?;
    let mut history = None;
    for (id, row) in rows.iter() {
        row.validate(*id).map_err(|error| error.to_string())?;
        if history.is_some_and(|height| height != row.history_started) {
            return Err("runtime retirement census heights differ".into());
        }
        history = Some(row.history_started);
        if row.receipt.is_some()
            && (needed.contains(id) || !excluded(world.runtime_selection(), *id))
        {
            return Err("permanently excluded definition became executable".into());
        }
    }
    Ok(())
}

/// Bind every current or undo receipt to the original authenticated root and captured height.
pub(crate) fn validate_root(
    world: &impl WorldReadOnly,
    root: RuntimeRetirementRootV1,
    height: u64,
) -> Result<(), String> {
    validate_persistence(world)?;
    if enabled(world.runtime_selection())?
        && world
            .runtime_definitions()
            .iter()
            .any(|(id, _)| world.runtime_retirements().get(id).is_none())
    {
        return Err("authenticated retirement history omits a published definition".into());
    }
    for (_, row) in world.runtime_retirements().iter() {
        if row.history_started > height
            || row.last_required.is_some_and(|last| last > height)
            || row.governed_excluded_at.is_some_and(|at| at > height)
            || row
                .receipt
                .as_ref()
                .is_some_and(|receipt| receipt.root != root || receipt.receipt_height > height)
        {
            return Err(
                "runtime retirement history differs from the authenticated root cut".into(),
            );
        }
    }
    Ok(())
}

/// A permanent receipt cannot lose its service/storage, restriction or dependency exclusion.
pub(crate) fn check_transition(
    world: &impl WorldReadOnly,
    previous: &RootSelectionV1,
    successor: &RootSelectionV1,
) -> Result<(), String> {
    if enabled(previous)? && !enabled(successor)? {
        return Err("permanent retirement history cannot discard its selected producer".into());
    }
    if !enabled(previous)?
        && enabled(successor)?
        && world.runtime_retirements().iter().next().is_some()
    {
        return Err("retirement activation cannot adopt preexisting unproved history".into());
    }
    let lookup = PublishedDefinitions(world.runtime_definitions());
    // Newly published dependencies are checked by ChangeSet::apply; retained tombstones must
    // never reactivate directly even if the successor contains definitions from another release.
    for (id, row) in world.runtime_retirements().iter() {
        if row.receipt.is_some() && !excluded(successor, *id) {
            return Err("governance cannot lift a permanent runtime exclusion".into());
        }
    }
    if enabled(successor)? && !successor.installed_storage.iter().any(|id| matches!(lookup.definition(id), Some(DefinitionDescriptorV1::Storage(storage)) if storage.namespace == "world.runtime_retirements"))
        && enabled(previous)? {
        return Err("permanent runtime history storage cannot be retired".into());
    }
    Ok(())
}

/// Check the permanent census bound before a change set can publish any new descriptor.
/// A future unsupported service retains its own activation semantics; an already selected
/// permanent history continues to enforce its unchanged finite capacity.
pub(crate) fn check_publication_capacity(
    world: &impl WorldReadOnly,
    successor: &RootSelectionV1,
    published: &std::collections::BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) -> Result<(), String> {
    if !enabled(world.runtime_selection())? && !enabled(successor).unwrap_or(false) {
        return Ok(());
    }
    let rows = world.runtime_retirements();
    let definitions = world.runtime_definitions();
    let count = rows
        .iter()
        .map(|_| ())
        .chain(
            definitions
                .iter()
                .filter(|(id, _)| rows.get(id).is_none())
                .map(|_| ()),
        )
        .chain(
            published
                .keys()
                .filter(|id| rows.get(id).is_none() && definitions.get(id).is_none())
                .map(|_| ()),
        )
        .take(MAX_RETIREMENT_DEFINITIONS + 1)
        .count();
    if count > MAX_RETIREMENT_DEFINITIONS {
        return Err("runtime retirement history capacity would be exhausted by publication".into());
    }
    Ok(())
}

/// Seed all previously published definitions once, then record only code-needed use each height.
pub(crate) fn record_block(
    world: &mut WorldTransaction<'_, '_>,
    height: u64,
    now_ms: u64,
) -> Result<(), StateStorageAdmissionError> {
    let rejected =
        |reason: String| StateStorageAdmissionError::UnsupportedRuntime { height, reason };
    if !enabled(world.runtime_selection.get()).map_err(rejected)? {
        return Ok(());
    }
    validate_persistence(&*world).map_err(rejected)?;
    let start = world
        .runtime_retirements
        .iter()
        .next()
        .map_or(height, |(_, row)| row.history_started);
    if start < 2 {
        return Err(rejected(
            "retirement history requires a governed non-genesis activation".into(),
        ));
    }
    if world
        .runtime_definitions
        .iter()
        .take(MAX_RETIREMENT_DEFINITIONS + 1)
        .count()
        > MAX_RETIREMENT_DEFINITIONS
    {
        return Err(rejected(
            "published runtime definitions exceed history capacity".into(),
        ));
    }
    let definitions: Vec<_> = world
        .runtime_definitions
        .iter()
        .map(|(id, _)| *id)
        .collect();
    let new = definitions
        .iter()
        .filter(|id| world.runtime_retirements.get(*id).is_none())
        .count();
    let total = world
        .runtime_retirements
        .iter()
        .count()
        .checked_add(new)
        .ok_or_else(|| rejected("retirement history capacity overflow".into()))?;
    if total > MAX_RETIREMENT_DEFINITIONS {
        return Err(rejected("retirement history capacity exhausted".into()));
    }
    let lookup = PublishedDefinitions(&world.runtime_definitions);
    let mut needed = selected_needed(world.runtime_selection.get(), &lookup).map_err(rejected)?;
    needed.extend(
        LiveObligations::of(world)
            .with_registered(world, now_ms)
            .required_definitions(world.runtime_selection.get(), &lookup)
            .map_err(rejected)?,
    );
    for id in &needed {
        if world
            .runtime_retirements
            .get(id)
            .is_some_and(|row| row.receipt.is_some())
        {
            return Err(rejected(
                "permanent runtime exclusion still has a live code dependency".into(),
            ));
        }
    }
    for id in definitions {
        if world.runtime_retirements.get(&id).is_none() {
            world.runtime_retirements.insert(
                id,
                RuntimeRetirementStateV1 {
                    history_started: start,
                    last_required: Some(height),
                    governed_excluded_at: None,
                    receipt: None,
                },
            );
        } else if needed.contains(&id) {
            let mut row = world
                .runtime_retirements
                .get(&id)
                .expect("existing history")
                .clone();
            row.last_required = Some(height);
            row.governed_excluded_at = None;
            world.runtime_retirements.insert(id, row);
        }
    }
    Ok(())
}

/// Retain only an explicit successful governance transition's exclusion authorization.
/// Deactivation removes its selection restriction, so absence alone cannot reconstruct it.
fn governed_exclusions(
    previous: &RootSelectionV1,
    transitions: &[SelectionTransitionV1],
) -> BTreeSet<DefinitionId> {
    let mut restricted: BTreeSet<_> = previous
        .restrictions
        .iter()
        .filter(|entry| matches!(entry.restriction, RestrictionV1::Disabled(_)))
        .map(|entry| entry.definition)
        .collect();
    let mut exclusions = BTreeSet::new();
    for transition in transitions {
        match transition {
            SelectionTransitionV1::Restrict {
                definition,
                restriction: RestrictionKindV1::Disabled,
            } => {
                restricted.insert(*definition);
                exclusions.insert(*definition);
            }
            SelectionTransitionV1::Restrict { definition, .. }
            | SelectionTransitionV1::Lift(definition) => {
                restricted.remove(definition);
                exclusions.remove(definition);
            }
            SelectionTransitionV1::Deactivate(id) => {
                if restricted.remove(id) {
                    exclusions.insert(*id);
                }
            }
            SelectionTransitionV1::Activate(id) => {
                exclusions.remove(id);
            }
            _ => {}
        }
    }
    exclusions
}

/// Preserve authorized exclusions after an applied change has installed its exact successor.
pub(crate) fn record_governed_exclusions(
    world: &mut WorldTransaction<'_, '_>,
    previous: &RootSelectionV1,
    change: &ChangeSetV1,
    height: u64,
    now_ms: u64,
) -> Result<(), StateStorageAdmissionError> {
    if !enabled(world.runtime_selection.get())
        .map_err(|reason| StateStorageAdmissionError::UnsupportedRuntime { height, reason })?
    {
        return Ok(());
    }
    record_block(world, height, now_ms)?;
    // A withdrawn pending authorization must not survive a same-change reactivation
    // or lift, even when the final selection leaves the definition inactive again.
    for transition in &change.transitions {
        let id = match transition {
            SelectionTransitionV1::Activate(id) | SelectionTransitionV1::Lift(id) => *id,
            SelectionTransitionV1::Restrict {
                definition,
                restriction,
            } if !matches!(restriction, RestrictionKindV1::Disabled) => *definition,
            _ => continue,
        };
        if let Some(mut row) = world.runtime_retirements.get(&id).cloned()
            && row.receipt.is_none()
            && row.governed_excluded_at.take().is_some()
        {
            world.runtime_retirements.insert(id, row);
        }
    }
    for id in governed_exclusions(previous, &change.transitions) {
        if !excluded(world.runtime_selection.get(), id) {
            continue;
        }
        if let Some(mut row) = world.runtime_retirements.get(&id).cloned()
            && row.governed_excluded_at.is_none()
        {
            row.governed_excluded_at = Some(height);
            world.runtime_retirements.insert(id, row);
        }
    }
    Ok(())
}

impl Execute for RetireRuntimeDefinitionV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), IsiError> {
        if !enabled(state.world.runtime_selection.get()).map_err(invalid)? {
            return Err(invalid("retirement producer is not selected"));
        }
        validate_persistence(state.world()).map_err(invalid)?;
        let descriptor = state
            .world
            .runtime_definitions
            .get(&self.definition)
            .ok_or_else(|| invalid("definition was never published on this root"))?;
        if !matches!(descriptor, DefinitionDescriptorV1::Implementation(implementation) if matches!(implementation.code, ImplementationCodeV1::Native { .. }))
        {
            return Err(invalid(
                "stored codecs, readers, profiles and non-native definitions are retained",
            ));
        }
        if !excluded(state.world.runtime_selection.get(), self.definition)
            || state
                .world
                .runtime_retirements
                .get(&self.definition)
                .is_none_or(|row| row.governed_excluded_at.is_none())
        {
            return Err(invalid("implementation has no retained governed exclusion"));
        }
        let now_ms =
            u64::try_from(state._curr_block.creation_time().as_millis()).unwrap_or(u64::MAX);
        let height = state.block_height();
        let lookup = PublishedDefinitions(&state.world.runtime_definitions);
        let live = LiveObligations::of(&state.world).with_registered(&state.world, now_ms);
        if selected_needed(state.world.runtime_selection.get(), &lookup)
            .map_err(invalid)?
            .contains(&self.definition)
            || live
                .required_definitions(state.world.runtime_selection.get(), &lookup)
                .map_err(invalid)?
                .contains(&self.definition)
        {
            return Err(invalid(
                "definition still belongs to a live execution or completion graph",
            ));
        }
        let floor = state
            .world
            .runtime_boundaries
            .iter()
            .filter_map(|(_, boundary)| match boundary {
                RuntimeBoundaryV1::StartupFloor(floor) if floor.effective_ms <= now_ms => {
                    Some(floor.floor)
                }
                _ => None,
            })
            .next_back()
            .ok_or_else(|| invalid("no effective governed startup floor"))?;
        let mut row = state
            .world
            .runtime_retirements
            .get(&self.definition)
            .cloned()
            .ok_or_else(|| invalid("definition has no conservative history census"))?;
        if row.last_required.is_some_and(|last| last >= floor.height) {
            return Err(invalid(
                "startup floor does not exceed the last code-needed height",
            ));
        }
        let scope = crate::executor::root_scope::execution_root_scope(state).map_err(invalid)?;
        let instance = scope
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                *state.network_id(),
                state.chain_id().as_str(),
            )
            .map_err(invalid)?
            .0;
        let receipt = RuntimeRetirementReceiptV1 {
            root: RuntimeRetirementRootV1 {
                network: *state.network_id(),
                scope,
                instance,
            },
            definition: self.definition,
            history_started: row.history_started,
            last_required: row.last_required,
            excluded_at: row
                .receipt
                .as_ref()
                .map_or(height, |receipt| receipt.excluded_at),
            startup_floor: floor,
            receipt_height: height,
        };
        receipt.validate().map_err(invalid)?;
        let bytes = norito::core::classify_decode_attempt(|| norito::encode_canonical(&receipt))
            .map_err(|error| {
                crate::sumeragi::amx::amx_error(
                    &iroha_data_model::sumeragi_amx::AmxError::Resource(error),
                    state,
                )
            })?;
        let key = receipt.witness_key();
        row.receipt = Some(receipt);
        crate::exec_witness::record_write_runtime_retirement(key, bytes);
        state.world.runtime_retirements.insert(self.definition, row);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
