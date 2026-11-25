//! Descriptor-selected canonical carrier reconstruction for the dormant generic kernel.
//!
//! The input selection and lookup come from authenticated runtime State, as for native dispatch.
//! Reconstruction preserves the opaque envelope's original bytes and produces no execution or
//! storage capability. The catalog still refuses activation until the same invocation reaches
//! CoreHost's existing ordered effects, rollback and keyed storage owners.

use iroha_data_model::{
    isi::{InstructionBox, error::InstructionExecutionError},
    runtime::{
        carrier::validated_carrier_payload_v1,
        definition::{
            DefinitionDescriptorV1, DefinitionId, DefinitionLookup, EntrypointV1,
            ImplementationCodeV1, ImplementationDescriptorV1, InterfaceDescriptorV1, ProjectionV1,
            RootSelectionV1, SchemaV1, StorageShapeV1,
        },
        lifecycle::RuntimeRejectionV1,
    },
};
use norito::core::{DecodeAttemptError, DecodeAttemptErrorKind};

use super::catalog::{ReleaseCatalog, generic_carriers as profile};

pub(crate) mod admission;
pub(crate) mod artifact_staging;
pub(crate) mod capability;
pub(crate) mod execution;
pub(crate) mod storage;

/// A failed reconstruction; local refusal remains distinct from a committed rejection.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// The ordinary resolved interface or implementation restriction refused execution.
    #[error("carrier resolution: {0}")]
    Rejected(#[from] InstructionExecutionError),
    /// Authenticated State does not satisfy this new profile's closed contract.
    #[error("carrier definition contract: {0}")]
    Contract(&'static str),
    /// Original enclosing codec budget or allocator refusal; never a malformed-payload verdict.
    #[error("carrier original codec refusal")]
    Codec(#[source] DecodeAttemptError),
}

/// Borrowed selected definitions and the validated original frame; no invocation capability.
#[derive(Debug)]
pub(crate) struct DecodedInstruction<'a> {
    /// Exact selected interface identity.
    pub(crate) interface_id: DefinitionId,
    /// Exact selected implementation identity.
    pub(crate) implementation_id: DefinitionId,
    /// Published implementation; the execution owner must separately acquire its artifact.
    pub(crate) implementation: &'a ImplementationDescriptorV1,
    /// Original canonical frame, retained without re-encoding the opaque envelope; its bare
    /// payload satisfies the selected interface's carrier schema.
    pub(crate) frame: &'a [u8],
}

fn require(condition: bool, reason: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Contract(reason))
    }
}

fn malformed(wire_id: &str) -> Error {
    Error::Rejected(
        RuntimeRejectionV1::MalformedPayload {
            wire_id: wire_id.into(),
        }
        .into(),
    )
}

fn unrestricted(selection: &RootSelectionV1, id: &DefinitionId, height: u64) -> Result<(), Error> {
    if let Some(restriction) = super::blocking_restriction(selection, id, height) {
        return Err(Error::Rejected(
            RuntimeRejectionV1::DefinitionRestricted {
                definition: *id,
                restriction,
            }
            .into(),
        ));
    }
    Ok(())
}

fn storage_shape_contract(
    selection: &RootSelectionV1,
    published: &impl DefinitionLookup,
    id: &DefinitionId,
) -> Result<(), Error> {
    require(
        selection.installed_storage.binary_search(id).is_ok(),
        "storage is not installed",
    )?;
    let Some(DefinitionDescriptorV1::Storage(storage)) = published.definition(id) else {
        return Err(Error::Contract("storage descriptor is not published"));
    };
    require(
        matches!(
            &storage.shape,
            StorageShapeV1::Table {
                key: SchemaV1::Carrier(_)
            }
        ) && matches!(&storage.value, SchemaV1::Carrier(_))
            && storage.initial.is_none()
            && storage.projection == ProjectionV1::BareNorito,
        "generic storage requires an empty-installed carrier table and bare projection",
    )?;
    let (key, value) = storage::row_schema_parts(selection, published, *id)?;
    iroha_data_model::runtime::carrier::carrier_schema_hash_v1(key).map_err(Error::Codec)?;
    iroha_data_model::runtime::carrier::carrier_schema_hash_v1(value).map_err(Error::Codec)?;
    Ok(())
}

