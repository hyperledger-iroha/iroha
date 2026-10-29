//! Live runtime upgrades: the definitions this release executes and the selection each root runs
//! (`specs/live_runtime_upgrades.md`).
//!
//! A World of this release is founded on the baseline of [`catalog`]: its `runtime_definitions`
//! table holds the published definitions and its `runtime_selection` cell the one selection the
//! universal root executes. Execution resolves every instruction against that selection, and the
//! State commitment covers exactly the storage it installs.

pub mod catalog;

use iroha_data_model::runtime::{
    definition::{DefinitionDescriptorV1, DefinitionId, RestrictionV1, RootSelectionV1},
    lifecycle::RuntimeRejectionV1,
};
use mv::storage::Storage;

pub use catalog::{CatalogError, ReleaseCatalog, release_catalog};

/// The definitions a new World is founded with: the baseline's.
///
/// # Errors
/// The release catalog cannot be built (a build defect).
pub(crate) fn founded_definitions()
-> Result<Storage<DefinitionId, DefinitionDescriptorV1>, CatalogError> {
    let catalog = release_catalog()?;
    Ok(catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect())
}

/// The selection a new World is founded with: the baseline.
///
/// # Errors
/// The release catalog cannot be built (a build defect).
pub(crate) fn founded_selection() -> Result<RootSelectionV1, CatalogError> {
    Ok(release_catalog()?.baseline.clone())
}

/// Resolve `instruction` against the selection its root executes at `height` (§5.2).
///
/// The instruction's wire ID must be bound, and neither its interface nor the bound
/// implementation may have admission closed, be completion-only or be disabled at `height`
/// (§7.1). Every node resolves an envelope against the same committed selection, so the first
/// envelope a transaction cannot execute is the same everywhere. The bound implementation is
/// native code of this release: a node whose release cannot execute the selection stops before
/// the height rather than reaching this check (§6).
///
/// An instruction without a wire ID never arrives in a transaction or contract: decoding
/// accepts only registered wire IDs. Native code constructs it as a private primitive inside its
/// own reference closure (§3.2), which the resolution of the instruction that invoked it covers.
///
/// # Errors
/// The stable rejection of the live-upgrade mechanism.
pub(crate) fn resolve(
    instruction: &iroha_data_model::isi::InstructionBox,
    selection: &RootSelectionV1,
    height: u64,
) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
    let Some(wire_id) = iroha_data_model::isi::instruction_wire_id(instruction) else {
        return Ok(());
    };
    let binding = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .map(|index| &selection.bindings[index])
        .map_err(|_| RuntimeRejectionV1::DefinitionInactive {
            wire_id: wire_id.to_owned(),
        })?;
    for definition in [binding.interface, binding.implementation] {
        if let Some(restriction) = blocking_restriction(selection, &definition, height) {
            return Err(RuntimeRejectionV1::DefinitionRestricted {
                definition,
                restriction,
            }
            .into());
        }
    }
    Ok(())
}

/// The restriction of `definition` that stops new work at `height`, if any (§7.1).
///
/// Deprecation never stops execution. Completion capabilities are registered with obligations
/// (§7.2); until an obligation registers one, completion-only stops a definition like admission
/// closure.
fn blocking_restriction(
    selection: &RootSelectionV1,
    definition: &DefinitionId,
    height: u64,
) -> Option<RestrictionV1> {
    let index = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(definition))
        .ok()?;
    let restriction = selection.restrictions[index].restriction;
    match restriction {
        RestrictionV1::Deprecated(_) => None,
        RestrictionV1::AdmissionClosed(from)
        | RestrictionV1::CompletionOnly(from)
        | RestrictionV1::Disabled(from) => (from <= height).then_some(restriction),
    }
}

#[cfg(test)]
mod tests;
