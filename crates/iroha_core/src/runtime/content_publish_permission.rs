//! The governed content-publishing permission this release adds after its frozen baseline
//! (`specs/content_hosting.md`; `specs/live_runtime_upgrades.md` §3.1, §4).
//!
//! The frozen baseline admits `PublishContentBundle` and `RetireContentBundle` in genesis only
//! under the Initial executor and treats [`NAME`] as an unknown custom permission name.
//! Recognizing it is new behaviour, so the release publishes it as one Permission definition after
//! the baseline and nothing else: no implementation, interface, rebind or storage. The definition
//! therefore activates from every selection a root may run, and `iroha.grant` and `iroha.revoke`
//! keep whichever implementation the root selected; each of them reaches the Initial executor's
//! one rule for this name (`crate::executor`).
//!
//! - **Not selected** (every root until a governed change set activates the definition): the root
//!   executes exactly as the frozen baseline does.
//! - **Selected** (active, whatever restriction names it): only a holder of `CanSetParameters`,
//!   directly or through a role, grants or revokes the exact unit permission on an account or a
//!   role, and may remove it from a role it does not hold. Holding it never authorizes delegation;
//!   a holder may renounce its own direct grant. The creator of a content bundle may retire it
//!   after genesis, so every published bundle keeps its terminal path.
//! - **Recognized** (selected, and no `Disabled` restriction names it): the Initial executor admits
//!   post-genesis `PublishContentBundle` for an authority holding the permission directly or
//!   through a role. A `Disabled` restriction stops publishing and new grants; revocation
//!   continues, so the grant set only shrinks while it lasts. §7.1 defines the other restrictions
//!   for interfaces and implementations only; they do not change a permission.
//! - **Changing selection** (activation or deactivation) lapses while any account or role holds a
//!   permission of this name (`validate_inventory`): a grant made under the baseline's rules
//!   never becomes publishing authority, and no grant outlives the definition as an opaque token
//!   that those rules would let its holder pass on.
//!
//! Recognition is never stored: every reader derives it from the selection it executes.

use std::sync::LazyLock;

use iroha_data_model::{
    NetworkId,
    permission::Permission,
    runtime::{
        definition::{
            DefinitionDescriptorV1, DefinitionError, DefinitionId, DefinitionLookup,
            PermissionDescriptorV1, RestrictionV1, RootSelectionV1, SchemaV1,
        },
        lifecycle::{ChangeSetV1, RootScopeV1, SelectionTransitionV1},
    },
};

use super::catalog::CatalogError;
use crate::state::{StorageReadOnly, WorldReadOnly};

/// Name of the permission
/// ([`iroha_executor_data_model::permission::content::CanPublishContentBundle`]).
pub const NAME: &str = "CanPublishContentBundle";

/// Module name an operator drafts the activation under ([`crate::runtime::draft_addition`]).
pub const MODULE: &str = "iroha.permission.content_publish.v1";

/// The Permission definition this release publishes after the baseline. Its payload is the exact
/// unit token.
#[must_use]
pub fn descriptor() -> DefinitionDescriptorV1 {
    let layout = crate::state::authority_registry::V1_LAYOUT;
    DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
        name: NAME.into(),
        payload: SchemaV1::Codec(format!(
            "semantic:iroha.content-publish.unit.json.v1@{}.{}.{}",
            layout.major, layout.minor, layout.flags,
        )),
    })
}

static DEFINITION: LazyLock<Result<DefinitionId, DefinitionError>> =
    LazyLock::new(|| descriptor().id());

/// The only form of the permission that carries authority.
#[must_use]
pub fn token() -> Permission {
    iroha_executor_data_model::permission::content::CanPublishContentBundle.into()
}

/// Whether the root running `selection` activates the definition, whatever restriction names it.
#[must_use]
pub fn selected(selection: &RootSelectionV1) -> bool {
    DEFINITION
        .as_ref()
        .is_ok_and(|id| selection.active.binary_search(id).is_ok())
}

