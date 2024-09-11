//! Definition of Iroha default executor and accompanying validation functions
#![allow(missing_docs, clippy::missing_errors_doc)]

use alloc::format;

pub use account::{
    visit_register_account, visit_remove_account_key_value, visit_set_account_key_value,
    visit_unregister_account,
};
pub use asset::{
    visit_burn_asset_numeric, visit_mint_asset_numeric, visit_register_asset,
    visit_remove_asset_key_value, visit_set_asset_key_value, visit_transfer_asset_numeric,
    visit_transfer_asset_store, visit_unregister_asset,
};
pub use asset_definition::{
    visit_register_asset_definition, visit_remove_asset_definition_key_value,
    visit_set_asset_definition_key_value, visit_transfer_asset_definition,
    visit_unregister_asset_definition,
};
pub use custom::visit_custom;
pub use domain::{
    visit_register_domain, visit_remove_domain_key_value, visit_set_domain_key_value,
    visit_transfer_domain, visit_unregister_domain,
};
pub use executor::visit_upgrade;
use iroha_smart_contract::data_model::{prelude::*, visit::Visit};
pub use log::visit_log;
pub use parameter::visit_set_parameter;
pub use peer::{visit_register_peer, visit_unregister_peer};
pub use permission::{visit_grant_account_permission, visit_revoke_account_permission};
pub use role::{
    visit_grant_account_role, visit_grant_role_permission, visit_register_role,
    visit_revoke_account_role, visit_revoke_role_permission, visit_unregister_role,
};
pub use trigger::{
    visit_burn_trigger_repetitions, visit_execute_trigger, visit_mint_trigger_repetitions,
    visit_register_trigger, visit_remove_trigger_key_value, visit_set_trigger_key_value,
    visit_unregister_trigger,
};

use crate::{
    deny, execute,
    permission::{AnyPermission, ExecutorPermision as _},
    Validate,
};

// NOTE: If any new `visit_..` functions are introduced in this module, one should
// not forget to update the default executor boilerplate too, specifically the
// `iroha_executor::derive::default::impl_derive_visit` function
// signature list.

/// Default validation for [`SignedTransaction`].
///
/// # Warning
///
/// Each instruction is executed in sequence following successful validation.
/// [`Executable::Wasm`] is not executed because it is validated on the host side.
pub fn visit_transaction<V: Validate + Visit + ?Sized>(
    executor: &mut V,
    authority: &AccountId,
    transaction: &SignedTransaction,
) {
    match transaction.instructions() {
        Executable::Wasm(wasm) => executor.visit_wasm(authority, wasm),
        Executable::Instructions(instructions) => {
            for isi in instructions {
                if executor.verdict().is_ok() {
                    executor.visit_instruction(authority, isi);
                }
            }
        }
    }
}

/// Default validation for [`InstructionBox`].
///
/// # Warning
///
/// Instruction is executed following successful validation
pub fn visit_instruction<V: Validate + Visit + ?Sized>(
    executor: &mut V,
    authority: &AccountId,
    isi: &InstructionBox,
) {
    match isi {
        InstructionBox::SetParameter(isi) => {
            executor.visit_set_parameter(authority, isi);
        }
        InstructionBox::Log(isi) => {
            executor.visit_log(authority, isi);
        }
        InstructionBox::ExecuteTrigger(isi) => {
            executor.visit_execute_trigger(authority, isi);
        }
        InstructionBox::Burn(isi) => {
            executor.visit_burn(authority, isi);
        }
        InstructionBox::Grant(isi) => {
            executor.visit_grant(authority, isi);
        }
        InstructionBox::Mint(isi) => {
            executor.visit_mint(authority, isi);
        }
        InstructionBox::Register(isi) => {
            executor.visit_register(authority, isi);
        }
        InstructionBox::RemoveKeyValue(isi) => {
            executor.visit_remove_key_value(authority, isi);
        }
        InstructionBox::Revoke(isi) => {
            executor.visit_revoke(authority, isi);
        }
        InstructionBox::SetKeyValue(isi) => {
            executor.visit_set_key_value(authority, isi);
        }
        InstructionBox::Transfer(isi) => {
            executor.visit_transfer(authority, isi);
        }
        InstructionBox::Unregister(isi) => {
            executor.visit_unregister(authority, isi);
        }
        InstructionBox::Upgrade(isi) => {
            executor.visit_upgrade(authority, isi);
        }
        InstructionBox::Custom(isi) => {
            executor.visit_custom(authority, isi);
        }
    }
}

