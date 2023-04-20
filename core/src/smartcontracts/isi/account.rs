//! This module contains implementations of smart-contract traits and instructions for [`Account`] structure
//! and implementations of [`Query`]'s to [`WorldStateView`] about [`Account`].

use iroha_data_model::{asset::AssetsMap, prelude::*, query::error::FindError, role::RoleIds};
use iroha_telemetry::metrics;

use super::{prelude::*, Context};
use crate::{ValidQuery, WorldStateView};

impl Registrable for iroha_data_model::account::NewAccount {
    type Target = Account;

    #[must_use]
    #[inline]
    fn build(self) -> Self::Target {
        Self::Target {
            id: self.id,
            signatories: self.signatories,
            assets: AssetsMap::default(),
            signature_check_condition: SignatureCheckCondition::default(),
            metadata: self.metadata,
            roles: RoleIds::default(),
        }
    }
}

/// All instructions related to accounts:
/// - minting/burning public key into account signatories
/// - minting/burning signature condition check
/// - update metadata
/// - grant permissions and roles
/// - Revoke permissions or roles
pub mod isi {
    use iroha_data_model::{
        isi::{error::MintabilityError, InstructionType},
        query::error::QueryExecutionFailure,
    };

    use super::*;

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    impl Execute for Register<Asset> {
        type Error = Error;

        #[metrics(+"register_asset")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let asset_id = self.object.id;

            match wsv.asset(&asset_id) {
                Err(err) => match err {
                    QueryExecutionFailure::Find(find_err)
                        if matches!(*find_err, FindError::Asset(_)) =>
                    {
                        assert_can_register(&asset_id.definition_id, wsv, &self.object.value)?;
                        let asset = wsv
                            .asset_or_insert(&asset_id, self.object.value)
                            .expect("Account exists");

                        match asset.value {
                            AssetValue::Quantity(increment) => {
                                wsv.increase_asset_total_amount(
                                    &asset_id.definition_id,
                                    increment,
                                )?;
                            }
                            AssetValue::BigQuantity(increment) => {
                                wsv.increase_asset_total_amount(
                                    &asset_id.definition_id,
                                    increment,
                                )?;
                            }
                            AssetValue::Fixed(increment) => {
                                wsv.increase_asset_total_amount(
                                    &asset_id.definition_id,
                                    increment,
                                )?;
                            }
                            AssetValue::Store(_) => {
                                wsv.increase_asset_total_amount(&asset_id.definition_id, 1_u32)?;
                            }
                        }
                        Ok(())
                    }
                    _ => Err(err.into()),
                },
                Ok(_) => Err(Error::Repetition(
                    InstructionType::Register,
                    IdBox::AssetId(asset_id.clone()),
                )),
            }
        }
    }

    impl Execute for Unregister<Asset> {
        type Error = Error;

        #[metrics(+"unregister_asset")]
        fn execute(self, _authority: AccountId, wsv: &WorldStateView) -> Result<(), Self::Error> {
            let asset_id = self.object_id;
            let account_id = asset_id.account_id.clone();

            match wsv.asset(&asset_id)?.value {
                AssetValue::Quantity(increment) => {
                    wsv.decrease_asset_total_amount(&asset_id.definition_id, increment)?;
                }
                AssetValue::BigQuantity(increment) => {
                    wsv.decrease_asset_total_amount(&asset_id.definition_id, increment)?;
                }
                AssetValue::Fixed(increment) => {
                    wsv.decrease_asset_total_amount(&asset_id.definition_id, increment)?;
                }
                AssetValue::Store(_) => {
                    wsv.decrease_asset_total_amount(&asset_id.definition_id, 1_u32)?;
                }
            }

            wsv.modify_account(&account_id, |account| {
                account
                    .remove_asset(&asset_id)
                    .map(|asset| {
                        AccountEvent::Asset(AssetEvent::Removed(AssetChanged {
                            asset_id: asset.id,
                            amount: asset.value,
                        }))
                    })
                    .ok_or_else(|| Error::Find(Box::new(FindError::Asset(asset_id))))
            })
        }
    }

    impl Execute for Mint<Account, PublicKey> {
        type Error = Error;

        #[metrics(+"mint_account_pubkey")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let public_key = self.object;

            wsv.modify_account(&account_id, |account| {
                if account.signatories.contains(&public_key) {
                    return Err(
                        ValidationError::new("Account already contains this signatory").into(),
                    );
                }

                account.add_signatory(public_key);
                Ok(AccountEvent::AuthenticationAdded(account_id.clone()))
            })
        }
    }

    impl Execute for Burn<Account, PublicKey> {
        type Error = Error;

        #[metrics(+"burn_account_pubkey")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let public_key = self.object;

            wsv.modify_account(&account_id, |account| {
                if account.signatories.len() < 2 {
                    return Err(ValidationError::new(
                        "Public keys cannot be burned to nothing. \
                         If you want to delete the account, please use an unregister instruction.",
                    )
                    .into());
                }
                if !account.remove_signatory(&public_key) {
                    return Err(ValidationError::new("Public key not found").into());
                }

                Ok(AccountEvent::AuthenticationRemoved(account_id.clone()))
            })
        }
    }

    impl Execute for Mint<Account, SignatureCheckCondition> {
        type Error = Error;

        #[metrics(+"mint_account_signature_check_condition")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let signature_check_condition = self.object;

            wsv.modify_account(&account_id, |account| {
                account.signature_check_condition = signature_check_condition;
                Ok(AccountEvent::AuthenticationAdded(account_id.clone()))
            })
        }
    }

    impl Execute for SetKeyValue<Account, Name, Value> {
        type Error = Error;

        #[metrics(+"set_key_value_account_string_value")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.object_id;

            let account_metadata_limits = wsv.config.account_metadata_limits;

            wsv.modify_account(&account_id, |account| {
                account.metadata.insert_with_limits(
                    self.key.clone(),
                    self.value.clone(),
                    account_metadata_limits,
                )?;

                Ok(AccountEvent::MetadataInserted(MetadataChanged {
                    target_id: account_id.clone(),
                    key: self.key.clone(),
                    value: Box::new(self.value),
                }))
            })
        }
    }

    impl Execute for RemoveKeyValue<Account, Name> {
        type Error = Error;

        #[metrics(+"remove_account_key_value")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.object_id;

            wsv.modify_account(&account_id, |account| {
                let value = account
                    .metadata
                    .remove(&self.key)
                    .ok_or_else(|| FindError::MetadataKey(self.key.clone()))?;

                Ok(AccountEvent::MetadataRemoved(MetadataChanged {
                    target_id: account_id.clone(),
                    key: self.key,
                    value: Box::new(value),
                }))
            })
        }
    }

    impl Execute for Grant<Account, PermissionToken> {
        type Error = Error;

        #[metrics(+"grant_account_permission_token")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let permission = self.object;

            let definition = wsv
                .permission_token_definitions()
                .get(&permission.definition_id)
                .ok_or_else(|| {
                    FindError::PermissionTokenDefinition(permission.definition_id.clone())
                })?;

            permissions::check_permission_token_parameters(&permission, definition.value())?;

            wsv.modify_account(&account_id, |account| {
                let id = account.id();
                if wsv.account_contains_inherent_permission(id, &permission) {
                    return Err(ValidationError::new("Permission already exists").into());
                }

                let permission_id = permission.definition_id.clone();

                wsv.add_account_permission(id, permission);
                Ok(AccountEvent::PermissionAdded(AccountPermissionChanged {
                    account_id: id.clone(),
                    permission_id,
                }))
            })
        }
    }

    impl Execute for Revoke<Account, PermissionToken> {
        type Error = Error;

        #[metrics(+"revoke_account_permission_token")]
        fn execute(self, _authority: AccountId, wsv: &WorldStateView) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let permission = self.object;

            wsv.modify_account(&account_id, |_| {
                if !wsv
                    .permission_token_definitions()
                    .contains_key(&permission.definition_id)
                {
                    error!(%permission, "Revoking non-existent token");
                }
                if !wsv.remove_account_permission(&account_id, &permission) {
                    return Err(ValidationError::new("Permission not found").into());
                }
                Ok(AccountEvent::PermissionRemoved(AccountPermissionChanged {
                    account_id: account_id.clone(),
                    permission_id: permission.definition_id,
                }))
            })
        }
    }

    impl Execute for Grant<Account, RoleId> {
        type Error = Error;

        #[metrics(+"grant_account_role")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView,
        ) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let role_id = self.object;

            wsv.world()
                .roles
                .get(&role_id)
                .ok_or_else(|| FindError::Role(role_id.clone()))?;

            wsv.modify_account_multiple_events(&account_id, |account| {
                if !account.add_role(role_id.clone()) {
                    return Err(Error::Repetition(
                        InstructionType::Grant,
                        IdBox::RoleId(role_id),
                    ));
                }
                let events: Vec<_> = wsv
                    .roles()
                    .get(&role_id)
                    .iter()
                    .flat_map(|role| &role.permissions)
                    .map(|token| &token.definition_id)
                    .cloned()
                    .map(|permission_id| AccountPermissionChanged {
                        account_id: account_id.clone(),
                        permission_id,
                    })
                    .map(AccountEvent::PermissionAdded)
                    .chain(std::iter::once(AccountEvent::RoleGranted(
                        AccountRoleChanged {
                            account_id: account_id.clone(),
                            role_id,
                        },
                    )))
                    .collect();

                Ok(events)
            })
        }
    }

    impl Execute for Revoke<Account, RoleId> {
        type Error = Error;

        #[metrics(+"revoke_account_role")]
        fn execute(self, _authority: AccountId, wsv: &WorldStateView) -> Result<(), Self::Error> {
            let account_id = self.destination_id;
            let role_id = self.object;

            wsv.world()
                .roles
                .get(&role_id)
                .ok_or_else(|| FindError::Role(role_id.clone()))?;

            wsv.modify_account_multiple_events(&account_id, |account| {
                if !account.remove_role(&role_id) {
                    return Err(FindError::Role(role_id).into());
                }
                let events: Vec<_> = wsv
                    .roles()
                    .get(&role_id)
                    .iter()
                    .flat_map(|role| &role.permissions)
                    .map(|token| &token.definition_id)
                    .cloned()
                    .map(|permission_id| AccountPermissionChanged {
                        account_id: account_id.clone(),
                        permission_id,
                    })
                    .map(AccountEvent::PermissionRemoved)
                    .chain(std::iter::once(AccountEvent::RoleRevoked(
                        AccountRoleChanged {
                            account_id: account_id.clone(),
                            role_id,
                        },
                    )))
                    .collect();

                Ok(events)
            })
        }
    }

    /// Assert that this asset can be registered to an account.
    fn assert_can_register(
        definition_id: &AssetDefinitionId,
        wsv: &WorldStateView,
        value: &AssetValue,
    ) -> Result<(), Error> {
        let definition = asset::isi::assert_asset_type(definition_id, wsv, value.value_type())?;
        match definition.mintable {
            Mintable::Infinitely => Ok(()),
            Mintable::Not => Err(Error::Mintability(MintabilityError::MintUnmintable)),
            Mintable::Once => {
                if !value.is_zero_value() {
                    wsv.modify_asset_definition_entry(definition_id, |entry| {
                        forbid_minting(&mut entry.definition)?;
                        Ok(AssetDefinitionEvent::MintabilityChanged(
                            definition_id.clone(),
                        ))
                    })?;
                }
                Ok(())
            }
        }
    }

    /// Stop minting on the [`AssetDefinition`] globally.
    ///
    /// # Errors
    /// If the [`AssetDefinition`] is not `Mintable::Once`.
    #[inline]
    pub fn forbid_minting(definition: &mut AssetDefinition) -> Result<(), MintabilityError> {
        if definition.mintable == Mintable::Once {
            definition.mintable = Mintable::Not;
            Ok(())
        } else {
            Err(MintabilityError::ForbidMintOnMintable)
        }
    }

    #[cfg(test)]
    mod test {
        use iroha_data_model::{prelude::AssetDefinition, ParseError};

        use crate::smartcontracts::isi::Registrable as _;

        #[test]
        fn cannot_forbid_minting_on_asset_mintable_infinitely() -> Result<(), ParseError> {
            let mut definition = AssetDefinition::quantity("test#hello".parse()?).build();
            assert!(super::forbid_minting(&mut definition).is_err());
            Ok(())
        }
    }
}

