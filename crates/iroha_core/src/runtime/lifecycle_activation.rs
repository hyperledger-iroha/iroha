//! Governed lifecycle upgrades and committed activation provenance.

use super::*;
use crate::state::{StorageReadOnly, WorldReadOnly};
use iroha_data_model::{
    NetworkId,
    runtime::{
        definition::SelectionId,
        lifecycle::{
            ChangeSetId, ChangeSetStatusV1, ChangeSetV1, RestrictionKindV1, RootScopeV1,
            SelectionTransitionV1,
        },
    },
};

/// Prepare an ordinary governed lifecycle change set for the authenticated root without changing State.
///
/// Reads the current selection, so already-installed owner cutovers and service
/// definitions survive. Governance notice, provenance, unanimous release support
/// and enactment remain the ordinary committed checks.
/// # Errors
/// The current selection is unsupported, already captured, or has invalid closure.
pub fn prepare_lifecycle_change_set(
    world: &impl WorldReadOnly,
    network: NetworkId,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    let change = from_selection(
        release_catalog()?.clone(),
        world.runtime_selection(),
        &super::change_set::PublishedDefinitions(world.runtime_definitions()),
        network,
        effective_height,
        qualification,
    )?;
    super::draft_scope::bind(world, network, change).map_err(|error| CatalogError::Addition {
        module: "trigger_lifecycle".into(),
        reason: error.to_string(),
    })
}

pub(super) fn from_selection(
    mut catalog: ReleaseCatalog,
    current: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
    network: NetworkId,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    let successor = catalog.lifecycle_successor(current)?;
    let old: std::collections::BTreeSet<_> = current
        .bindings
        .iter()
        .map(|binding| binding.implementation)
        .collect();
    let new: std::collections::BTreeSet<_> = successor
        .bindings
        .iter()
        .map(|binding| binding.implementation)
        .collect();
    let mut transitions: Vec<_> = new
        .into_iter()
        .map(SelectionTransitionV1::Activate)
        .collect();
    transitions.extend(
        successor
            .bindings
            .iter()
            .cloned()
            .map(SelectionTransitionV1::Bind),
    );
    for definition in old {
        transitions.push(SelectionTransitionV1::Restrict {
            definition,
            restriction: RestrictionKindV1::Disabled,
        });
        transitions.push(SelectionTransitionV1::Deactivate(definition));
    }
    transitions.push(SelectionTransitionV1::SetServicesProfile(
        successor.services_profile,
    ));
    // Previously compiled descriptors may not yet be published on the network.
    let needed: std::collections::BTreeSet<_> = successor
        .bindings
        .iter()
        .map(|binding| binding.implementation)
        .chain(std::iter::once(successor.services_profile))
        .collect();
    let publish = catalog
        .definitions
        .iter()
        .filter(|(id, _)| needed.contains(id) && published.definition(id).is_none())
        .map(|(_, value)| value.clone())
        .collect();
    Ok(ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: current.id()?,
        successor: successor.id()?,
        effective_height,
        publish,
        transitions,
        qualification,
    })
}

/// Lifecycle readiness derived only from the committed selection and its activated lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommittedLifecyclePolicy {
    /// The currently selected immutable runtime identity.
    pub selection: SelectionId,
    /// Whether the committed services profile selects lifecycle output.
    pub active: bool,
    /// The governed boundary that enabled the current uninterrupted lifecycle interval.
    pub activation: Option<(ChangeSetId, std::num::NonZeroU64)>,
}

struct FoundedLineage {
    selection: SelectionId,
    root: RootScopeV1,
    lifecycle_enabled: bool,
}

