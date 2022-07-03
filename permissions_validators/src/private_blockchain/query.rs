//! Query Permissions.

use iroha_core::smartcontracts::permissions::ValidatorVerdict;

use super::*;

/// Allow queries that only access the data of the domain of the signer.
#[derive(Debug, Copy, Clone, Serialize)]
pub struct OnlyAccountsDomain;

impl IsAllowed for OnlyAccountsDomain {
    type Operation = QueryBox;

    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn check(
        &self,
        authority: &AccountId,
        query: &QueryBox,
        wsv: &WorldStateView,
    ) -> ValidatorVerdict {
        use QueryBox::*;
        let context = Context::new();
        match query {
            FindAssetsByAssetDefinitionId(_) | FindAssetsByName(_) | FindAllAssets(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom(
                    "Only access to the assets of the same domain is permitted."
                        .to_owned()
                        .into(),
                ))
            }
            FindAllAccounts(_) | FindAccountsByName(_) | FindAccountsWithAsset(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom(
                    "Only access to the accounts of the same domain is permitted."
                        .to_owned()
                        .into(),
                ))
            }
            FindAllAssetsDefinitions(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Only access to the asset definitions of the same domain is permitted."
                    .to_owned()
                    .into(),
            )),
            FindAllDomains(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Only access to the domain of the account is permitted."
                    .to_owned()
                    .into(),
            )),
            FindAllRoles(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Only access to roles of the same domain is permitted."
                    .to_owned()
                    .into(),
            )),
            FindAllRoleIds(_) => ValidatorVerdict::Allow, // In case you need to debug the permissions.
            FindRoleByRoleId(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Only access to roles of the same domain is permitted."
                    .to_owned()
                    .into(),
            )),
            FindAllPeers(_) => ValidatorVerdict::Allow, // Can be obtained in other ways, so why hide it.
            FindAllActiveTriggerIds(_) => ValidatorVerdict::Allow,
            // Private blockchains should have debugging too, hence
            // all accounts should also be
            FindTriggerById(query) => {
                let id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|e| e.to_string())?;
                wsv.triggers()
                    .inspect(&id, |action| {
                        if action.technical_account() == authority {
                            ValidatorVerdict::Allow
                        } else {
                            ValidatorVerdict::Deny(DenialReason::Custom(
                                "Cannot access Trigger if you're not the technical account."
                                    .to_owned()
                                    .into(),
                            ))
                        }
                    })
                    .ok_or_else(|| {
                        format!(
                            "A trigger with the specified Id: {} is not accessible to you",
                            id.clone()
                        )
                    })?
            }
            FindTriggerKeyValueByIdAndKey(query) => {
                let id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|e| e.to_string())?;
                wsv.triggers()
                    .inspect(&id, |action| {
                        if action.technical_account() == authority {
                            ValidatorVerdict::Allow
                        } else {
                            ValidatorVerdict::Deny(DenialReason::Custom(
                                "Cannot access Trigger internal state if you're not the technical account."
                            .to_owned().into()
                            ))
                        }
                    })
                    .ok_or_else(|| {
                        format!(
                            "A trigger with the specified Id: {} is not accessible to you",
                            id.clone()
                        )
                    })?
            }
            FindTriggersByDomainId(query) => {
                let domain_id = query
                    .domain_id
                    .evaluate(wsv, &context)
                    .map_err(|e| e.to_string())?;

                if domain_id == authority.domain_id {
                    return ValidatorVerdict::Allow;
                }

                ValidatorVerdict::Deny(DenialReason::Custom(
                    format!(
                        "Cannot access triggers with given domain {}, {} is permitted..",
                        domain_id, authority.domain_id
                    )
                    .into(),
                ))
            }
            FindAccountById(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindAccountKeyValueByIdAndKey(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindAccountsByDomainId(query) => {
                let domain_id = query
                    .domain_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access accounts from a different domain with name {}.",
                            domain_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetById(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if asset_id.account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access asset {} as it is in a different domain.",
                            asset_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetsByAccountId(query) => {
                let account_id = query
                    .account_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetsByDomainId(query) => {
                let domain_id = query
                    .domain_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access assets from a different domain with name {}.",
                            domain_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetsByDomainIdAndAssetDefinitionId(query) => {
                let domain_id = query
                    .domain_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access assets from a different domain with name {}.",
                            domain_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetDefinitionKeyValueByIdAndKey(query) => {
                let asset_definition_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if asset_definition_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access asset definition from a different domain. Asset definition domain: {}. Signer's account domain {}.",
                        asset_definition_id.domain_id,
                        authority.domain_id
                    ).into()))
                }
            }
            FindAssetQuantityById(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if asset_id.account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access asset {} as it is in a different domain.",
                            asset_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetKeyValueByIdAndKey(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if asset_id.account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access asset {} as it is in a different domain.",
                            asset_id
                        )
                        .into(),
                    ))
                }
            }
            FindDomainById(query::FindDomainById { id })
            | FindDomainKeyValueByIdAndKey(query::FindDomainKeyValueByIdAndKey { id, .. }) => {
                let domain_id = id.evaluate(wsv, &context).map_err(|err| err.to_string())?;
                if domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!("Cannot access a different domain: {}.", domain_id).into(),
                    ))
                }
            }
            FindAllBlocks(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Access to all blocks not permitted".to_owned().into(),
            )),
            FindAllTransactions(_) => ValidatorVerdict::Deny(DenialReason::Custom(
                "Only access to transactions in the same domain is permitted."
                    .to_owned()
                    .into(),
            )),
            FindTransactionsByAccountId(query) => {
                let account_id = query
                    .account_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindTransactionByHash(_query) => ValidatorVerdict::Allow,
            FindRolesByAccountId(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindPermissionTokensByAccountId(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if account_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(
                        format!(
                            "Cannot access account {} as it is in a different domain.",
                            account_id
                        )
                        .into(),
                    ))
                }
            }
            FindAssetDefinitionById(query) => {
                let asset_definition_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;

                if asset_definition_id.domain_id == authority.domain_id {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access asset definition from a different domain. Asset definition domain: {}. Signer's account domain {}.",
                        asset_definition_id.domain_id,
                        authority.domain_id,
                    ).into()))
                }
            }
        }
    }
}

