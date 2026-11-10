//! Authority for a governed exact State row read. Its payload lives in a direct account grant.

use crate::state::{StorageReadOnly, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::error::InstructionExecutionError,
    permission::Permission,
    runtime::definition::{
        DefinitionDescriptorV1, DefinitionId, PermissionDescriptorV1, RootSelectionV1, SchemaV1,
    },
};
use iroha_executor_data_model::permission::query::{CanReadAllLedgerData, CanReadStateRow};
use std::sync::LazyLock;

const NAME: &str = "CanReadStateRow";

/// Permission definition published and activated with the governed keyed commitment.
pub(crate) fn descriptor() -> DefinitionDescriptorV1 {
    let layout = crate::state::authority_registry::V1_LAYOUT;
    DefinitionDescriptorV1::Permission(PermissionDescriptorV1 {
        name: NAME.into(),
        payload: SchemaV1::Codec(format!(
            "semantic:iroha.state-row-read.json.v1@{}.{}.{}",
            layout.major, layout.minor, layout.flags,
        )),
    })
}

static DEFINITION: LazyLock<
    Result<DefinitionId, iroha_data_model::runtime::definition::DefinitionError>,
> = LazyLock::new(|| descriptor().id());

/// Whether the current selection permits use of the exact-row read capability.
pub fn active(selection: &RootSelectionV1) -> bool {
    selected(selection)
        && DEFINITION.as_ref().is_ok_and(|id| {
            !selection
                .restrictions
                .iter()
                .any(|entry| &entry.definition == id)
        })
}

/// A restriction on an already selected definition does not reinstall its grants.
pub(super) fn selected(selection: &RootSelectionV1) -> bool {
    super::keyed_state::selected(selection)
        && DEFINITION
            .as_ref()
            .is_ok_and(|id| selection.active.binary_search(id).is_ok())
}
/// Prevent a previous custom token with this name acquiring new authority at activation.
pub(crate) fn validate_activation_inventory(world: &impl WorldReadOnly) -> Result<(), String> {
    if world
        .account_permissions()
        .iter()
        .any(|(_, permissions)| permissions.iter().any(|p| p.name() == NAME))
    {
        return Err(
            "keyed State activation requires removal of preexisting direct CanReadStateRow grants"
                .into(),
        );
    }
    Ok(())
}

/// Check native grant or revoke even when an installed custom executor permits it.
pub(crate) fn guard_account_change(
    world: &impl WorldReadOnly,
    caller: &AccountId,
    permission: &Permission,
    revoke: bool,
) -> Result<(), InstructionExecutionError> {
    if permission.name() != NAME || !super::keyed_state::selected(world.runtime_selection()) {
        return Ok(());
    }
    let invalid = || {
        InstructionExecutionError::InvariantViolation(
            "invalid exact State row grant or issuer".into(),
        )
    };
    let reader = CanReadStateRow::try_from(permission).map_err(|_| invalid())?;
    if !reader.is_valid()
        || Permission::from(reader) != *permission
        || (!revoke && !active(world.runtime_selection()))
    {
        return Err(invalid());
    }
    let root: Permission = CanReadAllLedgerData.into();
    if world.account_contains_inherent_permission(caller, &root)
        || world.account_roles_iter(caller).any(|id| {
            world
                .roles()
                .get(id)
                .is_some_and(|role| role.permissions().any(|p| p == &root))
        })
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Exact read authority can only be issued directly to an account.
pub(crate) fn guard_role_grant(
    world: &impl WorldReadOnly,
    token: &Permission,
) -> Result<(), InstructionExecutionError> {
    if token.name() == NAME && super::keyed_state::selected(world.runtime_selection()) {
        return Err(InstructionExecutionError::InvariantViolation(
            "State row grants are direct account permissions".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::World;
    use iroha_data_model::{Account, Registrable};
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    #[test]
    fn exact_reads_require_a_selected_unrestricted_definition_and_a_global_issuer() {
        let token: Permission = CanReadStateRow {
            table: "world.assets".into(),
            key_hex: "00af".into(),
        }
        .into();
        let mut world = World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&ALICE_ID),
            ],
            [],
        );
        assert!(!active(world.view().runtime_selection()));
        assert!(
            guard_account_change(&world.view(), &BOB_ID, &token, false).is_ok(),
            "the baseline retains its own custom-permission behavior"
        );
        assert!(validate_activation_inventory(&world.view()).is_ok());
        world.account_permissions.insert(
            ALICE_ID.clone(),
            std::collections::BTreeSet::from([CanReadAllLedgerData.into()]),
        );
        super::super::keyed_state::install_for_test(&mut world);
        assert!(active(world.view().runtime_selection()));
        for revoke in [true, false] {
            assert!(guard_account_change(&world.view(), &ALICE_ID, &token, revoke).is_ok());
            assert!(guard_account_change(&world.view(), &BOB_ID, &token, revoke).is_err());
        }
        assert!(guard_role_grant(&world.view(), &token).is_err());
        let malformed: Permission = CanReadStateRow {
            table: "world.assets".into(),
            key_hex: "FF".into(),
        }
        .into();
        assert!(guard_account_change(&world.view(), &ALICE_ID, &malformed, false).is_err());
        world
            .account_permissions
            .insert(BOB_ID.clone(), std::collections::BTreeSet::from([token]));
        assert!(validate_activation_inventory(&world.view()).is_err());
        let mut selection = world.view().runtime_selection().clone();
        selection
            .restrictions
            .push(iroha_data_model::runtime::definition::RestrictionEntryV1 {
                definition: descriptor().id().unwrap(),
                restriction: iroha_data_model::runtime::definition::RestrictionV1::AdmissionClosed(
                    2,
                ),
            });
        assert!(!active(&selection));
    }
}
