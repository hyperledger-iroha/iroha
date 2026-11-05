//! Governed lifecycle upgrades and committed activation provenance.

use super::*;
use crate::state::WorldReadOnly;
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

/// Prepare an ordinary Parliament change set without changing State.
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
    from_selection(
        release_catalog()?.clone(),
        world.runtime_selection(),
        &super::change_set::PublishedDefinitions(world.runtime_definitions()),
        network,
        effective_height,
        qualification,
    )
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
    use crate::state::StorageReadOnly;
    let catalog = release_catalog().map_err(|error| error.to_string())?;
    let selected = world.runtime_selection();
    let selection = selected.id().map_err(|error| error.to_string())?;
    let active = trigger_lifecycle_enabled(selected).map_err(|error| error.to_string())?;
    let mut cursor = selection;
    let mut visited = std::collections::BTreeSet::new();
    let mut lineage = Vec::new();
    while cursor != catalog.baseline_id {
        if !visited.insert(cursor) {
            return Err("cyclic committed runtime activation lineage".into());
        }
        let mut candidates = world.runtime_change_sets().iter().filter(|(_, record)| {
            matches!(record.status, ChangeSetStatusV1::Activated(_))
                && record.change_set.successor == cursor
        });
        let (id, record) = candidates
            .next()
            .ok_or("selected runtime lacks committed activation provenance")?;
        if candidates.next().is_some() {
            return Err("ambiguous committed runtime activation lineage".into());
        }
        let ChangeSetStatusV1::Activated(activated_height) = record.status else {
            unreachable!("filtered activated records")
        };
        if record.change_set.network != network
            || record.change_set.root != RootScopeV1::Universal
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
    let mut enabled =
        trigger_lifecycle_enabled(&catalog.baseline).map_err(|error| error.to_string())?;
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