/// Account-related [`Query`] instructions.
pub mod query {

    use eyre::{Result, WrapErr};
    use iroha_data_model::query::error::QueryExecutionFailure as Error;

    use super::{super::Evaluate, *};

    impl ValidQuery for FindRolesByAccountId {
        #[metrics(+"find_roles_by_account_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let account_id = self
                .id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to evaluate account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%account_id, roles=?wsv.world.roles);
            let roles = wsv.map_account(&account_id, |account| {
                account.roles.iter().cloned().collect::<Vec<_>>()
            })?;
            Ok(roles)
        }
    }

    impl ValidQuery for FindPermissionTokensByAccountId {
        #[metrics(+"find_permission_tokens_by_account_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let account_id = self
                .id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to evaluate account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%account_id, accounts=?wsv.world.domains);
            let tokens = wsv.map_account(&account_id, |account| {
                wsv.account_permission_tokens(account)
            })?;
            Ok(tokens.into_iter().collect())
        }
    }

    impl ValidQuery for FindAllAccounts {
        #[metrics(+"find_all_accounts")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let mut vec = Vec::new();
            for domain in wsv.domains().iter() {
                for account in domain.accounts.values() {
                    vec.push(account.clone())
                }
            }
            Ok(vec)
        }
    }

    impl ValidQuery for FindAccountById {
        #[metrics(+"find_account_by_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = self
                .id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id);
            wsv.map_account(&id, Clone::clone).map_err(Into::into)
        }
    }

    impl ValidQuery for FindAccountsByName {
        #[metrics(+"find_account_by_name")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let name = self
                .name
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get account name")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%name);
            let mut vec = Vec::new();
            for domain in wsv.domains().iter() {
                for account in domain.accounts.values() {
                    if account.id().name == name {
                        vec.push(account.clone())
                    }
                }
            }
            Ok(vec)
        }
    }

    impl ValidQuery for FindAccountsByDomainId {
        #[metrics(+"find_accounts_by_domain_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = self
                .domain_id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get domain id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id);
            Ok(wsv.domain(&id)?.accounts.values().cloned().collect())
        }
    }

    impl ValidQuery for FindAccountKeyValueByIdAndKey {
        #[metrics(+"find_account_key_value_by_id_and_key")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = self
                .id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            let key = self
                .key
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get key")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id, %key);
            wsv.map_account(&id, |account| account.metadata.get(&key).map(Clone::clone))?
                .ok_or_else(|| FindError::MetadataKey(key).into())
        }
    }

    impl ValidQuery for FindAccountsWithAsset {
        #[metrics(+"find_accounts_with_asset")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let asset_definition_id = self
                .asset_definition_id
                .evaluate(&Context::new(wsv))
                .wrap_err("Failed to get asset id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%asset_definition_id);

            let domain_id = &asset_definition_id.domain_id;

            wsv.map_domain(domain_id, |domain| {
                let found = domain
                    .accounts
                    .values()
                    .filter(|account| {
                        let asset_id =
                            AssetId::new(asset_definition_id.clone(), account.id().clone());
                        account.assets.get(&asset_id).is_some()
                    })
                    .cloned()
                    .collect();
                Ok(found)
            })
            .map_err(Into::into)
        }
    }
}
