//! Live runtime upgrades: the definitions this release executes and the selection each root runs
//! (`specs/live_runtime_upgrades.md`).
//!
//! A World of this release is founded on the baseline of [`catalog`]: its `runtime_definitions`
//! table holds the published definitions and its `runtime_selection` cell the one selection the
//! universal root executes. Execution resolves every instruction against that selection, and the
//! State commitment covers exactly the storage it installs.

pub mod attestation;
pub mod catalog;
pub(crate) mod change_set;
pub(crate) mod containment;
pub(crate) mod expectation;
pub(crate) mod obligations;

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
/// (§7.1), unless the instruction continues or completes a live obligation that the restriction
/// lets through (§7.2): `obligation_step` says how it acts on one, and runs only for a restricted
/// binding. Admission closure lets work already admitted continue; completion-only lets it
/// complete. Every node resolves an envelope against the same committed selection and State, so
/// the first envelope a transaction cannot execute is the same everywhere. The bound implementation is
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
    obligation_step: impl FnOnce() -> Option<obligations::Step>,
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
    let mut obligation_step = Some(obligation_step);
    let mut step = None;
    for definition in [binding.interface, binding.implementation] {
        if let Some(restriction) = blocking_restriction(selection, &definition, height) {
            let step: Option<obligations::Step> =
                *step.get_or_insert_with(|| obligation_step.take().and_then(|step| step()));
            if !step.is_some_and(|step| step.runs_under(restriction)) {
                return Err(RuntimeRejectionV1::DefinitionRestricted {
                    definition,
                    restriction,
                }
                .into());
            }
        }
    }
    Ok(())
}

/// The restriction of `definition` that stops new work at `height`, if any (§7.1).
///
/// Deprecation never stops execution. [`resolve`] still lets a live obligation's continuations or
/// completions through admission closure and completion-only ([`obligations::Step::runs_under`]).
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
