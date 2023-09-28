//! This module contains implementations of smart-contract traits and instructions for [`Account`] structure
//! and implementations of [`Query`]'s to [`WorldStateView`] about [`Account`].

use iroha_data_model::{asset::AssetsMap, prelude::*, query::error::FindError, role::RoleIds};
use iroha_telemetry::metrics;

use super::prelude::*;
use crate::{ValidQuery, WorldStateView};

impl Registrable for iroha_data_model::account::NewAccount {
    type Target = Account;

    #[must_use]
    #[inline]
    fn build(self, _authority: &AccountId) -> Self::Target {
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
        isi::{
            error::{MintabilityError, RepetitionError},
            InstructionType,
        },
        query::error::QueryExecutionFail,
    };

    use super::*;

    #[allow(clippy::expect_used, clippy::unwrap_in_result)]
    impl Execute for Register<Asset> {
        #[metrics(+"register_asset")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let asset_id = self.object.id;

            match wsv.asset(&asset_id) {
                Err(err) => match err {
                    QueryExecutionFail::Find(find_err)
                        if matches!(find_err, FindError::Asset(_)) =>
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
                Ok(_) => Err(RepetitionError {
                    instruction_type: InstructionType::Register,
                    id: IdBox::AssetId(asset_id.clone()),
                }
                .into()),
            }
        }
    }

    impl Execute for Unregister<Asset> {
        #[metrics(+"unregister_asset")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let asset_id = self.object_id;
            let account_id = asset_id.account_id.clone();

            let asset = wsv.account_mut(&account_id).and_then(|account| {
                account
                    .remove_asset(&asset_id)
                    .ok_or_else(|| FindError::Asset(asset_id))
            })?;

            match asset.value {
                AssetValue::Quantity(increment) => {
                    wsv.decrease_asset_total_amount(&asset.id.definition_id, increment)?;
                }
                AssetValue::BigQuantity(increment) => {
                    wsv.decrease_asset_total_amount(&asset.id.definition_id, increment)?;
                }
                AssetValue::Fixed(increment) => {
                    wsv.decrease_asset_total_amount(&asset.id.definition_id, increment)?;
                }
                AssetValue::Store(_) => {
                    wsv.decrease_asset_total_amount(&asset.id.definition_id, 1_u32)?;
                }
            }

            wsv.emit_events(Some(AccountEvent::Asset(AssetEvent::Removed(
                AssetChanged {
                    asset_id: asset.id,
                    amount: asset.value,
                },
            ))));

