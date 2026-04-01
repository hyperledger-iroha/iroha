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

    fn is_idempotent_alias_permission(permission: &Permission) -> bool {
        iroha_executor_data_model::permission::account::CanManageAccountAlias::try_from(permission)
            .is_ok()
            || iroha_executor_data_model::permission::account::CanResolveAccountAlias::try_from(
                permission,
            )
            .is_ok()
    }

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
            if crate::smartcontracts::isi::multisig::is_reserved_multisig_metadata_key(&key) {
                return Err(Error::InvariantViolation(
                    format!("account metadata key `{key}` is reserved for native multisig state")
                        .into(),
                ));
            }
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
            if crate::smartcontracts::isi::multisig::is_reserved_multisig_metadata_key(self.key()) {
                return Err(Error::InvariantViolation(
                    format!(
                        "account metadata key `{}` is reserved for native multisig state",
                        self.key()
                    )
                    .into(),
                ));
            }

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
                if is_idempotent_alias_permission(&permission) {
                    return Ok(());
                }
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
        use core::num::NonZeroU64;

        use iroha_data_model::{
            domain::DomainId,
            error::ParseError,
            isi::error::InstructionExecutionError,
            prelude::{Account, AssetDefinition, Domain, Grant, Permission, Register},
        };
        use iroha_primitives::json::Json;
        use iroha_test_samples::{ALICE_ID, gen_account_in};

        use crate::smartcontracts::isi::Registrable as _;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            smartcontracts::Execute,
            state::{State, World, WorldReadOnly},
        };

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
        fn duplicate_alias_permission_grant_is_idempotent() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let wonderland: DomainId = "wonderland".parse().unwrap();

            Register::domain(Domain::new(wonderland.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let permission = Permission::from(
                iroha_executor_data_model::permission::account::CanManageAccountAlias {
                    scope: iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
                        iroha_data_model::nexus::DataSpaceId::GLOBAL,
                    ),
                },
            );

            Grant::account_permission(permission.clone(), ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Grant::account_permission(permission.clone(), ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let permissions = stx
                .world
                .account_permissions_iter(&ALICE_ID)
                .unwrap()
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(permissions.len(), 1);
            assert_eq!(permissions[0], permission);
        }

        #[test]
        fn duplicate_non_alias_permission_grant_still_rejects() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let wonderland: DomainId = "wonderland".parse().unwrap();

            Register::domain(Domain::new(wonderland.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let permission = Permission::new("custom_permission".into(), Json::new(()));

            Grant::account_permission(permission.clone(), ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            let err = Grant::account_permission(permission.clone(), ALICE_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("duplicate non-alias grant must still fail");

            assert!(matches!(err, InstructionExecutionError::Repetition(_)));
        }
    }
}

/// Implementations for account queries.
pub mod query {
    use std::{collections::BTreeSet, sync::Arc};

    use eyre::Result;
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::Account,
        permission::Permission,
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail as Error,
            json::PredicateJson,
        },
    };
    use norito::json::Value;

    use super::*;
    use crate::{
        smartcontracts::{ValidQuery, ValidSingularQuery},
        state::{StateReadOnly, WorldReadOnly},
    };

    fn account_from_entry(
        _world: &impl WorldReadOnly,
        account_id: &AccountId,
        account_value: &AccountValue,
    ) -> Account {
        let details = account_value.as_ref();
        Account {
            id: account_id.clone(),
            metadata: details.metadata.clone(),
            label: details.label.clone(),
            uaid: details.uaid,
            opaque_ids: details.opaque_ids.clone(),
        }
    }

    #[cfg(test)]
    fn seed_domain_name_lease(
        world: &mut crate::state::World,
        owner: &AccountId,
        domain_id: &DomainId,
    ) {
        let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    #[cfg(test)]
    fn seed_manage_account_alias_dataspace_permission(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
        authority: &AccountId,
        dataspace: iroha_data_model::nexus::DataSpaceId,
    ) {
        state_transaction.world.add_account_permission(
            authority,
            Permission::from(
                iroha_executor_data_model::permission::account::CanManageAccountAlias {
                    scope:
                        iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
                            dataspace,
                        ),
                },
            ),
        );
    }

    #[cfg(test)]
    fn alias_domain(domain: &DomainId) -> AccountAliasDomain {
        AccountAliasDomain::new(domain.name().clone())
    }

    #[cfg(test)]
    fn seed_manage_account_alias_permissions(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
        authority: &AccountId,
        domain: &DomainId,
        dataspace: iroha_data_model::nexus::DataSpaceId,
    ) {
        seed_manage_account_alias_dataspace_permission(state_transaction, authority, dataspace);
        state_transaction.world.add_account_permission(
            authority,
            Permission::from(
                iroha_executor_data_model::permission::account::CanManageAccountAlias {
                    scope:
                        iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Domain(
                            alias_domain(domain),
                        ),
                },
            ),
        );
    }

    #[cfg(test)]
    fn seed_authority_account(world: &mut crate::state::World, authority: &AccountId) {
        let account = Account {
            id: authority.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = iroha_data_model::IntoKeyValue::into_key_value(account);
        world.accounts.insert(account_id, account_value);
    }

    #[cfg(test)]
    fn seed_account_alias_lease(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
        owner: &AccountId,
        label: &AccountAlias,
    ) {
        let selector = crate::sns::selector_for_account_alias(
            label,
            &state_transaction.nexus.dataspace_catalog,
        )
        .expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            1_000,
            2_000,
            3_000,
            Metadata::default(),
        );
        state_transaction.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    fn account_alias_value(
        account_id: &AccountId,
        account_value: &AccountValue,
        field: &str,
    ) -> Option<Option<String>> {
        let details = account_value.as_ref();
        match field {
            "id" | "account" | "account_id" => Some(Some(account_id.to_string())),
            "uaid" | "universal_account_id" => Some(details.uaid().map(ToString::to_string)),
            _ => None,
        }
    }

    fn predicate_value_equals_str(value: &Value, expected: &str) -> bool {
        matches!(value, Value::String(raw) if raw == expected)
    }

    fn predicate_values_contain_str(values: &[Value], expected: &str) -> bool {
        values
            .iter()
            .any(|value| matches!(value, Value::String(raw) if raw == expected))
    }

    fn account_field_is_id(field: &str) -> bool {
        matches!(field, "id" | "account" | "account_id")
    }

    fn parse_account_id_value(value: &Value) -> Option<AccountId> {
        match value {
            Value::String(raw) => AccountId::parse_encoded(raw)
                .ok()
                .map(|parsed| parsed.into_account_id())
                .or_else(|| raw.parse::<PublicKey>().ok().map(AccountId::new)),
            _ => None,
        }
    }

    fn intersect_account_id_candidates(
        candidates: &mut Option<BTreeSet<AccountId>>,
        next: BTreeSet<AccountId>,
    ) {
        if let Some(existing) = candidates {
            existing.retain(|candidate| next.contains(candidate));
            return;
        }
        *candidates = Some(next);
    }

    /// Extract account-id candidates constrained by JSON predicate clauses.
    ///
    /// Returns `None` when the predicate does not constrain id/account fields.
    /// Returns `Some(empty-set)` when id/account constraints are unsatisfiable.
    fn account_predicate_candidate_ids(
        predicate: &PredicateJson,
    ) -> Option<Arc<BTreeSet<AccountId>>> {
        let mut candidates: Option<BTreeSet<AccountId>> = None;

        for cond in &predicate.equals {
            if !account_field_is_id(&cond.field) {
                continue;
            }
            let next = parse_account_id_value(&cond.value)
                .into_iter()
                .collect::<BTreeSet<_>>();
            intersect_account_id_candidates(&mut candidates, next);
        }

        for cond in &predicate.r#in {
            if !account_field_is_id(&cond.field) {
                continue;
            }
            let next = cond
                .values
                .iter()
                .filter_map(parse_account_id_value)
                .collect::<BTreeSet<_>>();
            intersect_account_id_candidates(&mut candidates, next);
        }

        candidates.map(Arc::new)
    }

    enum AccountSimpleIdPath {
        One(AccountId),
        Set(Arc<BTreeSet<AccountId>>),
    }

    fn account_predicate_simple_id_path(predicate: &PredicateJson) -> Option<AccountSimpleIdPath> {
        if !predicate.exists.is_empty() {
            return None;
        }

        if predicate.r#in.is_empty() && predicate.equals.len() == 1 {
            let cond = &predicate.equals[0];
            if !account_field_is_id(&cond.field) {
                return None;
            }
            return parse_account_id_value(&cond.value).map(AccountSimpleIdPath::One);
        }

        if predicate.equals.is_empty() && predicate.r#in.len() == 1 {
            let cond = &predicate.r#in[0];
            if !account_field_is_id(&cond.field) {
                return None;
            }
            let ids = cond
                .values
                .iter()
                .map(parse_account_id_value)
                .collect::<Option<BTreeSet<_>>>()?;
            return Some(AccountSimpleIdPath::Set(Arc::new(ids)));
        }

        None
    }

    fn predicate_is_id_only(predicate: &PredicateJson) -> bool {
        let has_any_clause = !predicate.equals.is_empty()
            || !predicate.r#in.is_empty()
            || !predicate.exists.is_empty();
        has_any_clause
            && predicate
                .equals
                .iter()
                .all(|cond| account_field_is_id(&cond.field))
            && predicate
                .r#in
                .iter()
                .all(|cond| account_field_is_id(&cond.field))
            && predicate
                .exists
                .iter()
                .all(|field| account_field_is_id(field))
    }

    /// Evaluate JSON predicate fields that can be resolved directly from account id/details
    /// without building a full `Account` object.
    ///
    /// Returns:
    /// - `Some(true|false)` when all predicate fields were alias-resolved.
    /// - `None` when at least one field requires full JSON evaluation fallback.
    fn predicate_matches_account_aliases(
        predicate: &PredicateJson,
        account_id: &AccountId,
        account_value: &AccountValue,
    ) -> Option<bool> {
        for cond in &predicate.equals {
            let Some(alias) = account_alias_value(account_id, account_value, &cond.field) else {
                return None;
            };
            let Some(alias) = alias else {
                return Some(false);
            };
            if !predicate_value_equals_str(&cond.value, &alias) {
                return Some(false);
            }
        }

        for cond in &predicate.r#in {
            let Some(alias) = account_alias_value(account_id, account_value, &cond.field) else {
                return None;
            };
            let Some(alias) = alias else {
                return Some(false);
            };
            if !predicate_values_contain_str(&cond.values, &alias) {
                return Some(false);
            }
        }

        for field in &predicate.exists {
            let Some(alias) = account_alias_value(account_id, account_value, field) else {
                return None;
            };
            if alias.is_none() {
                return Some(false);
            }
        }

        Some(true)
    }

    fn account_matches_filter(
        world: &impl WorldReadOnly,
        filter: &CompoundPredicate<Account>,
        predicate_json: Option<&PredicateJson>,
        account_id: &AccountId,
        account_value: &AccountValue,
    ) -> Option<Account> {
        if let Some(predicate) = predicate_json
            && let Some(matches) =
                predicate_matches_account_aliases(predicate, account_id, account_value)
        {
            if !matches {
                return None;
            }
            return Some(account_from_entry(world, account_id, account_value));
        }

        let account = account_from_entry(world, account_id, account_value);
        filter.applies(&account).then_some(account)
    }

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
            let world = state_ro.world();
            let filter_payload = filter.json_payload();
            if filter_payload.is_none() {
                let iter: Box<dyn Iterator<Item = Account> + '_> = Box::new(
                    world
                        .accounts_iter()
                        .map(|entry| account_from_entry(world, entry.id(), entry.value())),
                );
                return Ok(iter);
            }
            let predicate_json =
                filter_payload.and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
            let simple_id_path = predicate_json
                .as_ref()
                .and_then(account_predicate_simple_id_path);

            if let Some(path) = simple_id_path {
                let iter: Box<dyn Iterator<Item = Account> + '_> = match path {
                    AccountSimpleIdPath::One(account_id) => {
                        Box::new(world.accounts().get_key_value(&account_id).into_iter().map(
                            |(account_id, account_value)| {
                                account_from_entry(world, account_id, account_value)
                            },
                        ))
                    }
                    AccountSimpleIdPath::Set(account_ids) => {
                        let account_ids = account_ids.iter().cloned().collect::<Vec<_>>();
                        Box::new(account_ids.into_iter().filter_map(move |account_id| {
                            world.accounts().get_key_value(&account_id).map(
                                |(account_id, account_value)| {
                                    account_from_entry(world, account_id, account_value)
                                },
                            )
                        }))
                    }
                };
                return Ok(iter);
            }

            let candidate_ids = predicate_json
                .as_ref()
                .and_then(account_predicate_candidate_ids);
            let id_only_predicate = predicate_json.as_ref().is_some_and(predicate_is_id_only);

            if let Some(candidates) = candidate_ids {
                let candidates = candidates.iter().cloned().collect::<Vec<_>>();
                let iter: Box<dyn Iterator<Item = Account> + '_> =
                    Box::new(candidates.into_iter().filter_map(move |account_id| {
                        let Some((account_id, account_value)) =
                            world.accounts().get_key_value(&account_id)
                        else {
                            return None;
                        };
                        if id_only_predicate {
                            return Some(account_from_entry(world, account_id, account_value));
                        }
                        account_matches_filter(
                            world,
                            &filter,
                            predicate_json.as_ref(),
                            account_id,
                            account_value,
                        )
                    }));
                return Ok(iter);
            }

            let iter: Box<dyn Iterator<Item = Account> + '_> =
                Box::new(world.accounts_iter().filter_map(move |entry| {
                    account_matches_filter(
                        world,
                        &filter,
                        predicate_json.as_ref(),
                        entry.id(),
                        entry.value(),
                    )
                }));
            Ok(iter)
        }
    }

    impl ValidQuery for FindAccountIds {
        #[metrics(+"find_account_ids")]
        fn execute(
            self,
            filter: CompoundPredicate<AccountId>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = AccountId>, Error> {
            let world = state_ro.world();
            Ok(world
                .accounts_iter()
                .map(|entry| entry.id().clone())
                .filter(move |account_id| filter.applies(account_id)))
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
            let world = state_ro.world();
            let filter_payload = filter.json_payload();

            trace!(%asset_definition_id);

            if filter_payload.is_none() {
                let subjects = world
                    .asset_definition_holders()
                    .get(&asset_definition_id)
                    .map(|holders| holders.iter().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                let iter: Box<dyn Iterator<Item = Account> + '_> =
                    Box::new(subjects.into_iter().filter_map(move |subject| {
                        let Some((account_id, account_value)) =
                            world.accounts().get_key_value(&subject)
                        else {
                            return None;
                        };

                        let has_balance = world
                            .assets_in_account_by_definition_iter(account_id, &asset_definition_id)
                            // Skip zero-valued placeholders (including genesis seeds).
                            .any(|asset| !asset.value().is_zero());

                        if !has_balance {
                            return None;
                        }

                        Some(account_from_entry(world, account_id, account_value))
                    }));
                return Ok(iter);
            }

            let predicate_json =
                filter_payload.and_then(|raw| norito::json::from_str::<PredicateJson>(raw).ok());
            let simple_id_path = predicate_json
                .as_ref()
                .and_then(account_predicate_simple_id_path);

            if let Some(path) = simple_id_path {
                let subjects = match (
                    world.asset_definition_holders().get(&asset_definition_id),
                    path,
                ) {
                    (Some(holders), AccountSimpleIdPath::One(account_id)) => holders
                        .contains(&account_id)
                        .then_some(account_id)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    (Some(holders), AccountSimpleIdPath::Set(account_ids)) => account_ids
                        .iter()
                        .filter(|account_id| holders.contains(*account_id))
                        .cloned()
                        .collect::<Vec<_>>(),
                    (None, _) => Vec::new(),
                };

                let iter: Box<dyn Iterator<Item = Account> + '_> =
                    Box::new(subjects.into_iter().filter_map(move |subject| {
                        let Some((account_id, account_value)) =
                            world.accounts().get_key_value(&subject)
                        else {
                            return None;
                        };

                        let has_balance = world
                            .assets_in_account_by_definition_iter(account_id, &asset_definition_id)
                            // Skip zero-valued placeholders (including genesis seeds).
                            .any(|asset| !asset.value().is_zero());

                        if !has_balance {
                            return None;
                        }

                        Some(account_from_entry(world, account_id, account_value))
                    }));
                return Ok(iter);
            }

            let candidate_ids = predicate_json
                .as_ref()
                .and_then(account_predicate_candidate_ids);
            let id_only_predicate = predicate_json.as_ref().is_some_and(predicate_is_id_only);

            let subjects = match (
                world.asset_definition_holders().get(&asset_definition_id),
                candidate_ids.as_ref(),
            ) {
                (Some(holders), Some(candidates)) => candidates
                    .iter()
                    .filter(|account_id| holders.contains(*account_id))
                    .cloned()
                    .collect::<Vec<_>>(),
                (Some(holders), None) => holders.iter().cloned().collect::<Vec<_>>(),
                (None, _) => Vec::new(),
            };

            let iter: Box<dyn Iterator<Item = Account> + '_> =
                Box::new(subjects.into_iter().filter_map(move |subject| {
                    let Some((account_id, account_value)) =
                        world.accounts().get_key_value(&subject)
                    else {
                        return None;
                    };

                    let has_balance = world
                        .assets_in_account_by_definition_iter(account_id, &asset_definition_id)
                        // Skip zero-valued placeholders (including genesis seeds).
                        .any(|asset| !asset.value().is_zero());

                    if !has_balance {
                        return None;
                    }

                    if id_only_predicate {
                        return Some(account_from_entry(world, account_id, account_value));
                    }

                    account_matches_filter(
                        world,
                        &filter,
                        predicate_json.as_ref(),
                        account_id,
                        account_value,
                    )
                }));
            Ok(iter)
        }
    }

    impl ValidSingularQuery for FindAccountById {
        #[metrics(+"find_account_by_id")]
        fn execute(&self, state_ro: &impl StateReadOnly) -> Result<Account, Error> {
            let world = state_ro.world();
            let (account_id, account_value) = world
                .accounts()
                .get_key_value(self.account_id())
                .ok_or_else(|| Error::Find(FindError::Account(self.account_id().clone())))?;
            Ok(account_from_entry(world, account_id, account_value))
        }
    }

    impl ValidSingularQuery for FindAliasesByAccountId {
        #[metrics(+"find_aliases_by_account_id")]
        fn execute(
            &self,
            state_ro: &impl StateReadOnly,
        ) -> Result<Vec<AccountAliasBindingRecord>, Error> {
            let now_ms = state_ro
                .latest_block()
                .map(|block| u64::try_from(block.header().creation_time().as_millis()).unwrap_or(0))
                .unwrap_or(0);
            let dataspace_filter = self
                .dataspace()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|dataspace| {
                    state_ro
                        .nexus()
                        .dataspace_catalog
                        .by_alias(dataspace)
                        .map(|entry| entry.id)
                        .ok_or_else(|| {
                            Error::Conversion(format!("unknown dataspace alias: {dataspace}"))
                        })
                })
                .transpose()?;
            let domain_filter = self
                .domain()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|domain| {
                    domain
                        .parse::<iroha_data_model::account::rekey::AccountAliasDomain>()
                        .map_err(|err| {
                            Error::Conversion(format!("invalid alias domain segment: {err}"))
                        })
                })
                .transpose()?;

            let account_id = self.account_id();
            let Some(account) = state_ro.world().accounts().get(account_id) else {
                return Err(Error::NotFound);
            };

            let labels = state_ro
                .world()
                .account_aliases_by_account()
                .get(account_id)
                .cloned()
                .unwrap_or_default();
            labels
                .into_iter()
                .filter(|label| {
                    !dataspace_filter.is_some_and(|dataspace| label.dataspace != dataspace)
                        && !domain_filter
                            .as_ref()
                            .is_some_and(|domain| label.domain.as_ref() != Some(domain))
                })
                .map(|label| {
                    let alias = label
                        .to_literal(&state_ro.nexus().dataspace_catalog)
                        .map_err(|err| {
                            Error::Conversion(format!("invalid account alias binding: {err}"))
                        })?;
                    let dataspace = state_ro
                        .nexus()
                        .dataspace_catalog
                        .by_id(label.dataspace)
                        .ok_or_else(|| {
                            Error::Conversion(
                                "account alias dataspace is missing from the catalog".to_owned(),
                            )
                        })?
                        .alias
                        .clone();
                    let record = crate::sns::get_name_record(
                        state_ro.world(),
                        &state_ro.nexus().dataspace_catalog,
                        crate::sns::SnsNamespace::AccountAlias,
                        &alias,
                        now_ms,
                    )
                    .map_err(|err| {
                        Error::Conversion(format!("invalid account alias lease record: {err}"))
                    })?;
                    Ok(AccountAliasBindingRecord {
                        account_id: account_id.clone(),
                        alias,
                        dataspace,
                        domain: label.domain.as_ref().map(ToString::to_string),
                        is_primary: account.as_ref().label() == Some(&label),
                        status: record.status,
                        lease_expiry_ms: Some(record.expires_at_ms),
                        grace_until_ms: Some(record.grace_expires_at_ms),
                        bound_at_ms: record.registered_at_ms,
                    })
                })
                .collect()
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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
        fn find_accounts_returns_registered_accounts_for_pass_predicate() {
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

            let (acc1, _) = gen_account_in("wonderland");
            let (acc2, _) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let mut results: Vec<_> = FindAccounts
                .execute(CompoundPredicate::PASS, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            results.sort();

            let mut expected = vec![acc1, acc2];
            expected.sort();
            assert_eq!(results, expected);
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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

        fn find_accounts_applies_id_literal_predicate() {
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate =
                CompoundPredicate::<Account>::build(|p| p.equals("id", acc2.to_string()));
            let results: Vec<_> = FindAccounts
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc2]);
        }

        #[test]
        fn find_accounts_applies_id_in_literal_predicate() {
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
            let (acc3, _kp3) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc3.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate = CompoundPredicate::<Account>::build(|p| {
                p.in_values("id", [acc2.to_string(), acc3.to_string()])
            });
            let mut results: Vec<_> = FindAccounts
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            results.sort();
            let mut expected = vec![acc2, acc3];
            expected.sort();
            assert_eq!(results, expected);
        }

        #[test]
        fn find_accounts_applies_mixed_id_and_metadata_predicate() {
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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
            let predicate = CompoundPredicate::<Account>::build(|p| {
                p.equals("id", acc1.to_string())
                    .equals("metadata.tier", "gold")
            });
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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
        fn find_accounts_with_asset_applies_id_literal_predicate() {
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate =
                CompoundPredicate::<Account>::build(|p| p.equals("id", acc2.to_string()));
            let results: Vec<_> = FindAccountsWithAsset::new(ad)
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc2]);
        }

        #[test]
        fn find_accounts_with_asset_applies_id_in_literal_predicate() {
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
            let (acc3, _kp3) = gen_account_in("wonderland");
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc3.clone()))
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

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate = CompoundPredicate::<Account>::build(|p| {
                p.in_values("id", [acc2.to_string(), acc3.to_string()])
            });
            let results: Vec<_> = FindAccountsWithAsset::new(ad)
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc2]);
        }

        #[test]
        fn find_accounts_with_asset_applies_mixed_id_and_metadata_predicate() {
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
            Register::account(Account::new(acc1.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(acc2.clone()))
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
            let predicate = CompoundPredicate::<Account>::build(|p| {
                p.equals("id", acc1.to_string())
                    .equals("metadata.tier", "gold")
            });
            let results: Vec<_> = FindAccountsWithAsset::new(ad)
                .execute(predicate, &view)
                .unwrap()
                .map(|account| account.id)
                .collect();
            assert_eq!(results, vec![acc1]);
        }

        #[test]
        fn find_aliases_by_account_id_returns_primary_alias_bindings() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let linked_domain: DomainId = "hbl".parse().unwrap();
            let mut world = World::default();
            seed_domain_name_lease(&mut world, &ALICE_ID, &linked_domain);
            seed_authority_account(&mut world, &ALICE_ID);
            let state = State::new(world, kura, query_handle);
            state.nexus.write().dataspace_catalog =
                iroha_data_model::nexus::DataSpaceCatalog::new(vec![
                    iroha_data_model::nexus::DataSpaceMetadata::default(),
                    iroha_data_model::nexus::DataSpaceMetadata {
                        id: iroha_data_model::nexus::DataSpaceId::new(9),
                        alias: "centralbank".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("catalog");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(linked_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            seed_manage_account_alias_permissions(
                &mut stx,
                &ALICE_ID,
                &linked_domain,
                iroha_data_model::nexus::DataSpaceId::new(9),
            );

            let (account_id, _) = gen_account_in("hbl");
            let primary_label = AccountAlias::new_in_dataspace(
                "merchant".parse().expect("label"),
                Some(alias_domain(&linked_domain)),
                iroha_data_model::nexus::DataSpaceId::new(9),
            );
            seed_account_alias_lease(&mut stx, &ALICE_ID, &primary_label);
            Register::account(Account::new(account_id.clone()).with_label(Some(primary_label)))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let aliases = FindAliasesByAccountId::new(
                account_id.clone(),
                Some("centralbank".to_owned()),
                Some("hbl".to_owned()),
            )
            .execute(&view)
            .unwrap();
            assert_eq!(aliases.len(), 1);
            assert_eq!(aliases[0].alias, "merchant@hbl.centralbank");
            assert_eq!(aliases[0].dataspace, "centralbank");
            assert_eq!(aliases[0].domain.as_deref(), Some("hbl"));
            assert!(aliases[0].is_primary);
        }

        #[test]
        fn find_aliases_by_account_id_returns_empty_when_filters_do_not_match() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let linked_domain: DomainId = "hbl".parse().unwrap();
            let mut world = World::default();
            seed_domain_name_lease(&mut world, &ALICE_ID, &linked_domain);
            seed_authority_account(&mut world, &ALICE_ID);
            let state = State::new(world, kura, query_handle);
            state.nexus.write().dataspace_catalog =
                iroha_data_model::nexus::DataSpaceCatalog::new(vec![
                    iroha_data_model::nexus::DataSpaceMetadata::default(),
                    iroha_data_model::nexus::DataSpaceMetadata {
                        id: iroha_data_model::nexus::DataSpaceId::new(9),
                        alias: "centralbank".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("catalog");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            Register::domain(Domain::new(linked_domain.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            seed_manage_account_alias_permissions(
                &mut stx,
                &ALICE_ID,
                &linked_domain,
                iroha_data_model::nexus::DataSpaceId::new(9),
            );

            let (account_id, _) = gen_account_in("hbl");
            let primary_label = AccountAlias::new_in_dataspace(
                "merchant".parse().expect("label"),
                Some(alias_domain(&linked_domain)),
                iroha_data_model::nexus::DataSpaceId::new(9),
            );
            seed_account_alias_lease(&mut stx, &ALICE_ID, &primary_label);
            Register::account(Account::new(account_id.clone()).with_label(Some(primary_label)))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let aliases = FindAliasesByAccountId::new(
                account_id,
                Some("centralbank".to_owned()),
                Some("ubl".to_owned()),
            )
            .execute(&view)
            .unwrap();
            assert!(aliases.is_empty());
        }

        #[test]
        fn find_aliases_by_account_id_returns_root_dataspace_alias_without_domain() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let mut world = World::default();
            seed_authority_account(&mut world, &ALICE_ID);
            let state = State::new(world, kura, query_handle);
            state.nexus.write().dataspace_catalog =
                iroha_data_model::nexus::DataSpaceCatalog::new(vec![
                    iroha_data_model::nexus::DataSpaceMetadata::default(),
                    iroha_data_model::nexus::DataSpaceMetadata {
                        id: iroha_data_model::nexus::DataSpaceId::new(9),
                        alias: "centralbank".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("catalog");

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            seed_manage_account_alias_dataspace_permission(
                &mut stx,
                &ALICE_ID,
                iroha_data_model::nexus::DataSpaceId::new(9),
            );

            let (account_id, _) = gen_account_in("wonderland");
            let root_label = AccountAlias::new_in_dataspace(
                "merchant".parse().expect("label"),
                None,
                iroha_data_model::nexus::DataSpaceId::new(9),
            );
            seed_account_alias_lease(&mut stx, &ALICE_ID, &root_label);
            Register::account(Account::new(account_id.clone()).with_label(Some(root_label)))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let aliases =
                FindAliasesByAccountId::new(account_id, Some("centralbank".to_owned()), None)
                    .execute(&view)
                    .unwrap();
            assert_eq!(aliases.len(), 1);
            assert_eq!(aliases[0].alias, "merchant@centralbank");
            assert_eq!(aliases[0].dataspace, "centralbank");
            assert_eq!(aliases[0].domain, None);
            assert!(aliases[0].is_primary);
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
            Register::account(Account::new(source.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(destination.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(intruder.clone()))
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
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let (source, _) = gen_account_in("wonderland");
            let (destination, _) = gen_account_in("wonderland");
            Register::account(Account::new(source.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::account(Account::new(destination.clone()))
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