/// Validate the retained foundation of an already authenticated World. This does not turn an
/// ordinary parameter into genesis authority; startup and execution own that authentication.
/// World exposes no chain label, so its native instance remains bound by those original owners.
/// Keep new decoder refusals typed until the existing public string-returning query boundary.
fn founded_lineage(
    world: &impl WorldReadOnly,
    network: NetworkId,
    catalog: &ReleaseCatalog,
) -> Result<FoundedLineage, crate::execution_attempt::ExecutionAttemptError<String>> {
    use crate::execution_attempt::{
        ExecutionAttemptError, canonical_decode_attempt_error, json_decode_attempt_error,
    };
    use iroha_data_model::{
        block::consensus::SumeragiRootScope,
        runtime::{
            child_admin::{ChildAdminErrorV1, foundation::ChildRuntimeFoundationV1},
            definition::SELECTION_ID_DOMAIN,
        },
    };
    let invalid = |reason: &str| ExecutionAttemptError::Rejected(reason.to_owned());
    let model_error = |error| match error {
        ChildAdminErrorV1::Resource(original) => {
            canonical_decode_attempt_error(original, |error| error.to_string())
        }
        ChildAdminErrorV1::CryptoUnavailable(original) => ExecutionAttemptError::Deferred(
            ivm::ExecutionDeferral::CryptoImplementationUnavailable(original).into(),
        ),
        ChildAdminErrorV1::Json(original) => {
            json_decode_attempt_error(original, |error| error.to_string())
        }
        error => ExecutionAttemptError::Rejected(error.to_string()),
    };
    let scope = crate::sumeragi::lanes::routing::read_routing_root_scope(world)
        .map_err(ExecutionAttemptError::Deferred)?;
    let parameter = world
        .parameters()
        .custom()
        .get(ChildRuntimeFoundationV1::parameter_id());
    let authority = world.runtime_child_admin_state().as_ref();
    let Some(SumeragiRootScope::Dataspace {
        parent_network_id,
        dataspace_id,
    }) = scope
    else {
        if authority.is_some() {
            return Err(invalid("child foundation lacks immutable child root scope"));
        }
        // An ordinary global custom value with this name is inert. The new foundation
        // marker acquires meaning only together with the signed child root and its state.
        return Ok(FoundedLineage {
            selection: catalog.baseline_id,
            root: RootScopeV1::Universal,
            lifecycle_enabled: trigger_lifecycle_enabled(&catalog.baseline)
                .map_err(|error| invalid(&error.to_string()))?,
        });
    };
    let parameter =
        parameter.ok_or_else(|| invalid("child root lacks its signed foundation recipe"))?;
    let authority =
        authority.ok_or_else(|| invalid("child root lacks its founded administrator state"))?;
    let recipe = ChildRuntimeFoundationV1::from_custom_parameter(parameter).map_err(model_error)?;
    authority.validate().map_err(model_error)?;
    if authority.root.network != network
        || authority.root.parent_network != parent_network_id
        || authority.root.dataspace != dataspace_id
        || authority.policy.revision < recipe.policy.revision
        || (authority.policy.revision == recipe.policy.revision
            && authority.policy != recipe.policy)
        || authority.next_nonce < authority.policy.revision - recipe.policy.revision
    {
        return Err(invalid(
            "child foundation and committed administrator identity disagree",
        ));
    }
    let (selection, definitions) = catalog
        .child_foundation()
        .map_err(|error| invalid(&error.to_string()))?;
    let bytes = norito::core::classify_decode_attempt(|| norito::encode_canonical(&selection))
        .map_err(|original| canonical_decode_attempt_error(original, |error| error.to_string()))?;
    let identity =
        SelectionId(iroha_crypto::Hash::new_from_chunks(&[SELECTION_ID_DOMAIN, &bytes]).into());
    if identity != recipe.selection
        || definitions
            .iter()
            .any(|(id, expected)| world.runtime_definitions().get(id) != Some(expected))
    {
        return Err(invalid(
            "child foundation differs from its compiled immutable definitions",
        ));
    }
    let lifecycle_enabled =
        trigger_lifecycle_enabled(&selection).map_err(|error| invalid(&error.to_string()))?;
    // This foundation adds obligation/checkpoint storage and administration, not trigger
    // lifecycle execution. Enabling that execution still needs its own governed boundary.
    if lifecycle_enabled {
        return Err(invalid(
            "child foundation unexpectedly enables trigger lifecycle",
        ));
    }
    Ok(FoundedLineage {
        selection: identity,
        root: RootScopeV1::DataSpace(dataspace_id),
        lifecycle_enabled,
    })
}

/// Validate retained child founding data after the caller authenticated its complete native
/// root instance. This preserves original decoder refusals and never admits a universal root.
pub(crate) fn validate_child_foundation(
    world: &impl WorldReadOnly,
    network: NetworkId,
) -> Result<(), crate::execution_attempt::ExecutionAttemptError<String>> {
    use crate::execution_attempt::ExecutionAttemptError;
    let catalog =
        release_catalog().map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
    let foundation = founded_lineage(world, network, catalog)?;
    if !matches!(foundation.root, RootScopeV1::DataSpace(_)) {
        return Err(ExecutionAttemptError::Rejected(
            "administrator authority requires an authenticated child foundation".into(),
        ));
    }
    Ok(())
}

