//! Structural runtime binding checks before Torii independently authenticates current custody.
//!
//! Handles and catalog metadata are claims. They cannot qualify hardware possession, policy,
//! revocation, finality or signing completion. Torii owns fresh challenged startup verification
//! after its local committed history is available, and repeats it for each prepared operation.
use super::*;

/// Validate the exact two client routes and any independently supplied deployment anchor.
///
/// A broker resolves only the clients. A deployment registry may separately attach an approved
/// full custody floor; enabled Torii rejects its absence before serving issuance. This layer never
/// obtains that floor from either client, probes a raw signature, or invents finalized state.
pub(super) fn validate_dependency_bindings(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    use IrohaRuntimeProviderRegistryErrorV1 as Error;
    let slot = IrohaRuntimeProviderSlotV1::StreamTokenSigner;
    let Some(expected) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let metadata = expected
        .stream_token_hardware_binding()
        .ok_or(Error::InvalidBinding(slot))?;
    metadata.validate_network(bindings.chain_id(), bindings.network_id().as_bytes())?;
    let hardware = dependencies
        .sorafs_stream_token_hardware_client
        .as_ref()
        .ok_or(Error::IncompleteResolution)?;
    let observer = dependencies
        .sorafs_stream_token_state_observer
        .as_ref()
        .ok_or(Error::IncompleteResolution)?;
    let check_routes = || {
        let hardware_handle = hardware.handle();
        let observer_handle = observer.handle();
        if !is_production_runtime_handle(hardware_handle)
            || !is_production_runtime_handle(observer_handle)
        {
            return Err(Error::TestProviderRejected);
        }
        if hardware_handle != metadata.custody().runtime_handle.as_str()
            || observer_handle != metadata.observer_handle()
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    };
    check_routes()?;
    if dependencies
        .sorafs_stream_token_approved_anchor
        .is_some_and(|approved| approved.config_digest() != metadata.trust_pins_digest())
    {
        return Err(Error::BindingMismatch);
    }
    // Detect a mutable routing claim before handing off. Both reads remain non-authorizing;
    // the actual signed current-state challenge belongs to Torii's private startup owner.
    check_routes()
}

#[cfg(test)]
mod tests;
