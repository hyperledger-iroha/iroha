//! Captured execution closures for deferred consent and trigger actions (§8.2).
//!
//! Captures follow declared native calls transitively, including cycles, and retain immutable
//! host, storage, permission and service identities. Opaque code captures the complete callable
//! surface until it carries a bounded, authenticated call manifest. A capture never selects old
//! semantics: a changed closure needs new consent.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    isi::{InstructionBox, error::InstructionExecutionError as Error, instruction_wire_id},
    runtime::{
        definition::{
            BindingV1, DefinitionDescriptorV1, DefinitionId, DefinitionLookup, RootSelectionV1,
        },
        lifecycle::{ExpectationV1, RuntimeRejectionV1},
    },
    transaction::{Executable, executable::ExecutableBatchItem},
};

use crate::state::WorldTransaction;

/// Domain-separated canonical commitment to a closure and its installed executor code.
/// Production callers use [`digest_owned`] to retain encoding-refusal ownership.
fn digest(
    expectation: &ExpectationV1,
    executor: iroha_crypto::Hash,
) -> Result<iroha_crypto::Hash, norito::Error> {
    let bytes = norito::encode_canonical(expectation)?;
    Ok(iroha_crypto::Hash::new_from_chunks(&[
        b"iroha:runtime:expectation:v1\0",
        &bytes,
        executor.as_ref(),
    ]))
}

/// Preserve local encoding failures on the transaction's original retry owner.
pub(crate) fn digest_owned(
    expectation: &ExpectationV1,
    world: &WorldTransaction<'_, '_>,
) -> Result<iroha_crypto::Hash, Error> {
    digest(expectation, world.executor.runtime_consent_hash()).map_err(|error| {
        world.attempt_error_to_instruction_error(
            crate::execution_attempt::norito_decode_attempt_error(error, |error| {
                Error::Conversion(error.to_string())
            }),
        )
    })
}

/// Capture the executable retained by the trigger engine, including already loaded IVM code.
pub(crate) fn capture_loaded_executable(
    executable: &crate::smartcontracts::isi::triggers::set::ExecutableRef,
    world: &WorldTransaction<'_, '_>,
) -> Result<ExpectationV1, Error> {
    use crate::smartcontracts::isi::triggers::set::ExecutableRef;
    match executable {
        ExecutableRef::Instructions(instructions) => capture_instructions(instructions, world),
        ExecutableRef::ContractCall(_) | ExecutableRef::Ivm(_) => capture_dynamic(world),
        ExecutableRef::Batch(items) => capture_batch(items, world),
    }
}

/// Capture the immutable execution closure of a deferred instruction list.
pub(crate) fn capture_instructions(
    instructions: &[InstructionBox],
    world: &WorldTransaction<'_, '_>,
) -> Result<ExpectationV1, Error> {
    capture(
        world.runtime_selection.get(),
        &super::change_set::PublishedDefinitions(&world.runtime_definitions),
        instructions.iter().map(instruction_wire_id),
        false,
    )
}

/// Capture an action before it becomes eligible for any trigger event.
pub(crate) fn capture_executable(
    executable: &Executable,
    world: &WorldTransaction<'_, '_>,
) -> Result<ExpectationV1, Error> {
    match executable {
        Executable::Instructions(instructions) => capture_instructions(instructions, world),
        Executable::Batch(items) => capture_batch(items, world),
        Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => {
            capture_dynamic(world)
        }
    }
}

fn capture_dynamic(world: &WorldTransaction<'_, '_>) -> Result<ExpectationV1, Error> {
    capture(
        world.runtime_selection.get(),
        &super::change_set::PublishedDefinitions(&world.runtime_definitions),
        std::iter::empty(),
        true,
    )
}

fn capture_batch(
    items: &[ExecutableBatchItem],
    world: &WorldTransaction<'_, '_>,
) -> Result<ExpectationV1, Error> {
    capture(
        world.runtime_selection.get(),
        &super::change_set::PublishedDefinitions(&world.runtime_definitions),
        items.iter().filter_map(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => Some(instruction_wire_id(instruction)),
            ExecutableBatchItem::ContractCall(_) => None,
        }),
        items
            .iter()
            .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_))),
    )
}

