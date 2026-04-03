//! This module contains [`Nft`] instructions and queries implementations.

use iroha_telemetry::metrics;

use super::prelude::*;

/// ISI module contains all instructions related to NFTs:
/// - register/unregister NFT
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_data_model::{
        IntoKeyValue, isi::error::RepetitionError, permission::Permission, query::error::FindError,
    };
    use iroha_telemetry::metrics;

    use super::*;
    use crate::smartcontracts::isi::account_admission::ensure_receiving_account;

    fn is_permission_nft_associated(permission: &Permission, nft_id: &NftId) -> bool {
        if let Ok(permission) =
            iroha_executor_data_model::permission::nft::CanUnregisterNft::try_from(permission)
        {
            return &permission.nft == nft_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::nft::CanTransferNft::try_from(permission)
        {
            return &permission.nft == nft_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::nft::CanModifyNftMetadata::try_from(permission)
        {
            return &permission.nft == nft_id;
        }

        false
    }

    pub(crate) fn remove_nft_associated_permissions(
        state_transaction: &mut StateTransaction<'_, '_>,
        nft_id: &NftId,
    ) {
        let account_ids: Vec<AccountId> = state_transaction
            .world
            .account_permissions
            .iter()
            .map(|(holder, _)| holder.clone())
            .collect();

        for holder in account_ids {
            let should_remove = state_transaction
                .world
                .account_permissions
                .get(&holder)
                .is_some_and(|permissions| {
                    permissions
                        .iter()
                        .any(|permission| is_permission_nft_associated(permission, nft_id))
                });
            if !should_remove {
                continue;
            }

            let remove_entry = if let Some(permissions) =
                state_transaction.world.account_permissions.get_mut(&holder)
            {
                permissions.retain(|permission| !is_permission_nft_associated(permission, nft_id));
                permissions.is_empty()
            } else {
                false
            };

            if remove_entry {
                state_transaction
                    .world
                    .account_permissions
                    .remove(holder.clone());
            }

            state_transaction.invalidate_permission_cache_for_account(&holder);
        }

        let role_ids: Vec<RoleId> = state_transaction
            .world
            .roles
            .iter()
            .map(|(role_id, _)| role_id.clone())
            .collect();

        for role_id in role_ids {
            let should_remove = state_transaction
                .world
                .roles
                .get(&role_id)
                .is_some_and(|role| {
                    role.permissions()
                        .any(|permission| is_permission_nft_associated(permission, nft_id))
                });
            if !should_remove {
                continue;
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
                role.permissions
                    .retain(|permission| !is_permission_nft_associated(permission, nft_id));
                role.permission_epochs
                    .retain(|permission, _| role.permissions.contains(permission));
            }

            if !impacted_accounts.is_empty() {
                state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());
            }
        }
    }

    impl Execute for Register<Nft> {
        #[metrics(+"register_nft")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft = self.object().clone().build(authority);
            let (nft_id, nft_value) = nft.clone().into_key_value();

            if state_transaction.world.nft(&nft_id).is_ok() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::NftId(nft_id),
                }
                .into());
            }
            let _ = state_transaction.world.domain(nft_id.domain())?;

            state_transaction.world.nfts.insert(nft_id, nft_value);

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Nft(NftEvent::Created(nft))));

            Ok(())
        }
    }

    impl Execute for Unregister<Nft> {
        #[metrics(+"unregister_nft")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft_id = self.object().clone();

            remove_nft_associated_permissions(state_transaction, &nft_id);

            state_transaction
                .world
                .nfts
                .remove(nft_id.clone())
                .ok_or_else(|| FindError::Nft(nft_id.clone()))?;
            let _ = state_transaction.world.domain(nft_id.domain())?;

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Nft(NftEvent::Deleted(nft_id))));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Nft> {
        #[metrics(+"set_nft_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: nft_id,
                key,
                value,
            } = self;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .nft_mut(&nft_id)
                .map_err(Error::from)
                .map(|nft| nft.content.insert(key.clone(), value.clone()))?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataInserted(MetadataChanged {
                    target: nft_id,
                    key,
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<Nft> {
        #[metrics(+"remove_nft_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft_id = self.object().clone();

            let value = state_transaction.world.nft_mut(&nft_id).and_then(|nft| {
                nft.content
                    .remove(self.key().as_ref())
                    .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
            })?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataRemoved(MetadataChanged {
                    target: nft_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    // centralized in smartcontracts::limits

    impl Execute for Transfer<Account, NftId, Account> {
        #[metrics(+"transfer_nft")]
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

            state_transaction.world.account(&source)?;
            let _created =
                ensure_receiving_account(authority, &destination, None, state_transaction)?;
            let authority_is_source_owner = authority == &source;
            let authority_is_nft_domain_owner =
                state_transaction.world.domain(object.domain())?.owned_by() == authority;
            let required_permission: Permission =
                iroha_executor_data_model::permission::nft::CanTransferNft {
                    nft: object.clone(),
                }
                .into();
            let authority_has_transfer_permission = state_transaction
                .world
                .account_permissions_iter(authority)?
                .into_iter()
                .any(|permission| permission == &required_permission)
                || state_transaction
                    .world
                    .account_roles_iter(authority)
                    .any(|role_id| {
                        state_transaction
                            .world
                            .roles
                            .get(role_id)
                            .is_some_and(|role| {
                                role.permissions()
                                    .any(|permission| permission == &required_permission)
                            })
                    });
            if !(authority_is_source_owner
                || authority_is_nft_domain_owner
                || authority_has_transfer_permission)
            {
                return Err(Error::InvariantViolation(
                    "Can't transfer NFT of another account".to_owned().into(),
                ));
            }

            let nft = state_transaction.world.nft_mut(&object)?;

            if nft.owned_by != source {
                return Err(Error::InvariantViolation(
                    format!("Can't transfer NFT {object} since {source} doesn't own it",).into(),
                ));
            }

            nft.owned_by = destination.clone();
            state_transaction
                .world
                .emit_events(Some(NftEvent::OwnerChanged(NftOwnerChanged {
                    nft: object,
                    new_owner: destination,
                })));

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_crypto::KeyPair;
        use iroha_data_model::{
            permission::Permission,
            query::error::FindError,
            role::{Role, RoleId},
        };
        use iroha_test_samples::ALICE_ID;

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
        fn register_nft_rejects_missing_domain() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let nft_id: NftId = "nft1$wonderland".parse().unwrap();
            let err = Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("missing domain should be rejected");

            assert!(
                matches!(err, Error::Find(FindError::Domain(ref id)) if id == nft_id.domain()),
                "expected missing-domain error, got {err:?}"
            );
        }

        #[test]
        fn unregister_nft_rejects_missing_domain() {
            let mut world = World::default();
            let nft_id: NftId = "nft1$wonderland".parse().unwrap();
            let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&ALICE_ID);
            let (id, value) = nft.into_key_value();
            world.nfts.insert(id, value);

            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let err = Unregister::nft(nft_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("missing domain should be rejected");

            assert!(
                matches!(err, Error::Find(FindError::Domain(ref id)) if id == nft_id.domain()),
                "expected missing-domain error, got {err:?}"
            );
        }

        #[test]
        fn unregister_nft_removes_associated_permissions_from_accounts_and_roles() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId =
                DomainId::try_new("nft-cleanup", "universal").expect("domain id");
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register domain");

            let holder_id = AccountId::new(KeyPair::random().public_key().clone());
            Register::account(Account::new(holder_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register holder account");

            let nft_id: NftId = "cleanup$nft-cleanup".parse().expect("nft id");
            Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register nft");

            let permission: Permission =
                iroha_executor_data_model::permission::nft::CanModifyNftMetadata {
                    nft: nft_id.clone(),
                }
                .into();
            Grant::account_permission(permission.clone(), holder_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant permission to holder");

            let role_id: RoleId = "NFT_CLEANUP".parse().expect("role id");
            Register::role(Role::new(role_id.clone(), holder_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register role");
            Grant::role_permission(permission.clone(), role_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("grant permission to role");

            assert!(
                stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder should have permission before unregister"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                role.permissions().any(|perm| perm == &permission),
                "role should include permission before unregister"
            );

            Unregister::nft(nft_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("unregister nft");

            assert!(
                !stx.world
                    .account_permissions
                    .get(&holder_id)
                    .is_some_and(|perms| perms.contains(&permission)),
                "holder permission should be removed"
            );
            let role = stx.world.roles.get(&role_id).expect("role should exist");
            assert!(
                !role.permissions().any(|perm| perm == &permission),
                "role permission should be removed"
            );
            assert!(
                !role.permission_epochs().contains_key(&permission),
                "permission epoch should be pruned"
            );
        }

        #[test]
        fn transfer_nft_rejects_authority_without_ownership() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let users_domain: DomainId =
                DomainId::try_new("users", "universal").expect("domain id");
            let user1 = AccountId::new(iroha_crypto::KeyPair::random().into_parts().0);
            let user2 = AccountId::new(iroha_crypto::KeyPair::random().into_parts().0);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let alice_domain: DomainId =
                DomainId::try_new("wonderland", "universal").expect("domain id");
            Register::domain(Domain::new(alice_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register alice domain");
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register alice account");

            Register::domain(Domain::new(users_domain.clone()))
                .execute(&user1, &mut stx)
                .expect("register users domain");
            Register::account(Account::new(user1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register user1 account");
            Register::account(Account::new(user2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register user2 account");

            let nft_id: NftId = "ticket$users".parse().expect("nft id");
            Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
                .execute(&user1, &mut stx)
                .expect("register nft");

            let err = Transfer::nft(user1, nft_id.clone(), user2)
                .execute(&ALICE_ID, &mut stx)
                .expect_err("authority without ownership must not transfer nft");
            let err_string = err.to_string();
            assert!(
                err_string.contains("Can't transfer NFT of another account"),
                "unexpected error: {err_string}"
            );
        }

        #[test]
        fn transfer_nft_allows_nft_domain_owner() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let users_domain: DomainId =
                DomainId::try_new("users", "universal").expect("domain id");
            let user1 = AccountId::new(iroha_crypto::KeyPair::random().into_parts().0);
            let user2 = AccountId::new(iroha_crypto::KeyPair::random().into_parts().0);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let alice_domain: DomainId =
                DomainId::try_new("wonderland", "universal").expect("domain id");
            Register::domain(Domain::new(alice_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register alice domain");
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register alice account");

            Register::domain(Domain::new(users_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register users domain");
            Register::account(Account::new(user1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register user1 account");
            Register::account(Account::new(user2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register user2 account");

            let nft_id: NftId = "ticket$users".parse().expect("nft id");
            Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
                .execute(&user1, &mut stx)
                .expect("register nft");

            Transfer::nft(user1, nft_id.clone(), user2.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("nft-domain owner should be allowed to transfer");

            let nft = stx.world.nft(&nft_id).expect("nft remains after transfer");
            assert_eq!(
                nft.owned_by, user2,
                "destination should own transferred nft"
            );
        }
    }
}

/// NFT-related query implementations.
pub mod query {
    use eyre::Result;
    use iroha_data_model::query::{dsl::CompoundPredicate, error::QueryExecutionFail as Error};

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindNfts {
        #[metrics(+"find_nfts")]
        fn execute(
            self,
            filter: CompoundPredicate<Nft>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Nft>, Error> {
            use iroha_data_model::query::dsl::EvaluatePredicate;

            Ok(state_ro.world().nfts_iter().filter_map(move |entry| {
                let details = entry.value().clone().into_inner();
                let nft = Nft {
                    id: entry.id().clone(),
                    content: details.content,
                    owned_by: details.owned_by,
                };
                filter.applies(&nft).then_some(nft)
            }))
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_primitives::json::Json;
        use iroha_test_samples::ALICE_ID;

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
        fn find_nfts_applies_predicate() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let nft1_id: NftId = "nft1$wonderland".parse().unwrap();
            let nft2_id: NftId = "nft2$wonderland".parse().unwrap();
            Register::nft(Nft::new(nft1_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::nft(Nft::new(nft2_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let rarity_key: Name = "rarity".parse().unwrap();
            SetKeyValue::nft(nft1_id.clone(), rarity_key.clone(), Json::from("rare"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            SetKeyValue::nft(nft2_id.clone(), rarity_key, Json::from("common"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate = CompoundPredicate::<Nft>::build(|p| p.equals("content.rarity", "rare"));
            let results: Vec<_> = FindNfts
                .execute(predicate, &view)
                .unwrap()
                .map(|nft| nft.id)
                .collect();
            assert_eq!(results, vec![nft1_id]);
        }
    }
}
