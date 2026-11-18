//! Governed C10 host, event and certified result service.
use super::*;

const SERVICE: &str = "ivm_c10_execution_v1";
const SERVICE_CLOSURE: &[u8] = b"iroha.runtime.ivm_c10.v1:ExecutionOutputC10V1:TransactionResultC10V1:EventBoxC10V1:profile-bound-dispatch-v1";

pub(crate) fn selected(selection: &RootSelectionV1) -> bool {
    release_catalog().is_ok_and(|catalog| {
        matches!(
            catalog.service_enabled(
                selection.services_profile,
                SERVICE,
                iroha_crypto::Hash::new(SERVICE_CLOSURE).into()
            ),
            Ok(true)
        )
    })
}

/// A new selection may enable C10 only with all its compiled opening interfaces, host and storage.
/// Subsequent governed restrictions continue to operate on those same immutable definitions.
pub(crate) fn validate_transition(
    previous: &RootSelectionV1,
    selection: &RootSelectionV1,
) -> Result<(), String> {
    let catalog = release_catalog().map_err(|e| e.to_string())?;
    let Some(addition) = catalog
        .additions
        .iter()
        .find(|a| a.module == additions::IVM_C10.module)
    else {
        return Ok(());
    };
    let host = crate::smartcontracts::ivm::profile::ResolvedIvmProfile::of_id(
        ivm::IvmProfileId::C10_ABI_V1,
    )
    .definition_id();
    let host_active = selection.active.binary_search(&host).is_ok();
    let services = selected(selection);
    let selected_bindings = addition
        .interfaces
        .keys()
        .filter(|wire| {
            selection.bindings.iter().any(|binding| {
                binding.wire_id.as_str() == **wire
                    && catalog.native_module(&binding.implementation) == Some(addition.module)
            })
        })
        .count();
    let auxiliary_active = catalog
        .c10_compositions
        .get(&previous.services_profile)
        .is_some_and(|composition| composition.selects_auxiliary_definitions(selection));
    let installed = addition
        .storage
        .values()
        .filter(|id| selection.installed_storage.binary_search(id).is_ok())
        .count();
    // Retirement and restriction of existing obligations use the ordinary governance rules.
    // Only opening the C10 family requires this release's whole registration closure at once.
    if selected(previous) && (services || !host_active && selected_bindings == 0) {
        return Ok(());
    }
    // A closed service may retain installed tables while outstanding obligations drain.
    if !services && !host_active && selected_bindings == 0 {
        return Ok(());
    }
    if services
        && host_active
        && auxiliary_active
        && selected_bindings == addition.interfaces.len()
        && installed == addition.storage.len()
    {
        return Ok(());
    }
    Err("incomplete C10 execution service, interfaces, profile or storage".into())
}

impl ReleaseCatalog {
    pub(super) fn compile_ivm_c10(&mut self) -> Result<(), CatalogError> {
        if !self
            .additions
            .iter()
            .any(|addition| addition.module == additions::IVM_C10.module)
        {
            return Ok(());
        }
        let profile = ivm::IvmProfileId::C10_ABI_V1;
        let host = self.publish(DefinitionDescriptorV1::HostProfile(
            HostProfileDescriptorV1 {
                abi_version: 1,
                abi_hash: profile.abi_hash(),
                metering: ivm::gas::schedule_hash_for(profile.engine()).into(),
            },
        ))?;
        let layout = layout_suffix(crate::state::authority_registry::V1_LAYOUT);
        let event = self.publish(DefinitionDescriptorV1::Event(EventDescriptorV1 {
            name: "iroha.events.ivm_c10.v1".into(),
            schema: SchemaV1::Codec(format!(
                "norito:{}@{layout}",
                norito::schema::identity::nominal_name::<iroha_data_model::ivm_c10::EventBoxC10V1>(
                )
            )),
        }))?;
        self.c10_compositions = self.compose_service_addition(
            additions::IVM_C10.module,
            ServiceEntryV1 {
                name: SERVICE.into(),
                reference_closure: iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
            },
            Some(host),
            Some(event),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
