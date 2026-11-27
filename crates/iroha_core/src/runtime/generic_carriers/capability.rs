//! Read-only descriptor-language capability for a separately selected carrier root.
//!
//! This is not a release qualification, closure proof, artifact-availability check, support
//! attestation or activation capability. Those owners must authenticate the original selection,
//! published definitions and artifact custody independently. Native catalog support is unchanged.

use super::*;
use iroha_data_model::runtime::carrier::carrier_definition_id_v1;

/// Borrowed observation of the exact carrier contract understood by this compiled implementation.
/// The private result cannot be serialized or supplied as activation or artifact authority.
pub(crate) struct CarrierCapability<'a> {
    _selection: &'a RootSelectionV1,
}

fn descriptor<'a>(
    catalog: &ReleaseCatalog,
    published: &'a impl DefinitionLookup,
    id: &DefinitionId,
) -> Result<(&'a DefinitionDescriptorV1, bool), Error> {
    let definition = published.definition(id).ok_or(Error::Contract(
        "selected capability descriptor is not published",
    ))?;
    if let Some(compiled) = catalog.definitions.get(id) {
        require(
            definition == compiled,
            "selected compiled descriptor was substituted",
        )?;
        Ok((definition, true))
    } else {
        require(
            carrier_definition_id_v1(definition).map_err(Error::Codec)? == *id,
            "selected dynamic descriptor identity differs from its original bytes",
        )?;
        Ok((definition, false))
    }
}

fn selected_interface<'a>(
    selection: &RootSelectionV1,
    published: &'a impl DefinitionLookup,
    interface_id: &DefinitionId,
) -> Result<&'a InterfaceDescriptorV1, Error> {
    require(
        selection.active.binary_search(interface_id).is_ok(),
        "capability interface is inactive",
    )?;
    let Some(DefinitionDescriptorV1::Interface(interface)) = published.definition(interface_id)
    else {
        return Err(Error::Contract("capability interface is not published"));
    };
    let binding = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.cmp(&interface.wire_id))
        .map(|index| &selection.bindings[index])
        .map_err(|_| Error::Contract("active capability interface is unbound"))?;
    require(
        binding.interface == *interface_id,
        "capability binding substitutes its interface",
    )?;
    Ok(interface)
}

/// Check every selected dynamic declaration using the same contract as carrier invocation.
///
/// The caller supplies its original published lookup, not a list of claimed supported IDs.
/// Unselected additions are irrelevant. Restrictions do not erase retained code requirements;
/// invocation still enforces their exact height separately. This observation grants no release
/// support, attestation, artifact-availability or activation authority, even on success.
///
/// # Errors
/// Missing or substituted owners, an unsupported dynamic shape/wire ID, or the unchanged
/// original codec resource refusal. The existing native-only support API is not called or widened.
pub(crate) fn validate_compiled_selection<'a>(
    catalog: &ReleaseCatalog,
    selection: &'a RootSelectionV1,
    published: &'a impl DefinitionLookup,
) -> Result<CarrierCapability<'a>, Error> {
    require(
        selection.active.windows(2).all(|pair| pair[0] < pair[1])
            && selection
                .installed_storage
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && selection
                .bindings
                .windows(2)
                .all(|pair| pair[0].wire_id < pair[1].wire_id),
        "capability selection lists are not canonical",
    )?;
    let profile = profile_contract(catalog, selection, published)?;
    // This is the existing selected schema's allocation-free shape preflight. Its returned
    // geometry is not a reservation or authority to allocate a directory here.
    iroha_data_model::sumeragi_finality::keyed_schema::SelectedStateSchemaV1::carrier_selection_allocation_bytes(selection, published)
        .map_err(|error| match error {
            iroha_data_model::sumeragi_finality::keyed_schema::CarrierStateSchemaError::Invalid(reason) => Error::Contract(reason),
            iroha_data_model::sumeragi_finality::keyed_schema::CarrierStateSchemaError::Codec(original) => Error::Codec(original),
        })?;
    for (index, id) in selection.installed_storage.iter().enumerate() {
        let (definition, compiled) = descriptor(catalog, published, id)?;
        let DefinitionDescriptorV1::Storage(storage) = definition else {
            return Err(Error::Contract("installed capability owner is not storage"));
        };
        for before in &selection.installed_storage[..index] {
            if matches!(published.definition(before), Some(DefinitionDescriptorV1::Storage(prior)) if prior.namespace == storage.namespace)
            {
                return Err(Error::Contract("capability storage namespaces overlap"));
            }
        }
        if !compiled {
            require(
                !catalog
                    .world_storage
                    .contains_key(storage.namespace.as_str()),
                "dynamic capability namespace belongs to a compiled native storage owner",
            )?;
            storage_shape_contract(selection, published, id)?;
        }
    }
    for id in &selection.active {
        let (definition, compiled) = descriptor(catalog, published, id)?;
        if compiled {
            continue;
        }
        match definition {
            DefinitionDescriptorV1::Interface(_) => {
                let interface = selected_interface(selection, published, id)?;
                let binding = &selection.bindings[selection
                    .bindings
                    .binary_search_by(|binding| binding.wire_id.cmp(&interface.wire_id))
                    .expect("selected_interface found this exact binding")];
                binding_contract(
                    profile,
                    selection,
                    published,
                    &interface.wire_id,
                    *id,
                    binding.implementation,
                )?;
            }
            DefinitionDescriptorV1::Implementation(implementation) => {
                let EntrypointV1::Instructions(interfaces) = &implementation.entrypoint else {
                    return Err(Error::Contract(
                        "dynamic capability implementation is not an instruction",
                    ));
                };
                require(
                    interfaces.len() == 1,
                    "dynamic capability implementation has multiple interfaces",
                )?;
                let interface = selected_interface(selection, published, &interfaces[0])?;
                binding_contract(
                    profile,
                    selection,
                    published,
                    &interface.wire_id,
                    interfaces[0],
                    *id,
                )?;
            }
            _ => {
                return Err(Error::Contract(
                    "dynamic capability definition kind is unsupported",
                ));
            }
        }
    }
    for binding in &selection.bindings {
        let (interface, interface_compiled) = descriptor(catalog, published, &binding.interface)?;
        let (implementation, implementation_compiled) =
            descriptor(catalog, published, &binding.implementation)?;
        let DefinitionDescriptorV1::Interface(interface) = interface else {
            return Err(Error::Contract(
                "capability binding owner is not an interface",
            ));
        };
        require(
            interface.wire_id == binding.wire_id
                && selection.active.binary_search(&binding.interface).is_ok()
                && selection
                    .active
                    .binary_search(&binding.implementation)
                    .is_ok(),
            "capability binding is not the selected active interface and implementation",
        )?;
        if interface_compiled && implementation_compiled {
            require(
                matches!(implementation, DefinitionDescriptorV1::Implementation(implementation)
                if matches!(&implementation.entrypoint, EntrypointV1::Instructions(interfaces) if interfaces.binary_search(&binding.interface).is_ok())),
                "compiled capability binding does not answer its interface",
            )?;
        } else {
            binding_contract(
                profile,
                selection,
                published,
                &binding.wire_id,
                binding.interface,
                binding.implementation,
            )?;
        }
    }
    Ok(CarrierCapability {
        _selection: selection,
    })
}

#[cfg(test)]
mod tests;