            Ok(())
        }
    }

    impl Execute for Mint<PublicKey, Account> {
        #[metrics(+"mint_account_public_key")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let public_key = self.object;

            wsv.account_mut(&account_id)
                .map_err(Error::from)
                .and_then(|account| {
                    if account.signatories.contains(&public_key) {
                        return Err(RepetitionError {
                            instruction_type: InstructionType::Mint,
                            id: account_id.clone().into(),
                        }
                        .into());
                    }

                    account.add_signatory(public_key);
                    Ok(())
                })?;

            wsv.emit_events(Some(AccountEvent::AuthenticationAdded(account_id.clone())));

            Ok(())
        }
    }

    impl Execute for Burn<PublicKey, Account> {
        #[metrics(+"burn_account_public_key")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let public_key = self.object;

            wsv.account_mut(&account_id)
                .map_err(Error::from)
                .and_then(|account| {
                    if account.signatories.len() < 2 {
                        return Err(Error::InvariantViolation(String::from(
                            "Public keys cannot be burned to nothing, \
                            if you want to delete the account, please use an unregister instruction",
                        )));
                    }
                    if !account.remove_signatory(&public_key) {
                        return Err(FindError::PublicKey(public_key).into());
                    }
                    Ok(())
                })?;

            wsv.emit_events(Some(AccountEvent::AuthenticationRemoved(account_id)));

            Ok(())
        }
    }

    impl Execute for Mint<SignatureCheckCondition, Account> {
        #[metrics(+"mint_account_signature_check_condition")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let signature_check_condition = self.object;

            wsv.account_mut(&account_id)?.signature_check_condition = signature_check_condition;

            wsv.emit_events(Some(AccountEvent::AuthenticationAdded(account_id.clone())));

            Ok(())
        }
    }

    impl Execute for Transfer<Account, AssetDefinition, Account> {
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            wsv.asset_definition_mut(&self.object.id)?.owned_by = self.destination_id.clone();

            wsv.emit_events(Some(AssetDefinitionEvent::OwnerChanged(
                AssetDefinitionOwnerChanged {
                    asset_definition_id: self.object.id,
                    new_owner: self.destination_id,
                },
            )));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Account> {
        #[metrics(+"set_account_key_value")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.object_id;

            let account_metadata_limits = wsv.config.account_metadata_limits;

            wsv.account_mut(&account_id)
                .map_err(Error::from)
                .and_then(|account| {
                    account
                        .metadata
                        .insert_with_limits(
                            self.key.clone(),
                            self.value.clone(),
                            account_metadata_limits,
                        )
                        .map_err(Error::from)
                })?;

            wsv.emit_events(Some(AccountEvent::MetadataInserted(MetadataChanged {
                target_id: account_id.clone(),
                key: self.key.clone(),
                value: Box::new(self.value),
            })));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<Account> {
        #[metrics(+"remove_account_key_value")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.object_id;

            let value = wsv.account_mut(&account_id).and_then(|account| {
                account
                    .metadata
                    .remove(&self.key)
                    .ok_or_else(|| FindError::MetadataKey(self.key.clone()))
            })?;

            wsv.emit_events(Some(AccountEvent::MetadataRemoved(MetadataChanged {
                target_id: account_id.clone(),
                key: self.key,
                value: Box::new(value),
            })));

            Ok(())
        }
    }

    impl Execute for Grant<PermissionToken> {
        #[metrics(+"grant_account_permission")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let permission = self.object;
            let permission_id = permission.definition_id.clone();

            // Check if account exists
            wsv.account_mut(&account_id)?;

            if !wsv
                .permission_token_schema()
                .token_ids
                .contains(&permission_id)
            {
                return Err(FindError::PermissionToken(permission_id).into());
            }

            if wsv.account_contains_inherent_permission(&account_id, &permission) {
                return Err(RepetitionError {
                    instruction_type: InstructionType::Grant,
                    id: permission.definition_id.into(),
                }
                .into());
            }

            wsv.add_account_permission(&account_id, permission);

            wsv.emit_events(Some(AccountEvent::PermissionAdded(
                AccountPermissionChanged {
                    account_id,
                    permission_id,
                },
            )));

            Ok(())
        }
    }

    impl Execute for Revoke<PermissionToken> {
        #[metrics(+"revoke_account_permission")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let permission = self.object;

            // Check if account exists
            wsv.account_mut(&account_id)?;

            if !wsv.remove_account_permission(&account_id, &permission) {
                return Err(FindError::PermissionToken(permission.definition_id).into());
            }

            wsv.emit_events(Some(AccountEvent::PermissionRemoved(
                AccountPermissionChanged {
                    account_id,
                    permission_id: permission.definition_id,
                },
            )));

            Ok(())
        }
    }

    impl Execute for Grant<RoleId> {
        #[metrics(+"grant_account_role")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let role_id = self.object;

            let permissions = wsv
                .world()
                .roles
                .get(&role_id)
                .ok_or_else(|| FindError::Role(role_id.clone()))?
                .clone()
                .permissions
                .into_iter()
                .map(|token| token.definition_id);

            wsv.account_mut(&account_id)
                .map_err(Error::from)
                .and_then(|account| {
                    if !account.add_role(role_id.clone()) {
                        return Err(RepetitionError {
                            instruction_type: InstructionType::Grant,
                            id: IdBox::RoleId(role_id.clone()),
                        }
                        .into());
                    }
                    Ok(())
                })?;

            wsv.emit_events({
                let account_id_clone = account_id.clone();
                permissions
                    .zip(core::iter::repeat_with(move || account_id.clone()))
                    .map(|(permission_id, account_id)| AccountPermissionChanged {
                        account_id,
                        permission_id,
                    })
                    .map(AccountEvent::PermissionAdded)
                    .chain(std::iter::once(AccountEvent::RoleGranted(
                        AccountRoleChanged {
                            account_id: account_id_clone,
                            role_id,
                        },
                    )))
            });

            Ok(())
        }
    }

    impl Execute for Revoke<RoleId> {
        #[metrics(+"revoke_account_role")]
        fn execute(self, _authority: &AccountId, wsv: &mut WorldStateView) -> Result<(), Error> {
            let account_id = self.destination_id;
            let role_id = self.object;

            let permissions = wsv
                .world()
                .roles
                .get(&role_id)
                .ok_or_else(|| FindError::Role(role_id.clone()))?
                .clone()
                .permissions
                .into_iter()
                .map(|token| token.definition_id);

            wsv.account_mut(&account_id).and_then(|account| {
                if !account.remove_role(&role_id) {
                    return Err(FindError::Role(role_id.clone()));
                }
                Ok(())
            })?;

            wsv.emit_events({
                let account_id_clone = account_id.clone();
                permissions
                    .zip(core::iter::repeat_with(move || account_id.clone()))
                    .map(|(permission_id, account_id)| AccountPermissionChanged {
                        account_id,
                        permission_id,
                    })
                    .map(AccountEvent::PermissionRemoved)
                    .chain(std::iter::once(AccountEvent::RoleRevoked(
                        AccountRoleChanged {
                            account_id: account_id_clone,
                            role_id,
                        },
                    )))
            });

            Ok(())
        }
    }

    /// Assert that this asset can be registered to an account.
    fn assert_can_register(
        definition_id: &AssetDefinitionId,
        wsv: &mut WorldStateView,
        value: &AssetValue,
    ) -> Result<(), Error> {
        let definition = asset::isi::assert_asset_type(definition_id, wsv, value.value_type())?;
        match definition.mintable {
            Mintable::Infinitely => Ok(()),
            Mintable::Not => Err(Error::Mintability(MintabilityError::MintUnmintable)),
            Mintable::Once => {
                if !value.is_zero_value() {
                    let asset_definition = wsv.asset_definition_mut(definition_id)?;
                    forbid_minting(asset_definition)?;
                    wsv.emit_events(Some(AssetDefinitionEvent::MintabilityChanged(
                        definition_id.clone(),
                    )));
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
            let authority = "alice@wonderland".parse()?;
            let mut definition = AssetDefinition::quantity("test#hello".parse()?).build(&authority);
            assert!(super::forbid_minting(&mut definition).is_err());
            Ok(())
        }
    }
}

/// Account-related [`Query`] instructions.
pub mod query {

    use eyre::{Result, WrapErr};
    use iroha_data_model::{
        account::Account,
        evaluate::ExpressionEvaluator,
        permission::PermissionToken,
        query::{error::QueryExecutionFail as Error, MetadataValue},
    };

    use super::*;

    impl ValidQuery for FindRolesByAccountId {
        #[metrics(+"find_roles_by_account_id")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = RoleId> + 'wsv>, Error> {
            let account_id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to evaluate account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%account_id, roles=?wsv.world.roles);
            Ok(Box::new(
                wsv.map_account(&account_id, |account| &account.roles)?
                    .iter()
                    .cloned(),
            ))
        }
    }

    impl ValidQuery for FindPermissionTokensByAccountId {
        #[metrics(+"find_permission_tokens_by_account_id")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = PermissionToken> + 'wsv>, Error> {
            let account_id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to evaluate account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%account_id, accounts=?wsv.world.domains);
            Ok(Box::new(
                wsv.account_permission_tokens(&account_id)?.cloned(),
            ))
        }
    }

    impl ValidQuery for FindAllAccounts {
        #[metrics(+"find_all_accounts")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = Account> + 'wsv>, Error> {
            Ok(Box::new(
                wsv.domains()
                    .values()
                    .flat_map(|domain| domain.accounts.values())
                    .cloned(),
            ))
        }
    }

    impl ValidQuery for FindAccountById {
        #[metrics(+"find_account_by_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Account, Error> {
            let id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to evaluate id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id);
            wsv.map_account(&id, Clone::clone).map_err(Into::into)
        }
    }

    impl ValidQuery for FindAccountsByName {
        #[metrics(+"find_account_by_name")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = Account> + 'wsv>, Error> {
            let name = wsv
                .evaluate(&self.name)
                .wrap_err("Failed to evaluate account name")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%name);
            Ok(Box::new(
                wsv.domains()
                    .values()
                    .flat_map(move |domain| {
                        let name = name.clone();

                        domain
                            .accounts
                            .values()
                            .filter(move |account| account.id().name == name)
                    })
                    .cloned(),
            ))
        }
    }

    impl ValidQuery for FindAccountsByDomainId {
        #[metrics(+"find_accounts_by_domain_id")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = Account> + 'wsv>, Error> {
            let id = wsv
                .evaluate(&self.domain_id)
                .wrap_err("Failed to evaluate domain id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;

            iroha_logger::trace!(%id);
            Ok(Box::new(wsv.domain(&id)?.accounts.values().cloned()))
        }
    }

    impl ValidQuery for FindAccountKeyValueByIdAndKey {
        #[metrics(+"find_account_key_value_by_id_and_key")]
        fn execute(&self, wsv: &WorldStateView) -> Result<MetadataValue, Error> {
            let id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to evaluate account id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            let key = wsv
                .evaluate(&self.key)
                .wrap_err("Failed to evaluate key")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id, %key);
            wsv.map_account(&id, |account| account.metadata.get(&key).map(Clone::clone))?
                .ok_or_else(|| FindError::MetadataKey(key).into())
                .map(Into::into)
        }
    }

    impl ValidQuery for FindAccountsWithAsset {
        #[metrics(+"find_accounts_with_asset")]
        fn execute<'wsv>(
            &self,
            wsv: &'wsv WorldStateView,
        ) -> Result<Box<dyn Iterator<Item = Account> + 'wsv>, Error> {
            let asset_definition_id = wsv
                .evaluate(&self.asset_definition_id)
                .wrap_err("Failed to evaluate asset id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%asset_definition_id);

            Ok(Box::new(
                wsv.map_domain(&asset_definition_id.domain_id.clone(), move |domain| {
                    domain.accounts.values().filter(move |account| {
                        let asset_id =
                            AssetId::new(asset_definition_id.clone(), account.id().clone());
                        account.assets.get(&asset_id).is_some()
                    })
                })?
                .cloned(),
            ))
        }
    }
}