/// Allow queries that only access the signers account data.
#[derive(Debug, Copy, Clone, Serialize)]
pub struct OnlyAccountsData;

impl IsAllowed for OnlyAccountsData {
    type Operation = QueryBox;

    #[allow(clippy::too_many_lines, clippy::match_same_arms)]
    fn check(
        &self,
        authority: &AccountId,
        query: &QueryBox,
        wsv: &WorldStateView,
    ) -> ValidatorVerdict {
        use QueryBox::*;

        let context = Context::new();
        match query {
            FindAccountsByName(_)
                | FindAccountsByDomainId(_)
                | FindAccountsWithAsset(_)
                | FindAllAccounts(_) => {
                    ValidatorVerdict::Deny(DenialReason::Custom("Other accounts are private.".to_owned().into()))
                }
                | FindAllDomains(_)
                | FindDomainById(_)
                | FindDomainKeyValueByIdAndKey(_) => {
                    ValidatorVerdict::Deny(DenialReason::Custom("Only access to your account's data is permitted.".to_owned().into()))
                },
            FindAssetsByDomainIdAndAssetDefinitionId(_)
                | FindAssetsByName(_) // TODO: I think this is a mistake.
                | FindAssetsByDomainId(_)
                | FindAllAssetsDefinitions(_)
                | FindAssetsByAssetDefinitionId(_)
                | FindAssetDefinitionById(_)
                | FindAssetDefinitionKeyValueByIdAndKey(_)
                | FindAllAssets(_) => {
                    ValidatorVerdict::Deny(DenialReason::Custom("Only access to the assets of your account is permitted.".to_owned().into()))
                }
            FindAllRoles(_) | FindAllRoleIds(_) | FindRoleByRoleId(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom("Only access to roles of the same account is permitted.".to_owned().into()))
            },
            FindAllActiveTriggerIds(_) | FindTriggersByDomainId(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom("Only access to the triggers of the same account is permitted.".to_owned().into()))
            }
            FindAllPeers(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom("Only access to your account-local data is permitted.".to_owned().into()))
            }
            FindTriggerById(query) => {
                // TODO: should differentiate between global and domain-local triggers.
                let id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|e| e.to_string())?;
                if wsv.triggers().inspect(&id, |action|
                    action.technical_account() == authority
                ) == Some(true) {
                    return ValidatorVerdict::Allow
                }
                ValidatorVerdict::Deny(DenialReason::Custom(format!(
                    "A trigger with the specified Id: {} is not accessible to you",
                    id
                ).into()))
            }
            FindTriggerKeyValueByIdAndKey(query) => {
                // TODO: should differentiate between global and domain-local triggers.
                let id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if wsv.triggers().inspect(&id, |action|
                    action.technical_account() == authority
                ) == Some(true) {
                    return ValidatorVerdict::Allow
                }
                ValidatorVerdict::Deny(DenialReason::Custom(format!(
                    "A trigger with the specified Id: {} is not accessible to you",
                    id
                ).into()))
            }
            FindAccountById(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access account {} as only access to your own account, {} is permitted..",
                        account_id,
                        authority
                    ).into()))
                }
            }
            FindAccountKeyValueByIdAndKey(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access account {} as only access to your own account is permitted..",
                        account_id
                    ).into()))
                }
            }
            FindAssetById(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &asset_id.account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access asset {} as it is in a different account.",
                        asset_id
                    ).into()))
                }
            }
            FindAssetsByAccountId(query) => {
                let account_id = query
                    .account_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access a different account: {}.",
                        account_id
                    ).into()))
                }
            }

            FindAssetQuantityById(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &asset_id.account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access asset {} as it is in a different account.",
                        asset_id
                    ).into()))
                }
            }
            FindAssetKeyValueByIdAndKey(query) => {
                let asset_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &asset_id.account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!(
                        "Cannot access asset {} as it is in a different account.",
                        asset_id
                    ).into()))
                }
            }
            FindAllBlocks(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom("Access to all blocks not permitted".to_owned().into()))
            }
            FindAllTransactions(_) => {
                ValidatorVerdict::Deny(DenialReason::Custom("Only access to transactions of the same account is permitted.".to_owned().into()))
            },
            FindTransactionsByAccountId(query) => {
                let account_id = query
                    .account_id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!("Cannot access another account: {}.", account_id).into()))
                }
            }
            FindTransactionByHash(_query) => ValidatorVerdict::Allow,
            FindRolesByAccountId(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!("Cannot access another account: {}.", account_id).into()))
                }
            }
            FindPermissionTokensByAccountId(query) => {
                let account_id = query
                    .id
                    .evaluate(wsv, &context)
                    .map_err(|err| err.to_string())?;
                if &account_id == authority {
                    ValidatorVerdict::Allow
                } else {
                    ValidatorVerdict::Deny(DenialReason::Custom(format!("Cannot access another account: {}.", account_id).into()))
                }
            }
        }
    }
}
