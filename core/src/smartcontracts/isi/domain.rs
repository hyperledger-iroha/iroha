//! This module contains [`Domain`] structure and related implementations and trait implementations.

use eyre::Result;
use iroha_data_model::{
    account::AccountsMap,
    asset::{AssetDefinitionsMap, AssetTotalQuantityMap},
    prelude::*,
    query::error::FindError,
};
use iroha_telemetry::metrics;

use super::super::isi::prelude::*;

impl Registrable for iroha_data_model::domain::NewDomain {
    type Target = Domain;

    #[must_use]
    #[inline]
    fn build(self, _authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            accounts: AccountsMap::default(),
            asset_definitions: AssetDefinitionsMap::default(),
            asset_total_quantities: AssetTotalQuantityMap::default(),
            metadata: self.metadata,
            logo: self.logo,
        }
    }
}

/// ISI module contains all instructions related to domains:
/// - creating/changing assets
/// - registering/unregistering accounts
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_logger::prelude::*;

    use super::*;

    impl Execute for Register<Account> {
        #[metrics(+"register_account")]
        fn execute(self, authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let account: Account = self.object.build(authority);
            let account_id = account.id().clone();

            account_id
                .name
                .validate_len(wsv.config.borrow().ident_length_limits)
                .map_err(Error::from)?;

            wsv.modify_domain(&account_id.domain_id.clone(), |domain| {
                if domain.accounts.get(&account_id).is_some() {
                    return Err(Error::Repetition(
                        InstructionType::Register,
                        IdBox::AccountId(account_id),
                    ));
                }

                domain.add_account(account.clone());
                Ok(DomainEvent::Account(AccountEvent::Created(account)))
            })
        }
    }

    impl Execute for Unregister<Account> {
        #[metrics(+"unregister_account")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let account_id = self.object_id;

            wsv.modify_domain(&account_id.domain_id.clone(), |domain| {
                if domain.remove_account(&account_id).is_none() {
                    return Err(FindError::Account(account_id).into());
                }

                Ok(DomainEvent::Account(AccountEvent::Deleted(account_id)))
            })
        }
    }

    impl Execute for Register<AssetDefinition> {
        #[metrics(+"register_asset_definition")]
        fn execute(self, authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let asset_definition = self.object.build(authority);
            asset_definition
                .id()
                .name
                .validate_len(wsv.config.borrow().ident_length_limits)
                .map_err(Error::from)?;

            let asset_definition_id = asset_definition.id().clone();
            wsv.modify_domain(&asset_definition_id.domain_id.clone(), |domain| {
                if domain.asset_definitions.get(&asset_definition_id).is_some() {
                    return Err(Error::Repetition(
                        InstructionType::Register,
                        IdBox::AssetDefinitionId(asset_definition_id),
                    ));
                }

                #[allow(clippy::match_same_arms)]
                match asset_definition.value_type {
                    AssetValueType::Fixed => {
                        domain.add_asset_total_quantity(asset_definition_id, Fixed::ZERO);
                    }
                    AssetValueType::Quantity => {
                        domain.add_asset_total_quantity(asset_definition_id, u32::MIN);
                    }
                    AssetValueType::BigQuantity => {
                        domain.add_asset_total_quantity(asset_definition_id, u128::MIN);
                    }
                    AssetValueType::Store => {
                        domain.add_asset_total_quantity(asset_definition_id, u32::MIN);
                    }
                }

                domain.add_asset_definition(asset_definition.clone());
                Ok(DomainEvent::AssetDefinition(AssetDefinitionEvent::Created(
                    asset_definition,
                )))
            })
        }
    }

    impl Execute for Unregister<AssetDefinition> {
        #[metrics(+"unregister_asset_definition")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let asset_definition_id = self.object_id;

            let mut assets_to_remove = Vec::new();
            for domain in wsv.domains().iter() {
                for account in domain.accounts.values() {
                    assets_to_remove.extend(
                        account
                            .assets
                            .values()
                            .filter_map(|asset| {
                                if asset.id().definition_id == asset_definition_id {
                                    return Some(asset.id());
                                }

                                None
                            })
                            .cloned(),
                    )
                }
            }

            for asset_id in assets_to_remove {
                let account_id = asset_id.account_id.clone();
                wsv.modify_account(&account_id, |account| {
                    if account.remove_asset(&asset_id).is_none() {
                        error!(%asset_id, "asset not found. This is a bug");
                    }

                    Ok(AccountEvent::Asset(AssetEvent::Deleted(asset_id)))
                })?;
            }

            wsv.modify_domain(&asset_definition_id.domain_id.clone(), |domain| {
                if domain
                    .remove_asset_definition(&asset_definition_id)
                    .is_none()
                {
                    return Err(FindError::AssetDefinition(asset_definition_id).into());
                }

                domain.remove_asset_total_quantity(&asset_definition_id);

                Ok(DomainEvent::AssetDefinition(AssetDefinitionEvent::Deleted(
                    asset_definition_id,
                )))
            })?;

            Ok(())
        }
    }

    impl Execute for SetKeyValue<AssetDefinition> {
        #[metrics(+"set_key_value_asset_definition")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let asset_definition_id = self.object_id;

            let metadata_limits = wsv.config.borrow().asset_definition_metadata_limits;
            wsv.modify_asset_definition(&asset_definition_id.clone(), |asset_definition| {
                asset_definition.metadata.insert_with_limits(
                    self.key.clone(),
                    self.value.clone(),
                    metadata_limits,
                )?;

                Ok(AssetDefinitionEvent::MetadataInserted(MetadataChanged {
                    target_id: asset_definition_id,
                    key: self.key,
                    value: Box::new(self.value),
                }))
            })
        }
    }

    impl Execute for RemoveKeyValue<AssetDefinition> {
        #[metrics(+"remove_key_value_asset_definition")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let asset_definition_id = self.object_id;

            wsv.modify_asset_definition(&asset_definition_id.clone(), |asset_definition| {
                let value = asset_definition
                    .metadata
                    .remove(&self.key)
                    .ok_or_else(|| FindError::MetadataKey(self.key.clone()))?;

                Ok(AssetDefinitionEvent::MetadataRemoved(MetadataChanged {
                    target_id: asset_definition_id,
                    key: self.key,
                    value: Box::new(value),
                }))
            })
        }
    }

    impl Execute for SetKeyValue<Domain> {
        #[metrics(+"set_domain_key_value")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let domain_id = self.object_id;

            let limits = wsv.config.borrow().domain_metadata_limits;

            wsv.modify_domain(&domain_id.clone(), |domain| {
                domain
                    .metadata
                    .insert_with_limits(self.key.clone(), self.value.clone(), limits)?;

                Ok(DomainEvent::MetadataInserted(MetadataChanged {
                    target_id: domain_id,
                    key: self.key,
                    value: Box::new(self.value),
                }))
            })
        }
    }

    impl Execute for RemoveKeyValue<Domain> {
        #[metrics(+"remove_domain_key_value")]
        fn execute(self, _authority: &AccountId, wsv: &WorldStateView) -> Result<(), Error> {
            let domain_id = self.object_id;

            wsv.modify_domain(&domain_id.clone(), |domain| {
                let value = domain
                    .metadata
                    .remove(&self.key)
                    .ok_or_else(|| FindError::MetadataKey(self.key.clone()))?;

                Ok(DomainEvent::MetadataRemoved(MetadataChanged {
                    target_id: domain_id,
                    key: self.key,
                    value: Box::new(value),
                }))
            })
        }
    }
}