fn profile_contract(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
    published: &impl DefinitionLookup,
) -> Result<profile::ProfileDefinitions, Error> {
    let profile = catalog.generic_carriers.ok_or(Error::Contract(
        "carrier profile is not in this release catalog",
    ))?;
    require(
        selection.kernel == profile.kernel,
        "selection does not use the exact carrier kernel",
    )?;
    require(
        selection.active.binary_search(&profile.host).is_ok(),
        "carrier host is inactive",
    )?;
    for id in [profile.kernel, profile.host, selection.services_profile] {
        require(
            published.definition(&id).is_some()
                && published.definition(&id) == catalog.definitions.get(&id),
            "carrier kernel, host or services differ from the exact compiled contract",
        )?;
    }
    require(
        selection.commitment_profile == profile.commitment
            && selection
                .installed_storage
                .binary_search(&profile.backing)
                .is_ok()
            && published.definition(&profile.backing) == catalog.definitions.get(&profile.backing)
            && published.definition(&profile.commitment)
                == catalog.definitions.get(&profile.commitment),
        "carrier invocation requires its exact keyed profile and physical backing",
    )?;
    let artifacts = catalog
        .world_storage
        .get("world.runtime_generic_artifacts")
        .ok_or(Error::Contract(
            "carrier artifact storage is not in this release catalog",
        ))?;
    require(
        selection.installed_storage.binary_search(artifacts).is_ok()
            && catalog.definitions.contains_key(artifacts)
            && published.definition(artifacts) == catalog.definitions.get(artifacts),
        "carrier invocation requires its exact installed artifact storage",
    )?;
    Ok(profile)
}

fn binding_contract<'a>(
    profile: profile::ProfileDefinitions,
    selection: &RootSelectionV1,
    published: &'a impl DefinitionLookup,
    wire_id: &str,
    interface_id: DefinitionId,
    implementation_id: DefinitionId,
) -> Result<(&'a InterfaceDescriptorV1, &'a ImplementationDescriptorV1), Error> {
    require(
        selection.active.binary_search(&interface_id).is_ok()
            && selection.active.binary_search(&implementation_id).is_ok(),
        "carrier interface or implementation is inactive",
    )?;
    let Some(DefinitionDescriptorV1::Interface(interface)) = published.definition(&interface_id)
    else {
        return Err(Error::Contract("carrier interface is not published"));
    };
    require(
        interface.wire_id == wire_id,
        "carrier wire ID differs from its selected interface",
    )?;
    require(
        iroha_data_model::isi::is_well_formed_wire_id(&interface.wire_id)
            && !iroha_data_model::isi::registry::built_in_instruction_schemas()
                .any(|entry| entry.wire_id == interface.wire_id),
        "carrier interface collides with a registered native wire ID or has invalid syntax",
    )?;
    require(
        interface.metering_class == profile::METERING_CLASS,
        "carrier interface names a different metering class",
    )?;
    let SchemaV1::Carrier(schema) = &interface.payload else {
        return Err(Error::Contract(
            "carrier interface has no carrier payload schema",
        ));
    };
    iroha_data_model::runtime::carrier::carrier_schema_hash_v1(schema).map_err(Error::Codec)?;
    let Some(DefinitionDescriptorV1::Implementation(implementation)) =
        published.definition(&implementation_id)
    else {
        return Err(Error::Contract("carrier implementation is not published"));
    };
    require(
        matches!(&implementation.code, ImplementationCodeV1::Ivm { entrypoint, .. } if entrypoint == "main")
            && matches!(&implementation.entrypoint, EntrypointV1::Instructions(interfaces)
            if interfaces.len() == 1 && interfaces[0] == interface_id)
            && implementation.host_profile == Some(profile.host)
            && implementation.services_profile == selection.services_profile,
        "carrier implementation must be one IVM instruction under the exact selected profiles",
    )?;
    require(
        interface.events.is_empty()
            && implementation.events.is_empty()
            && implementation.permissions.is_empty()
            && implementation.parameters.is_empty()
            && implementation.invokes.is_empty()
            && implementation.scans.is_empty(),
        "initial carrier profile forbids events, permissions, parameters, scans and nested calls",
    )?;
    require(
        implementation.limits.max_gas > 0
            && implementation.limits.max_gas <= profile::MAX_GAS
            && implementation.limits.max_effects > 0
            && implementation.limits.max_effects <= profile::MAX_EFFECTS
            && implementation.limits.max_call_depth == 1,
        "carrier resource limits exceed the restricted profile",
    )?;
    for dependencies in [&implementation.reads, &implementation.writes] {
        require(
            dependencies.len() <= profile::MAX_STORAGE_DEPENDENCIES
                && dependencies.windows(2).all(|pair| pair[0] < pair[1]),
            "carrier storage dependencies exceed the canonical bounded list",
        )?;
        for id in dependencies {
            storage_shape_contract(selection, published, id)?;
        }
    }
    Ok((interface, implementation))
}

