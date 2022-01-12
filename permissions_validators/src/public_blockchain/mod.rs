//! Permission checks asociated with use cases that can be summarized as public blockchains.
use super::*;

pub mod burn;
pub mod key_value;
pub mod mint;
pub mod transfer;
pub mod unregister;

/// Origin asset id param used in permission tokens.
pub static ASSET_ID_TOKEN_PARAM_NAME: Lazy<Name> = Lazy::new(|| Name::test("asset_id"));
/// Origin account id param used in permission tokens.
pub static ACCOUNT_ID_TOKEN_PARAM_NAME: Lazy<Name> = Lazy::new(|| Name::test("account_id"));
/// Origin asset definition param used in permission tokens.
pub static ASSET_DEFINITION_ID_TOKEN_PARAM_NAME: Lazy<Name> =
    Lazy::new(|| Name::test("asset_definition_id"));

/// A preconfigured set of permissions for simple use cases.
pub fn default_permissions<W: WorldTrait>() -> IsInstructionAllowedBoxed<W> {
    // Grant instruction checks are or unioned, so that if one permission validator approves this Grant it will succeed.
    let grant_instruction_validator = ValidatorBuilder::new()
        .with_validator(transfer::GrantMyAssetAccess)
        .with_validator(unregister::GrantRegisteredByMeAccess)
        .with_validator(mint::GrantRegisteredByMeAccess)
        .with_validator(burn::GrantMyAssetAccess)
        .with_validator(burn::GrantRegisteredByMeAccess)
        .with_validator(key_value::GrantMyAssetAccessRemove)
        .with_validator(key_value::GrantMyAssetAccessSet)
        .with_validator(key_value::GrantMyMetadataAccessSet)
        .with_validator(key_value::GrantMyMetadataAccessRemove)
        .with_validator(key_value::GrantMyAssetDefinitionSet)
        .with_validator(key_value::GrantMyAssetDefinitionRemove)
        .any_should_succeed("Grant instruction validator.");
    ValidatorBuilder::new()
        .with_recursive_validator(grant_instruction_validator)
        .with_recursive_validator(transfer::OnlyOwnedAssets.or(transfer::GrantedByAssetOwner))
        .with_recursive_validator(
            unregister::OnlyAssetsCreatedByThisAccount.or(unregister::GrantedByAssetCreator),
        )
        .with_recursive_validator(
            mint::OnlyAssetsCreatedByThisAccount.or(mint::GrantedByAssetCreator),
        )
        .with_recursive_validator(burn::OnlyOwnedAssets.or(burn::GrantedByAssetOwner))
        .with_recursive_validator(
            burn::OnlyAssetsCreatedByThisAccount.or(burn::GrantedByAssetCreator),
        )
        .with_recursive_validator(
            key_value::AccountSetOnlyForSignerAccount.or(key_value::SetGrantedByAccountOwner),
        )
        .with_recursive_validator(
            key_value::AccountRemoveOnlyForSignerAccount.or(key_value::RemoveGrantedByAccountOwner),
        )
        .with_recursive_validator(
            key_value::AssetSetOnlyForSignerAccount.or(key_value::SetGrantedByAssetOwner),
        )
        .with_recursive_validator(
            key_value::AssetRemoveOnlyForSignerAccount.or(key_value::RemoveGrantedByAssetOwner),
        )
        .with_recursive_validator(
            key_value::AssetDefinitionSetOnlyForSignerAccount
                .or(key_value::SetGrantedByAssetDefinitionOwner),
        )
        .with_recursive_validator(
            key_value::AssetDefinitionRemoveOnlyForSignerAccount
                .or(key_value::RemoveGrantedByAssetDefinitionOwner),
        )
        .all_should_succeed()
}

/// Checks that `authority` is account owner for account supplied in `permission_token`.
///
/// # Errors
/// - The `permission_token` is of improper format.
/// - Account owner is not `authority`
pub fn check_account_owner_for_token(
    permission_token: &PermissionToken,
    authority: &AccountId,
) -> Result<(), String> {
    let account_id = if let Value::Id(IdBox::AccountId(account_id)) = permission_token
        .params
        .get(&ACCOUNT_ID_TOKEN_PARAM_NAME.clone())
        .ok_or(format!(
            "Failed to find permission param {}.",
            ACCOUNT_ID_TOKEN_PARAM_NAME.clone()
        ))? {
        account_id
    } else {
        return Err(format!(
            "Permission param {} is not an AccountId.",
            ACCOUNT_ID_TOKEN_PARAM_NAME.clone()
        ));
    };
    if account_id != authority {
        return Err("Account specified in permission token is not owned by signer.".to_owned());
    }
    Ok(())
}