/// Query module provides [`Query`] Domain related implementations.
pub mod query {
    use eyre::{Result, WrapErr};
    use iroha_data_model::query::error::QueryExecutionFailure as Error;

    use super::*;

    impl ValidQuery for FindAllDomains {
        #[metrics(+"find_all_domains")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            Ok(wsv
                .domains()
                .iter()
                .map(|guard| guard.value().clone())
                .collect())
        }
    }

    impl ValidQuery for FindDomainById {
        #[metrics(+"find_domain_by_id")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to get domain id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id);
            Ok(wsv.domain(&id)?.clone())
        }
    }

    impl ValidQuery for FindDomainKeyValueByIdAndKey {
        #[metrics(+"find_domain_key_value_by_id_and_key")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to get domain id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            let key = wsv
                .evaluate(&self.key)
                .wrap_err("Failed to get key")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id, %key);
            wsv.map_domain(&id, |domain| {
                Ok(domain.metadata.get(&key).map(Clone::clone))
            })?
            .ok_or_else(|| FindError::MetadataKey(key).into())
        }
    }

    impl ValidQuery for FindAssetDefinitionKeyValueByIdAndKey {
        #[metrics(+"find_asset_definition_key_value_by_id_and_key")]
        fn execute(&self, wsv: &WorldStateView) -> Result<Self::Output, Error> {
            let id = wsv
                .evaluate(&self.id)
                .wrap_err("Failed to get asset definition id")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            let key = wsv
                .evaluate(&self.key)
                .wrap_err("Failed to get key")
                .map_err(|e| Error::Evaluate(e.to_string()))?;
            iroha_logger::trace!(%id, %key);
            Ok(wsv
                .asset_definition(&id)?
                .metadata
                .get(&key)
                .ok_or(FindError::MetadataKey(key))?
                .clone())
        }
    }
}
