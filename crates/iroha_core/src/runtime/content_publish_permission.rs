//! The governed content-publishing permission this release adds after its frozen baseline
//! (`specs/content_hosting.md`, "Publishers"; `specs/live_runtime_upgrades.md` §3.1, §4, §7.1).
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
//! - **Selected** (whatever restriction names the definition): the permission is a direct account
//!   permission in its exact unit form, the deleted publisher allow-list one entry per account. No
//!   role carries it (`guard_role_grant`, checked by Core for every executor), so only its issuer
//!   ever confers it: a holder of `CanSetParameters`, directly or through a role, grants and
//!   revokes it. Holding it never authorizes delegation; a holder may renounce its own grant. The
//!   creator of a content bundle may retire it after genesis.
//! - **Active** (selected, and no restriction names it): the Initial executor admits post-genesis
//!   `PublishContentBundle` for an account holding the permission, and the issuer may grant it.
//!   Every restriction kind stops both (§7.1); revocation, renunciation and retirement continue,
//!   so grants and bundles only shrink while it lasts.
//! - **Activation** lapses while any account or role holds a permission of this name, so a token
//!   granted under the baseline's rules never becomes publishing authority.
//! - **Retirement** (leaving the selection) lapses while any account or role holds it or any
//!   bundle published after genesis remains, expired ones included (`check_retirement`).
//!   Retirement returns `RetireContentBundle` to the baseline's genesis-only rule, and a grant left
//!   behind would be an unknown token blocking a later activation. Governance keeps the definition
//!   restricted until both clear.
//! - **Cleanup** stays executable while it is selected (`check_cleanup`): a change set that would
//!   leave `iroha.revoke` unbound or disabled lapses while a direct grant remains, and one that
//!   would do so to `RetireContentBundle` lapses while a bundle published after genesis remains.
//!   Under closed admission or completion-only, revoking a held grant and retiring an existing
//!   bundle still run as completion work (`runtime::obligations`). Nothing else is held back: a
//!   rebinding of either instruction, a kernel or profile change and an executor replacement
//!   proceed.
//!
//! Recognition is never stored: every reader derives it from the selection it executes.

use std::sync::LazyLock;

use iroha_data_model::{
    NetworkId,
    isi::error::InstructionExecutionError,
    permission::Permission,
    runtime::{
        definition::{
            DefinitionDescriptorV1, DefinitionError, DefinitionId, DefinitionLookup,
            PermissionDescriptorV1, RootSelectionV1, SchemaV1,
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

/// Height of the genesis block (`BlockHeader::is_genesis`). A content bundle created above it was
/// published under the permission.
pub(super) const GENESIS_HEIGHT: u64 = 1;

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

/// Whether the root running `selection` admits publishing and new grants under the permission:
/// the definition is selected and no restriction of any kind names it.
#[must_use]
pub fn active(selection: &RootSelectionV1) -> bool {
    selected(selection)
        && DEFINITION.as_ref().is_ok_and(|id| {
            !selection
                .restrictions
                .iter()
                .any(|entry| &entry.definition == id)
        })
}

/// Refuse to put the permission in a role on a root that selected the definition. Core checks
/// this for every executor, so only the issuer's direct account grants confer it.
///
/// # Errors
/// `token` names this permission and the root selected the definition.
pub(crate) fn guard_role_grant(
    world: &impl WorldReadOnly,
    token: &Permission,
) -> Result<(), InstructionExecutionError> {
    if token.name() == NAME && selected(world.runtime_selection()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "content-publishing grants are direct account permissions".into(),
        ));
    }
    Ok(())
}

/// Check that no account or role of `world` holds a permission named [`NAME`].
fn unheld(world: &impl WorldReadOnly, change: &str) -> Result<(), String> {
    let named = |permission: &Permission| permission.name() == NAME;
    if world
        .account_permissions()
        .iter()
        .any(|(_, permissions)| permissions.iter().any(named))
    {
        return Err(format!(
            "{change} {NAME} requires removal of every direct grant of it"
        ));
    }
    if world
        .roles()
        .iter()
        .any(|(_, role)| role.permissions().any(named))
    {
        return Err(format!(
            "{change} {NAME} requires removal of it from every role"
        ));
    }
    Ok(())
}

/// Check that a change set may select the definition on the root of `world`: no account or role
/// holds a permission of this name, so a token granted under the baseline's rules never becomes
/// publishing authority. The change set lapses otherwise (`crate::runtime::change_set`).
///
/// # Errors
/// Where a permission of this name is still held.
pub(crate) fn validate_activation_inventory(world: &impl WorldReadOnly) -> Result<(), String> {
    unheld(world, "activating")
}

/// Check a change set that moves the root of `world` from `current` to `next`: it may stop
/// selecting the definition only once no account or role holds the permission and no content
/// bundle published after genesis remains, expired ones included. The change set lapses
/// otherwise (§6).
///
/// # Errors
/// What still holds the definition in place.
pub(crate) fn check_retirement(
    world: &impl WorldReadOnly,
    current: &RootSelectionV1,
    next: &RootSelectionV1,
) -> Result<(), String> {
    if !selected(current) || selected(next) {
        return Ok(());
    }
    unheld(world, "retiring")?;
    if world
        .content_bundles()
        .iter()
        .any(|(_, record)| record.created_height > GENESIS_HEIGHT)
    {
        return Err(format!(
            "retiring {NAME} requires retiring every content bundle published after genesis"
        ));
    }
    Ok(())
}

/// Check a change set that moves the root of `world` from `current` to `next` while both select
/// the definition: it may stop executing `iroha.revoke` only once no account holds a direct
/// grant, and `RetireContentBundle` only once no content bundle published after genesis remains,
/// expired ones included. An instruction stops executing when `next` leaves it unbound, or
/// disables or drops the interface or implementation it binds; closed admission and
/// completion-only keep cleanup running (`runtime::obligations`). Rebinding either instruction
/// and changing the kernel or a profile are not gaps: every implementation a root selects reaches
/// the executor's one rule for this name. The change set lapses otherwise (§7.1, Permissions).
///
/// # Errors
/// Which cleanup instruction `next` would stop executing, and what still needs it.
pub(crate) fn check_cleanup(
    world: &impl WorldReadOnly,
    current: &RootSelectionV1,
    next: &RootSelectionV1,
) -> Result<(), String> {
    use iroha_data_model::isi::{RevokeBox, content::RetireContentBundle, wire_id_of};

    if !selected(current) || !selected(next) {
        return Ok(());
    }
    let stops = |wire_id: Option<&str>| {
        wire_id.is_some_and(|wire_id| {
            super::obligations::executable(current, wire_id)
                && !super::obligations::executable(next, wire_id)
        })
    };
    if stops(wire_id_of::<RevokeBox>())
        && world
            .account_permissions()
            .iter()
            .any(|(_, permissions)| permissions.iter().any(|held| held.name() == NAME))
    {
        return Err(format!(
            "revocation must stay executable until every direct grant of {NAME} is removed"
        ));
    }
    if stops(wire_id_of::<RetireContentBundle>())
        && world
            .content_bundles()
            .iter()
            .any(|(_, record)| record.created_height > GENESIS_HEIGHT)
    {
        return Err(
            "content bundle retirement must stay executable until every content bundle \
             published after genesis is retired"
                .into(),
        );
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
pub(crate) fn restrict_for_testing(
    world: &crate::state::World,
    restriction: iroha_data_model::runtime::definition::RestrictionV1,
) {
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