pub mod peer {
    use iroha_executor_data_model::permission::peer::CanUnregisterAnyPeer;

    use super::*;

    pub fn visit_register_peer<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        _authority: &AccountId,
        isi: &Register<Peer>,
    ) {
        execute!(executor, isi)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn visit_unregister_peer<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Peer>,
    ) {
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        if CanUnregisterAnyPeer.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister peer");
    }
}

pub mod domain {
    use iroha_executor_data_model::permission::domain::{
        CanRemoveKeyValueInDomain, CanSetKeyValueInDomain, CanUnregisterDomain,
    };
    use iroha_smart_contract::data_model::domain::DomainId;

    use super::*;
    use crate::permission::{
        account::is_account_owner, accounts_permissions, domain::is_domain_owner, roles_permissions,
    };

    pub fn visit_register_domain<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        _authority: &AccountId,
        isi: &Register<Domain>,
    ) {
        execute!(executor, isi)
    }

    pub fn visit_unregister_domain<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Domain>,
    ) {
        let domain_id = isi.object();

        if is_genesis(executor)
            || match is_domain_owner(domain_id, authority) {
                Err(err) => deny!(executor, err),
                Ok(is_domain_owner) => is_domain_owner,
            }
            || {
                let can_unregister_domain_token = CanUnregisterDomain {
                    domain: domain_id.clone(),
                };
                can_unregister_domain_token.is_owned_by(authority)
            }
        {
            use iroha_smart_contract::ExecuteOnHost as _;

            for (owner_id, permission) in accounts_permissions() {
                if is_token_domain_associated(&permission, domain_id) {
                    let isi = Revoke::account_permission(permission, owner_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            for (role_id, permission) in roles_permissions() {
                if is_token_domain_associated(&permission, domain_id) {
                    let isi = Revoke::role_permission(permission, role_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister domain");
    }

    pub fn visit_transfer_domain<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Transfer<Account, DomainId, Account>,
    ) {
        let source_id = isi.source();
        let domain_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_account_owner(source_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        match is_domain_owner(domain_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        deny!(executor, "Can't transfer domain of another account");
    }

    pub fn visit_set_domain_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetKeyValue<Domain>,
    ) {
        let domain_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_domain_token = CanSetKeyValueInDomain {
            domain: domain_id.clone(),
        };
        if can_set_key_value_in_domain_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't set key value in domain metadata");
    }

    pub fn visit_remove_domain_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &RemoveKeyValue<Domain>,
    ) {
        let domain_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_domain_token = CanRemoveKeyValueInDomain {
            domain: domain_id.clone(),
        };
        if can_remove_key_value_in_domain_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't remove key value in domain metadata");
    }

    #[allow(clippy::too_many_lines)]
    fn is_token_domain_associated(permission: &Permission, domain_id: &DomainId) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanSetKeyValueInDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanRemoveKeyValueInDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanRegisterAccountInDomain(permission) => {
                &permission.domain == domain_id
            }
            AnyPermission::CanRegisterAssetDefinitionInDomain(permission) => {
                &permission.domain == domain_id
            }
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanSetKeyValueInAssetDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanRemoveKeyValueInAssetDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanRegisterAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanUnregisterAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanBurnUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanTransferUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanUnregisterUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanSetKeyValueInUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanRemoveKeyValueInUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanMintUserAsset(permission) => {
                permission.asset.definition().domain() == domain_id
                    || permission.asset.account().domain() == domain_id
            }
            AnyPermission::CanUnregisterAccount(permission) => {
                permission.account.domain() == domain_id
            }
            AnyPermission::CanSetKeyValueInAccount(permission) => {
                permission.account.domain() == domain_id
            }
            AnyPermission::CanRemoveKeyValueInAccount(permission) => {
                permission.account.domain() == domain_id
            }
            AnyPermission::CanRegisterUserTrigger(permission) => {
                permission.account.domain() == domain_id
            }
            AnyPermission::CanUnregisterUserTrigger(permission) => {
                permission.account.domain() == domain_id
            }
            AnyPermission::CanExecuteUserTrigger(_)
            | AnyPermission::CanBurnUserTrigger(_)
            | AnyPermission::CanMintUserTrigger(_)
            | AnyPermission::CanSetKeyValueInTrigger(_)
            | AnyPermission::CanRemoveKeyValueInTrigger(_)
            | AnyPermission::CanUnregisterAnyPeer(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanUnregisterAnyRole(_)
            | AnyPermission::CanUpgradeExecutor(_) => false,
        }
    }
}

pub mod account {
    use iroha_executor_data_model::permission::{
        account::{CanRemoveKeyValueInAccount, CanSetKeyValueInAccount, CanUnregisterAccount},
        domain::CanRegisterAccountInDomain,
    };

    use super::*;
    use crate::permission::{account::is_account_owner, accounts_permissions, roles_permissions};

    pub fn visit_register_account<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Register<Account>,
    ) {
        let domain_id = isi.object().id().domain();

        match crate::permission::domain::is_domain_owner(domain_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_register_account_in_domain = CanRegisterAccountInDomain {
            domain: domain_id.clone(),
        };
        if can_register_account_in_domain.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't register account in a domain owned by another account"
        );
    }

    pub fn visit_unregister_account<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Account>,
    ) {
        let account_id = isi.object();

        if is_genesis(executor)
            || match is_account_owner(account_id, authority) {
                Err(err) => deny!(executor, err),
                Ok(is_account_owner) => is_account_owner,
            }
            || {
                let can_unregister_user_account = CanUnregisterAccount {
                    account: account_id.clone(),
                };
                can_unregister_user_account.is_owned_by(authority)
            }
        {
            use iroha_smart_contract::ExecuteOnHost as _;

            for (owner_id, permission) in accounts_permissions() {
                if is_token_account_associated(&permission, account_id) {
                    let isi = Revoke::account_permission(permission, owner_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            for (role_id, permission) in roles_permissions() {
                if is_token_account_associated(&permission, account_id) {
                    let isi = Revoke::role_permission(permission, role_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister another account");
    }

    pub fn visit_set_account_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetKeyValue<Account>,
    ) {
        let account_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_account_owner(account_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_user_account_token = CanSetKeyValueInAccount {
            account: account_id.clone(),
        };
        if can_set_key_value_in_user_account_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the metadata of another account"
        );
    }

    pub fn visit_remove_account_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &RemoveKeyValue<Account>,
    ) {
        let account_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_account_owner(account_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_user_account_token = CanRemoveKeyValueInAccount {
            account: account_id.clone(),
        };
        if can_remove_key_value_in_user_account_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the metadata of another account"
        );
    }

    fn is_token_account_associated(permission: &Permission, account_id: &AccountId) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAccount(permission) => &permission.account == account_id,
            AnyPermission::CanSetKeyValueInAccount(permission) => &permission.account == account_id,
            AnyPermission::CanRemoveKeyValueInAccount(permission) => {
                &permission.account == account_id
            }
            AnyPermission::CanBurnUserAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanTransferUserAsset(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanUnregisterUserAsset(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanSetKeyValueInUserAsset(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanRemoveKeyValueInUserAsset(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanMintUserAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanRegisterUserTrigger(permission) => &permission.account == account_id,
            AnyPermission::CanUnregisterUserTrigger(permission) => {
                &permission.account == account_id
            }
            AnyPermission::CanExecuteUserTrigger(_)
            | AnyPermission::CanBurnUserTrigger(_)
            | AnyPermission::CanMintUserTrigger(_)
            | AnyPermission::CanSetKeyValueInTrigger(_)
            | AnyPermission::CanRemoveKeyValueInTrigger(_)
            | AnyPermission::CanUnregisterAnyPeer(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanSetKeyValueInDomain(_)
            | AnyPermission::CanRemoveKeyValueInDomain(_)
            | AnyPermission::CanRegisterAccountInDomain(_)
            | AnyPermission::CanRegisterAssetDefinitionInDomain(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanSetKeyValueInAssetDefinition(_)
            | AnyPermission::CanRemoveKeyValueInAssetDefinition(_)
            | AnyPermission::CanRegisterAssetWithDefinition(_)
            | AnyPermission::CanUnregisterAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanUnregisterAnyRole(_)
            | AnyPermission::CanUpgradeExecutor(_) => false,
        }
    }
}

pub mod asset_definition {
    use iroha_executor_data_model::permission::{
        asset_definition::{
            CanRemoveKeyValueInAssetDefinition, CanSetKeyValueInAssetDefinition,
            CanUnregisterAssetDefinition,
        },
        domain::CanRegisterAssetDefinitionInDomain,
    };
    use iroha_smart_contract::data_model::asset::AssetDefinitionId;

    use super::*;
    use crate::permission::{
        account::is_account_owner, accounts_permissions,
        asset_definition::is_asset_definition_owner, roles_permissions,
    };

    pub fn visit_register_asset_definition<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Register<AssetDefinition>,
    ) {
        let domain_id = isi.object().id().domain();

        match crate::permission::domain::is_domain_owner(domain_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_register_asset_definition_in_domain_token = CanRegisterAssetDefinitionInDomain {
            domain: domain_id.clone(),
        };
        if can_register_asset_definition_in_domain_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't register asset definition in a domain owned by another account"
        );
    }

    pub fn visit_unregister_asset_definition<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if is_genesis(executor)
            || match is_asset_definition_owner(asset_definition_id, authority) {
                Err(err) => deny!(executor, err),
                Ok(is_asset_definition_owner) => is_asset_definition_owner,
            }
            || {
                let can_unregister_asset_definition_token = CanUnregisterAssetDefinition {
                    asset_definition: asset_definition_id.clone(),
                };
                can_unregister_asset_definition_token.is_owned_by(authority)
            }
        {
            use iroha_smart_contract::ExecuteOnHost as _;

            for (owner_id, permission) in accounts_permissions() {
                if is_token_asset_definition_associated(&permission, asset_definition_id) {
                    let isi = Revoke::account_permission(permission, owner_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            for (role_id, permission) in roles_permissions() {
                if is_token_asset_definition_associated(&permission, asset_definition_id) {
                    let isi = Revoke::role_permission(permission, role_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister asset definition in a domain owned by another account"
        );
    }

    pub fn visit_transfer_asset_definition<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Transfer<Account, AssetDefinitionId, Account>,
    ) {
        let source_id = isi.source();
        let asset_definition_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_account_owner(source_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        match is_asset_definition_owner(asset_definition_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        deny!(
            executor,
            "Can't transfer asset definition of another account"
        );
    }

    pub fn visit_set_asset_definition_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(asset_definition_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_asset_definition_token = CanSetKeyValueInAssetDefinition {
            asset_definition: asset_definition_id.clone(),
        };
        if can_set_key_value_in_asset_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the asset definition metadata created by another account"
        );
    }

    pub fn visit_remove_asset_definition_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &RemoveKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(asset_definition_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_asset_definition_token = CanRemoveKeyValueInAssetDefinition {
            asset_definition: asset_definition_id.clone(),
        };
        if can_remove_key_value_in_asset_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the asset definition metadata created by another account"
        );
    }

    fn is_token_asset_definition_associated(
        permission: &Permission,
        asset_definition_id: &AssetDefinitionId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanSetKeyValueInAssetDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanRemoveKeyValueInAssetDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanRegisterAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanUnregisterAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanBurnUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanTransferUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanUnregisterUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanSetKeyValueInUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanRemoveKeyValueInUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanMintUserAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanSetKeyValueInAccount(_)
            | AnyPermission::CanRemoveKeyValueInAccount(_)
            | AnyPermission::CanRegisterUserTrigger(_)
            | AnyPermission::CanUnregisterUserTrigger(_)
            | AnyPermission::CanExecuteUserTrigger(_)
            | AnyPermission::CanBurnUserTrigger(_)
            | AnyPermission::CanMintUserTrigger(_)
            | AnyPermission::CanSetKeyValueInTrigger(_)
            | AnyPermission::CanRemoveKeyValueInTrigger(_)
            | AnyPermission::CanUnregisterAnyPeer(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanSetKeyValueInDomain(_)
            | AnyPermission::CanRemoveKeyValueInDomain(_)
            | AnyPermission::CanRegisterAccountInDomain(_)
            | AnyPermission::CanRegisterAssetDefinitionInDomain(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanUnregisterAnyRole(_)
            | AnyPermission::CanUpgradeExecutor(_) => false,
        }
    }
}

pub mod asset {
    use iroha_executor_data_model::permission::asset::{
        CanBurnAssetWithDefinition, CanBurnUserAsset, CanMintAssetWithDefinition, CanMintUserAsset,
        CanRegisterAssetWithDefinition, CanRemoveKeyValueInUserAsset, CanSetKeyValueInUserAsset,
        CanTransferAssetWithDefinition, CanTransferUserAsset, CanUnregisterAssetWithDefinition,
        CanUnregisterUserAsset,
    };
    use iroha_smart_contract::data_model::{
        asset::AssetValue, isi::BuiltInInstruction, metadata::Metadata,
    };
    use iroha_smart_contract_utils::Encode;

    use super::*;
    use crate::permission::{asset::is_asset_owner, asset_definition::is_asset_definition_owner};

    pub fn visit_register_asset<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Register<Asset>,
    ) {
        let asset = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(asset.id().definition(), authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_register_assets_with_definition_token = CanRegisterAssetWithDefinition {
            asset_definition: asset.id().definition().clone(),
        };
        if can_register_assets_with_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't register assets with definitions registered by other accounts"
        );
    }

    pub fn visit_unregister_asset<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Asset>,
    ) {
        let asset_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_owner(asset_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        match is_asset_definition_owner(asset_id.definition(), authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_unregister_assets_with_definition_token = CanUnregisterAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_unregister_assets_with_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }
        let can_unregister_user_asset_token = CanUnregisterUserAsset {
            asset: asset_id.clone(),
        };
        if can_unregister_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister asset from another account");
    }

    fn validate_mint_asset<V, Q>(executor: &mut V, authority: &AccountId, isi: &Mint<Q, Asset>)
    where
        V: Validate + Visit + ?Sized,
        Q: Into<AssetValue>,
        Mint<Q, Asset>: BuiltInInstruction + Encode,
    {
        let asset_id = isi.destination();
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(asset_id.definition(), authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_assets_with_definition_token = CanMintAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_mint_assets_with_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }
        let can_mint_user_asset_token = CanMintUserAsset {
            asset: asset_id.clone(),
        };
        if can_mint_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't mint assets with definitions registered by other accounts"
        );
    }

    pub fn visit_mint_asset_numeric<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Mint<Numeric, Asset>,
    ) {
        validate_mint_asset(executor, authority, isi);
    }

    fn validate_burn_asset<V, Q>(executor: &mut V, authority: &AccountId, isi: &Burn<Q, Asset>)
    where
        V: Validate + Visit + ?Sized,
        Q: Into<AssetValue>,
        Burn<Q, Asset>: BuiltInInstruction + Encode,
    {
        let asset_id = isi.destination();
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_owner(asset_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        match is_asset_definition_owner(asset_id.definition(), authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_burn_assets_with_definition_token = CanBurnAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_burn_assets_with_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }
        let can_burn_user_asset_token = CanBurnUserAsset {
            asset: asset_id.clone(),
        };
        if can_burn_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't burn assets from another account");
    }

    pub fn visit_burn_asset_numeric<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Burn<Numeric, Asset>,
    ) {
        validate_burn_asset(executor, authority, isi);
    }

    fn validate_transfer_asset<V, Q>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Transfer<Asset, Q, Account>,
    ) where
        V: Validate + Visit + ?Sized,
        Q: Into<AssetValue>,
        Transfer<Asset, Q, Account>: BuiltInInstruction + Encode,
    {
        let asset_id = isi.source();
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_owner(asset_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        match is_asset_definition_owner(asset_id.definition(), authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_transfer_assets_with_definition_token = CanTransferAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_transfer_assets_with_definition_token.is_owned_by(authority) {
            execute!(executor, isi);
        }
        let can_transfer_user_asset_token = CanTransferUserAsset {
            asset: asset_id.clone(),
        };
        if can_transfer_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't transfer assets of another account");
    }

    pub fn visit_transfer_asset_numeric<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Transfer<Asset, Numeric, Account>,
    ) {
        validate_transfer_asset(executor, authority, isi);
    }

    pub fn visit_transfer_asset_store<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Transfer<Asset, Metadata, Account>,
    ) {
        validate_transfer_asset(executor, authority, isi);
    }

    pub fn visit_set_asset_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetKeyValue<Asset>,
    ) {
        let asset_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_owner(asset_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_set_key_value_in_user_asset_token = CanSetKeyValueInUserAsset {
            asset: asset_id.clone(),
        };
        if can_set_key_value_in_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the asset metadata of another account"
        );
    }

    pub fn visit_remove_asset_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &RemoveKeyValue<Asset>,
    ) {
        let asset_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_asset_owner(asset_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_user_asset_token = CanRemoveKeyValueInUserAsset {
            asset: asset_id.clone(),
        };
        if can_remove_key_value_in_user_asset_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the asset metadata of another account"
        );
    }
}

pub mod parameter {
    use iroha_executor_data_model::permission::parameter::CanSetParameters;

    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    pub fn visit_set_parameter<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetParameter,
    ) {
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        if CanSetParameters.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set executor configuration parameters without permission"
        );
    }
}

pub mod role {
    use iroha_executor_data_model::permission::role::CanUnregisterAnyRole;
    use iroha_smart_contract::data_model::role::Role;

    use super::*;

    macro_rules! impl_validate_grant_revoke_account_role {
        ($executor:ident, $isi:ident, $authority:ident, $method:ident) => {
            let role_id = $isi.object();

            let find_role_query_res =
                match crate::data_model::query::builder::QueryBuilderExt::execute_single(
                    iroha_smart_contract::query(FindRoles)
                        .filter_with(|role| role.id.eq(role_id.clone())),
                ) {
                    Ok(res) => res,
                    Err(crate::data_model::query::builder::SingleQueryError::QueryError(error)) => {
                        deny!($executor, error);
                    }
                    Err(
                        crate::data_model::query::builder::SingleQueryError::ExpectedOneGotNone,
                    ) => {
                        // assuming that only a "not found" case is possible here
                        $executor.deny($crate::data_model::ValidationFail::QueryFailed(
                            $crate::data_model::query::error::QueryExecutionFail::Find(
                                $crate::data_model::query::error::FindError::Role(role_id.clone()),
                            ),
                        ));
                        return;
                    }
                    Err(_) => {
                        unreachable!();
                    }
                };
            let role = Role::try_from(find_role_query_res).unwrap();

            let mut unknown_tokens = alloc::vec::Vec::new();
            for permission in role.permissions() {
                if let Ok(permission) = AnyPermission::try_from(permission) {
                    if !is_genesis($executor) {
                        if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                            &permission,
                            $authority,
                            $executor.block_height(),
                        ) {
                            deny!($executor, error);
                        }
                    }

                    continue;
                }

                unknown_tokens.push(permission);
            }

            assert!(
                unknown_tokens.is_empty(),
                "Role contains unknown permission tokens: {unknown_tokens:?}"
            );
            execute!($executor, $isi)
        };
    }

    macro_rules! impl_validate_grant_revoke_role_permission {
        ($executor:ident, $isi:ident, $authority:ident, $method:ident, $isi_type:ty) => {
            let role_id = $isi.destination().clone();
            let permission = $isi.object();

            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !is_genesis($executor) {
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        $authority,
                        $executor.block_height(),
                    ) {
                        deny!($executor, error);
                    }
                }

                let isi = <$isi_type>::role_permission(any_permission, role_id);
                execute!($executor, isi);
            }

            deny!(
                $executor,
                ValidationFail::NotPermitted(format!(
                    "{permission:?}: Unknown permission permission"
                ))
            );
        };
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn visit_register_role<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        _authority: &AccountId,
        isi: &Register<Role>,
    ) {
        let role = isi.object().inner();

        // Unify permission tokens inside role and deduplicate them
        let mut new_role = Role::new(role.id().clone());
        let mut unknown_tokens = alloc::vec::Vec::new();
        for permission in role.permissions() {
            iroha_smart_contract::debug!(&format!("Checking `{permission:?}`"));

            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                new_role = new_role.add_permission(any_permission);
                continue;
            }

            unknown_tokens.push(permission);
        }

        if !unknown_tokens.is_empty() {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{unknown_tokens:?}: Unrecognised permission tokens"
                ))
            );
        }

        let isi = Register::role(new_role);
        execute!(executor, isi);
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn visit_unregister_role<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Role>,
    ) {
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        if CanUnregisterAnyRole.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister role");
    }

    pub fn visit_grant_account_role<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Grant<RoleId, Account>,
    ) {
        impl_validate_grant_revoke_account_role!(executor, isi, authority, validate_grant);
    }

    pub fn visit_revoke_account_role<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Revoke<RoleId, Account>,
    ) {
        impl_validate_grant_revoke_account_role!(executor, isi, authority, validate_revoke);
    }

    pub fn visit_grant_role_permission<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Grant<Permission, Role>,
    ) {
        impl_validate_grant_revoke_role_permission!(executor, isi, authority, validate_grant, Grant<Permission, Role>);
    }

    pub fn visit_revoke_role_permission<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Revoke<Permission, Role>,
    ) {
        impl_validate_grant_revoke_role_permission!(executor, isi, authority, validate_revoke, Revoke<Permission, Role>);
    }
}