/// Read the activated change-set lineage back to the founded runtime selection.
/// Pending, lapsed and cancelled proposals never select a boundary. A selected
/// successor without its original committed activation record fails closed.
///
/// # Errors
/// The selection or its activation lineage is unsupported, missing, ambiguous,
/// cyclic, foreign, or inconsistent with the committed height.
pub fn committed_policy(
    world: &impl WorldReadOnly,
    network: NetworkId,
    height: u64,
) -> Result<CommittedLifecyclePolicy, String> {
    let catalog = release_catalog().map_err(|error| error.to_string())?;
    // This existing read-only public API returns diagnostic text. The private foundation
    // reader preserves original local refusals; execution must not use this string as rejection.
    let foundation = founded_lineage(world, network, catalog).map_err(|error| error.to_string())?;
    let selected = world.runtime_selection();
    let selection = selected.id().map_err(|error| error.to_string())?;
    let active = trigger_lifecycle_enabled(selected).map_err(|error| error.to_string())?;
    let mut cursor = selection;
    let mut visited = std::collections::BTreeSet::new();
    let mut lineage = Vec::new();
    while cursor != foundation.selection {
        if !visited.insert(cursor) {
            return Err("cyclic committed runtime activation lineage".into());
        }
        let mut candidates = world.runtime_change_sets().iter().filter(|(_, record)| {
            matches!(record.status, ChangeSetStatusV1::Activated(_))
                && record.change_set.successor == cursor
        });
        let Some((id, record)) = candidates.next() else {
            if foundation.root != RootScopeV1::Universal {
                return Err("selected child runtime lacks committed activation provenance".into());
            }
            // The certified repair boundary selected its successor in a transaction overlay. Its
            // retained certificate is the provenance of that one step, which changes no services.
            let repair =
                crate::smartcontracts::isi::sorafs_bootstrap::read_retained_repair_certificate(
                    world,
                )
                .map_err(|error| error.to_string())?
                .filter(|certificate| certificate.plan.change_set.successor == cursor)
                .ok_or("selected runtime lacks committed activation provenance")?;
            cursor = repair.plan.change_set.predecessor;
            continue;
        };
        if candidates.next().is_some() {
            return Err("ambiguous committed runtime activation lineage".into());
        }
        let ChangeSetStatusV1::Activated(activated_height) = record.status else {
            unreachable!("filtered activated records")
        };
        if record.change_set.network != network
            || record.change_set.root != foundation.root
            || record.change_set.id().map_err(|error| error.to_string())? != *id
            || activated_height != record.change_set.effective_height
            || activated_height < 2
            || activated_height > height
        {
            return Err("inconsistent committed runtime activation provenance".into());
        }
        lineage.push((*id, activated_height, &record.change_set));
        cursor = record.change_set.predecessor;
    }
    let mut enabled = foundation.lifecycle_enabled;
    let mut activation = None;
    let mut previous_order = None;
    for (id, activated_height, change) in lineage.into_iter().rev() {
        let order = (activated_height, id);
        if previous_order.is_some_and(|previous| previous >= order) {
            return Err("unordered committed runtime activation lineage".into());
        }
        previous_order = Some(order);
        for transition in &change.transitions {
            if let SelectionTransitionV1::SetServicesProfile(profile) = transition {
                let next = catalog
                    .service_enabled(
                        *profile,
                        "trigger_lifecycle",
                        iroha_crypto::Hash::new(
                            iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
                        )
                        .into(),
                    )
                    .map_err(|error| error.to_string())?;
                if next && !enabled {
                    activation = Some((
                        id,
                        std::num::NonZeroU64::new(activated_height).expect("checked height"),
                    ));
                } else if !next {
                    activation = None;
                }
                enabled = next;
            }
        }
    }
    if enabled != active || active != activation.is_some() {
        return Err(
            "selected lifecycle profile disagrees with committed activation provenance".into(),
        );
    }
    Ok(CommittedLifecyclePolicy {
        selection,
        active,
        activation,
    })
}

/// Bind readiness to signed genesis, the selected runtime, and its original
/// governed activation. Ordinary committed tip advancement does not change it.
#[must_use]
pub fn configuration_fingerprint(
    signed_genesis: iroha_crypto::Hash,
    policy: &CommittedLifecyclePolicy,
) -> iroha_crypto::Hash {
    let mut bytes = b"iroha:governed:lifecycle-activation:v1\0".to_vec();
    bytes.extend_from_slice(signed_genesis.as_ref());
    bytes.extend_from_slice(&policy.selection.0);
    bytes.push(u8::from(policy.active));
    match policy.activation {
        Some((id, height)) => {
            bytes.push(1);
            bytes.extend_from_slice(&id.0);
            bytes.extend_from_slice(&height.get().to_le_bytes());
        }
        None => bytes.push(0),
    }
    iroha_crypto::Hash::new(bytes)
}

#[cfg(test)]
mod tests;
