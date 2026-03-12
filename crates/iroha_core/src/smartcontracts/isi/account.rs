//! This module contains implementations of smart-contract traits and instructions for [`Account`] structure
//! and implementations for account queries.

use iroha_data_model::{prelude::*, query::error::FindError};
use iroha_telemetry::metrics;

use super::prelude::*;

/// All instructions related to accounts:
/// - minting/burning public key into account signatories
/// - minting/burning signature condition check
/// - update metadata
/// - grant permissions and roles
/// - Revoke permissions or roles
pub mod isi {
    use iroha_data_model::isi::{
        InstructionType,
        error::{MintabilityError, RepetitionError},
    };

    use super::*;
    use crate::{role::RoleIdWithOwner, state::StateTransaction};

    impl Execute for Transfer<Account, AssetDefinitionId, Account> {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let Transfer {
                source,
                object,
                destination,
            } = self;

            let _ = state_transaction.world.account(&source)?;
            let _ = state_transaction.world.account(&destination)?;

            let authority_is_source_owner = authority == &source;
            if !authority_is_source_owner {
                return Err(Error::InvariantViolation(
                    "Can't transfer asset definition of another account"
                        .to_owned()
                        .into(),
                ));
            }

            let asset_definition = state_transaction.world.asset_definition_mut(&object)?;

            if asset_definition.owned_by() != &source {
                return Err(Error::Find(FindError::Account(source)));
            }