fn capture<'a>(
    selection: &RootSelectionV1,
    lookup: &impl DefinitionLookup,
    wires: impl Iterator<Item = Option<&'a str>>,
    mut exact_bindings: bool,
) -> Result<ExpectationV1, Error> {
    let mut pending = vec![
        selection.kernel,
        selection.commitment_profile,
        selection.services_profile,
    ];
    let mut bindings = BTreeMap::<String, BindingV1>::new();
    for wire in wires {
        let wire = wire.ok_or_else(|| {
            Error::Runtime(RuntimeRejectionV1::ExpectationMismatch { wire_id: None })
        })?;
        // Custom instructions are interpreted by the active executor and can dispatch nested
        // instruction vectors; their payload is not a closed native invocation declaration.
        exact_bindings |= wire == "iroha.custom";
        add_binding(selection, wire, &mut bindings, &mut pending)?;
    }
    if exact_bindings {
        for binding in &selection.bindings {
            add_binding(selection, &binding.wire_id, &mut bindings, &mut pending)?;
        }
    }
    // Host-backed execution can be reached by native wrappers as well as an IVM executable.
    // Retain every currently admitted host; their immutable identities bind ABI and metering.
    pending.extend(
        selection
            .active
            .iter()
            .filter(|id| {
                matches!(
                    lookup.definition(id),
                    Some(DefinitionDescriptorV1::HostProfile(_))
                )
            })
            .copied(),
    );
    let mut definitions = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !definitions.insert(id) {
            continue;
        }
        let descriptor = lookup.definition(&id).ok_or_else(|| {
            Error::Runtime(RuntimeRejectionV1::ExpectationMismatch { wire_id: None })
        })?;
        pending.extend(
            descriptor
                .typed_dependencies()
                .into_iter()
                .map(|(_, dependency)| dependency),
        );
        if let DefinitionDescriptorV1::Implementation(implementation) = descriptor {
            for interface in &implementation.invokes {
                let Some(DefinitionDescriptorV1::Interface(interface)) =
                    lookup.definition(interface)
                else {
                    return Err(RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.into());
                };
                add_binding(selection, &interface.wire_id, &mut bindings, &mut pending)?;
            }
        }
    }
    let expectation = ExpectationV1 {
        bindings: bindings.into_values().collect(),
        services_profile: selection.services_profile,
        kernel: selection.kernel,
        commitment_profile: selection.commitment_profile,
        definitions: definitions.into_iter().collect(),
        exact_bindings,
    };
    expectation.check(selection)?;
    Ok(expectation)
}

fn add_binding(
    selection: &RootSelectionV1,
    wire: &str,
    bindings: &mut BTreeMap<String, BindingV1>,
    pending: &mut Vec<DefinitionId>,
) -> Result<(), Error> {
    let binding = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire))
        .ok()
        .map(|index| &selection.bindings[index])
        .ok_or_else(|| {
            Error::Runtime(RuntimeRejectionV1::DefinitionInactive {
                wire_id: wire.to_owned(),
            })
        })?;
    if bindings.insert(wire.to_owned(), binding.clone()).is_none() {
        pending.extend([binding.interface, binding.implementation]);
    }
    Ok(())
}

/// Refuse changed semantics and any restriction affecting the captured dependency closure.
pub(crate) fn check(
    expectation: &ExpectationV1,
    world: &WorldTransaction<'_, '_>,
    height: u64,
) -> Result<(), Error> {
    check_selection(expectation, world.runtime_selection.get(), height)
}

fn check_selection(
    expectation: &ExpectationV1,
    selection: &RootSelectionV1,
    height: u64,
) -> Result<(), Error> {
    expectation.check(selection)?;
    for definition in &expectation.definitions {
        if let Some(restriction) = super::blocking_restriction(selection, definition, height) {
            return Err(RuntimeRejectionV1::DefinitionRestricted {
                definition: *definition,
                restriction,
            }
            .into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