pub mod trigger {
    use iroha_executor_data_model::permission::trigger::{
        CanBurnUserTrigger, CanExecuteUserTrigger, CanMintUserTrigger, CanRegisterUserTrigger,
        CanRemoveKeyValueInTrigger, CanSetKeyValueInTrigger, CanUnregisterUserTrigger,
    };
    use iroha_smart_contract::data_model::trigger::Trigger;

    use super::*;
    use crate::permission::{
        accounts_permissions,
        domain::is_domain_owner,
        roles_permissions,
        trigger::{find_trigger, is_trigger_owner},
    };

    pub fn visit_register_trigger<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Register<Trigger>,
    ) {
        let trigger = isi.object();

        if is_genesis(executor)
            || {
                match is_domain_owner(trigger.action().authority().domain(), authority) {
                    Err(err) => deny!(executor, err),
                    Ok(is_domain_owner) => is_domain_owner,
                }
            }
            || {
                let can_register_user_trigger_token = CanRegisterUserTrigger {
                    account: isi.object().action().authority().clone(),
                };
                can_register_user_trigger_token.is_owned_by(authority)
            }
        {
            execute!(executor, isi)
        }
        deny!(executor, "Can't register trigger owned by another account");
    }

    pub fn visit_unregister_trigger<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Unregister<Trigger>,
    ) {
        let trigger_id = isi.object();

        if is_genesis(executor)
            || match is_trigger_owner(trigger_id, authority) {
                Err(err) => deny!(executor, err),
                Ok(is_trigger_owner) => is_trigger_owner,
            }
            || {
                let can_unregister_user_trigger_token = CanUnregisterUserTrigger {
                    account: find_trigger(trigger_id)
                        .unwrap()
                        .action()
                        .authority()
                        .clone(),
                };
                can_unregister_user_trigger_token.is_owned_by(authority)
            }
        {
            use iroha_smart_contract::ExecuteOnHost as _;

            for (owner_id, permission) in accounts_permissions() {
                if is_token_trigger_associated(&permission, trigger_id) {
                    let isi = Revoke::account_permission(permission, owner_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            for (role_id, permission) in roles_permissions() {
                if is_token_trigger_associated(&permission, trigger_id) {
                    let isi = Revoke::role_permission(permission, role_id.clone());
                    if let Err(_err) = isi.execute() {
                        deny!(executor, "Can't revoke associated permission");
                    }
                }
            }
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister trigger owned by another account"
        );
    }

    pub fn visit_mint_trigger_repetitions<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Mint<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanMintUserTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't mint execution count for trigger owned by another account"
        );
    }

    pub fn visit_burn_trigger_repetitions<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Burn<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanBurnUserTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't burn execution count for trigger owned by another account"
        );
    }

    pub fn visit_execute_trigger<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &ExecuteTrigger,
    ) {
        let trigger_id = isi.trigger();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_execute_trigger_token = CanExecuteUserTrigger {
            trigger: trigger_id.clone(),
        };
        if can_execute_trigger_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't execute trigger owned by another account");
    }

    pub fn visit_set_trigger_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &SetKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_user_trigger_token = CanSetKeyValueInTrigger {
            trigger: trigger_id.clone(),
        };
        if can_set_key_value_in_user_trigger_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the metadata of another trigger"
        );
    }

    pub fn visit_remove_trigger_key_value<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &RemoveKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();

        if is_genesis(executor) {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, authority) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_trigger_token = CanRemoveKeyValueInTrigger {
            trigger: trigger_id.clone(),
        };
        if can_remove_key_value_in_trigger_token.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the metadata of another trigger"
        );
    }

    fn is_token_trigger_associated(permission: &Permission, trigger_id: &TriggerId) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanExecuteUserTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanBurnUserTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanMintUserTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanSetKeyValueInTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanRemoveKeyValueInTrigger(permission) => {
                &permission.trigger == trigger_id
            }
            AnyPermission::CanRegisterUserTrigger(_)
            | AnyPermission::CanUnregisterUserTrigger(_)
            | AnyPermission::CanUnregisterAnyPeer(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanSetKeyValueInDomain(_)
            | AnyPermission::CanRemoveKeyValueInDomain(_)
            | AnyPermission::CanRegisterAccountInDomain(_)
            | AnyPermission::CanRegisterAssetDefinitionInDomain(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanSetKeyValueInAccount(_)
            | AnyPermission::CanRemoveKeyValueInAccount(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanSetKeyValueInAssetDefinition(_)
            | AnyPermission::CanRemoveKeyValueInAssetDefinition(_)
            | AnyPermission::CanRegisterAssetWithDefinition(_)
            | AnyPermission::CanUnregisterAssetWithDefinition(_)
            | AnyPermission::CanUnregisterUserAsset(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanBurnUserAsset(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanTransferUserAsset(_)
            | AnyPermission::CanSetKeyValueInUserAsset(_)
            | AnyPermission::CanRemoveKeyValueInUserAsset(_)
            | AnyPermission::CanMintUserAsset(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanUnregisterAnyRole(_)
            | AnyPermission::CanUpgradeExecutor(_) => false,
        }
    }
}

pub mod permission {
    use super::*;

    macro_rules! impl_validate {
        ($executor:ident, $authority:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let account_id = $isi.destination().clone();
            let permission = $isi.object();

            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !is_genesis($executor) {
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        $authority,
                        $executor.block_height(),
                    ) {
                        deny!($executor, error);
                    }
                }

                let isi = <$isi_type>::account_permission(any_permission, account_id);
                execute!($executor, isi);
            }

            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }

    pub fn visit_grant_account_permission<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Grant<Permission, Account>,
    ) {
        impl_validate!(
            executor,
            authority,
            isi,
            validate_grant,
            Grant<Permission, Account>
        );
    }

    pub fn visit_revoke_account_permission<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Revoke<Permission, Account>,
    ) {
        impl_validate!(
            executor,
            authority,
            isi,
            validate_revoke,
            Revoke<Permission, Account>
        );
    }
}

pub mod executor {
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;

    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    pub fn visit_upgrade<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        authority: &AccountId,
        isi: &Upgrade,
    ) {
        if is_genesis(executor) {
            execute!(executor, isi);
        }
        if CanUpgradeExecutor.is_owned_by(authority) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't upgrade executor");
    }
}

pub mod log {
    use super::*;

    pub fn visit_log<V: Validate + Visit + ?Sized>(
        executor: &mut V,
        _authority: &AccountId,
        isi: &Log,
    ) {
        execute!(executor, isi)
    }
}

pub mod custom {
    use super::*;

    pub fn visit_custom<V: Validate + ?Sized>(
        executor: &mut V,
        _authority: &AccountId,
        _isi: &CustomInstruction,
    ) {
        deny!(
            executor,
            "Custom instructions should be handled in custom executor"
        )
    }
}

fn is_genesis<V: Validate + Visit + ?Sized>(executor: &V) -> bool {
    executor.block_height() == 0
}