/// Reconstruct one opaque instruction only under the exact separately selected carrier profile.
///
/// `None` leaves all pre-existing kernels and already typed instructions to their original
/// dispatch. A selected carrier kernel resolves the ordinary binding/restrictions first, then
/// checks its exact descriptor contract before reading payload bytes. No obligation exception
/// exists in this initial profile, whose nested calls and obligation operations are forbidden.
///
/// This is a decoder for authenticated, closure-checked State; it does not authenticate a
/// caller-built descriptor map, acquire an artifact or authorize execution. The catalog's
/// support refusal prevents this preparatory profile from reaching block admission today.
///
/// # Errors
/// An ordinary restriction/rejection, unsupported descriptor shape, or unchanged local refusal.
pub(crate) fn decode_selected<'a>(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
    published: &'a impl DefinitionLookup,
    height: u64,
    instruction: &'a InstructionBox,
) -> Result<Option<DecodedInstruction<'a>>, Error> {
    let Some(profile) = catalog.generic_carriers else {
        return Ok(None);
    };
    if selection.kernel != profile.kernel {
        return Ok(None);
    }
    let Some(opaque) = instruction.as_opaque() else {
        return Ok(None);
    };
    let implementation_id = super::resolve(instruction, selection, height, || None)?.ok_or(
        Error::Contract("carrier envelope has no resolved implementation"),
    )?;
    let wire_id = opaque.wire_id();
    let binding = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .map(|index| &selection.bindings[index])
        .map_err(|_| Error::Contract("resolved carrier binding disappeared"))?;
    require(
        selection.active.binary_search(&binding.interface).is_ok()
            && selection.active.binary_search(&implementation_id).is_ok()
            && selection.active.binary_search(&profile.host).is_ok(),
        "carrier interface, implementation or host is inactive",
    )?;
    profile_contract(catalog, selection, published)?;
    unrestricted(
        selection,
        &catalog.world_storage["world.runtime_generic_artifacts"],
        height,
    )?;
    unrestricted(selection, &profile.host, height)?;
    let (interface, implementation) = binding_contract(
        profile,
        selection,
        published,
        wire_id,
        binding.interface,
        implementation_id,
    )?;
    for id in implementation.reads.iter().chain(&implementation.writes) {
        unrestricted(selection, id, height)?;
    }
    let SchemaV1::Carrier(schema) = &interface.payload else {
        unreachable!("shared carrier contract checked the exact schema")
    };
    if opaque.is_dense() {
        return Err(malformed(wire_id));
    }
    validated_carrier_payload_v1(schema, opaque.frame()).map_err(|error| {
        if error.kind() == DecodeAttemptErrorKind::Invalid {
            malformed(wire_id)
        } else {
            Error::Codec(error)
        }
    })?;
    Ok(Some(DecodedInstruction {
        interface_id: binding.interface,
        implementation_id,
        implementation,
        frame: opaque.frame(),
    }))
}

#[cfg(test)]
mod tests;