/// Whether the root running `selection` admits publishing under the permission: the definition
/// is selected and no `Disabled` restriction names it.
#[must_use]
pub fn recognized(selection: &RootSelectionV1) -> bool {
    selected(selection)
        && DEFINITION.as_ref().is_ok_and(|id| {
            !selection.restrictions.iter().any(|entry| {
                &entry.definition == id && matches!(entry.restriction, RestrictionV1::Disabled(_))
            })
        })
}

/// Check that no account or role of `world` holds a permission named [`NAME`]. A change set that
/// activates or deactivates the definition lapses otherwise (`crate::runtime::change_set`).
///
/// # Errors
/// Where a permission of this name is still held.
pub(crate) fn validate_inventory(world: &impl WorldReadOnly) -> Result<(), String> {
    let named = |permission: &Permission| permission.name() == NAME;
    if world
        .account_permissions()
        .iter()
        .any(|(_, permissions)| permissions.iter().any(named))
    {
        return Err(format!(
            "selecting or retiring {NAME} requires removal of every direct grant of it"
        ));
    }
    if world
        .roles()
        .iter()
        .any(|(_, role)| role.permissions().any(named))
    {
        return Err(format!(
            "selecting or retiring {NAME} requires removal of it from every role"
        ));
    }
    Ok(())
}

/// The change set that makes the universal root of `current` select the permission (§4): it
/// publishes the definition if `published` lacks it and activates it at `effective_height`. It
/// installs, binds and restricts nothing, so it applies to every selection a root may run.
///
/// # Errors
/// The definition is already active, this release does not publish it, or the change set does
/// not apply to `current`.
pub fn activation(
    current: &RootSelectionV1,
    published: &impl DefinitionLookup,
    network: NetworkId,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    let fail = |reason: String| CatalogError::Addition {
        module: MODULE.to_owned(),
        reason,
    };
    if selected(current) {
        return Err(fail(format!("{NAME} is already active")));
    }
    let id = descriptor().id()?;
    let descriptor = super::release_catalog()?
        .definitions
        .get(&id)
        .cloned()
        .ok_or_else(|| fail(format!("this release does not publish {NAME}")))?;
    let mut successor = current.clone();
    successor.active.push(id);
    successor.active.sort_unstable();
    let change_set = ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: current.id()?,
        successor: successor.id()?,
        effective_height,
        publish: if published.definition(&id).is_none() {
            vec![descriptor]
        } else {
            Vec::new()
        },
        transitions: vec![SelectionTransitionV1::Activate(id)],
        qualification,
    };
    change_set
        .apply(current, published)
        .map_err(|error| fail(error.to_string()))?;
    Ok(change_set)
}

/// Select the permission on `world` as its governed activation would at height 1.
///
/// # Errors
/// The activation cannot be drafted or does not apply.
#[cfg(test)]
pub(crate) fn activate_for_testing(world: &crate::state::World) -> Result<(), String> {
    // A change set's network scope does not shape the selection it produces.
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0; 32]),
    ));
    let mut block = world.block();
    let current = block.runtime_selection.get().clone();
    let published = super::change_set::PublishedDefinitions(&block.runtime_definitions);
    let change =
        activation(&current, &published, network, 1, [0; 32]).map_err(|error| error.to_string())?;
    let applied = change
        .apply(&current, &published)
        .map_err(|error| error.to_string())?;
    for (definition, descriptor) in applied.published {
        block.runtime_definitions.insert(definition, descriptor);
    }
    *block.runtime_selection.get_mut() = applied.selection;
    block.commit();
    Ok(())
}

/// Restrict the selected definition of `world` as a governed change set would at height 1.
#[cfg(test)]
pub(crate) fn restrict_for_testing(world: &crate::state::World, restriction: RestrictionV1) {
    let id = descriptor().id().expect("compiled permission identity");
    let mut block = world.block();
    let selection = block.runtime_selection.get_mut();
    selection
        .restrictions
        .retain(|entry| entry.definition != id);
    selection
        .restrictions
        .push(iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition: id,
            restriction,
        });
    selection.restrictions.sort_by_key(|entry| entry.definition);
    block.commit();
}

#[cfg(test)]
mod tests;