            asset_definition.set_owned_by(destination.clone());
            state_transaction
                .world
                .emit_events(Some(AssetDefinitionEvent::OwnerChanged(
                    AssetDefinitionOwnerChanged {
                        asset_definition: object,
                        new_owner: destination,
                    },
                )));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Account> {
        #[metrics(+"set_account_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            // Destructure to move key/value once; avoid duplicate clones.
            let SetKeyValue {
                object: account_id,
                key,
                value,
            } = self;
            // Enforce metadata value size limit (custom parameter or default)
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            // Insert into account metadata; move key/value into the map directly.
            state_transaction
                .world
                .account_mut(&account_id)
                .map_err(Error::from)
                .map(|account| account.insert(key.clone(), value.clone()))?;

            // Emit event with a single extra clone from inserted value.
            state_transaction
                .world
                .emit_events(Some(AccountEvent::MetadataInserted(MetadataChanged {
                    target: account_id,
                    key,
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<Account> {
        #[metrics(+"remove_account_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.object().clone();

            let value = state_transaction
                .world
                .account_mut(&account_id)
                .and_then(|account| {
                    account
                        .remove(self.key())
                        .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
                })?;

            state_transaction
                .world
                .emit_events(Some(AccountEvent::MetadataRemoved(MetadataChanged {
                    target: account_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    // centralized in smartcontracts::limits

    impl Execute for Grant<Permission, Account> {
        #[metrics(+"grant_account_permission")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.destination().clone();
            let permission = self.object().clone();

            // Check if account exists
            state_transaction.world.account_mut(&account_id)?;

            if state_transaction
                .world
                .account_contains_inherent_permission(&account_id, &permission)
            {
                return Err(RepetitionError {
                    instruction: InstructionType::Grant,
                    id: permission.into(),
                }
                .into());
            }

            state_transaction
                .world
                .add_account_permission(&account_id, permission.clone());

            state_transaction
                .world
                .emit_events(Some(AccountEvent::PermissionAdded(
                    AccountPermissionChanged {
                        account: account_id.clone(),
                        permission,
                    },
                )));

            state_transaction.invalidate_permission_cache_for_account(&account_id);

            Ok(())
        }
    }

    impl Execute for Revoke<Permission, Account> {
        #[metrics(+"revoke_account_permission")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.destination().clone();
            let permission = self.object().clone();

            // Check if account exists
            state_transaction.world.account(&account_id)?;

            if !state_transaction
                .world
                .remove_account_permission(&account_id, &permission)
            {
                return Err(FindError::Permission(permission.into()).into());
            }

            state_transaction
                .world
                .emit_events(Some(AccountEvent::PermissionRemoved(
                    AccountPermissionChanged {
                        account: account_id.clone(),
                        permission,
                    },
                )));

            state_transaction.invalidate_permission_cache_for_account(&account_id);

            Ok(())
        }
    }

    impl Execute for Grant<RoleId, Account> {
        #[metrics(+"grant_account_role")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.destination().clone();
            let role_id = self.object().clone();

            state_transaction.world.role(&role_id)?;
            state_transaction.world.account(&account_id)?;

            if state_transaction
                .world
                .account_roles
                .insert(
                    RoleIdWithOwner::new(account_id.clone(), role_id.clone()),
                    (),
                )
                .is_some()
            {
                return Err(RepetitionError {
                    instruction: InstructionType::Grant,
                    id: IdBox::RoleId(role_id),
                }
                .into());
            }

            state_transaction
                .world
                .emit_events(Some(AccountEvent::RoleGranted(AccountRoleChanged {
                    account: account_id.clone(),
                    role: role_id,
                })));

            state_transaction.invalidate_permission_cache_for_account(&account_id);

            Ok(())
        }
    }

    impl Execute for Revoke<RoleId, Account> {
        #[metrics(+"revoke_account_role")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.destination().clone();
            let role_id = self.object().clone();

            if state_transaction
                .world
                .account_roles
                .remove(RoleIdWithOwner {
                    account: account_id.clone(),
                    id: role_id.clone(),
                })
                .is_none()
            {
                return Err(FindError::Role(role_id).into());
            }

            state_transaction
                .world
                .emit_events(Some(AccountEvent::RoleRevoked(AccountRoleChanged {
                    account: account_id.clone(),
                    role: role_id,
                })));

            state_transaction.invalidate_permission_cache_for_account(&account_id);

            Ok(())
        }
    }

    /// Stop minting on the [`AssetDefinition`] globally.
    ///
    /// # Errors
    /// If the [`AssetDefinition`] is not `Mintable::Once`.
    #[inline]
    pub fn forbid_minting(definition: &mut AssetDefinition) -> Result<(), MintabilityError> {
        if definition.mintable() == Mintable::Once {
            definition.set_mintable(Mintable::Not);
            Ok(())
        } else {
            Err(MintabilityError::ForbidMintOnMintable)
        }
    }

    #[cfg(test)]
    mod test {
        use iroha_data_model::{error::ParseError, prelude::AssetDefinition};
        use iroha_test_samples::gen_account_in;

        use crate::smartcontracts::isi::Registrable as _;

        #[test]
        fn cannot_forbid_minting_on_asset_mintable_infinitely() -> Result<(), ParseError> {
            let (authority, _authority_keypair) = gen_account_in("wonderland");
            let mut definition = {
                let __asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
                    "hello".parse()?,
                    "test".parse()?,
                );
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&authority);
            assert!(super::forbid_minting(&mut definition).is_err());
            Ok(())
        }
    }
}

/// Implementations for account queries.
pub mod query {

    use eyre::Result;
    use iroha_data_model::{
        account::Account,
        permission::Permission,
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail as Error,
        },
    };

    use super::*;
    use crate::{
        smartcontracts::{ValidQuery, ValidSingularQuery},
        state::StateReadOnly,
    };

    impl ValidQuery for FindRolesByAccountId {
        #[metrics(+"find_roles_by_account_id")]
        fn execute(
            self,
            filter: CompoundPredicate<RoleId>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = RoleId>, Error> {
            let account_id = self.account_id();
            state_ro.world().account(account_id)?;
            Ok(state_ro
                .world()
                .account_roles_iter(account_id)
                .filter(move |&role_id| filter.applies(role_id))
                .cloned())
        }
    }

    impl ValidQuery for FindPermissionsByAccountId {
        #[metrics(+"find_permissions_by_account_id")]
        fn execute(
            self,
            filter: CompoundPredicate<Permission>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Permission>, Error> {
            let account_id = self.account_id();
            Ok(state_ro
                .world()
                .account_permissions_iter(account_id)?
                .filter(move |&permission| filter.applies(permission))
                .cloned())
        }
    }

    impl ValidQuery for FindAccounts {
        #[metrics(+"find_accounts")]
        fn execute(
            self,
            filter: CompoundPredicate<Account>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Account>, Error> {
            Ok(state_ro.world().accounts_iter().filter_map(move |entry| {
                let details = entry.value().clone().into_inner();
                let account = Account {
                    id: entry.id().clone(),
                    metadata: details.metadata,
                    label: details.label,
                    uaid: details.uaid,
                    opaque_ids: details.opaque_ids,
                };
                filter.applies(&account).then_some(account)
            }))
        }
    }

    impl ValidQuery for FindAccountsWithAsset {
        #[metrics(+"find_accounts_with_asset")]
        fn execute(
            self,
            filter: CompoundPredicate<Account>,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<impl Iterator<Item = Account>, Error> {
            let asset_definition_id = self.asset_definition_id().clone();

            trace!(%asset_definition_id);

            Ok(state_ro.world().accounts_iter().filter_map(move |entry| {
                let has_balance = state_ro
                    .world()
                    .assets_in_account_iter(entry.id())
                    .filter(|asset| asset.id().definition() == &asset_definition_id)
                    // Skip zero-valued placeholders (including genesis seeds).
                    .any(|asset| !asset.value().is_zero());

                if !has_balance {
                    return None;
                }
                let details = entry.value().clone().into_inner();
                let account = Account {
                    id: entry.id().clone(),
                    metadata: details.metadata,
                    label: details.label,
                    uaid: details.uaid,
                    opaque_ids: details.opaque_ids,
                };
                filter.applies(&account).then_some(account)
            }))
        }
    }

    impl ValidSingularQuery for FindDomainsByAccountId {
        #[metrics(+"find_domains_by_account_id")]
        fn execute(&self, state_ro: &impl StateReadOnly) -> Result<Vec<DomainId>, Error> {
            Ok(state_ro
                .world()
                .domains_for_subject(&self.account_id().subject_id()))
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_primitives::json::Json;
        use iroha_test_samples::{ALICE_ID, gen_account_in};

        use super::*;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };

        fn new_dummy_block() -> crate::block::CommittedBlock {
            let (leader_public_key, leader_private_key) =
                iroha_crypto::KeyPair::random().into_parts();
            let peer_id = crate::PeerId::new(leader_public_key);
            let topology = crate::sumeragi::network_topology::Topology::new(vec![peer_id]);
            ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
                h.set_height(NonZeroU64::new(1).unwrap());
            })
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
        }

        #[test]
        fn find_accounts_with_asset_ignores_zero_holdings() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            // Setup domain and two accounts
            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (acc1, _kp1) = gen_account_in("wonderland");
            let (acc2, _kp2) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            // Register asset definition and mint zero to acc1, one to acc2
            let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "test_coin".parse().unwrap(),
            );
            Register::asset_definition({
                let __asset_definition_id = ad.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            let a1 = AssetId::new(ad.clone(), acc1.clone());
            let a2 = AssetId::new(ad.clone(), acc2.clone());
            // minting zero yields an asset entry with zero quantity
            Mint::asset_numeric(Numeric::zero(), a1)
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Mint::asset_numeric(1u32, a2)
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            // Query should only return acc2
            let view = state.view();
            let results: Vec<_> = FindAccountsWithAsset::new(ad)
                .execute(CompoundPredicate::PASS, &view)
                .unwrap()
                .map(|a| a.id)
                .collect();
            assert_eq!(results, vec![acc2]);
        }

        #[test]
        fn find_accounts_applies_predicate() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (acc1, _kp1) = gen_account_in("wonderland");
            let (acc2, _kp2) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let tier_key: Name = "tier".parse().unwrap();
            SetKeyValue::account(acc1.clone(), tier_key.clone(), Json::from("gold"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            SetKeyValue::account(acc2.clone(), tier_key, Json::from("silver"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate =
                CompoundPredicate::<Account>::build(|p| p.equals("metadata.tier", "gold"));
            let results: Vec<_> = FindAccounts
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc1]);
        }

        #[test]
        fn find_accounts_with_asset_applies_predicate() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (acc1, _kp1) = gen_account_in("wonderland");
            let (acc2, _kp2) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone().to_account_id(domain_id.clone())))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let ad: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "test_coin".parse().unwrap(),
            );
            Register::asset_definition({
                let __asset_definition_id = ad.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            Mint::asset_numeric(1u32, AssetId::new(ad.clone(), acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Mint::asset_numeric(1u32, AssetId::new(ad.clone(), acc2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let tier_key: Name = "tier".parse().unwrap();
            SetKeyValue::account(acc1.clone(), tier_key.clone(), Json::from("gold"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            SetKeyValue::account(acc2.clone(), tier_key, Json::from("silver"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate =
                CompoundPredicate::<Account>::build(|p| p.equals("metadata.tier", "gold"));
            let results: Vec<_> = FindAccountsWithAsset::new(ad)
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc1]);
        }

        #[test]
        fn find_domains_by_account_id_returns_linked_domains_for_subject() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let wonderland: DomainId = "wonderland".parse().unwrap();
            let acme: DomainId = "acme".parse().unwrap();
            Register::domain(Domain::new(wonderland.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::domain(Domain::new(acme.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (account_id, _) = gen_account_in("wonderland");
            Register::account(Account::new(
                account_id.clone().to_account_id(wonderland.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

            iroha_data_model::isi::domain_link::LinkAccountDomain {
                account: account_id.clone(),
                domain: acme.clone(),
            }
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let domains = FindDomainsByAccountId::new(account_id)
                .execute(&view)
                .unwrap();
            assert_eq!(domains, vec![acme, wonderland]);
        }

        #[test]
        fn transfer_asset_definition_rejects_unauthorized_authority() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (source, _) = gen_account_in("wonderland");
            let (destination, _) = gen_account_in("wonderland");
            let (intruder, _) = gen_account_in("wonderland");
            Register::account(Account::new(
                source.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            Register::account(Account::new(
                destination.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            Register::account(Account::new(
                intruder.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

            let asset_definition: AssetDefinitionId =
                iroha_data_model::asset::AssetDefinitionId::new(
                    "wonderland".parse().unwrap(),
                    "bond".parse().unwrap(),
                );
            Register::asset_definition({
                let __asset_definition_id = asset_definition.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            stx.world
                .asset_definition_mut(&asset_definition)
                .unwrap()
                .set_owned_by(source.clone());

            let err = Transfer::asset_definition(
                source.clone(),
                asset_definition.clone(),
                destination.clone(),
            )
            .execute(&intruder, &mut stx)
            .expect_err("unauthorized authority must not transfer asset definition ownership");
            assert!(
                err.to_string().contains("Can't transfer asset definition"),
                "unexpected error: {err}"
            );
            assert_eq!(
                stx.world
                    .asset_definition(&asset_definition)
                    .unwrap()
                    .owned_by(),
                &source,
                "owner must remain unchanged on failed transfer"
            );
        }

        #[test]
        fn transfer_asset_definition_allows_source_owner() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(
                ALICE_ID.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

            let (source, _) = gen_account_in("wonderland");
            let (destination, _) = gen_account_in("wonderland");
            Register::account(Account::new(
                source.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            Register::account(Account::new(
                destination.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();

            let asset_definition: AssetDefinitionId =
                iroha_data_model::asset::AssetDefinitionId::new(
                    "wonderland".parse().unwrap(),
                    "bond".parse().unwrap(),
                );
            Register::asset_definition({
                let __asset_definition_id = asset_definition.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            })
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            stx.world
                .asset_definition_mut(&asset_definition)
                .unwrap()
                .set_owned_by(source.clone());

            Transfer::asset_definition(
                source.clone(),
                asset_definition.clone(),
                destination.clone(),
            )
            .execute(&source, &mut stx)
            .expect("source owner must be allowed to transfer ownership");
            assert_eq!(
                stx.world
                    .asset_definition(&asset_definition)
                    .unwrap()
                    .owned_by(),
                &destination
            );
        }
    }
}