/// Checks that `authority` is asset owner for asset supplied in `permission_token`.
///
/// # Errors
/// - The `permission_token` is of improper format.
/// - Asset owner is not `authority`
pub fn check_asset_owner_for_token(
    permission_token: &PermissionToken,
    authority: &AccountId,
) -> Result<(), String> {
    let asset_id = if let Value::Id(IdBox::AssetId(asset_id)) = permission_token
        .params
        .get(&ASSET_ID_TOKEN_PARAM_NAME.clone())
        .ok_or(format!(
            "Failed to find permission param {}.",
            ASSET_ID_TOKEN_PARAM_NAME.clone()
        ))? {
        asset_id
    } else {
        return Err(format!(
            "Permission param {} is not an AssetId.",
            ASSET_ID_TOKEN_PARAM_NAME.clone()
        ));
    };
    if &asset_id.account_id != authority {
        return Err("Asset specified in permission token is not owned by signer.".to_owned());
    }
    Ok(())
}

/// Checks that asset creator is `authority` in the supplied `permission_token`.
///
/// # Errors
/// - The `permission_token` is of improper format.
/// - Asset creator is not `authority`
pub fn check_asset_creator_for_token<W: WorldTrait>(
    permission_token: &PermissionToken,
    authority: &AccountId,
    wsv: &WorldStateView<W>,
) -> Result<(), String> {
    let definition_id = if let Value::Id(IdBox::AssetDefinitionId(definition_id)) = permission_token
        .params
        .get(&ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone())
        .ok_or(format!(
            "Failed to find permission param {}.",
            ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone()
        ))? {
        definition_id
    } else {
        return Err(format!(
            "Permission param {} is not an AssetDefinitionId.",
            ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone()
        ));
    };
    let registered_by_signer_account = wsv
        .asset_definition_entry(definition_id)
        .map(|asset_definition_entry| &asset_definition_entry.registered_by == authority)
        .unwrap_or(false);
    if !registered_by_signer_account {
        return Err("Can not grant access for assets, registered by another account.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::restriction)]

    use std::collections::{BTreeMap, BTreeSet};

    use iroha_core::wsv::World;

    use super::*;

    fn new_xor_definition(xor_id: &AssetDefinitionId) -> AssetDefinition {
        AssetDefinition::new_quantity(xor_id.clone())
    }

    #[test]
    fn transfer_only_owned_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "bob", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let transfer = Instruction::Transfer(TransferBox {
            source_id: IdBox::AssetId(alice_xor_id).into(),
            object: Value::U32(10).into(),
            destination_id: IdBox::AssetId(bob_xor_id).into(),
        });
        assert!(transfer::OnlyOwnedAssets
            .check(&alice_id, &transfer, &wsv)
            .is_ok());
        assert!(transfer::OnlyOwnedAssets
            .check(&bob_id, &transfer, &wsv)
            .is_err());
    }

    #[test]
    fn transfer_granted_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "bob", "test");
        let mut domain = Domain::test("test");
        let mut bob_account = Account::new(bob_id.clone());
        let _ = bob_account.permission_tokens.insert(PermissionToken::new(
            transfer::CAN_TRANSFER_USER_ASSETS_TOKEN.clone(),
            [(
                ASSET_ID_TOKEN_PARAM_NAME.clone(),
                alice_xor_id.clone().into(),
            )],
        ));
        domain.accounts.insert(bob_id.clone(), bob_account);
        let domains = vec![(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, BTreeSet::new()));
        let transfer = Instruction::Transfer(TransferBox {
            source_id: IdBox::AssetId(alice_xor_id).into(),
            object: Value::U32(10).into(),
            destination_id: IdBox::AssetId(bob_xor_id).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = transfer::OnlyOwnedAssets
            .or(transfer::GrantedByAssetOwner)
            .into();
        assert!(validator.check(&alice_id, &transfer, &wsv).is_ok());
        assert!(validator.check(&bob_id, &transfer, &wsv).is_ok());
    }

    #[test]
    fn grant_transfer_of_my_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let permission_token_to_alice = PermissionToken::new(
            transfer::CAN_TRANSFER_USER_ASSETS_TOKEN.clone(),
            [(ASSET_ID_TOKEN_PARAM_NAME.to_owned(), alice_xor_id.into())],
        );
        let wsv = WorldStateView::<World>::new(World::new());
        let grant = Instruction::Grant(GrantBox {
            object: permission_token_to_alice.into(),
            destination_id: IdBox::AccountId(bob_id.clone()).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = transfer::GrantMyAssetAccess.into();
        assert!(validator.check(&alice_id, &grant, &wsv).is_ok());
        assert!(validator.check(&bob_id, &grant, &wsv).is_err());
    }

    #[test]
    fn unregister_only_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let wsv = WorldStateView::<World>::new(World::with(
            [(
                DomainId::test("test"),
                Domain {
                    accounts: BTreeMap::new(),
                    id: DomainId::test("test"),
                    asset_definitions: [(
                        xor_id.clone(),
                        AssetDefinitionEntry {
                            definition: xor_definition,
                            registered_by: alice_id.clone(),
                        },
                    )]
                    .into(),
                    metadata: Metadata::new(),
                },
            )],
            [],
        ));
        let unregister =
            Instruction::Unregister(UnregisterBox::new(IdBox::AssetDefinitionId(xor_id)));
        assert!(unregister::OnlyAssetsCreatedByThisAccount
            .check(&alice_id, &unregister, &wsv)
            .is_ok());
        assert!(unregister::OnlyAssetsCreatedByThisAccount
            .check(&bob_id, &unregister, &wsv)
            .is_err());
    }

    #[test]
    fn unregister_granted_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let mut domain = Domain::test("test");
        let mut bob_account = Account::new(bob_id.clone());
        let _ = bob_account.permission_tokens.insert(PermissionToken::new(
            unregister::CAN_UNREGISTER_ASSET_WITH_DEFINITION.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone(),
                xor_id.clone().into(),
            )],
        ));
        domain.accounts.insert(bob_id.clone(), bob_account);
        domain.asset_definitions.insert(
            xor_id.clone(),
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, []));
        let instruction = Instruction::Unregister(UnregisterBox::new(xor_id));
        let validator: IsInstructionAllowedBoxed<World> =
            unregister::OnlyAssetsCreatedByThisAccount
                .or(unregister::GrantedByAssetCreator)
                .into();
        assert!(validator.check(&alice_id, &instruction, &wsv).is_ok());
        assert!(validator.check(&bob_id, &instruction, &wsv).is_ok());
    }

    #[test]
    fn grant_unregister_of_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let permission_token_to_alice = PermissionToken::new(
            unregister::CAN_UNREGISTER_ASSET_WITH_DEFINITION.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.to_owned(),
                xor_id.clone().into(),
            )],
        );
        let mut domain = Domain::test("test");
        domain.asset_definitions.insert(
            xor_id,
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];

        let wsv = WorldStateView::<World>::new(World::with(domains, []));
        let grant = Instruction::Grant(GrantBox {
            object: permission_token_to_alice.into(),
            destination_id: IdBox::AccountId(bob_id.clone()).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> =
            unregister::GrantRegisteredByMeAccess.into();
        assert!(validator.check(&alice_id, &grant, &wsv).is_ok());
        assert!(validator.check(&bob_id, &grant, &wsv).is_err());
    }

    #[test]
    fn mint_only_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let wsv = WorldStateView::<World>::new(World::with(
            [(
                DomainId::test("test"),
                Domain {
                    accounts: BTreeMap::new(),
                    id: DomainId::test("test"),
                    asset_definitions: [(
                        xor_id,
                        AssetDefinitionEntry {
                            definition: xor_definition,
                            registered_by: alice_id.clone(),
                        },
                    )]
                    .into(),
                    metadata: Metadata::new(),
                },
            )],
            [],
        ));
        let mint = Instruction::Mint(MintBox {
            object: Value::U32(100).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        assert!(mint::OnlyAssetsCreatedByThisAccount
            .check(&alice_id, &mint, &wsv)
            .is_ok());
        assert!(mint::OnlyAssetsCreatedByThisAccount
            .check(&bob_id, &mint, &wsv)
            .is_err());
    }

    #[test]
    fn mint_granted_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let mut domain = Domain::test("test");
        let mut bob_account = Account::new(bob_id.clone());
        let _ = bob_account.permission_tokens.insert(PermissionToken::new(
            mint::CAN_MINT_USER_ASSET_DEFINITIONS_TOKEN.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone(),
                xor_id.clone().into(),
            )],
        ));
        domain.accounts.insert(bob_id.clone(), bob_account);
        domain.asset_definitions.insert(
            xor_id,
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, []));
        let instruction = Instruction::Mint(MintBox {
            object: Value::U32(100).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = mint::OnlyAssetsCreatedByThisAccount
            .or(mint::GrantedByAssetCreator)
            .into();
        assert!(validator.check(&alice_id, &instruction, &wsv).is_ok());
        assert!(validator.check(&bob_id, &instruction, &wsv).is_ok());
    }

    #[test]
    fn grant_mint_of_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let permission_token_to_alice = PermissionToken::new(
            mint::CAN_MINT_USER_ASSET_DEFINITIONS_TOKEN.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.to_owned(),
                xor_id.clone().into(),
            )],
        );
        let mut domain = Domain::test("test");
        domain.asset_definitions.insert(
            xor_id,
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, vec![]));
        let grant = Instruction::Grant(GrantBox {
            object: permission_token_to_alice.into(),
            destination_id: IdBox::AccountId(bob_id.clone()).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = mint::GrantRegisteredByMeAccess.into();
        assert!(validator.check(&alice_id, &grant, &wsv).is_ok());
        assert!(validator.check(&bob_id, &grant, &wsv).is_err());
    }

    #[test]
    fn burn_only_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let wsv = WorldStateView::<World>::new(World::with(
            [(
                DomainId::test("test"),
                Domain {
                    accounts: [].into(),
                    id: DomainId::test("test"),
                    asset_definitions: [(
                        xor_id,
                        AssetDefinitionEntry {
                            definition: xor_definition,
                            registered_by: alice_id.clone(),
                        },
                    )]
                    .into(),
                    metadata: Metadata::new(),
                },
            )],
            [],
        ));
        let burn = Instruction::Burn(BurnBox {
            object: Value::U32(100).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        assert!(burn::OnlyAssetsCreatedByThisAccount
            .check(&alice_id, &burn, &wsv)
            .is_ok());
        assert!(burn::OnlyAssetsCreatedByThisAccount
            .check(&bob_id, &burn, &wsv)
            .is_err());
    }

    #[test]
    fn burn_granted_asset_definition() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let mut domain = Domain::test("test");
        let mut bob_account = Account::new(bob_id.clone());
        let _ = bob_account.permission_tokens.insert(PermissionToken::new(
            burn::CAN_BURN_ASSET_WITH_DEFINITION.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.clone(),
                xor_id.clone().into(),
            )],
        ));
        domain.accounts.insert(bob_id.clone(), bob_account);
        domain.asset_definitions.insert(
            xor_id,
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, vec![]));
        let instruction = Instruction::Burn(BurnBox {
            object: Value::U32(100).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = burn::OnlyAssetsCreatedByThisAccount
            .or(burn::GrantedByAssetCreator)
            .into();
        assert!(validator.check(&alice_id, &instruction, &wsv).is_ok());
        assert!(validator.check(&bob_id, &instruction, &wsv).is_ok());
    }

    #[test]
    fn grant_burn_of_assets_created_by_this_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let permission_token_to_alice = PermissionToken::new(
            burn::CAN_BURN_ASSET_WITH_DEFINITION.clone(),
            [(
                ASSET_DEFINITION_ID_TOKEN_PARAM_NAME.to_owned(),
                xor_id.clone().into(),
            )],
        );
        let mut domain = Domain::test("test");
        domain.asset_definitions.insert(
            xor_id,
            AssetDefinitionEntry::new(xor_definition, alice_id.clone()),
        );
        let domains = [(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, vec![]));
        let grant = Instruction::Grant(GrantBox {
            object: permission_token_to_alice.into(),
            destination_id: IdBox::AccountId(bob_id.clone()).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = burn::GrantRegisteredByMeAccess.into();
        assert!(validator.check(&alice_id, &grant, &wsv).is_ok());
        assert!(validator.check(&bob_id, &grant, &wsv).is_err());
    }

    #[test]
    fn burn_only_owned_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let burn = Instruction::Burn(BurnBox {
            object: Value::U32(100).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        assert!(burn::OnlyOwnedAssets.check(&alice_id, &burn, &wsv).is_ok());
        assert!(burn::OnlyOwnedAssets.check(&bob_id, &burn, &wsv).is_err());
    }

    #[test]
    fn burn_granted_assets() -> Result<(), String> {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let mut domain = Domain::test("test");
        let mut bob_account = Account::new(bob_id.clone());
        let _ = bob_account.permission_tokens.insert(PermissionToken::new(
            burn::CAN_BURN_USER_ASSETS_TOKEN.clone(),
            [(
                ASSET_ID_TOKEN_PARAM_NAME.clone(),
                alice_xor_id.clone().into(),
            )],
        ));
        domain.accounts.insert(bob_id.clone(), bob_account);
        let domains = vec![(DomainId::test("test"), domain)];
        let wsv = WorldStateView::<World>::new(World::with(domains, vec![]));
        let transfer = Instruction::Burn(BurnBox {
            object: Value::U32(10).into(),
            destination_id: IdBox::AssetId(alice_xor_id).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> =
            burn::OnlyOwnedAssets.or(burn::GrantedByAssetOwner).into();
        validator.check(&alice_id, &transfer, &wsv)?;
        assert!(validator.check(&bob_id, &transfer, &wsv).is_ok());
        Ok(())
    }

    #[test]
    fn grant_burn_of_my_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let permission_token_to_alice = PermissionToken::new(
            burn::CAN_BURN_USER_ASSETS_TOKEN.clone(),
            [(ASSET_ID_TOKEN_PARAM_NAME.to_owned(), alice_xor_id.into())],
        );
        let wsv = WorldStateView::<World>::new(World::new());
        let grant = Instruction::Grant(GrantBox {
            object: permission_token_to_alice.into(),
            destination_id: IdBox::AccountId(bob_id.clone()).into(),
        });
        let validator: IsInstructionAllowedBoxed<World> = burn::GrantMyAssetAccess.into();
        assert!(validator.check(&alice_id, &grant, &wsv).is_ok());
        assert!(validator.check(&bob_id, &grant, &wsv).is_err());
    }

    #[test]
    fn set_to_only_owned_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let set = Instruction::SetKeyValue(SetKeyValueBox::new(
            IdBox::AssetId(alice_xor_id),
            Value::from("key".to_owned()),
            Value::from("value".to_owned()),
        ));
        assert!(key_value::AssetSetOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AssetSetOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }

    #[test]
    fn remove_to_only_owned_assets() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let alice_xor_id = <Asset as Identifiable>::Id::test("xor", "test", "alice", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let set = Instruction::RemoveKeyValue(RemoveKeyValueBox::new(
            IdBox::AssetId(alice_xor_id),
            Value::from("key".to_owned()),
        ));
        assert!(key_value::AssetRemoveOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AssetRemoveOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }

    #[test]
    fn set_to_only_owned_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let set = Instruction::SetKeyValue(SetKeyValueBox::new(
            IdBox::AccountId(alice_id.clone()),
            Value::from("key".to_owned()),
            Value::from("value".to_owned()),
        ));
        assert!(key_value::AccountSetOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AccountSetOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }

    #[test]
    fn remove_to_only_owned_account() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let wsv = WorldStateView::<World>::new(World::new());
        let set = Instruction::RemoveKeyValue(RemoveKeyValueBox::new(
            IdBox::AccountId(alice_id.clone()),
            Value::from("key".to_owned()),
        ));
        assert!(key_value::AccountRemoveOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AccountRemoveOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }

    #[test]
    fn set_to_only_owned_asset_definition() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let wsv = WorldStateView::<World>::new(World::with(
            [(
                DomainId::test("test"),
                Domain {
                    accounts: BTreeMap::new(),
                    id: DomainId::test("test"),
                    asset_definitions: [(
                        xor_id.clone(),
                        AssetDefinitionEntry {
                            definition: xor_definition,
                            registered_by: alice_id.clone(),
                        },
                    )]
                    .into(),
                    metadata: Metadata::new(),
                },
            )],
            [],
        ));
        let set = Instruction::SetKeyValue(SetKeyValueBox::new(
            IdBox::AssetDefinitionId(xor_id),
            Value::from("key".to_owned()),
            Value::from("value".to_owned()),
        ));
        assert!(key_value::AssetDefinitionSetOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AssetDefinitionSetOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }

    #[test]
    fn remove_to_only_owned_asset_definition() {
        let alice_id = <Account as Identifiable>::Id::test("alice", "test");
        let bob_id = <Account as Identifiable>::Id::test("bob", "test");
        let xor_id = <AssetDefinition as Identifiable>::Id::test("xor", "test");
        let xor_definition = new_xor_definition(&xor_id);
        let wsv = WorldStateView::<World>::new(World::with(
            [(
                DomainId::test("test"),
                Domain {
                    accounts: BTreeMap::new(),
                    id: DomainId::test("test"),
                    asset_definitions: [(
                        xor_id.clone(),
                        AssetDefinitionEntry {
                            definition: xor_definition,
                            registered_by: alice_id.clone(),
                        },
                    )]
                    .into(),
                    metadata: Metadata::new(),
                },
            )],
            [],
        ));
        let set = Instruction::RemoveKeyValue(RemoveKeyValueBox::new(
            IdBox::AssetDefinitionId(xor_id),
            Value::from("key".to_owned()),
        ));
        assert!(key_value::AssetDefinitionRemoveOnlyForSignerAccount
            .check(&alice_id, &set, &wsv)
            .is_ok());
        assert!(key_value::AssetDefinitionRemoveOnlyForSignerAccount
            .check(&bob_id, &set, &wsv)
            .is_err());
    }
}
