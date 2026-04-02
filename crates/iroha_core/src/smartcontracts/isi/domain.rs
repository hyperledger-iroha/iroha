//! This module contains [`Domain`] structure and related implementations and trait implementations.

use eyre::Result;
use iroha_data_model::{account::rekey::AccountRekeyRecord, prelude::*, query::error::FindError};
use iroha_telemetry::metrics;

use super::super::isi::prelude::*;

/// ISI module contains all instructions related to domains:
/// - creating/changing assets
/// - registering/unregistering accounts
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use std::{
        collections::{BTreeSet, btree_map::Entry},
        str::FromStr,
    };

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId, IntoKeyValue,
        account::{
            AccountController,
            curve::{CurveId, CurveRegistryError},
        },
        asset::definition::{
            validate_asset_alias, validate_asset_description, validate_asset_name,
        },
        isi::error::{InstructionExecutionError, InvalidParameterError, RepetitionError},
        metadata::Metadata,
        name::Name,
        offline::OFFLINE_ASSET_ENABLED_METADATA_KEY,
    };
    use iroha_logger::prelude::*;

    use super::*;
    use crate::{
        alias::authority_can_manage_account_alias,
        state::{WorldReadOnly as _, account_label_is_pii},
    };

    /// Domain-separation tag for deterministic offline escrow derivation.
    const OFFLINE_ESCROW_SEED_LABEL: &str = "iroha.offline.escrow.v1";
    /// Alias grace window after lease expiry (369 hours).
    const ASSET_ALIAS_GRACE_MS: u64 = 369u64 * 60 * 60 * 1_000;

    fn alias_grace_until_ms(lease_expiry_ms: Option<u64>) -> Option<u64> {
        lease_expiry_ms.map(|expiry| expiry.saturating_add(ASSET_ALIAS_GRACE_MS))
    }

    fn upsert_account_rekey_record(
        state_transaction: &mut StateTransaction<'_, '_>,
        label: &AccountAlias,
        account: &AccountId,
    ) {
        let record = match state_transaction
            .world
            .account_rekey_records
            .get(label)
            .cloned()
        {
            Some(record) => record.repoint_to_account(account.clone()),
            None => AccountRekeyRecord::new(label.clone(), account.clone()),
        };
        state_transaction
            .world
            .account_rekey_records
            .insert(label.clone(), record);
    }

    fn purge_stale_account_label_state(
        state_transaction: &mut StateTransaction<'_, '_>,
        label: &AccountAlias,
    ) {
        if let Some(existing_owner) = state_transaction.world.account_aliases.get(label).cloned()
            && state_transaction.world.account(&existing_owner).is_err()
        {
            warn!(
                "purging stale account alias binding label={:?} missing_owner={}",
                label, existing_owner
            );
            state_transaction.world.remove_account_alias_binding(label);
        }

        if let Some(record) = state_transaction
            .world
            .account_rekey_records
            .get(label)
            .cloned()
            && state_transaction
                .world
                .account(&record.active_account_id)
                .is_err()
        {
            warn!(
                "purging stale account rekey record label={:?} missing_owner={}",
                label, record.active_account_id
            );
            state_transaction
                .world
                .account_rekey_records
                .remove(label.clone());
        }
    }

    fn ensure_active_account_alias_lease(
        state_transaction: &StateTransaction<'_, '_>,
        label: &AccountAlias,
    ) -> Result<(), InstructionExecutionError> {
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if crate::sns::active_account_alias_owner(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            label,
            now_ms,
        )
        .is_some()
        {
            Ok(())
        } else {
            Err(InstructionExecutionError::InvariantViolation(
                "account alias requires an active SNS lease"
                    .to_owned()
                    .into(),
            ))
        }
    }

    fn refresh_account_alias_lease_if_requested(
        state_transaction: &mut StateTransaction<'_, '_>,
        label: &AccountAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Result<(), InstructionExecutionError> {
        let Some(lease_expiry_ms) = lease_expiry_ms else {
            return Ok(());
        };
        let literal = label
            .to_literal(&state_transaction.nexus.dataspace_catalog)
            .map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.to_string().into(),
                ))
            })?;
        crate::sns::set_name_lease_expiry(
            state_transaction,
            crate::sns::SnsNamespace::AccountAlias,
            &literal,
            lease_expiry_ms,
        )
        .map(|_| ())
        .map_err(|err| match err {
            crate::sns::SnsError::BadRequest(message) => {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    message.into(),
                ))
            }
            other => InstructionExecutionError::InvariantViolation(other.to_string().into()),
        })
    }

    fn contract_alias_matches_account_label(
        alias: &ContractAlias,
        label: &AccountAlias,
        catalog: &DataSpaceCatalog,
    ) -> bool {
        let Some(dataspace_alias) = catalog
            .by_id(label.dataspace)
            .map(|entry| entry.alias.as_str())
        else {
            return false;
        };
        alias.name_segment() == label.label.as_ref()
            && alias.domain_segment() == label.domain.as_ref().map(|domain| domain.name().as_ref())
            && alias.dataspace_segment() == dataspace_alias
    }

    fn ensure_contract_alias_namespace_available(
        state_transaction: &StateTransaction<'_, '_>,
        label: &AccountAlias,
    ) -> Result<(), InstructionExecutionError> {
        let now_ms = state_transaction.block_unix_timestamp_ms();
        let catalog = &state_transaction.nexus.dataspace_catalog;
        let has_conflict = state_transaction
            .world
            .contract_alias_bindings()
            .iter()
            .any(|(_, binding)| {
                !binding.is_grace_expired_at(now_ms)
                    && contract_alias_matches_account_label(&binding.alias, label, catalog)
            });
        if has_conflict {
            Err(InstructionExecutionError::InvariantViolation(
                "account alias collides with an active contract alias"
                    .to_owned()
                    .into(),
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_account_alias_namespace_available_for_contract_alias(
        state_transaction: &StateTransaction<'_, '_>,
        alias: &ContractAlias,
    ) -> Result<(), InstructionExecutionError> {
        let catalog = &state_transaction.nexus.dataspace_catalog;
        let (label_name, domain, dataspace) = alias.resolve_components(catalog).map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                err.to_string().into(),
            ))
        })?;
        let label = AccountAlias::new_in_dataspace(label_name, domain, dataspace);
        let selector = crate::sns::selector_for_account_alias(&label, catalog).map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                err.to_string().into(),
            ))
        })?;
        let Some(record) = crate::sns::record_by_selector(state_transaction.world(), &selector)
        else {
            return Ok(());
        };
        let status =
            crate::sns::effective_status(&record, state_transaction.block_unix_timestamp_ms());
        if matches!(
            status,
            iroha_data_model::sns::NameStatus::Active
                | iroha_data_model::sns::NameStatus::GracePeriod
        ) {
            Err(InstructionExecutionError::InvariantViolation(
                "contract alias collides with an active account alias"
                    .to_owned()
                    .into(),
            ))
        } else {
            Ok(())
        }
    }

    fn account_created_event_domain(
        state_transaction: &StateTransaction<'_, '_>,
        account_id: &AccountId,
    ) -> DomainId {
        state_transaction
            .world
            .alias_domains_for_account(account_id)
            .into_iter()
            .next()
            .unwrap_or_else(|| {
                crate::sns::RESERVED_UNIVERSAL_DATASPACE_ALIAS
                    .parse()
                    .expect("reserved universal dataspace alias must parse as DomainId")
            })
    }

    fn ensure_asset_definition_human_fields(
        asset_definition: &AssetDefinition,
    ) -> Result<(), InstructionExecutionError> {
        validate_asset_name(asset_definition.name()).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid asset definition name: {err}").into(),
            )
        })?;
        validate_asset_description(asset_definition.description().as_deref()).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid asset definition description: {err}").into(),
            )
        })?;
        validate_asset_alias(asset_definition.alias().as_ref(), asset_definition.name()).map_err(
            |err| {
                InstructionExecutionError::InvariantViolation(
                    format!("invalid asset definition alias: {err}").into(),
                )
            },
        )?;
        Ok(())
    }

    /// Read the `offline.enabled` metadata flag from an asset definition.
    pub(crate) fn asset_definition_offline_enabled(
        metadata: &Metadata,
    ) -> Result<bool, InstructionExecutionError> {
        let key = Name::from_str(OFFLINE_ASSET_ENABLED_METADATA_KEY).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid metadata key `{OFFLINE_ASSET_ENABLED_METADATA_KEY}`: {err}")
                    .into(),
            )
        })?;
        let Some(value) = metadata.get(&key) else {
            return Ok(false);
        };

        if let Ok(flag) = value.try_into_any::<bool>() {
            return Ok(flag);
        }
        if let Ok(text) = value.try_into_any::<String>() {
            let trimmed = text.trim();
            if trimmed.eq_ignore_ascii_case("true") {
                return Ok(true);
            }
            if trimmed.eq_ignore_ascii_case("false") {
                return Ok(false);
            }
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "metadata entry `{OFFLINE_ASSET_ENABLED_METADATA_KEY}` must be `true` or `false`"
                )
                .into(),
            ));
        }

        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "metadata entry `{OFFLINE_ASSET_ENABLED_METADATA_KEY}` must be a boolean or string"
            )
            .into(),
        ))
    }

    /// Derive the deterministic offline escrow account for an asset definition.
    pub(crate) fn offline_escrow_account_id(
        chain_id: &ChainId,
        definition_id: &AssetDefinitionId,
    ) -> AccountId {
        let seed_material = format!(
            "{OFFLINE_ESCROW_SEED_LABEL}|{}|{definition_id}",
            chain_id.as_str()
        );
        let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
        let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    pub(crate) fn ensure_offline_escrow_account(
        asset_definition: &AssetDefinition,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !asset_definition_offline_enabled(asset_definition.metadata())? {
            return Ok(());
        }

        let definition_id = asset_definition.id();
        let derived = offline_escrow_account_id(state_transaction.chain_id(), definition_id);
        let escrow_account = match state_transaction
            .settlement
            .offline
            .escrow_accounts
            .entry(definition_id.clone())
        {
            Entry::Vacant(entry) => entry.insert(derived.clone()).clone(),
            Entry::Occupied(mut entry) => {
                if entry.get() != &derived {
                    warn!(
                        definition = %definition_id,
                        configured = %entry.get(),
                        derived = %derived,
                        "offline escrow account overridden by deterministic derivation"
                    );
                    entry.insert(derived.clone());
                }
                entry.get().clone()
            }
        };

        ensure_controller_capabilities(
            escrow_account.controller(),
            &state_transaction.crypto.allowed_signing,
            &state_transaction.crypto.allowed_curve_ids,
        )?;

        if state_transaction.world.account(&escrow_account).is_ok() {
            return Ok(());
        }

        let account = Account {
            id: escrow_account.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = account.clone().into_key_value();
        state_transaction
            .world
            .accounts
            .insert(account_id.clone(), account_value);

        Ok(())
    }

    fn account_subject_matches(left: &AccountId, right: &AccountId) -> bool {
        left.subject_id() == right.subject_id()
    }

    fn is_permission_account_associated(permission: &Permission, account_id: &AccountId) -> bool {
        if let Ok(permission) =
            iroha_executor_data_model::permission::nexus::CanUseFeeSponsor::try_from(permission)
        {
            return account_subject_matches(&permission.sponsor, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanMintAsset::try_from(permission)
        {
            return account_subject_matches(permission.asset.account(), account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanBurnAsset::try_from(permission)
        {
            return account_subject_matches(permission.asset.account(), account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanTransferAsset::try_from(permission)
        {
            return account_subject_matches(permission.asset.account(), account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanModifyAssetMetadata::try_from(
                permission,
            )
        {
            return account_subject_matches(permission.asset.account(), account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::account::CanUnregisterAccount::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.account, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.account, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::trigger::CanRegisterTrigger::try_from(permission)
        {
            return account_subject_matches(&permission.authority, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::governance::CanRecordCitizenService::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.owner, account_id);
        }

        false
    }

    fn remove_account_associated_permissions(
        state_transaction: &mut StateTransaction<'_, '_>,
        account_id: &AccountId,
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
                        .any(|permission| is_permission_account_associated(permission, account_id))
                });
            if !should_remove {
                continue;
            }

            let remove_entry = if let Some(permissions) =
                state_transaction.world.account_permissions.get_mut(&holder)
            {
                permissions
                    .retain(|permission| !is_permission_account_associated(permission, account_id));
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
                        .any(|permission| is_permission_account_associated(permission, account_id))
                });
            if !should_remove {
                continue;
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
                role.permissions
                    .retain(|permission| !is_permission_account_associated(permission, account_id));
                role.permission_epochs
                    .retain(|permission, _| role.permissions.contains(permission));
            }

            if !impacted_accounts.is_empty() {
                state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());
            }
        }
    }

    fn is_permission_asset_definition_associated(
        permission: &Permission,
        asset_definition_id: &AssetDefinitionId,
    ) -> bool {
        if let Ok(permission) = iroha_executor_data_model::permission::asset_definition::CanUnregisterAssetDefinition::try_from(permission) {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) = iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata::try_from(permission) {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition::try_from(
                permission,
            )
        {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition::try_from(
                permission,
            )
        {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition::try_from(
                permission,
            )
        {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) = iroha_executor_data_model::permission::asset::CanModifyAssetMetadataWithDefinition::try_from(permission) {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanMintAsset::try_from(permission)
        {
            return permission.asset.definition() == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanBurnAsset::try_from(permission)
        {
            return permission.asset.definition() == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanTransferAsset::try_from(permission)
        {
            return permission.asset.definition() == asset_definition_id;
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanModifyAssetMetadata::try_from(
                permission,
            )
        {
            return permission.asset.definition() == asset_definition_id;
        }

        false
    }

    fn remove_asset_definition_associated_permissions(
        state_transaction: &mut StateTransaction<'_, '_>,
        asset_definition_id: &AssetDefinitionId,
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
                    permissions.iter().any(|permission| {
                        is_permission_asset_definition_associated(permission, asset_definition_id)
                    })
                });
            if !should_remove {
                continue;
            }

            let remove_entry = if let Some(permissions) =
                state_transaction.world.account_permissions.get_mut(&holder)
            {
                permissions.retain(|permission| {
                    !is_permission_asset_definition_associated(permission, asset_definition_id)
                });
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
                    role.permissions().any(|permission| {
                        is_permission_asset_definition_associated(permission, asset_definition_id)
                    })
                });
            if !should_remove {
                continue;
            }

            let impacted_accounts = state_transaction.accounts_with_role(&role_id);

            if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
                role.permissions.retain(|permission| {
                    !is_permission_asset_definition_associated(permission, asset_definition_id)
                });
                role.permission_epochs
                    .retain(|permission, _| role.permissions.contains(permission));
            }

            if !impacted_accounts.is_empty() {
                state_transaction.invalidate_permission_cache_for(impacted_accounts.iter());
            }
        }
    }

    fn resolve_config_account_literal(
        world: &impl crate::state::WorldReadOnly,
        dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
        raw: &str,
        field_path: &'static str,
    ) -> Result<AccountId, Error> {
        crate::block::parse_account_literal_with_world(world, dataspace_catalog, raw)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "invalid {field_path} account literal `{raw}`: expected canonical I105 account id or on-chain alias"
                    )
                    .into(),
                )
                .into()
            })
    }

    fn config_account_matches(
        world: &impl crate::state::WorldReadOnly,
        dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
        raw: &str,
        account_id: &AccountId,
        field_path: &'static str,
    ) -> Result<bool, Error> {
        let configured = resolve_config_account_literal(world, dataspace_catalog, raw, field_path)?;
        Ok(configured == *account_id)
    }

    fn parse_config_asset_definition_id(
        world: &impl crate::state::WorldReadOnly,
        raw: &str,
        now_ms: u64,
    ) -> Option<AssetDefinitionId> {
        crate::block::parse_asset_definition_literal_with_world(world, raw, now_ms)
    }

    impl Execute for Register<Account> {
        #[metrics(+"register_account")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account: Account = self.object().clone().build(authority);
            ensure_controller_capabilities(
                account.controller(),
                &state_transaction.crypto.allowed_signing,
                &state_transaction.crypto.allowed_curve_ids,
            )?;
            let (account_id, account_value) = account.clone().into_key_value();
            if state_transaction.world.account(&account_id).is_ok() {
                let plain_domainless_self_registration = self.object().metadata().is_empty()
                    && self.object().label().is_none()
                    && self.object().uaid.is_none()
                    && self.object().opaque_ids.is_empty();
                if plain_domainless_self_registration {
                    return Ok(());
                }
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::AccountId(account_id),
                }
                .into());
            }
            if let Some(uaid) = account.uaid() {
                if let Some(existing) = state_transaction.world.uaid_accounts.get(uaid) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!("UAID {uaid} already bound to account {existing}").into(),
                    ));
                }
            } else if !account.opaque_ids().is_empty() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Opaque identifiers require a UAID".to_owned().into(),
                ));
            }

            if let Some(label) = account.label() {
                if account_label_is_pii(label) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label looks like raw PII; use UAID/opaque identifiers instead"
                            .to_owned()
                            .into(),
                    ));
                }
                if !authority_can_manage_account_alias(&state_transaction.world, authority, label) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "authority is not permitted to register this account label"
                            .to_owned()
                            .into(),
                    ));
                }
                ensure_active_account_alias_lease(state_transaction, label)?;
                purge_stale_account_label_state(state_transaction, label);
                if state_transaction.world.account_aliases.get(label).is_some()
                    || state_transaction
                        .world
                        .account_rekey_records
                        .get(label)
                        .is_some()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label already registered".to_owned().into(),
                    ));
                }
            }

            if account.uaid().is_some() {
                let mut seen = BTreeSet::new();
                for opaque in account.opaque_ids() {
                    if !seen.insert(*opaque) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            format!(
                                "Account {account_id} contains duplicate opaque identifier {opaque}"
                            )
                            .into(),
                        ));
                    }
                    if let Some(existing) = state_transaction.world.opaque_uaids.get(opaque) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            format!("Opaque identifier {opaque} already bound to UAID {existing}")
                                .into(),
                        ));
                    }
                }
            }
            state_transaction
                .world
                .accounts
                .insert(account_id.clone(), account_value);

            if let Some(uaid) = account.uaid() {
                state_transaction
                    .world
                    .uaid_accounts
                    .insert(*uaid, account_id.clone());
                state_transaction.rebuild_space_directory_bindings(*uaid);
            }

            if let Some(label) = account.label() {
                if state_transaction
                    .world
                    .insert_account_alias_binding(label.clone(), account_id.clone())
                    .is_some()
                {
                    state_transaction.world.accounts.remove(account_id.clone());
                    if let Some(uaid) = account.uaid() {
                        state_transaction.world.uaid_accounts.remove(*uaid);
                        state_transaction.rebuild_space_directory_bindings(*uaid);
                    }
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label already registered".to_owned().into(),
                    ));
                }
            }

            if let Some(uaid) = account.uaid() {
                for opaque in account.opaque_ids() {
                    if state_transaction
                        .world
                        .opaque_uaids
                        .insert(*opaque, *uaid)
                        .is_some()
                    {
                        state_transaction.world.accounts.remove(account_id.clone());
                        state_transaction.world.uaid_accounts.remove(*uaid);
                        if let Some(label) = account.label() {
                            state_transaction.world.remove_account_alias_binding(label);
                        }
                        for inserted in account.opaque_ids() {
                            state_transaction.world.opaque_uaids.remove(*inserted);
                        }
                        state_transaction.rebuild_space_directory_bindings(*uaid);
                        return Err(InstructionExecutionError::InvariantViolation(
                            "Opaque identifier already registered".to_owned().into(),
                        ));
                    }
                }
            }

            if let Some(record) = AccountRekeyRecord::from_account(&account)
                && state_transaction
                    .world
                    .account_rekey_records
                    .insert(record.label.clone(), record)
                    .is_some()
            {
                state_transaction.world.accounts.remove(account_id.clone());
                if let Some(uaid) = account.uaid() {
                    state_transaction.world.uaid_accounts.remove(*uaid);
                    state_transaction.rebuild_space_directory_bindings(*uaid);
                }
                if let Some(label) = account.label() {
                    state_transaction.world.remove_account_alias_binding(label);
                }
                for opaque in account.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                }
                return Err(InstructionExecutionError::InvariantViolation(
                    "Account label already registered".to_owned().into(),
                ));
            }

            // Account lifecycle events still require a domain wrapper in the current event model.
            // Prefer a concrete alias domain when one exists and otherwise use the stable
            // universal dataspace alias as synthetic context for domainless accounts.
            let created_domain = account_created_event_domain(state_transaction, &account_id);
            state_transaction
                .world
                .emit_events(Some(DomainEvent::Account(AccountEvent::Created(
                    AccountCreated::new(account, created_domain),
                ))));

            Ok(())
        }
    }

    impl Execute for Unregister<Account> {
        #[metrics(+"unregister_account")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.object().clone();

            if let Some((owned_domain_id, _)) = state_transaction
                .world
                .domains
                .iter()
                .find(|(_, domain)| domain.owned_by() == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it owns domain {owned_domain_id}; transfer ownership first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((owned_definition_id, _)) = state_transaction
                .world
                .asset_definitions
                .iter()
                .find(|(_, definition)| definition.owned_by() == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it owns asset definition {owned_definition_id}; transfer ownership first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.gov.bond_escrow_account {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as governance bond escrow account (`gov.bond_escrow_account`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.gov.citizenship_escrow_account {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as governance citizenship escrow account (`gov.citizenship_escrow_account`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.gov.slash_receiver_account {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as governance slash receiver account (`gov.slash_receiver_account`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id
                == state_transaction
                    .gov
                    .viral_incentives
                    .incentive_pool_account
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as governance viral incentive pool account (`gov.viral_incentives.incentive_pool_account`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.gov.viral_incentives.escrow_account {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as governance viral escrow account (`gov.viral_incentives.escrow_account`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.oracle.economics.reward_pool {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as oracle reward pool account (`oracle.economics.reward_pool`); update oracle config first"
                    )
                    .into(),
                )
                .into());
            }
            if account_id == state_transaction.oracle.economics.slash_receiver {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as oracle slash receiver account (`oracle.economics.slash_receiver`); update oracle config first"
                    )
                    .into(),
                )
                .into());
            }
            let nexus_fee_sink_matches = config_account_matches(
                &state_transaction.world,
                &state_transaction.nexus.dataspace_catalog,
                &state_transaction.nexus.fees.fee_sink_account_id,
                &account_id,
                "nexus.fees.fee_sink_account_id",
            )?;
            let nexus_stake_escrow_matches = config_account_matches(
                &state_transaction.world,
                &state_transaction.nexus.dataspace_catalog,
                &state_transaction.nexus.staking.stake_escrow_account_id,
                &account_id,
                "nexus.staking.stake_escrow_account_id",
            )?;
            let nexus_slash_sink_matches = config_account_matches(
                &state_transaction.world,
                &state_transaction.nexus.dataspace_catalog,
                &state_transaction.nexus.staking.slash_sink_account_id,
                &account_id,
                "nexus.staking.slash_sink_account_id",
            )?;

            if nexus_fee_sink_matches {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as nexus fee sink account (`nexus.fees.fee_sink_account_id`); update nexus config first"
                    )
                    .into(),
                )
                .into());
            }
            if nexus_stake_escrow_matches {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as nexus staking escrow account (`nexus.staking.stake_escrow_account_id`); update nexus config first"
                    )
                    .into(),
                )
                .into());
            }
            if nexus_slash_sink_matches {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as nexus staking slash sink account (`nexus.staking.slash_sink_account_id`); update nexus config first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((escrow_definition_id, _)) = state_transaction
                .settlement
                .offline
                .escrow_accounts
                .iter()
                .find(|(definition_id, escrow_account)| {
                    *escrow_account == &account_id
                        && state_transaction
                            .world
                            .asset_definitions
                            .get(*definition_id)
                            .is_some()
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as offline escrow account for active asset definition {escrow_definition_id} (`settlement.offline.escrow_accounts`); update settlement config first"
                    )
                    .into(),
                )
                .into());
            }
            if state_transaction
                .content
                .publish_allow_accounts
                .iter()
                .any(|publisher| publisher == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as content publish allow-list account (`content.publish_allow_accounts`); update content config first"
                    )
                    .into(),
                )
                .into());
            }
            if state_transaction
                .gov
                .sorafs_telemetry
                .submitters
                .iter()
                .any(|submitter| submitter == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as SoraFS telemetry submitter (`gov.sorafs_telemetry.submitters`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((provider_id, _)) = state_transaction
                .gov
                .sorafs_telemetry
                .per_provider_submitters
                .iter()
                .find(|(_, submitters)| submitters.iter().any(|submitter| submitter == &account_id))
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as per-provider SoraFS telemetry submitter for provider {} (`gov.sorafs_telemetry.per_provider_submitters`); update governance config first",
                        hex::encode(provider_id.as_bytes())
                    )
                    .into(),
                )
                .into());
            }
            if let Some((provider_id, _)) = state_transaction
                .gov
                .sorafs_provider_owners
                .iter()
                .find(|(_, owner)| *owner == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is configured as SoraFS provider owner for provider {} (`gov.sorafs_provider_owners`); update governance config first",
                        hex::encode(provider_id.as_bytes())
                    )
                    .into(),
                )
                .into());
            }
            if let Some((provider_id, _)) = state_transaction
                .world
                .provider_owners
                .iter()
                .find(|(_, owner)| *owner == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it owns SoraFS provider {}; unregister or reassign provider owner first",
                        hex::encode(provider_id.as_bytes())
                    )
                    .into(),
                )
                .into());
            }
            if state_transaction.world.citizens.get(&account_id).is_some() {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has an active citizenship record; revoke citizenship first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, validator), _)) =
                state_transaction.world.public_lane_validators.iter().find(
                    |((_, validator), record)| {
                        validator == &account_id || record.stake_account == account_id
                    },
                )
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active public-lane validator stake state (lane {lane_id}, validator {validator}); exit validator first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, validator, staker), _)) = state_transaction
                .world
                .public_lane_stake_shares
                .iter()
                .find(|((_, validator, staker), record)| {
                    validator == &account_id
                        || staker == &account_id
                        || record.validator == account_id
                        || record.staker == account_id
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active public-lane stake share state (lane {lane_id}, validator {validator}, staker {staker}); unbond first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, epoch), _)) = state_transaction
                .world
                .public_lane_rewards
                .iter()
                .find(|(_, record)| {
                    record.asset.account() == &account_id
                        || record
                            .shares
                            .iter()
                            .any(|share| share.account == account_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active public-lane reward ledger state (lane {lane_id}, epoch {epoch}); settle or prune rewards first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, claimant, asset_id), _)) = state_transaction
                .world
                .public_lane_reward_claims
                .iter()
                .find(|((_, claimant, asset_id), _)| {
                    claimant == &account_id || asset_id.account() == &account_id
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has pending public-lane reward claim state as claimant or reward-asset owner (lane {lane_id}, account {claimant}, asset {asset_id}); claim or clear rewards first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((feed_id, _)) =
                state_transaction
                    .world
                    .oracle_feeds
                    .iter()
                    .find(|(_, feed)| {
                        feed.providers
                            .iter()
                            .any(|provider| provider == &account_id)
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle feed provider state (feed {feed_id}); update feed providers first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((change_id, _)) =
                state_transaction
                    .world
                    .oracle_changes
                    .iter()
                    .find(|(_, change)| {
                        change.proposer == account_id
                            || change
                                .feed
                                .providers
                                .iter()
                                .any(|provider| provider == &account_id)
                            || change.stages.iter().any(|stage| {
                                stage.approvals.contains(&account_id)
                                    || stage.rejections.contains(&account_id)
                            })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle governance state (change {change_id:?}); resolve or prune oracle change state first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((dispute_id, _)) =
                state_transaction
                    .world
                    .oracle_disputes
                    .iter()
                    .find(|(_, dispute)| {
                        dispute.challenger == account_id || dispute.target == account_id
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle dispute state (dispute {dispute_id:?}); resolve dispute first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((provider_key, _)) = state_transaction
                .world
                .oracle_provider_stats
                .iter()
                .find(|(provider_key, _)| provider_key.provider_id == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle provider stats state (feed {}); clear provider stats first",
                        provider_key.feed_id
                    )
                    .into(),
                )
                .into());
            }
            if let Some((observation_key, _)) = state_transaction
                .world
                .oracle_observations
                .iter()
                .find(|(_, window)| window.observations.contains_key(&account_id))
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle observation window state ({observation_key:?}); clear observation state first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((feed_id, _)) =
                state_transaction
                    .world
                    .oracle_history
                    .iter()
                    .find(|(_, history)| {
                        history.iter().any(|record| {
                            matches!(
                                &record.event.outcome,
                                iroha_data_model::oracle::FeedEventOutcome::Success(success)
                                    if success
                                        .entries
                                        .iter()
                                        .any(|entry| entry.oracle_id == account_id)
                            )
                        })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active oracle feed history state (feed {feed_id}); retain provider account for oracle audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((agreement_id, _)) =
                state_transaction
                    .world
                    .repo_agreements
                    .iter()
                    .find(|(_, agreement)| {
                        agreement.initiator == account_id
                            || agreement.counterparty == account_id
                            || agreement
                                .custodian
                                .as_ref()
                                .is_some_and(|custodian| custodian == &account_id)
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active repo agreement state ({agreement_id}); close repo agreement first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((settlement_id, _)) = state_transaction
                .world
                .settlement_ledgers
                .iter()
                .find(|(_, ledger)| {
                    ledger.entries.iter().any(|entry| {
                        entry.authority == account_id
                            || entry
                                .legs
                                .iter()
                                .any(|leg| leg.leg.from == account_id || leg.leg.to == account_id)
                    })
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active settlement ledger state ({settlement_id}); retain account for settlement audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((certificate_id, _)) = state_transaction
                .world
                .offline_allowances
                .iter()
                .find(|(_, record)| {
                    record.certificate.controller == account_id
                        || record.certificate.operator == account_id
                        || record.certificate.allowance.asset.account() == &account_id
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active offline allowance state (certificate {certificate_id}); revoke or rotate allowance first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((bundle_id, _)) = state_transaction
                .world
                .offline_to_online_transfers
                .iter()
                .find(|(_, record)| {
                    record.controller == account_id
                        || record.transfer.receiver == account_id
                        || record.transfer.deposit_account == account_id
                        || record.transfer.receipts.iter().any(|receipt| {
                            receipt.from == account_id
                                || receipt.to == account_id
                                || receipt.asset.account() == &account_id
                        })
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active offline transfer state (bundle {bundle_id}); settle or prune transfer history first"
                    )
                    .into(),
                )
                    .into());
            }
            if let Some((verdict_id, _)) = state_transaction
                .world
                .offline_verdict_revocations
                .iter()
                .find(|(_, record)| record.issuer == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active offline verdict revocation state (verdict {verdict_id}); retain account for revocation audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((proposal_id, _)) = state_transaction
                .world
                .governance_proposals
                .iter()
                .find(|(_, record)| record.proposer == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active governance proposal state (proposal {}); retain proposer account for governance audit references",
                        hex::encode(*proposal_id)
                    )
                    .into(),
                )
                .into());
            }
            if let Some((referendum_id, _)) = state_transaction
                .world
                .governance_stage_approvals
                .iter()
                .find(|(_, approvals)| {
                    approvals
                        .stages
                        .values()
                        .any(|stage| stage.approvers.contains(&account_id))
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active governance stage approval state (referendum {referendum_id}); retain approver account for governance audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((referendum_id, _)) =
                state_transaction
                    .world
                    .governance_locks
                    .iter()
                    .find(|(_, locks)| {
                        locks.locks.iter().any(|(owner, record)| {
                            owner == &account_id || record.owner == account_id
                        })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active governance lock state (referendum {referendum_id}); unlock governance bonds first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((referendum_id, _)) = state_transaction
                .world
                .governance_slashes
                .iter()
                .find(|(_, slashes)| slashes.slashes.keys().any(|owner| owner == &account_id))
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active governance slash ledger state (referendum {referendum_id}); retain account for governance audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((epoch, _)) = state_transaction.world.council.iter().find(|(_, term)| {
                term.members.contains(&account_id) || term.alternates.contains(&account_id)
            }) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is present in governance council roster state (epoch {epoch}); rotate roster first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((epoch, _)) =
                state_transaction
                    .world
                    .parliament_bodies
                    .iter()
                    .find(|(_, bodies)| {
                        bodies.rosters.values().any(|roster| {
                            roster.members.contains(&account_id)
                                || roster.alternates.contains(&account_id)
                        })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is present in governance parliament roster state (epoch {epoch}); rotate roster first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((bundle_id, _)) = state_transaction
                .world
                .content_bundles
                .iter()
                .find(|(_, bundle)| bundle.created_by == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is referenced by content bundle state ({bundle_id}); retain account for content audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((upgrade_id, _)) = state_transaction
                .world
                .runtime_upgrades
                .iter()
                .find(|(_, record)| record.proposer == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active runtime upgrade proposal state (upgrade 0x{}); retain proposer account for governance audit references",
                        hex::encode(upgrade_id.0)
                    )
                    .into(),
                )
                .into());
            }
            if let Some((binding_digest, _)) = state_transaction
                .world
                .twitter_bindings
                .iter()
                .find(|(_, record)| record.provider == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active twitter binding oracle provider state (binding {binding_digest}); revoke binding or rotate provider first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((binding_digest, _)) = state_transaction
                .world
                .viral_escrows
                .iter()
                .find(|(_, record)| record.sender == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active viral escrow state (binding {binding_digest}); settle escrow first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((digest, _)) = state_transaction
                .world
                .pin_manifests
                .iter()
                .find(|(_, record)| record.submitted_by == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active SoraFS pin manifest state (digest 0x{}); retain submitter account for storage audit references",
                        hex::encode(digest.as_bytes())
                    )
                    .into(),
                )
                .into());
            }
            if let Some((alias_id, _)) = state_transaction
                .world
                .manifest_aliases
                .iter()
                .find(|(_, record)| record.bound_by == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active SoraFS manifest alias state (alias {}/{}) bound by this account; rotate alias binding first",
                        alias_id.namespace, alias_id.name
                    )
                    .into(),
                )
                .into());
            }
            if let Some((order_id, _)) = state_transaction
                .world
                .replication_orders
                .iter()
                .find(|(_, record)| record.issued_by == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active SoraFS replication order state (order {}); retain issuer account for storage audit references",
                        hex::encode(order_id.as_bytes())
                    )
                    .into(),
                )
                .into());
            }
            if let Some((ticket_id, record)) = state_transaction
                .world
                .da_pin_intents_by_ticket
                .iter()
                .find(|(_, record)| {
                    record
                        .intent
                        .owner
                        .as_ref()
                        .is_some_and(|owner| owner == &account_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has active DA pin intent owner state (ticket 0x{}, block {} index {}); clear DA pin intent ownership first",
                        hex::encode(ticket_id.as_bytes()),
                        record.location.block_height,
                        record.location.index_in_bundle
                    )
                    .into(),
                )
                .into());
            }
            if let Some((proposal_id, _)) = state_transaction
                .world
                .governance_proposals
                .iter()
                .find(|(_, proposal)| {
                    proposal
                        .parliament_snapshot
                        .as_ref()
                        .is_some_and(|snapshot| {
                            snapshot.bodies.rosters.values().any(|roster| {
                                roster.members.contains(&account_id)
                                    || roster.alternates.contains(&account_id)
                            })
                        })
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is present in governance proposal parliament snapshot state (proposal {}); retain account for governance audit references",
                        hex::encode(*proposal_id)
                    )
                    .into(),
                )
                .into());
            }
            if let Some((rwa_id, _)) = state_transaction
                .world
                .rwas
                .iter()
                .find(|(_, rwa)| rwa.owned_by == account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it owns RWA {rwa_id}; transfer or redeem the lot first"
                    )
                    .into(),
                )
                .into());
            }

            remove_account_associated_permissions(state_transaction, &account_id);

            state_transaction
                .world()
                .triggers()
                .inspect_by_action(
                    |action| action.authority() == &account_id,
                    |trigger_id, _| trigger_id.clone(),
                )
                .collect::<Vec<_>>()
                .into_iter()
                .for_each(|trigger_id| {
                    let removed = state_transaction.world.triggers.remove(&trigger_id);
                    removed.then_some(()).expect("should succeed");
                    crate::smartcontracts::isi::triggers::isi::remove_trigger_associated_permissions(
                        state_transaction,
                        &trigger_id,
                    );
                });

            state_transaction
                .world
                .account_permissions
                .remove(account_id.clone());

            state_transaction.world.remove_account_roles(&account_id);

            let remove_assets: Vec<AssetId> = state_transaction
                .world
                .assets_in_account_iter(&account_id)
                .map(|ad| ad.id().clone())
                .collect();
            for asset_id in remove_assets {
                state_transaction
                    .world
                    .remove_asset_and_metadata_with_total(&asset_id)?;
            }

            let remove_nfts: Vec<NftId> = state_transaction
                .world
                .nfts
                .iter()
                .filter(|(_, nft)| nft.owned_by == account_id)
                .map(|(id, _)| id.clone())
                .collect();
            for nft_id in remove_nfts {
                crate::smartcontracts::isi::nft::isi::remove_nft_associated_permissions(
                    state_transaction,
                    &nft_id,
                );
                state_transaction.world.nfts.remove(nft_id.clone());
                state_transaction
                    .world
                    .emit_events(Some(DomainEvent::Nft(NftEvent::Deleted(nft_id))));
            }

            let removed = state_transaction.world.accounts.remove(account_id.clone());
            let Some(account_value) = removed else {
                return Err(FindError::Account(account_id).into());
            };

            state_transaction
                .world
                .tx_sequences
                .remove(account_id.clone());
            state_transaction
                .world
                .offline_transfer_sender_index
                .remove(account_id.clone());
            state_transaction
                .world
                .offline_transfer_receiver_index
                .remove(account_id.clone());

            for label in state_transaction
                .world
                .remove_account_alias_bindings_for_account(&account_id)
            {
                state_transaction
                    .world
                    .account_rekey_records
                    .remove(label.clone());
            }

            if let Some(uaid) = account_value.uaid().copied() {
                state_transaction.world.uaid_accounts.remove(uaid);
                for opaque in account_value.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                    state_transaction.world.identifier_claims.remove(*opaque);
                }
                state_transaction.rebuild_space_directory_bindings(uaid);
            } else {
                for opaque in account_value.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                    state_transaction.world.identifier_claims.remove(*opaque);
                }
            }

            state_transaction
                .world
                .emit_events(Some(AccountEvent::Deleted(account_id)));

            Ok(())
        }
    }

    impl Execute for Register<AssetDefinition> {
        #[metrics(+"register_asset_definition")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition = self.object().clone().build(authority);
            ensure_asset_definition_human_fields(&asset_definition)?;

            let asset_definition_id = asset_definition.id().clone();
            if state_transaction
                .world
                .asset_definition(&asset_definition_id)
                .is_ok()
            {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::AssetDefinitionId(asset_definition_id),
                }
                .into());
            }
            if let Some(alias) = asset_definition.alias()
                && let Some(existing) = state_transaction
                    .world
                    .asset_definition_aliases
                    .get(alias)
                    .cloned()
                && existing != asset_definition_id
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("asset definition alias `{alias}` is already bound").into(),
                )
                .into());
            }
            let mut stored_definition = asset_definition.clone();
            stored_definition.alias = None;
            state_transaction
                .world
                .asset_definitions
                .insert(asset_definition_id.clone(), stored_definition);
            state_transaction
                .world
                .track_asset_definition_domain(&asset_definition_id);
            if let Some(alias) = asset_definition.alias().as_ref().cloned() {
                let bound_at_ms = state_transaction.block_unix_timestamp_ms();
                state_transaction.world.bind_asset_definition_alias(
                    &asset_definition_id,
                    alias,
                    None,
                    None,
                    bound_at_ms,
                )?;
            }

            ensure_offline_escrow_account(&asset_definition, authority, state_transaction)?;

            state_transaction
                .world
                .emit_events(Some(DomainEvent::AssetDefinition(
                    AssetDefinitionEvent::Created(asset_definition),
                )));

            Ok(())
        }
    }

    impl Execute for Unregister<AssetDefinition> {
        #[metrics(+"unregister_asset_definition")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition_id = self.object().clone();

            if asset_definition_id == state_transaction.gov.voting_asset_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as governance voting asset definition (`gov.voting_asset_id`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.gov.citizenship_asset_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as governance citizenship asset definition (`gov.citizenship_asset_id`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.gov.parliament_eligibility_asset_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as governance parliament eligibility asset definition (`gov.parliament_eligibility_asset_id`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id
                == state_transaction
                    .gov
                    .viral_incentives
                    .reward_asset_definition_id
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as governance viral reward asset definition (`gov.viral_incentives.reward_asset_definition_id`); update governance config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.oracle.economics.reward_asset {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as oracle reward asset definition (`oracle.economics.reward_asset`); update oracle config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.oracle.economics.slash_asset {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as oracle slash asset definition (`oracle.economics.slash_asset`); update oracle config first"
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.oracle.economics.dispute_bond_asset {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as oracle dispute bond asset definition (`oracle.economics.dispute_bond_asset`); update oracle config first"
                    )
                    .into(),
                )
                .into());
            }
            if parse_config_asset_definition_id(
                &state_transaction.world,
                &state_transaction.nexus.fees.fee_asset_id,
                state_transaction.block_unix_timestamp_ms(),
            )
            .is_some_and(|configured| configured == asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as nexus fee asset definition (`nexus.fees.fee_asset_id`); update nexus config first"
                    )
                    .into(),
                )
                .into());
            }
            if parse_config_asset_definition_id(
                &state_transaction.world,
                &state_transaction.nexus.staking.stake_asset_id,
                state_transaction.block_unix_timestamp_ms(),
            )
            .is_some_and(|configured| configured == asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as nexus staking asset definition (`nexus.staking.stake_asset_id`); update nexus config first"
                    )
                    .into(),
                )
                .into());
            }
            if state_transaction
                .settlement
                .repo
                .eligible_collateral
                .iter()
                .any(|definition_id| definition_id == &asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as settlement repo eligible collateral (`settlement.repo.eligible_collateral`); update settlement config first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((base_definition_id, _)) = state_transaction
                .settlement
                .repo
                .collateral_substitution_matrix
                .iter()
                .find(|(base_definition_id, substitutes)| {
                    *base_definition_id == &asset_definition_id
                        || substitutes
                            .iter()
                            .any(|definition_id| definition_id == &asset_definition_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured in settlement repo collateral substitution matrix (`settlement.repo.collateral_substitution_matrix`, base {base_definition_id}); update settlement config first"
                    )
                    .into(),
                )
                .into());
            }

            if let Some((agreement_id, _)) =
                state_transaction
                    .world
                    .repo_agreements
                    .iter()
                    .find(|(_, agreement)| {
                        agreement.cash_leg().asset_definition_id() == &asset_definition_id
                            || agreement.collateral_leg().asset_definition_id()
                                == &asset_definition_id
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is referenced by repo agreement state ({agreement_id}); retain asset definition for settlement audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((settlement_id, _)) = state_transaction
                .world
                .settlement_ledgers
                .iter()
                .find(|(_, ledger)| {
                    ledger.entries.iter().any(|entry| {
                        entry
                            .legs
                            .iter()
                            .any(|leg| leg.leg.asset_definition_id() == &asset_definition_id)
                    })
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is referenced by settlement ledger state ({settlement_id}); retain asset definition for settlement audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, epoch), _)) = state_transaction
                .world
                .public_lane_rewards
                .iter()
                .find(|(_, record)| record.asset.definition() == &asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it has active public-lane reward ledger state (lane {lane_id}, epoch {epoch}); settle or prune rewards first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, claimant, asset_id), _)) = state_transaction
                .world
                .public_lane_reward_claims
                .iter()
                .find(|((_, _, asset_id), _)| asset_id.definition() == &asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it has pending public-lane reward claim state (lane {lane_id}, account {claimant}, asset {asset_id}); claim or clear rewards first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((certificate_id, _)) = state_transaction
                .world
                .offline_allowances
                .iter()
                .find(|(_, record)| {
                    record.certificate.allowance.asset.definition() == &asset_definition_id
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it has active offline allowance state (certificate {certificate_id}); revoke or rotate allowance first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((bundle_id, _)) = state_transaction
                .world
                .offline_to_online_transfers
                .iter()
                .find(|(_, record)| {
                    record
                        .transfer
                        .receipts
                        .iter()
                        .any(|receipt| receipt.asset.definition() == &asset_definition_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it has active offline transfer state (bundle {bundle_id}); settle or prune transfer history first"
                    )
                    .into(),
                )
                .into());
            }

            remove_asset_definition_associated_permissions(state_transaction, &asset_definition_id);

            let mut assets_to_remove = Vec::new();
            assets_to_remove.extend(
                state_transaction
                    .world
                    .assets
                    .iter()
                    .filter(|(asset_id, _)| asset_id.definition() == &asset_definition_id)
                    .map(|(asset_id, _)| asset_id)
                    .cloned(),
            );

            let mut events = Vec::with_capacity(assets_to_remove.len() + 1);
            for asset_id in assets_to_remove {
                if state_transaction
                    .world
                    .remove_asset_and_metadata_with_total(&asset_id)?
                    .is_none()
                {
                    error!(%asset_id, "asset not found. This is a bug");
                }

                events.push(AssetEvent::Deleted(asset_id).into());
            }

            if state_transaction
                .world
                .asset_definitions
                .remove(asset_definition_id.clone())
                .is_none()
            {
                return Err(FindError::AssetDefinition(asset_definition_id).into());
            }
            state_transaction
                .world
                .untrack_asset_definition_domain(&asset_definition_id);
            state_transaction
                .world
                .clear_asset_definition_alias(&asset_definition_id);
            state_transaction
                .world
                .zk_assets
                .remove(asset_definition_id.clone());
            state_transaction
                .settlement
                .offline
                .escrow_accounts
                .remove(&asset_definition_id);

            events.push(DataEvent::from(AssetDefinitionEvent::Deleted(
                asset_definition_id,
            )));

            state_transaction.world.emit_events(events);

            Ok(())
        }
    }

    impl Execute for SetAssetDefinitionAlias {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetAssetDefinitionAlias {
                asset_definition_id,
                alias,
                lease_expiry_ms,
            } = self;

            if alias.is_none() && lease_expiry_ms.is_some() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "lease_expiry_ms requires alias binding".into(),
                    ),
                )
                .into());
            }

            // Ensure definition exists and validate alias semantics against the required human name.
            let definition = state_transaction
                .world
                .asset_definition(&asset_definition_id)
                .map_err(Error::from)?;
            if let Some(alias) = alias.as_ref() {
                validate_asset_alias(Some(alias), definition.name()).map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!("invalid asset definition alias: {err}").into(),
                    )
                })?;
            }

            if let Some(alias) = alias {
                let bound_at_ms = state_transaction.block_unix_timestamp_ms();
                if lease_expiry_ms.is_some_and(|lease_expiry_ms| lease_expiry_ms <= bound_at_ms) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "lease_expiry_ms must be greater than the current block timestamp"
                                .into(),
                        ),
                    )
                    .into());
                }
                let grace_until_ms = alias_grace_until_ms(lease_expiry_ms);
                state_transaction.world.bind_asset_definition_alias(
                    &asset_definition_id,
                    alias,
                    lease_expiry_ms,
                    grace_until_ms,
                    bound_at_ms,
                )?;
            } else {
                state_transaction
                    .world
                    .clear_asset_definition_alias(&asset_definition_id);
            }

            Ok(())
        }
    }

    impl Execute for SetContractAlias {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetContractAlias {
                contract_address,
                alias,
                lease_expiry_ms,
            } = self;

            if alias.is_none() && lease_expiry_ms.is_some() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "lease_expiry_ms requires alias binding".into(),
                    ),
                )
                .into());
            }

            let contract_dataspace_id = contract_address.dataspace_id().map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.to_string().into(),
                ))
            })?;
            let Some(contract_dataspace) = state_transaction
                .nexus
                .dataspace_catalog
                .by_id(contract_dataspace_id)
            else {
                return Err(InstructionExecutionError::InvariantViolation(
                    "contract address dataspace is unknown".to_owned().into(),
                )
                .into());
            };
            if state_transaction
                .world
                .contract_instances()
                .get(&(
                    contract_dataspace.alias.clone(),
                    contract_address.to_string(),
                ))
                .is_none()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("contract {contract_address} is not deployed").into(),
                )
                .into());
            }

            if let Some(alias) = alias {
                let (_, _, alias_dataspace_id) = alias
                    .resolve_components(&state_transaction.nexus.dataspace_catalog)
                    .map_err(|err| {
                        InstructionExecutionError::InvalidParameter(
                            InvalidParameterError::SmartContract(err.to_string().into()),
                        )
                    })?;
                if alias_dataspace_id != contract_dataspace_id {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "contract alias dataspace must match contract address dataspace".into(),
                        ),
                    )
                    .into());
                }

                ensure_account_alias_namespace_available_for_contract_alias(
                    state_transaction,
                    &alias,
                )?;

                let bound_at_ms = state_transaction.block_unix_timestamp_ms();
                if lease_expiry_ms.is_some_and(|lease_expiry_ms| lease_expiry_ms <= bound_at_ms) {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "lease_expiry_ms must be greater than the current block timestamp"
                                .into(),
                        ),
                    )
                    .into());
                }
                let grace_until_ms = alias_grace_until_ms(lease_expiry_ms);
                state_transaction.world.bind_contract_alias(
                    &contract_address,
                    alias,
                    lease_expiry_ms,
                    grace_until_ms,
                    bound_at_ms,
                )?;
            } else {
                state_transaction
                    .world
                    .clear_contract_alias(&contract_address);
            }

            Ok(())
        }
    }

    impl Execute for SetKeyValue<AssetDefinition> {
        #[metrics(+"set_key_value_asset_definition")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: asset_definition_id,
                key,
                value,
            } = self;
            let ensure_offline_escrow = key.as_ref() == OFFLINE_ASSET_ENABLED_METADATA_KEY;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .asset_definition_mut(&asset_definition_id)
                .map_err(Error::from)
                .map(|asset_definition| {
                    asset_definition
                        .metadata_mut()
                        .insert(key.clone(), value.clone())
                })?;

            if ensure_offline_escrow {
                let asset_definition = state_transaction
                    .world
                    .asset_definition(&asset_definition_id)
                    .map_err(Error::from)?;
                ensure_offline_escrow_account(&asset_definition, authority, state_transaction)?;
            }

            state_transaction
                .world
                .emit_events(Some(AssetDefinitionEvent::MetadataInserted(
                    MetadataChanged {
                        target: asset_definition_id,
                        key,
                        value,
                    },
                )));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<AssetDefinition> {
        #[metrics(+"remove_key_value_asset_definition")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition_id = self.object().clone();

            let value = state_transaction
                .world
                .asset_definition_mut(&asset_definition_id)
                .and_then(|asset_definition| {
                    asset_definition
                        .metadata_mut()
                        .remove(self.key().as_ref())
                        .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
                })?;

            state_transaction
                .world
                .emit_events(Some(AssetDefinitionEvent::MetadataRemoved(
                    MetadataChanged {
                        target: asset_definition_id,
                        key: self.key().clone(),
                        value,
                    },
                )));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Domain> {
        #[metrics(+"set_domain_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: domain_id,
                key,
                value,
            } = self;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            let domain = state_transaction.world.domain_mut(&domain_id)?;
            domain.metadata_mut().insert(key.clone(), value.clone());

            state_transaction
                .world
                .emit_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
                    target: domain_id,
                    key,
                    value,
                })));

            Ok(())
        }
    }

    // centralized in smartcontracts::limits

    impl Execute for RemoveKeyValue<Domain> {
        #[metrics(+"remove_domain_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let domain_id = self.object().clone();

            let domain = state_transaction.world.domain_mut(&domain_id)?;
            let value = domain
                .metadata_mut()
                .remove(self.key().as_ref())
                .ok_or_else(|| FindError::MetadataKey(self.key().clone()))?;

            state_transaction
                .world
                .emit_events(Some(DomainEvent::MetadataRemoved(MetadataChanged {
                    target: domain_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for SetAccountAliasBinding {
        #[metrics(+"set_account_alias_binding")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetAccountAliasBinding {
                account,
                alias,
                lease_expiry_ms,
            } = self;
            let existing_label = state_transaction.world.account(&account)?.label().cloned();
            let Some(alias) = alias else {
                if lease_expiry_ms.is_some() {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "lease_expiry_ms requires alias binding".into(),
                        ),
                    )
                    .into());
                }
                let existing_aliases = state_transaction
                    .world
                    .account_aliases_by_account
                    .get(&account)
                    .cloned()
                    .unwrap_or_default();
                for existing_alias in existing_aliases {
                    if existing_label.as_ref() == Some(&existing_alias) {
                        continue;
                    }
                    state_transaction
                        .world
                        .remove_account_alias_binding(&existing_alias);
                    state_transaction
                        .world
                        .account_rekey_records
                        .remove(existing_alias.clone());
                }
                return Ok(());
            };
            if account_label_is_pii(&alias) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Account alias looks like raw PII; use UAID/opaque identifiers instead"
                        .to_owned()
                        .into(),
                )
                .into());
            }

            if !authority_can_manage_account_alias(&state_transaction.world, authority, &alias) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "authority is not permitted to bind this account alias"
                        .to_owned()
                        .into(),
                )
                .into());
            }
            refresh_account_alias_lease_if_requested(state_transaction, &alias, lease_expiry_ms)?;
            ensure_active_account_alias_lease(state_transaction, &alias)?;
            ensure_contract_alias_namespace_available(state_transaction, &alias)?;

            purge_stale_account_label_state(state_transaction, &alias);
            let existing_alias_binding =
                state_transaction.world.account_aliases.get(&alias).cloned();
            if let Some(existing_owner) = existing_alias_binding.as_ref() {
                if existing_owner != &account {
                    let displaced_label = state_transaction
                        .world
                        .account(existing_owner)?
                        .label()
                        .cloned();
                    if displaced_label.as_ref() == Some(&alias) {
                        state_transaction
                            .world
                            .account_mut(existing_owner)?
                            .set_label(None);
                    }
                    state_transaction.world.remove_account_alias_binding(&alias);
                }
            }
            if existing_alias_binding.is_none()
                && state_transaction
                    .world
                    .account_rekey_records
                    .get(&alias)
                    .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "account alias already registered".to_owned().into(),
                )
                .into());
            }

            state_transaction
                .world
                .insert_account_alias_binding(alias.clone(), account.clone());
            upsert_account_rekey_record(state_transaction, &alias, &account);

            Ok(())
        }
    }

    impl Execute for SetPrimaryAccountAlias {
        #[metrics(+"set_primary_account_alias")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetPrimaryAccountAlias {
                account,
                alias,
                lease_expiry_ms,
            } = self;
            let existing_label = state_transaction.world.account(&account)?.label().cloned();
            let Some(alias) = alias else {
                if lease_expiry_ms.is_some() {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "lease_expiry_ms requires alias binding".into(),
                        ),
                    )
                    .into());
                }
                if let Some(previous_label) = existing_label.as_ref() {
                    state_transaction
                        .world
                        .remove_account_alias_binding(previous_label);
                    state_transaction
                        .world
                        .account_rekey_records
                        .remove(previous_label.clone());
                }
                state_transaction
                    .world
                    .account_mut(&account)?
                    .set_label(None);
                return Ok(());
            };
            if account_label_is_pii(&alias) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Account alias looks like raw PII; use UAID/opaque identifiers instead"
                        .to_owned()
                        .into(),
                )
                .into());
            }

            if !authority_can_manage_account_alias(&state_transaction.world, authority, &alias) {
                return Err(InstructionExecutionError::InvariantViolation(
                    "authority is not permitted to set this account alias"
                        .to_owned()
                        .into(),
                )
                .into());
            }
            refresh_account_alias_lease_if_requested(state_transaction, &alias, lease_expiry_ms)?;
            ensure_active_account_alias_lease(state_transaction, &alias)?;
            ensure_contract_alias_namespace_available(state_transaction, &alias)?;

            purge_stale_account_label_state(state_transaction, &alias);
            let existing_alias_owner = state_transaction.world.account_aliases.get(&alias).cloned();
            if let Some(existing_owner) = existing_alias_owner.as_ref() {
                if existing_owner != &account {
                    let displaced_label = state_transaction
                        .world
                        .account(existing_owner)?
                        .label()
                        .cloned();
                    if displaced_label.as_ref() == Some(&alias) {
                        state_transaction
                            .world
                            .account_mut(existing_owner)?
                            .set_label(None);
                    }
                    state_transaction.world.remove_account_alias_binding(&alias);
                }
            }
            if existing_label.as_ref() != Some(&alias)
                && state_transaction
                    .world
                    .account_rekey_records
                    .get(&alias)
                    .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "account alias already registered".to_owned().into(),
                )
                .into());
            }

            if let Some(previous_label) = existing_label.as_ref() {
                state_transaction
                    .world
                    .remove_account_alias_binding(previous_label);
                state_transaction
                    .world
                    .account_rekey_records
                    .remove(previous_label.clone());
            }

            state_transaction
                .world
                .account_mut(&account)?
                .set_label(Some(alias.clone()));
            state_transaction
                .world
                .insert_account_alias_binding(alias.clone(), account.clone());
            upsert_account_rekey_record(state_transaction, &alias, &account);

            Ok(())
        }
    }

    impl Execute for Transfer<Account, DomainId, Account> {
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
            let authority_is_transferred_domain_owner =
                state_transaction.world.domain(&object)?.owned_by() == authority;
            if !(authority_is_source_owner || authority_is_transferred_domain_owner) {
                return Err(Error::InvariantViolation(
                    "Can't transfer domain of another account".to_owned().into(),
                ));
            }

            let domain = state_transaction.world.domain_mut(&object)?;

            if domain.owned_by() != &source {
                return Err(Error::InvariantViolation(
                    format!("Can't transfer domain {domain} since {source} doesn't own it",).into(),
                ));
            }

            domain.set_owned_by(destination.clone());
            state_transaction
                .world
                .emit_events(Some(DomainEvent::OwnerChanged(DomainOwnerChanged {
                    domain: object,
                    new_owner: destination,
                })));

            Ok(())
        }
    }

    pub(crate) fn ensure_controller_capabilities(
        controller: &AccountController,
        allowed_algorithms: &[Algorithm],
        allowed_curve_ids: &[u8],
    ) -> Result<(), InstructionExecutionError> {
        if let Some(disallowed) = first_disallowed_algorithm(controller, allowed_algorithms) {
            let allowed_summary = if allowed_algorithms.is_empty() {
                "none".to_string()
            } else {
                allowed_algorithms
                    .iter()
                    .copied()
                    .map(Algorithm::as_static_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "account controller uses signing algorithm {disallowed} which is not \
                     permitted by crypto.allowed_signing (allowed: {allowed_summary})"
                )
                .into(),
            ));
        }

        match first_disallowed_curve(controller, allowed_curve_ids) {
            Ok(Some(curve)) => {
                let algo = curve.algorithm();
                let curve_code: u8 = curve.into();
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account controller uses curve id {curve_code:#04X} ({}) which is not \
                         permitted by crypto.curves.allowed_curve_ids",
                        algo.as_static_str()
                    )
                    .into(),
                ));
            }
            Ok(None) => {}
            Err((algo, err)) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account controller uses signing algorithm {} which is not registered in \
                         the account curve registry: {err}",
                        algo.as_static_str()
                    )
                    .into(),
                ));
            }
        }

        Ok(())
    }

    fn first_disallowed_algorithm(
        controller: &AccountController,
        allowed: &[Algorithm],
    ) -> Option<Algorithm> {
        match controller {
            AccountController::Single(signatory) => {
                algorithm_if_disallowed(signatory.algorithm(), allowed)
            }
            AccountController::Multisig(policy) => policy
                .members()
                .iter()
                .find_map(|member| algorithm_if_disallowed(member.algorithm(), allowed)),
        }
    }

    fn algorithm_if_disallowed(algo: Algorithm, allowed: &[Algorithm]) -> Option<Algorithm> {
        if allowed.contains(&algo) || is_bls_algorithm(algo) {
            None
        } else {
            Some(algo)
        }
    }

    fn first_disallowed_curve(
        controller: &AccountController,
        allowed_curve_ids: &[u8],
    ) -> Result<Option<CurveId>, (Algorithm, CurveRegistryError)> {
        match controller {
            AccountController::Single(signatory) => {
                let algo = signatory.algorithm();
                curve_if_disallowed(algo, allowed_curve_ids).map_err(|err| (algo, err))
            }
            AccountController::Multisig(policy) => {
                for member in policy.members() {
                    let algo = member.algorithm();
                    match curve_if_disallowed(algo, allowed_curve_ids) {
                        Ok(Some(curve)) => return Ok(Some(curve)),
                        Ok(None) => {}
                        Err(err) => return Err((algo, err)),
                    }
                }
                Ok(None)
            }
        }
    }

    fn curve_if_disallowed(
        algo: Algorithm,
        allowed_curve_ids: &[u8],
    ) -> Result<Option<CurveId>, CurveRegistryError> {
        if is_bls_algorithm(algo) {
            // Consensus validators rely on BLS controller keys even when admission is restricted.
            return Ok(None);
        }
        let curve = CurveId::try_from_algorithm(algo)?;
        if allowed_curve_ids.contains(&curve.as_u8()) {
            Ok(None)
        } else {
            Ok(Some(curve))
        }
    }

    fn is_bls_algorithm(algo: Algorithm) -> bool {
        #[cfg(feature = "bls")]
        {
            matches!(algo, Algorithm::BlsNormal | Algorithm::BlsSmall)
        }
        #[cfg(not(feature = "bls"))]
        {
            let _ = algo;
            false
        }
    }
}

/// Implementations for domain queries.
pub mod query {
    use iroha_data_model::{
        domain::Domain,
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail,
        },
    };

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindDomains {
        #[metrics(+"find_domains")]
        fn execute(
            self,
            filter: CompoundPredicate<Domain>,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<impl Iterator<Item = Domain>, QueryExecutionFail> {
            Ok(state_ro
                .world()
                .domains_iter()
                .filter(move |&v| filter.applies(v))
                .cloned())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        IntoKeyValue,
        account::{
            AccountAddress, NewAccount, OpaqueAccountId,
            controller::{MultisigMember, MultisigPolicy},
            rekey::AccountAlias,
        },
        asset::definition::AssetConfidentialPolicy,
        asset::{
            Asset, AssetDefinition, AssetDefinitionAlias, AssetDefinitionId, AssetId, Mintable,
            NewAssetDefinition,
        },
        block::BlockHeader,
        events::data::space_directory::{
            SpaceDirectoryEvent, SpaceDirectoryManifestActivated, SpaceDirectoryManifestRevoked,
        },
        metadata::Metadata,
        name::Name,
        nexus::{AssetPermissionManifest, DataSpaceId, ManifestVersion, UniversalAccountId},
        nft::{Nft, NftId},
        offline::{
            OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineAllowanceCommitment, OfflineAllowanceRecord,
            OfflineCounterState, OfflineWalletCertificate, OfflineWalletPolicy,
        },
        permission::Permission,
        prelude::Domain,
        role::{Role, RoleId},
        sns::{NameControllerV1, NameRecordV1},
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount,
    };
    use iroha_primitives::{
        json::Json,
        numeric::{Numeric, NumericSpec},
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
        prelude::World,
        query::store::LiveQueryStore,
        state::State,
    };

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(domain_id.clone(), domain);
    }

    fn seed_account(state: &mut State, account_id: &AccountId, domain_id: &DomainId) {
        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let _ = domain_id;
        let (account_id, account_value) = account.into_key_value();
        state.world.accounts.insert(account_id, account_value);
    }

    fn alias_domain(domain: &DomainId) -> AccountAliasDomain {
        AccountAliasDomain::new(domain.name().clone())
    }

    fn alias_in_domain(domain: &DomainId, label: Name) -> AccountAlias {
        AccountAlias::new(label, Some(alias_domain(domain)), DataSpaceId::GLOBAL)
    }

    fn seed_account_alias_lease(
        tx: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        alias: &AccountAlias,
    ) {
        let selector = crate::sns::selector_for_account_alias(alias, &tx.nexus.dataspace_catalog)
            .expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            1_000,
            2_000,
            3_000,
            Metadata::default(),
        );
        tx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    fn seed_domainful_alias_manage_permissions(
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        domain: &DomainId,
    ) {
        tx.world.add_account_permission(
            authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(alias_domain(domain)),
            }),
        );
        tx.world.add_account_permission(
            authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::GLOBAL),
            }),
        );
    }

    fn seed_manifest_record<F>(
        world: &mut World,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
        configure: F,
    ) -> Hash
    where
        F: FnOnce(&mut SpaceDirectoryManifestRecord),
    {
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid,
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let mut record = SpaceDirectoryManifestRecord::new(manifest);
        configure(&mut record);
        let manifest_hash = record.manifest_hash;
        let mut set = SpaceDirectoryManifestSet::default();
        set.upsert(record);
        world.space_directory_manifests.insert(uaid, set);
        manifest_hash
    }

    #[test]
    fn account_label_registration_and_cleanup() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(account_label.clone()));

        // Execute register with label.
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_lease(&mut tx, &authority, &account_label);
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with label");
        assert!(
            tx.world.account_rekey_records.get(&account_label).is_some(),
            "rekey record should be inserted"
        );
        assert_eq!(
            tx.world.account_aliases.get(&account_label),
            Some(&account_id),
            "alias index should be inserted"
        );

        // Duplicate label should be rejected.
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let dup_account = Account::new(second_id.clone()).with_label(Some(account_label.clone()));
        let err = Register::account(dup_account).execute(&authority, &mut tx);
        assert!(err.is_err(), "duplicate label must raise error");

        // Unregister removes label mapping.
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");
        assert!(
            tx.world.accounts.get(&account_id).is_none(),
            "account should be removed from world"
        );
        assert!(
            tx.world.account_rekey_records.get(&account_label).is_none(),
            "label record must be removed on unregister"
        );
        assert!(
            tx.world.account_aliases.get(&account_label).is_none(),
            "alias index must be removed on unregister"
        );
    }

    #[test]
    fn register_domainless_account_indexes_alias() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let account_id = AccountId::new(KeyPair::random().public_key().clone());
        let account_label = AccountAlias::domainless(
            "primary".parse::<Name>().expect("label"),
            DataSpaceId::GLOBAL,
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_lease(&mut tx, &authority, &account_label);
        Register::account(Account::new(account_id.clone()).with_label(Some(account_label.clone())))
            .execute(&authority, &mut tx)
            .expect("register domainless account");

        let account = tx.world.account(&account_id).expect("account should exist");
        assert_eq!(account.label(), Some(&account_label));
        assert_eq!(
            tx.world.account_aliases.get(&account_label),
            Some(&account_id)
        );
    }

    #[test]
    fn register_domainless_account_emits_created_event_with_universal_domain() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register domainless account");

        let expected_domain: DomainId = crate::sns::RESERVED_UNIVERSAL_DATASPACE_ALIAS
            .parse()
            .expect("reserved universal dataspace alias should parse as domain");
        let created = tx
            .world
            .internal_event_buf
            .iter()
            .find_map(|event| match event.as_ref() {
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(created)))
                    if created.account.id() == &account_id =>
                {
                    Some(created.clone())
                }
                _ => None,
            })
            .expect("account created event");

        assert_eq!(created.domain, expected_domain);
    }

    #[test]
    fn register_account_with_label_emits_created_event_with_alias_domain() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);

        let account_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease(&mut tx, &authority, &account_label);
        Register::account(Account::new(account_id.clone()).with_label(Some(account_label)))
            .execute(&authority, &mut tx)
            .expect("register account with label");

        let created = tx
            .world
            .internal_event_buf
            .iter()
            .find_map(|event| match event.as_ref() {
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(created)))
                    if created.account.id() == &account_id =>
                {
                    Some(created.clone())
                }
                _ => None,
            })
            .expect("account created event");

        assert_eq!(created.domain, domain_id);
    }

    #[test]
    fn register_account_with_label_requires_active_sns_lease() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);

        let label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        let err = Register::account(Account::new(account_id.clone()).with_label(Some(label)))
            .execute(&authority, &mut tx)
            .expect_err("alias lease should be required");

        assert!(
            err.to_string().contains("active SNS lease"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_account_label_relabels_existing_single_key_account() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let old_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let new_label = alias_in_domain(&domain_id, "treasury".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(old_label.clone()));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_lease(&mut tx, &authority, &old_label);
        seed_account_alias_lease(&mut tx, &authority, &new_label);
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with initial label");

        SetPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(new_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("relabel existing account");

        assert!(
            tx.world.account_aliases.get(&old_label).is_none(),
            "old alias index should be removed"
        );
        assert!(
            tx.world.account_rekey_records.get(&old_label).is_none(),
            "old rekey record should be removed"
        );
        assert_eq!(
            tx.world.account_aliases.get(&new_label),
            Some(&account_id),
            "new alias index should be inserted"
        );
        assert!(
            tx.world.account_rekey_records.get(&new_label).is_some(),
            "new rekey record should be inserted"
        );
        assert_eq!(
            tx.world
                .account(&account_id)
                .expect("account should exist")
                .label(),
            Some(&new_label),
            "account should expose the updated label"
        );
    }

    #[test]
    fn set_primary_account_alias_allows_domainful_alias_without_domain_link() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_id = AccountId::new(KeyPair::random().public_key().clone());
        let label = alias_in_domain(&domain_id, "treasury".parse::<Name>().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register domainless account");
        seed_account_alias_lease(&mut tx, &authority, &label);

        SetPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("domainful alias should no longer require a domain link");
        assert_eq!(tx.world.account_aliases.get(&label), Some(&account_id));
        assert_eq!(
            tx.world
                .account(&account_id)
                .expect("account should exist")
                .label(),
            Some(&label)
        );
    }

    #[test]
    fn set_account_label_reclaims_stale_alias_binding_with_missing_owner() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let stale_owner = AccountId::new(KeyPair::random().public_key().clone());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .account_aliases
            .insert(label.clone(), stale_owner.clone());
        tx.world.account_rekey_records.insert(
            label.clone(),
            AccountRekeyRecord::new(label.clone(), stale_owner),
        );
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &authority, &label);

        SetPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("set label should reclaim stale binding");

        assert_eq!(
            tx.world.account_aliases.get(&label),
            Some(&account_id),
            "label should resolve to the live account"
        );
        assert_eq!(
            tx.world
                .account_rekey_records
                .get(&label)
                .expect("rekey record should exist")
                .active_account_id,
            account_id,
            "rekey record should be repointed to the live account"
        );
    }

    #[test]
    fn bind_account_alias_requires_active_sns_lease() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let err = SetAccountAliasBinding {
            account: account_id,
            alias: Some(alias),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("alias lease should be required");

        assert!(
            err.to_string().contains("active SNS lease"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_account_label_binds_existing_multisig_account_with_rekey_record() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let member_a = MultisigMember::new(KeyPair::random().public_key().clone(), 1)
            .expect("multisig member");
        let member_b = MultisigMember::new(KeyPair::random().public_key().clone(), 1)
            .expect("multisig member");
        let policy = MultisigPolicy::new(2, vec![member_a, member_b]).expect("multisig policy");
        let account_id = AccountId::new_multisig(policy);
        let account_label = alias_in_domain(&domain_id, "cbdc".parse::<Name>().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register unlabeled multisig account");
        seed_account_alias_lease(&mut tx, &authority, &account_label);

        SetPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(account_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind label to existing multisig account");

        assert_eq!(
            tx.world.account_aliases.get(&account_label),
            Some(&account_id),
            "multisig alias index should be inserted"
        );
        let rekey_record = tx
            .world
            .account_rekey_records
            .get(&account_label)
            .expect("multisig aliases should create rekey records");
        assert_eq!(
            rekey_record.active_account_id, account_id,
            "rekey record should point at the multisig account"
        );
        assert!(
            rekey_record.active_signatory.is_none(),
            "multisig rekey records should not invent a single-key signatory"
        );
        assert_eq!(
            tx.world
                .account(&account_id)
                .expect("account should exist")
                .label(),
            Some(&account_label),
            "multisig account should expose the bound label"
        );
    }

    #[test]
    fn set_account_label_allows_account_registrar_to_repoint_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let permission: Permission = CanRegisterAccount {
            domain: domain_id.clone(),
        }
        .into();

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant registrar permission");
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetPrimaryAccountAlias {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");

        SetPrimaryAccountAlias {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("registrar should repoint alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&second_id),
            "alias should resolve to the replacement account"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "previous account label should be cleared after repoint"
        );
        assert_eq!(
            tx.world
                .account(&second_id)
                .expect("second account should exist")
                .label(),
            Some(&alias),
            "replacement account should expose the moved label"
        );
        let rekey_record = tx
            .world
            .account_rekey_records
            .get(&alias)
            .expect("single-key repoint should refresh the rekey record");
        assert_eq!(
            rekey_record.active_account_id, second_id,
            "rekey record should follow the replacement account"
        );
        assert_eq!(
            rekey_record.previous_account_ids,
            vec![first_id],
            "rekey record should retain the prior concrete account"
        );
        assert_eq!(
            rekey_record.active_signatory,
            Some(second_keypair.public_key().clone()),
            "rekey record should follow the replacement account"
        );
    }

    #[test]
    fn set_account_label_allows_global_account_registrar_to_repoint_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let permission = Permission::new(
            "CanRegisterAccount".parse().expect("permission name"),
            iroha_primitives::json::Json::new(()),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant global registrar permission");
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetPrimaryAccountAlias {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");

        SetPrimaryAccountAlias {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("global registrar should repoint alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&second_id),
            "alias should resolve to the replacement account"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "previous account label should be cleared after repoint"
        );
        assert_eq!(
            tx.world
                .account(&second_id)
                .expect("second account should exist")
                .label(),
            Some(&alias),
            "replacement account should expose the moved label"
        );
    }

    #[test]
    fn bind_account_alias_adds_multiple_aliases_to_existing_multisig_account() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let member_a = MultisigMember::new(KeyPair::random().public_key().clone(), 1)
            .expect("multisig member");
        let member_b = MultisigMember::new(KeyPair::random().public_key().clone(), 1)
            .expect("multisig member");
        let policy = MultisigPolicy::new(2, vec![member_a, member_b]).expect("multisig policy");
        let account_id = AccountId::new_multisig(policy);
        let banking_label = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let issuance_label = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register unlabeled multisig account");
        seed_account_alias_lease(&mut tx, &authority, &banking_label);
        seed_account_alias_lease(&mut tx, &authority, &issuance_label);

        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(banking_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind banking alias");
        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(issuance_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind issuance alias");

        assert_eq!(
            tx.world.account_aliases.get(&banking_label),
            Some(&account_id),
            "banking alias should resolve to the multisig account"
        );
        assert_eq!(
            tx.world.account_aliases.get(&issuance_label),
            Some(&account_id),
            "issuance alias should resolve to the same multisig account"
        );
        let banking_record = tx
            .world
            .account_rekey_records
            .get(&banking_label)
            .expect("banking alias should create a rekey record");
        assert_eq!(
            banking_record.active_account_id, account_id,
            "banking rekey record should resolve to the multisig account"
        );
        assert!(
            banking_record.active_signatory.is_none(),
            "multisig alias records should not expose a single-key signatory"
        );
        let issuance_record = tx
            .world
            .account_rekey_records
            .get(&issuance_label)
            .expect("issuance alias should create a rekey record");
        assert_eq!(
            issuance_record.active_account_id, account_id,
            "issuance rekey record should resolve to the multisig account"
        );
        assert!(
            issuance_record.active_signatory.is_none(),
            "multisig alias records should not expose a single-key signatory"
        );
        assert!(
            tx.world
                .account(&account_id)
                .expect("account should exist")
                .label()
                .is_none(),
            "binding extra aliases should not overwrite the account's canonical label"
        );
    }

    #[test]
    fn unregister_account_removes_all_bound_aliases() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);

        let primary_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let bound_label =
            AccountAlias::domainless("public".parse::<Name>().unwrap(), DataSpaceId::GLOBAL);
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease(&mut tx, &authority, &primary_label);
        seed_account_alias_lease(&mut tx, &authority, &bound_label);
        Register::account(Account::new(account_id.clone()).with_label(Some(primary_label.clone())))
            .execute(&authority, &mut tx)
            .expect("register account with primary label");
        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(bound_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind additional alias");

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(tx.world.account_aliases.get(&primary_label).is_none());
        assert!(tx.world.account_aliases.get(&bound_label).is_none());
        assert!(tx.world.account_rekey_records.get(&primary_label).is_none());
        assert!(tx.world.account_rekey_records.get(&bound_label).is_none());
        assert!(
            tx.world
                .account_aliases_by_account
                .get(&account_id)
                .is_none(),
            "reverse alias index must be cleared on unregister"
        );
    }

    #[test]
    fn clear_account_alias_binding_removes_non_primary_aliases_only() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);

        let primary_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let root_alias =
            AccountAlias::domainless("public".parse::<Name>().unwrap(), DataSpaceId::GLOBAL);
        let domain_alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease(&mut tx, &authority, &primary_label);
        seed_account_alias_lease(&mut tx, &authority, &root_alias);
        seed_account_alias_lease(&mut tx, &authority, &domain_alias);
        Register::account(Account::new(account_id.clone()).with_label(Some(primary_label.clone())))
            .execute(&authority, &mut tx)
            .expect("register account with primary label");
        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(root_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind root alias");
        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(domain_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind domain alias");

        SetAccountAliasBinding::clear(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("clear secondary aliases");

        assert_eq!(
            tx.world.account_aliases.get(&primary_label),
            Some(&account_id),
            "primary alias binding must remain"
        );
        assert!(tx.world.account_aliases.get(&root_alias).is_none());
        assert!(tx.world.account_aliases.get(&domain_alias).is_none());
        assert!(
            tx.world.account_rekey_records.get(&root_alias).is_none(),
            "cleared secondary alias must drop its rekey record"
        );
        assert!(
            tx.world.account_rekey_records.get(&domain_alias).is_none(),
            "cleared secondary alias must drop its rekey record"
        );
        assert_eq!(
            tx.world
                .account(&account_id)
                .expect("account should exist")
                .label(),
            Some(&primary_label),
            "clear must not remove the primary alias"
        );
        let remaining_aliases = tx
            .world
            .account_aliases_by_account
            .get(&account_id)
            .expect("reverse index should keep the primary alias");
        assert_eq!(remaining_aliases.len(), 1);
        assert!(remaining_aliases.contains(&primary_label));
    }

    #[test]
    fn bind_account_alias_reclaims_stale_binding_with_missing_owner() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let stale_owner = AccountId::new(KeyPair::random().public_key().clone());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .account_aliases
            .insert(alias.clone(), stale_owner.clone());
        tx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), stale_owner),
        );
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &authority, &alias);

        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind should reclaim stale alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&account_id),
            "alias should resolve to the live account"
        );
        assert_eq!(
            tx.world
                .account_rekey_records
                .get(&alias)
                .expect("rekey record should exist")
                .active_account_id,
            account_id,
            "rekey record should be repointed to the live account"
        );
    }

    #[test]
    fn bind_account_alias_allows_account_registrar_for_domain() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let permission: Permission = CanRegisterAccount {
            domain: domain_id.clone(),
        }
        .into();

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant registrar permission");
        Register::account(Account::new(account_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("registrar should bind alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&account_id),
            "registrar-bound alias should resolve to the target account"
        );
    }

    #[test]
    fn bind_account_alias_allows_global_account_registrar() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let permission = Permission::new(
            "CanRegisterAccount".parse().expect("permission name"),
            iroha_primitives::json::Json::new(()),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant global registrar permission");
        Register::account(Account::new(account_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("global registrar should bind alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&account_id),
            "global-registrar-bound alias should resolve to the target account"
        );
    }

    #[test]
    fn bind_account_alias_rejects_alias_owned_by_different_account_without_registrar_rights() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let unauthorized = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &unauthorized, &domain_id);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("bind alias to first account");

        let err = SetAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&unauthorized, &mut tx)
        .expect_err("alias collision should be rejected");

        assert!(
            err.to_string().contains("already registered"),
            "error should mention alias collision: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "existing alias binding must remain unchanged"
        );
    }

    #[test]
    fn bind_account_alias_allows_account_registrar_to_repoint_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let permission: Permission = CanRegisterAccount {
            domain: domain_id.clone(),
        }
        .into();

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant registrar permission");
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");

        SetAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("registrar should repoint alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&second_id),
            "alias should resolve to the replacement account"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "previous primary label should be cleared when its alias is rebound"
        );
    }

    #[test]
    fn bind_account_alias_allows_global_account_registrar_to_repoint_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);

        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let permission = Permission::new(
            "CanRegisterAccount".parse().expect("permission name"),
            iroha_primitives::json::Json::new(()),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(permission, registrar.clone())
            .execute(&domain_owner, &mut tx)
            .expect("grant global registrar permission");
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &domain_owner, &alias);

        SetAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");

        SetAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect("global registrar should repoint alias");

        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&second_id),
            "alias should resolve to the replacement account"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "previous primary label should be cleared when its alias is rebound"
        );
    }

    #[test]
    fn register_account_rejects_phone_like_label() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_label = alias_in_domain(&domain_id, "+819398553445".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(account_label));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("phone-like label should be rejected");
        assert!(
            err.to_string().contains("raw PII"),
            "error should mention raw PII: {err}"
        );
    }

    #[test]
    fn transfer_domain_rejects_authority_without_ownership() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let users_domain_id: DomainId = "users".parse().expect("users domain id");
        let transferred_domain_id: DomainId = "foo".parse().expect("foo domain id");
        let user1 = AccountId::new(KeyPair::random().into_parts().0);
        let user2 = AccountId::new(KeyPair::random().into_parts().0);

        let authority_domain: DomainId = "wonderland".parse().expect("domain id");
        seed_domain(&mut state, &authority_domain, &authority);
        seed_account(&mut state, &authority, &authority_domain);
        seed_domain(&mut state, &users_domain_id, &user1);
        seed_domain(&mut state, &transferred_domain_id, &user1);
        seed_account(&mut state, &user1, &users_domain_id);
        seed_account(&mut state, &user2, &users_domain_id);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = Transfer::domain(user1.clone(), transferred_domain_id.clone(), user2)
            .execute(&authority, &mut tx)
            .expect_err("transfer must fail for authority that does not own source or domain");
        assert!(
            err.to_string()
                .contains("Can't transfer domain of another account"),
            "unexpected transfer error: {err}"
        );
        assert_eq!(
            tx.world
                .domain(&transferred_domain_id)
                .expect("domain should still exist")
                .owned_by(),
            &user1
        );
    }

    #[test]
    fn register_account_rejects_opaque_ids_without_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_id = AccountId::new(KeyPair::random().public_key().clone());
        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::missing-uaid"));
        let new_account = NewAccount {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: vec![opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("opaque ids without UAID should be rejected");
        assert!(
            err.to_string()
                .contains("Opaque identifiers require a UAID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_account_rejects_duplicate_opaque_ids() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.dupes".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_id = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-dupes"));
        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::dupe"));
        let new_account = NewAccount {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: Some(uaid),
            opaque_ids: vec![opaque, opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("duplicate opaque ids should be rejected");
        assert!(
            err.to_string().contains("duplicate opaque identifier"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_account_rejects_opaque_id_collisions() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.collide".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::collide"));
        let first_id = AccountId::new(KeyPair::random().public_key().clone());
        let second_id = AccountId::new(KeyPair::random().public_key().clone());
        let first_uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-collide-1"));
        let second_uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-collide-2"));

        let first_account = NewAccount {
            id: first_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(first_uaid),
            opaque_ids: vec![opaque],
        };
        let second_account = NewAccount {
            id: second_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(second_uaid),
            opaque_ids: vec![opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(first_account)
            .execute(&authority, &mut tx)
            .expect("register first account");

        let err = Register::account(second_account)
            .execute(&authority, &mut tx)
            .expect_err("opaque id collisions should be rejected");
        assert!(
            err.to_string().contains("already bound to UAID"),
            "unexpected error: {err}"
        );
        assert_eq!(
            tx.world.opaque_uaids.get(&opaque),
            Some(&first_uaid),
            "opaque id should remain bound to first UAID"
        );
        assert!(
            tx.world.accounts.get(&second_id).is_none(),
            "colliding account must not be inserted"
        );
    }

    #[test]
    fn register_account_rejects_disallowed_algorithms() {
        let mut state = test_state();
        let domain_id: DomainId = "disallowed.curves".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_signing = vec![Algorithm::Ed25519];
            cfg.allowed_curve_ids =
                iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                    &cfg.allowed_signing,
                );
            *guard = Arc::new(cfg);
        }

        let secp_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let account_id = AccountId::new(secp_pair.public_key().clone());
        let new_account = Account::new(account_id.clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("registration with disallowed algorithm must fail");
        let err_string = err.to_string();
        assert!(
            err_string.contains("crypto.allowed_signing"),
            "error should reference allowed_signing gating: {err_string}"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn register_account_allows_bls_even_when_not_in_allowed_signing() {
        let mut state = test_state();
        let domain_id: DomainId = "bls.allowed".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_signing = vec![Algorithm::Ed25519];
            cfg.allowed_curve_ids =
                iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                    &cfg.allowed_signing,
                );
            *guard = Arc::new(cfg);
        }

        let bls_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_id = AccountId::new(bls_pair.public_key().clone());
        let new_account = Account::new(account_id.clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("BLS controllers should be allowed for consensus accounts");
    }

    #[test]
    fn register_account_rejects_disallowed_curve_ids() {
        let mut state = test_state();
        let domain_id: DomainId = "restricted.curves".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_curve_ids.clear();
            *guard = Arc::new(cfg);
        }

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("registration with disallowed curve ids must fail");
        let err_string = err.to_string();
        assert!(
            err_string.contains("crypto.curves.allowed_curve_ids"),
            "error should reference curve gating: {err_string}"
        );
    }

    #[test]
    fn register_account_updates_space_directory_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.bindings".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::register_bindings"));
        let dataspace = DataSpaceId::new(17);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(5);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with UAID");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        let bindings = view
            .world()
            .uaid_dataspaces()
            .get(&uaid)
            .expect("bindings exist after registration");
        let dataspace_entry = bindings
            .iter()
            .find(|(id, _)| **id == dataspace)
            .expect("dataspace should be present");
        assert!(
            dataspace_entry.1.contains(&account_id),
            "account must be bound to dataspace"
        );
    }

    #[test]
    fn register_account_rejects_duplicate_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = "uaid.duplicates".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::duplicate"));
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let first_account = NewAccount::new(first_id.clone()).with_uaid(Some(uaid));

        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let second_account = NewAccount::new(second_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(first_account)
            .execute(&authority, &mut tx)
            .expect("register first account");

        let err = Register::account(second_account)
            .execute(&authority, &mut tx)
            .expect_err("duplicate UAID must be rejected");
        let err_string = err.to_string();
        assert!(
            err_string.contains("UAID"),
            "error should reference UAID conflict: {err_string}"
        );
        assert_eq!(
            tx.world.uaid_accounts.get(&uaid),
            Some(&first_id),
            "UAID index must retain the first account"
        );
        assert!(
            tx.world.accounts.get(&second_id).is_none(),
            "duplicate account should not be inserted"
        );

        tx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        assert_eq!(view.world().uaid_accounts().get(&uaid), Some(&first_id));
    }

    #[test]
    fn unregister_account_removes_space_directory_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.cleanup".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unregister_bindings"));
        let dataspace = DataSpaceId::new(21);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(3);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        assert!(
            view.world().uaid_dataspaces().get(&uaid).is_none(),
            "bindings should be removed after account deletion"
        );
        assert!(
            view.world().uaid_accounts().get(&uaid).is_none(),
            "UAID index should be cleared after account deletion"
        );
    }

    #[test]
    fn unregister_account_removes_owned_nfts_and_asset_metadata() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let other_domain_id: DomainId = "other.world".parse().expect("domain id");
        seed_domain(&mut state, &other_domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().unwrap());
        Register::asset_definition({
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
        let asset = Asset::new(asset_id.clone(), Numeric::new(5, 0));
        let (asset_id, asset_value) = asset.into_key_value();
        tx.world.assets.insert(asset_id.clone(), asset_value);
        tx.world.track_asset_holder(&asset_id);

        let key: Name = "tag".parse().unwrap();
        let value = Json::from(norito::json!("owned"));
        let mut metadata = Metadata::default();
        metadata.insert(key, value);
        tx.world.asset_metadata.insert(asset_id.clone(), metadata);

        let nft_id = NftId::new(other_domain_id.clone(), "dragon".parse().unwrap());
        let nft = Nft {
            id: nft_id.clone(),
            content: Metadata::default(),
            owned_by: account_id.clone(),
        };
        let (nft_id, nft_value) = nft.into_key_value();
        tx.world.nfts.insert(nft_id.clone(), nft_value);

        tx.nexus.fees.fee_sink_account_id = authority.to_string();
        tx.nexus.staking.stake_escrow_account_id = authority.to_string();
        tx.nexus.staking.slash_sink_account_id = authority.to_string();

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(
            tx.world.assets.get(&asset_id).is_none(),
            "asset should be removed"
        );
        assert!(
            tx.world.asset_metadata.get(&asset_id).is_none(),
            "asset metadata should be removed with asset"
        );
        assert!(
            tx.world.nfts.get(&nft_id).is_none(),
            "owned NFT should be removed"
        );
    }

    #[test]
    fn unregister_account_removes_foreign_nft_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let foreign_domain_id: DomainId = "foreign.world".parse().expect("domain id");
        seed_domain(&mut state, &foreign_domain_id, &authority);

        let holder_domain: DomainId = "holders.world".parse().expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register target account");
        Register::account(NewAccount::new(holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register holder account");

        let nft_id = NftId::new(foreign_domain_id, "dragon".parse().unwrap());
        let nft = Nft {
            id: nft_id.clone(),
            content: Metadata::default(),
            owned_by: account_id.clone(),
        };
        let (nft_id, nft_value) = nft.into_key_value();
        tx.world.nfts.insert(nft_id.clone(), nft_value);

        let permission: Permission = iroha_executor_data_model::permission::nft::CanTransferNft {
            nft: nft_id.clone(),
        }
        .into();
        Grant::account_permission(permission.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to holder");

        let role_id: RoleId = "NFT_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to role");

        assert!(
            tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder should have permission before unregister"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            role.permissions().any(|perm| perm == &permission),
            "role should include permission before unregister"
        );

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(
            tx.world.nfts.get(&nft_id).is_none(),
            "foreign-domain NFT owned by removed account should be removed"
        );
        assert!(
            !tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder permission should be removed"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role.permissions().any(|perm| perm == &permission),
            "role permission should be removed"
        );
        assert!(
            !role.permission_epochs().contains_key(&permission),
            "permission epochs should be pruned"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_owns_domain() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let external_domain: DomainId = "external.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        seed_domain(&mut state, &external_domain, &account_id);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account owning a domain must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("owns domain"),
            "error should explain ownership conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_removes_associated_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let holder_domain: DomainId = "holders.world".parse().expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register target account");
        Register::account(NewAccount::new(holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register holder account");

        let permission: Permission =
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                account: account_id.clone(),
            }
            .into();
        assert!(
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata::try_from(
                &permission
            )
            .is_ok(),
            "permission should decode as CanModifyAccountMetadata"
        );
        Grant::account_permission(permission.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to holder");

        let role_id: RoleId = "ACCOUNT_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to role");

        assert!(
            tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder should have permission before unregister"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            role.permissions().any(|perm| perm == &permission),
            "role should include permission before unregister"
        );

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(
            !tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder permission should be removed"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role.permissions().any(|perm| perm == &permission),
            "role permission should be removed"
        );
        assert!(
            !role.permission_epochs().contains_key(&permission),
            "permission epochs should be pruned"
        );
    }

    #[test]
    fn unregister_account_removes_citizen_service_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let holder_domain: DomainId = "holders.world".parse().expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register target account");
        Register::account(NewAccount::new(holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register holder account");

        let permission: Permission =
            iroha_executor_data_model::permission::governance::CanRecordCitizenService {
                owner: account_id.clone(),
            }
            .into();
        Grant::account_permission(permission.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to holder");

        let role_id: RoleId = "CITIZEN_SERVICE_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to role");

        assert!(
            tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder should have permission before unregister"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            role.permissions().any(|perm| perm == &permission),
            "role should include permission before unregister"
        );

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(
            !tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder permission should be removed"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role.permissions().any(|perm| perm == &permission),
            "role permission should be removed"
        );
        assert!(
            !role.permission_epochs().contains_key(&permission),
            "permission epochs should be pruned"
        );
    }

    #[test]
    fn unregister_account_removes_permissions_for_deleted_account() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let retained_domain: DomainId = "retained.world".parse().expect("domain id");
        seed_domain(&mut state, &retained_domain, &authority);

        let holder_domain: DomainId = "holders.world".parse().expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);

        let keypair = KeyPair::random();
        let target_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(target_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register target account");
        Register::account(NewAccount::new(holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register holder account");

        let permission: Permission =
            iroha_executor_data_model::permission::account::CanModifyAccountMetadata {
                account: target_id.clone(),
            }
            .into();
        Grant::account_permission(permission.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to holder");

        let role_id: RoleId = "CROSS_DOMAIN_PRESERVE".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant permission to role");

        Unregister::account(target_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister target account");

        assert!(
            !tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| perms.contains(&permission)),
            "holder permission for removed subject should be pruned"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role.permissions().any(|perm| perm == &permission),
            "role permission for removed subject should be pruned"
        );
        assert!(
            !role.permission_epochs().contains_key(&permission),
            "permission epoch should be pruned for removed subject"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_owns_asset_definition() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(domain_id.clone(), "bond".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        Register::asset_definition({
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.world
            .asset_definition_mut(&asset_def_id)
            .expect("definition exists")
            .set_owned_by(account_id.clone());

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account owning an asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("owns asset definition"),
            "error should explain ownership conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_governance_bond_escrow_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.gov.bond_escrow_account = account_id.clone();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("governance bond escrow account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("governance bond escrow account"),
            "error should explain governance bond escrow conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_governance_viral_incentive_pool_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.gov.viral_incentives.incentive_pool_account = account_id.clone();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("governance viral incentive pool account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("governance viral incentive pool account"),
            "error should explain governance viral incentive pool conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_oracle_reward_pool() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.oracle.economics.reward_pool = account_id.clone();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("oracle reward pool account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("oracle reward pool account"),
            "error should explain oracle reward-pool conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_nexus_fee_sink_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let helper_account_id = AccountId::new(KeyPair::random().public_key().clone());
        Register::account(NewAccount::new(helper_account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register helper account");
        tx.nexus.fees.fee_sink_account_id = account_id.to_string();
        tx.nexus.staking.stake_escrow_account_id = helper_account_id.to_string();
        tx.nexus.staking.slash_sink_account_id = helper_account_id.to_string();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("nexus fee sink account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus fee sink account"),
            "error should explain nexus fee-sink conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_nexus_fee_sink_account_is_configured() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let sink_domain: DomainId = "sink.world".parse().expect("domain id");
        let remove_domain: DomainId = "remove.world".parse().expect("domain id");
        seed_domain(&mut state, &sink_domain, &authority);
        seed_domain(&mut state, &remove_domain, &authority);

        let account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register configured account");
        tx.nexus.fees.fee_sink_account_id = account_id.to_string();
        tx.nexus.staking.stake_escrow_account_id = account_id.to_string();
        tx.nexus.staking.slash_sink_account_id = account_id.to_string();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("configured account must remain protected");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus fee sink account"),
            "error should explain nexus fee-sink conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "configured sink account should remain"
        );
    }

    #[test]
    fn unregister_account_rejects_when_nexus_fee_sink_literal_is_invalid() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.nexus.fees.fee_sink_account_id = "not-an-account-literal".to_owned();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("invalid nexus fee sink literal must fail closed");
        let err_string = err.to_string();
        assert!(
            err_string.contains("invalid nexus.fees.fee_sink_account_id account literal"),
            "error should explain invalid nexus fee-sink literal: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_allows_unrelated_account_when_fee_sink_is_different_account() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let remove_domain: DomainId = "remove.world".parse().expect("domain id");
        let sink_domain: DomainId = "sink.world".parse().expect("domain id");
        seed_domain(&mut state, &remove_domain, &authority);
        seed_domain(&mut state, &sink_domain, &authority);

        let remove_account_id = AccountId::new(KeyPair::random().public_key().clone());
        let sink_account_id = AccountId::new(KeyPair::random().public_key().clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(sink_account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register sink account");
        Register::account(NewAccount::new(remove_account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register removal candidate account");
        tx.nexus.fees.fee_sink_account_id = sink_account_id.to_string();

        Unregister::account(remove_account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unrelated account should not be blocked by the configured sink account");
        assert!(
            tx.world.accounts.get(&remove_account_id).is_none(),
            "removal candidate should be deleted"
        );
        assert!(
            tx.world.accounts.get(&sink_account_id).is_some(),
            "configured sink account should remain"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_nexus_staking_escrow_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let helper_account_id = AccountId::new(KeyPair::random().public_key().clone());
        Register::account(NewAccount::new(helper_account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register helper account");
        tx.nexus.fees.fee_sink_account_id = helper_account_id.to_string();
        tx.nexus.staking.stake_escrow_account_id = account_id.to_string();
        tx.nexus.staking.slash_sink_account_id = helper_account_id.to_string();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("nexus staking escrow account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus staking escrow account"),
            "error should explain nexus staking-escrow conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_nexus_staking_slash_sink_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let helper_account_id = AccountId::new(KeyPair::random().public_key().clone());
        Register::account(NewAccount::new(helper_account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register helper account");
        tx.nexus.fees.fee_sink_account_id = helper_account_id.to_string();
        tx.nexus.staking.stake_escrow_account_id = helper_account_id.to_string();
        tx.nexus.staking.slash_sink_account_id = account_id.to_string();

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("nexus staking slash sink account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus staking slash sink account"),
            "error should explain nexus staking slash-sink conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_offline_escrow_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let asset_definition_id = AssetDefinitionId::new(domain_id.clone(), "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.settlement
            .offline
            .escrow_accounts
            .insert(asset_definition_id, account_id.clone());

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("offline escrow account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("offline escrow account"),
            "error should explain offline escrow conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_content_publish_allow_account() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.content.publish_allow_accounts = vec![account_id.clone()];

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("content publish allow-list account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("content publish allow-list account"),
            "error should explain content publish-allow conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_sorafs_telemetry_submitter() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.gov.sorafs_telemetry.submitters = vec![account_id.clone()];

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("SoraFS telemetry submitter account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("SoraFS telemetry submitter"),
            "error should explain telemetry-submitter conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_configured_sorafs_provider_owner() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xD4; 32]);
        tx.gov
            .sorafs_provider_owners
            .insert(provider_id, account_id.clone());

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("configured SoraFS provider-owner account must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("configured as SoraFS provider owner"),
            "error should explain configured provider-owner conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_owns_sorafs_provider() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xB1; 32]);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world
            .provider_owners
            .insert(provider_id, account_id.clone());

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account owning a provider must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("owns SoraFS provider"),
            "error should explain ownership conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_citizenship_record() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.citizens.insert(
            account_id.clone(),
            crate::state::CitizenshipRecord::new(account_id.clone(), 100, 1),
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with citizenship record must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active citizenship record"),
            "error should explain citizenship conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_public_lane_validator_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.public_lane_validators.insert(
            (LaneId::SINGLE, account_id.clone()),
            iroha_data_model::nexus::PublicLaneValidatorRecord {
                lane_id: LaneId::SINGLE,
                validator: account_id.clone(),
                peer_id: PeerId::from(account_id.signatory().clone()),
                stake_account: account_id.clone(),
                total_stake: Numeric::new(1, 0),
                self_stake: Numeric::new(1, 0),
                metadata: Metadata::default(),
                status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                activation_epoch: Some(1),
                activation_height: Some(1),
                last_reward_epoch: None,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with validator stake state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("public-lane validator stake state"),
            "error should explain staking conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_public_lane_reward_record_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.public_lane_rewards.insert(
            (LaneId::SINGLE, 1),
            iroha_data_model::nexus::PublicLaneRewardRecord {
                lane_id: LaneId::SINGLE,
                epoch: 1,
                asset: AssetId::new(
                    AssetDefinitionId::new(domain_id.clone(), "fee".parse().unwrap()),
                    account_id.clone(),
                ),
                total_reward: Numeric::new(1, 0),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: account_id.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: Numeric::new(1, 0),
                }],
                metadata: Metadata::default(),
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with public-lane reward state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("public-lane reward ledger state"),
            "error should explain reward-state conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_is_reward_claim_asset_owner() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.public_lane_reward_claims.insert(
            (
                LaneId::SINGLE,
                authority.clone(),
                AssetId::new(
                    AssetDefinitionId::new(domain_id.clone(), "fee".parse().unwrap()),
                    account_id.clone(),
                ),
            ),
            1,
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account referenced by reward-claim asset owner must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("public-lane reward claim state"),
            "error should explain reward-claim conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_repo_agreement_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let repo_id: iroha_data_model::repo::RepoAgreementId =
            "repoguard".parse().expect("repo agreement id");
        let agreement = iroha_data_model::repo::RepoAgreement::new(
            repo_id.clone(),
            account_id.clone(),
            authority.clone(),
            iroha_data_model::repo::RepoCashLeg {
                asset_definition_id: AssetDefinitionId::new(
                    domain_id.clone(),
                    "usd".parse().unwrap(),
                ),
                quantity: Numeric::new(10, 0),
            },
            iroha_data_model::repo::RepoCollateralLeg::new(
                AssetDefinitionId::new(domain_id.clone(), "bond".parse().unwrap()),
                Numeric::new(12, 0),
            ),
            250,
            1000,
            1,
            iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
            None,
        );
        tx.world.repo_agreements.insert(repo_id, agreement);

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with repo agreement state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active repo agreement state"),
            "error should explain repo-state conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_settlement_ledger_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let settlement_id: iroha_data_model::isi::SettlementId =
            "settleguard".parse().expect("settlement id");
        let mut ledger = iroha_data_model::isi::SettlementLedger::default();
        ledger.push(iroha_data_model::isi::SettlementLedgerEntry {
            settlement_id: settlement_id.clone(),
            kind: iroha_data_model::isi::SettlementKind::Dvp,
            authority: account_id.clone(),
            plan: iroha_data_model::isi::SettlementPlan::default(),
            metadata: Metadata::default(),
            block_height: 1,
            block_hash: iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0; Hash::LENGTH]),
            ),
            executed_at_ms: 1,
            legs: vec![iroha_data_model::isi::SettlementLegSnapshot {
                role: iroha_data_model::isi::SettlementLegRole::Delivery,
                leg: iroha_data_model::isi::SettlementLeg::new(
                    AssetDefinitionId::new(domain_id.clone(), "usd".parse().unwrap()),
                    Numeric::new(1, 0),
                    account_id.clone(),
                    authority.clone(),
                ),
                committed: true,
            }],
            outcome: iroha_data_model::isi::SettlementOutcomeRecord::Success(
                iroha_data_model::isi::SettlementSuccessRecord {
                    first_committed: true,
                    second_committed: true,
                    fx_window_ms: None,
                },
            ),
        });
        tx.world.settlement_ledgers.insert(settlement_id, ledger);

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with settlement ledger state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active settlement ledger state"),
            "error should explain settlement-state conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_oracle_feed_provider_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let mut feed = iroha_data_model::oracle::kits::price_xor_usd().feed_config;
        feed.providers = vec![account_id.clone()];
        tx.world.oracle_feeds.insert(feed.feed_id.clone(), feed);

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with oracle provider state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active oracle feed provider state"),
            "error should explain oracle-state conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_oracle_feed_history_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let feed = iroha_data_model::oracle::kits::price_xor_usd().feed_config;
        let feed_id = feed.feed_id.clone();
        tx.world.oracle_history.insert(
            feed_id.clone(),
            vec![iroha_data_model::events::data::oracle::FeedEventRecord {
                event: iroha_data_model::oracle::FeedEvent {
                    feed_id: feed_id.clone(),
                    feed_config_version: feed.feed_config_version,
                    slot: 1,
                    outcome: iroha_data_model::oracle::FeedEventOutcome::Success(
                        iroha_data_model::oracle::FeedSuccess {
                            value: iroha_data_model::oracle::ObservationValue::new(1_000, 2),
                            entries: vec![iroha_data_model::oracle::ReportEntry {
                                oracle_id: account_id.clone(),
                                observation_hash: iroha_crypto::HashOf::from_untyped_unchecked(
                                    Hash::new(b"oracle-history-account-guard"),
                                ),
                                value: iroha_data_model::oracle::ObservationValue::new(1_000, 2),
                                outlier: false,
                            }],
                        },
                    ),
                },
                evidence_hashes: Vec::new(),
            }],
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with oracle history state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active oracle feed history state"),
            "error should explain oracle-history conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_offline_allowance_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let definition_id = AssetDefinitionId::new(domain_id.clone(), "coin".parse().unwrap());
        let allowance = OfflineAllowanceCommitment::new(
            AssetId::new(definition_id, account_id.clone()),
            Numeric::new(10, 0),
            vec![0xAB],
        );
        let certificate = OfflineWalletCertificate::new(
            account_id.clone(),
            authority.clone(),
            allowance,
            KeyPair::random().public_key().clone(),
            Vec::new(),
            1,
            2,
            OfflineWalletPolicy::new(Numeric::new(10, 0), Numeric::new(5, 0), 2),
            Signature::from_bytes(&[0; 64]),
            Metadata::default(),
            None,
            None,
            None,
        );
        tx.world.offline_allowances.insert(
            Hash::new(b"offline-allowance-account-guard"),
            OfflineAllowanceRecord {
                certificate,
                current_commitment: vec![0xAB],
                registered_at_ms: 1,
                remaining_amount: Numeric::new(10, 0),
                counter_state: OfflineCounterState::default(),
                verdict_id: None,
                attestation_nonce: None,
                refresh_at_ms: None,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with offline allowance state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active offline allowance state"),
            "error should explain offline allowance conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_offline_verdict_revocation_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let verdict_id = Hash::new(b"offline-verdict-account-guard");
        tx.world.offline_verdict_revocations.insert(
            verdict_id,
            iroha_data_model::offline::OfflineVerdictRevocation {
                verdict_id,
                issuer: account_id.clone(),
                revoked_at_ms: 1,
                reason: iroha_data_model::offline::OfflineVerdictRevocationReason::IssuerRequest,
                note: None,
                metadata: Metadata::default(),
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with offline verdict revocation state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active offline verdict revocation state"),
            "error should explain offline verdict revocation conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_governance_proposal_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let proposal_id = [0xA5; 32];
        let kind = iroha_data_model::governance::types::ProposalKind::DeployContract(
            iroha_data_model::governance::types::DeployContractProposal {
                namespace: "gov".to_string(),
                contract_id: "proposal-guard".to_string(),
                code_hash_hex: iroha_data_model::governance::types::ContractCodeHash::new(
                    [0x11; 32],
                ),
                abi_hash_hex: iroha_data_model::governance::types::ContractAbiHash::new([0x22; 32]),
                abi_version: iroha_data_model::governance::types::AbiVersion::new(1),
                manifest_provenance: None,
            },
        );
        tx.world.governance_proposals.insert(
            proposal_id,
            crate::state::GovernanceProposalRecord {
                proposer: account_id.clone(),
                kind,
                created_height: 1,
                status: crate::state::GovernanceProposalStatus::Proposed,
                pipeline: crate::state::GovernancePipeline::default(),
                parliament_snapshot: None,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with governance proposal state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active governance proposal state"),
            "error should explain governance proposal conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_content_bundle_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let bundle_id = Hash::new(b"content-bundle-account-guard");
        let stripe_layout = iroha_data_model::da::prelude::DaStripeLayout::default();
        let manifest = iroha_data_model::content::ContentBundleManifest {
            bundle_id,
            index_hash: [0x44; 32],
            dataspace: DataSpaceId::GLOBAL,
            lane: LaneId::SINGLE,
            blob_class: iroha_data_model::da::types::BlobClass::GovernanceArtifact,
            retention: iroha_data_model::da::types::RetentionPolicy::default(),
            cache: iroha_data_model::content::ContentCachePolicy {
                max_age_seconds: 60,
                immutable: false,
            },
            auth: iroha_data_model::content::ContentAuthMode::Public,
            stripe_layout,
            mime_overrides: std::collections::BTreeMap::new(),
        };
        tx.world.content_bundles.insert(
            bundle_id,
            iroha_data_model::content::ContentBundleRecord {
                bundle_id,
                manifest,
                total_bytes: 0,
                chunk_size: 1,
                chunk_hashes: Vec::new(),
                chunk_root: [0; 32],
                stripe_layout,
                pdp_commitment: None,
                files: Vec::new(),
                created_by: account_id.clone(),
                created_height: 1,
                expires_at_height: None,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with content bundle state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("content bundle state"),
            "error should explain content-bundle conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_runtime_upgrade_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let manifest = iroha_data_model::runtime::RuntimeUpgradeManifest {
            name: "runtime-guard".to_string(),
            description: "guard".to_string(),
            abi_version: 1,
            abi_hash: [0x51; 32],
            added_syscalls: Vec::new(),
            added_pointer_types: Vec::new(),
            start_height: 1,
            end_height: 2,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        };
        let upgrade_id = manifest.id();
        tx.world.runtime_upgrades.insert(
            upgrade_id,
            iroha_data_model::runtime::RuntimeUpgradeRecord {
                manifest,
                status: iroha_data_model::runtime::RuntimeUpgradeStatus::Proposed,
                proposer: account_id.clone(),
                created_height: 1,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with runtime upgrade state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active runtime upgrade proposal state"),
            "error should explain runtime-upgrade conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_viral_escrow_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let binding_digest = Hash::new(b"viral-escrow-account-guard");
        tx.world.viral_escrows.insert(
            binding_digest,
            iroha_data_model::social::ViralEscrowRecord {
                binding_hash: iroha_data_model::oracle::KeyedHash {
                    pepper_id: "pepper".to_string(),
                    digest: binding_digest,
                },
                sender: account_id.clone(),
                amount: Numeric::new(1, 0),
                created_at_ms: 1,
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with viral escrow state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active viral escrow state"),
            "error should explain viral-escrow conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_sorafs_pin_manifest_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let digest = iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xAB; 32]);
        tx.world.pin_manifests.insert(
            digest,
            iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
                digest,
                iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
                    profile_id: 1,
                    namespace: "sorafs".to_string(),
                    name: "sf1".to_string(),
                    semver: "1.0.0".to_string(),
                    multihash_code: 0x1E,
                },
                [0xCD; 32],
                iroha_data_model::sorafs::pin_registry::PinPolicy::default(),
                account_id.clone(),
                1,
                None,
                None,
                Metadata::default(),
            ),
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with SoraFS pin manifest state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active SoraFS pin manifest state"),
            "error should explain SoraFS pin-manifest conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_has_da_pin_intent_owner_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let ticket_id = iroha_data_model::da::types::StorageTicketId::new([0xD1; 32]);
        tx.world.da_pin_intents_by_ticket.insert(
            ticket_id,
            iroha_data_model::da::pin_intent::DaPinIntentWithLocation {
                intent: iroha_data_model::da::pin_intent::DaPinIntent {
                    lane_id: LaneId::new(1),
                    epoch: 1,
                    sequence: 1,
                    storage_ticket: ticket_id,
                    manifest_hash: iroha_data_model::sorafs::pin_registry::ManifestDigest::new(
                        [0xE2; 32],
                    ),
                    alias: None,
                    owner: Some(account_id.clone()),
                },
                location: iroha_data_model::da::commitment::DaCommitmentLocation {
                    block_height: 1,
                    index_in_bundle: 0,
                },
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account with DA pin intent owner state must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("active DA pin intent owner state"),
            "error should explain DA pin intent owner conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_account_allows_peer_based_lane_relay_emergency_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let peer = PeerId::new(
            KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        tx.world.lane_relay_emergency_validators.insert(
            LaneId::new(0),
            iroha_data_model::nexus::LaneRelayEmergencyValidatorSet {
                peers: vec![peer],
                expires_at_height: 10,
                metadata: Metadata::default(),
            },
        );

        assert!(
            Unregister::account(account_id.clone())
                .execute(&authority, &mut tx)
                .is_ok(),
            "peer-based emergency override state should not block account unregister"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_none(),
            "account should be removed when lane-relay override stores peers instead"
        );
    }

    #[test]
    fn unregister_account_rejects_when_account_in_governance_parliament_snapshot_state() {
        let mut state = test_state();
        let domain_id: DomainId = "owner.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let proposal_id = [0xC5; 32];
        let kind = iroha_data_model::governance::types::ProposalKind::DeployContract(
            iroha_data_model::governance::types::DeployContractProposal {
                namespace: "gov".to_string(),
                contract_id: "snapshot-guard".to_string(),
                code_hash_hex: iroha_data_model::governance::types::ContractCodeHash::new(
                    [0x51; 32],
                ),
                abi_hash_hex: iroha_data_model::governance::types::ContractAbiHash::new([0x61; 32]),
                abi_version: iroha_data_model::governance::types::AbiVersion::new(1),
                manifest_provenance: None,
            },
        );
        let roster = iroha_data_model::governance::types::ParliamentRoster {
            body: iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
            epoch: 1,
            members: vec![account_id.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidate_count: 0,
            derived_by: Default::default(),
        };
        tx.world.governance_proposals.insert(
            proposal_id,
            crate::state::GovernanceProposalRecord {
                proposer: authority.clone(),
                kind,
                created_height: 1,
                status: crate::state::GovernanceProposalStatus::Proposed,
                pipeline: crate::state::GovernancePipeline::default(),
                parliament_snapshot: Some(crate::state::GovernanceParliamentSnapshot {
                    selection_epoch: 1,
                    beacon: [0x71; 32],
                    roster_root: [0x72; 32],
                    bodies: iroha_data_model::governance::types::ParliamentBodies {
                        selection_epoch: 1,
                        rosters: std::collections::BTreeMap::from([(
                            iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
                            roster,
                        )]),
                    },
                }),
            },
        );

        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("account in governance parliament snapshot must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("governance proposal parliament snapshot state"),
            "error should explain governance parliament snapshot conflict: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }

    #[test]
    fn space_directory_events_drive_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.events".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::events"));
        let dataspace = DataSpaceId::new(33);
        let manifest_hash = seed_manifest_record(&mut state.world, uaid, dataspace, |_| {});

        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");

        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "inactive manifest should not bind accounts"
        );

        tx.world
            .emit_events(Some(SpaceDirectoryEvent::ManifestActivated(
                SpaceDirectoryManifestActivated {
                    dataspace,
                    uaid,
                    manifest_hash,
                    activation_epoch: 10,
                    expiry_epoch: None,
                },
            )));

        let bindings = tx
            .world
            .uaid_dataspaces
            .get(&uaid)
            .expect("bindings must exist after activation");
        assert!(
            bindings
                .iter()
                .any(|(id, accounts)| *id == dataspace && accounts.contains(&account_id))
        );
        let manifest_record = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .and_then(|set| set.get(&dataspace))
            .expect("manifest record present");
        assert_eq!(
            manifest_record.lifecycle.activated_epoch,
            Some(10),
            "activation epoch recorded"
        );

        tx.world
            .emit_events(Some(SpaceDirectoryEvent::ManifestRevoked(
                SpaceDirectoryManifestRevoked {
                    dataspace,
                    uaid,
                    manifest_hash,
                    revoked_epoch: 25,
                    reason: Some("operator request".to_string()),
                },
            )));

        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "bindings cleared after revocation"
        );
        let manifest_record = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .and_then(|set| set.get(&dataspace))
            .expect("manifest record still present");
        assert!(
            manifest_record.lifecycle.revocation.is_some(),
            "revocation metadata recorded"
        );
        assert_eq!(
            manifest_record.lifecycle.revocation.as_ref().unwrap().epoch,
            25
        );

        tx.apply();
        block.commit().unwrap();
    }

    #[test]
    fn register_asset_definition_auto_creates_offline_escrow_account() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "offline".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_name: Name = "usd".parse().expect("asset name");
        let definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
        let mut metadata = Metadata::default();
        metadata.insert(
            OFFLINE_ASSET_ENABLED_METADATA_KEY
                .parse()
                .expect("metadata key"),
            Json::new(true),
        );

        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "USD".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata,
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let escrow_account = tx
            .settlement
            .offline
            .escrow_accounts
            .get(&definition_id)
            .expect("escrow mapping created")
            .clone();
        let expected = super::isi::offline_escrow_account_id(tx.chain_id(), &definition_id);
        assert_eq!(escrow_account, expected);
        assert!(
            tx.world.account(&escrow_account).is_ok(),
            "escrow account should exist"
        );
    }

    #[test]
    fn register_asset_definition_without_offline_flag_skips_escrow() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "offline2".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_name: Name = "eur".parse().expect("asset name");
        let definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "EUR".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&definition_id)
                .is_none(),
            "escrow mapping should not be created"
        );
    }

    #[test]
    fn register_asset_definition_rejects_missing_explicit_name() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "missing-name".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let definition_id =
            AssetDefinitionId::new(domain_id, "usd".parse().expect("asset definition name"));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = Register::asset_definition(AssetDefinition::numeric(definition_id))
            .execute(&authority, &mut tx)
            .expect_err("registration without explicit name must fail");
        assert!(
            err.to_string().contains("invalid asset definition name"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_asset_definition_rejects_duplicate_alias() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "alias-test".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let id1 = AssetDefinitionId::new(domain_id.clone(), "usd1".parse().expect("asset name"));
        let id2 = AssetDefinitionId::new(domain_id, "usd2".parse().expect("asset name"));
        let alias: AssetDefinitionAlias = "USD#issuer.main".parse().expect("alias");

        let first = NewAssetDefinition {
            id: id1,
            name: "USD".to_owned(),
            description: None,
            alias: Some(alias.clone()),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };
        let second = NewAssetDefinition {
            id: id2,
            name: "USD".to_owned(),
            description: None,
            alias: Some(alias),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition(first)
            .execute(&authority, &mut tx)
            .expect("first registration should succeed");

        let err = Register::asset_definition(second)
            .execute(&authority, &mut tx)
            .expect_err("duplicate alias must fail");
        assert!(
            err.to_string().contains("already bound"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_asset_definition_alias_updates_world_indexes() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "alias-update".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let definition_id = AssetDefinitionId::new(domain_id, "usd".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "USD".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let alias: AssetDefinitionAlias = "USD#issuer.main".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), Some(11_000))
            .execute(&authority, &mut tx)
            .expect("bind alias");

        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id),
            "alias index must resolve to definition id"
        );
        let binding = tx
            .world
            .asset_definition_alias_bindings
            .get(&definition_id)
            .expect("binding should be present");
        assert_eq!(binding.alias, alias);
        assert_eq!(binding.lease_expiry_ms, Some(11_000));
        assert_eq!(
            binding.grace_until_ms,
            Some(11_000 + 369u64 * 60 * 60 * 1_000)
        );
        let updated = tx
            .world
            .asset_definition(&definition_id)
            .expect("definition should exist");
        assert_eq!(updated.alias().as_ref(), Some(&binding.alias));
    }

    #[test]
    fn set_asset_definition_alias_clear_removes_indexes() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "alias-clear".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let definition_id = AssetDefinitionId::new(domain_id, "usd".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "USD".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let alias: AssetDefinitionAlias = "USD#issuer.main".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("bind alias");
        SetAssetDefinitionAlias::clear(definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("clear alias");

        assert!(
            tx.world.asset_definition_aliases.get(&alias).is_none(),
            "alias index should be removed"
        );
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none(),
            "binding index should be removed"
        );
        let updated = tx
            .world
            .asset_definition(&definition_id)
            .expect("definition should exist");
        assert!(
            updated.alias().is_none(),
            "definition alias should be cleared"
        );
    }

    #[test]
    fn set_contract_alias_updates_world_indexes() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address =
            ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL).expect("address");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.contract_instances.insert(
            ("universal".to_owned(), contract_address.to_string()),
            Hash::new("contract-alias"),
        );

        let alias: ContractAlias = "router::universal".parse().expect("alias");
        SetContractAlias::bind(contract_address.clone(), alias.clone(), Some(11_000))
            .execute(&authority, &mut tx)
            .expect("bind contract alias");

        assert_eq!(
            tx.world.contract_aliases.get(&alias),
            Some(&contract_address),
            "alias index must resolve to contract address"
        );
        let binding = tx
            .world
            .contract_alias_bindings
            .get(&contract_address)
            .expect("binding should be present");
        assert_eq!(binding.alias, alias);
        assert_eq!(binding.lease_expiry_ms, Some(11_000));
        assert_eq!(
            binding.grace_until_ms,
            Some(11_000 + 369u64 * 60 * 60 * 1_000)
        );
    }

    #[test]
    fn set_contract_alias_rejects_account_alias_collision() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address =
            ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL).expect("address");
        let label = AccountAlias::domainless("router".parse().expect("label"), DataSpaceId::GLOBAL);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.contract_instances.insert(
            ("universal".to_owned(), contract_address.to_string()),
            Hash::new("contract-alias"),
        );
        seed_account_alias_lease(&mut tx, &authority, &label);

        let err = SetContractAlias::bind(
            contract_address,
            "router::universal".parse().expect("alias"),
            Some(11_000),
        )
        .execute(&authority, &mut tx)
        .expect_err("account alias collision must fail");
        assert!(
            err.to_string()
                .contains("collides with an active account alias"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn bind_account_alias_rejects_contract_alias_collision() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address =
            ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL).expect("address");
        let label = AccountAlias::domainless("router".parse().expect("label"), DataSpaceId::GLOBAL);
        let account = Account {
            id: authority.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = account.into_key_value();
        state.world.accounts.insert(account_id, account_value);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::GLOBAL),
            }),
        );
        seed_account_alias_lease(&mut tx, &authority, &label);
        tx.world.contract_instances.insert(
            ("universal".to_owned(), contract_address.clone().to_string()),
            Hash::new("contract-alias"),
        );
        tx.world
            .bind_contract_alias(
                &contract_address,
                "router::universal".parse().expect("alias"),
                Some(11_000),
                Some(11_000 + 369u64 * 60 * 60 * 1_000),
                10_000,
            )
            .expect("seed contract alias");

        let err = SetAccountAliasBinding {
            account: authority.clone(),
            alias: Some(label),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("contract alias collision must fail");
        assert!(
            err.to_string()
                .contains("collides with an active contract alias"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_asset_definition_alias_grace_window_sweeps_after_expiry() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "alias-grace".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let definition_id = AssetDefinitionId::new(domain_id, "usd".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "USD".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let alias: AssetDefinitionAlias = "USD#issuer.main".parse().expect("alias");
        let lease_expiry = 11_000_u64;
        let grace_until = lease_expiry + 369u64 * 60 * 60 * 1_000;
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), Some(lease_expiry))
            .execute(&authority, &mut tx)
            .expect("bind alias");

        let swept_at_grace = tx.world.sweep_expired_asset_definition_aliases(grace_until);
        assert!(
            swept_at_grace.is_empty(),
            "alias must remain bound while still inside grace window"
        );
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id),
            "alias should still resolve during grace window"
        );

        let swept_after_grace = tx
            .world
            .sweep_expired_asset_definition_aliases(grace_until + 1);
        assert_eq!(
            swept_after_grace,
            vec![definition_id.clone()],
            "sweep should unbind alias after grace window"
        );
        assert!(
            tx.world.asset_definition_aliases.get(&alias).is_none(),
            "alias index should be removed after grace expiry"
        );
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none(),
            "binding record should be removed after grace expiry"
        );
    }

    #[test]
    fn set_asset_definition_alias_rejects_expired_lease_at_bind_time() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "alias-past-expiry".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let definition_id = AssetDefinitionId::new(domain_id, "usd".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "USD".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let alias: AssetDefinitionAlias = "USD#issuer.main".parse().expect("alias");
        let err = SetAssetDefinitionAlias::bind(definition_id, alias, Some(10_000))
            .execute(&authority, &mut tx)
            .expect_err("expired lease should be rejected");
        let debug = format!("{err:?}");

        assert!(
            debug.contains("lease_expiry_ms must be greater than the current block timestamp"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn set_asset_definition_offline_enabled_creates_escrow_account() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "offline3".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_name: Name = "gbp".parse().expect("asset name");
        let definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "GBP".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&definition_id)
                .is_none(),
            "escrow mapping should not be created before metadata update"
        );

        SetKeyValue::asset_definition(
            definition_id.clone(),
            OFFLINE_ASSET_ENABLED_METADATA_KEY
                .parse()
                .expect("metadata key"),
            Json::new(true),
        )
        .execute(&authority, &mut tx)
        .expect("set offline.enabled metadata");

        let escrow_account = tx
            .settlement
            .offline
            .escrow_accounts
            .get(&definition_id)
            .expect("escrow mapping created")
            .clone();
        let expected = super::isi::offline_escrow_account_id(tx.chain_id(), &definition_id);
        assert_eq!(escrow_account, expected);
        assert!(
            tx.world.account(&escrow_account).is_ok(),
            "escrow account should exist"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_has_repo_agreement_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        let counterparty_domain: DomainId = "counter.guard".parse().expect("counterparty domain");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &counterparty_domain, &authority);

        let initiator = AccountId::new(KeyPair::random().public_key().clone());
        let counterparty = AccountId::new(KeyPair::random().public_key().clone());
        let asset_definition_id =
            AssetDefinitionId::new(asset_domain.clone(), "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");

        let repo_id: iroha_data_model::repo::RepoAgreementId =
            "repo_asset_guard".parse().expect("repo agreement id");
        tx.world.repo_agreements.insert(
            repo_id.clone(),
            iroha_data_model::repo::RepoAgreement::new(
                repo_id,
                initiator,
                counterparty,
                iroha_data_model::repo::RepoCashLeg {
                    asset_definition_id: asset_definition_id.clone(),
                    quantity: Numeric::new(10, 0),
                },
                iroha_data_model::repo::RepoCollateralLeg::new(
                    AssetDefinitionId::new(asset_domain, "bond".parse().unwrap()),
                    Numeric::new(12, 0),
                ),
                250,
                1_000,
                1,
                iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
                None,
            ),
        );

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("asset definition with repo agreement reference must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("repo agreement state"),
            "error should explain repo agreement conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_governance_voting_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.gov.voting_asset_id = asset_definition_id.clone();

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("governance voting asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("governance voting asset definition"),
            "error should explain governance voting-asset conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_governance_viral_reward_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.gov.viral_incentives.reward_asset_definition_id = asset_definition_id.clone();

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("governance viral reward asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("governance viral reward asset definition"),
            "error should explain governance viral reward-asset conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_oracle_reward_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.oracle.economics.reward_asset = asset_definition_id.clone();

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("oracle reward asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("oracle reward asset definition"),
            "error should explain oracle reward-asset conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_nexus_fee_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.nexus.fees.fee_asset_id = asset_definition_id.to_string();

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("nexus fee asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus fee asset definition"),
            "error should explain nexus fee-asset conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_nexus_staking_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.nexus.staking.stake_asset_id = asset_definition_id.to_string();

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("nexus staking asset definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("nexus staking asset definition"),
            "error should explain nexus staking-asset conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_settlement_repo_eligible_collateral()
    {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.settlement.repo.eligible_collateral = vec![asset_definition_id.clone()];

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("repo eligible collateral definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("settlement repo eligible collateral"),
            "error should explain settlement repo eligible-collateral conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_settlement_repo_substitution_entry() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let base_definition_id =
            AssetDefinitionId::new(asset_domain.clone(), "base".parse().unwrap());
        let asset_definition_id = AssetDefinitionId::new(asset_domain, "sub".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = base_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register base asset definition");
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register substitute asset definition");
        tx.settlement
            .repo
            .collateral_substitution_matrix
            .insert(base_definition_id, vec![asset_definition_id.clone()]);

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("repo substitution definition must not be unregistered");
        let err_string = err.to_string();
        assert!(
            err_string.contains("collateral substitution matrix"),
            "error should explain settlement repo substitution conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_removes_associated_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        let holder_domain: DomainId = "holder.guard".parse().expect("holder domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &holder_domain, &authority);

        let asset_definition_id =
            AssetDefinitionId::new(asset_domain.clone(), "usd".parse().unwrap());
        let asset_account = AccountId::new(KeyPair::random().public_key().clone());
        let holder_id = AccountId::new(KeyPair::random().public_key().clone());
        let asset_id = AssetId::new(asset_definition_id.clone(), asset_account.clone());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::account(NewAccount::new(asset_account.clone()))
            .execute(&authority, &mut tx)
            .expect("register asset account");
        Register::account(NewAccount::new(holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register holder account");
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");

        let permission_with_definition: Permission = iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        }
        .into();
        let permission_with_asset: Permission =
            iroha_executor_data_model::permission::asset::CanModifyAssetMetadata {
                asset: asset_id,
            }
            .into();
        Grant::account_permission(permission_with_definition.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant definition permission to holder");
        Grant::account_permission(permission_with_asset.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant asset permission to holder");

        let role_id: RoleId = "ASSET_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission_with_definition.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant definition permission to role");
        Grant::role_permission(permission_with_asset.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant asset permission to role");

        assert!(
            tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| {
                    perms.contains(&permission_with_definition)
                        && perms.contains(&permission_with_asset)
                }),
            "holder should have permissions before unregister"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            role.permissions()
                .any(|perm| perm == &permission_with_definition),
            "role should include definition permission before unregister"
        );
        assert!(
            role.permissions()
                .any(|perm| perm == &permission_with_asset),
            "role should include asset permission before unregister"
        );

        Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister asset definition");

        assert!(
            !tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| {
                    perms.contains(&permission_with_definition)
                        || perms.contains(&permission_with_asset)
                }),
            "holder permissions should be removed"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role
                .permissions()
                .any(|perm| perm == &permission_with_definition),
            "role definition permission should be removed"
        );
        assert!(
            !role
                .permissions()
                .any(|perm| perm == &permission_with_asset),
            "role asset permission should be removed"
        );
        assert!(
            !role
                .permission_epochs()
                .contains_key(&permission_with_definition),
            "definition permission epoch should be pruned"
        );
        assert!(
            !role
                .permission_epochs()
                .contains_key(&permission_with_asset),
            "asset permission epoch should be pruned"
        );
    }

    #[test]
    fn unregister_asset_definition_removes_offline_escrow_mapping() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);

        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.settlement
            .offline
            .escrow_accounts
            .insert(asset_definition_id.clone(), ALICE_ID.clone());
        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&asset_definition_id)
                .is_some(),
            "escrow mapping should exist before unregister"
        );

        Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister asset definition");

        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&asset_definition_id)
                .is_none(),
            "escrow mapping should be removed with asset definition"
        );
    }

    #[test]
    fn unregister_asset_definition_rejects_when_definition_has_settlement_ledger_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = "asset.guard".parse().expect("asset domain id");
        let counterparty_domain: DomainId = "counter.guard".parse().expect("counterparty domain");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &counterparty_domain, &authority);

        let from = AccountId::new(KeyPair::random().public_key().clone());
        let to = AccountId::new(KeyPair::random().public_key().clone());
        let asset_definition_id = AssetDefinitionId::new(asset_domain, "usd".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");

        let settlement_id: iroha_data_model::isi::SettlementId =
            "settle_asset_guard".parse().expect("settlement id");
        let mut ledger = iroha_data_model::isi::SettlementLedger::default();
        ledger.push(iroha_data_model::isi::SettlementLedgerEntry {
            settlement_id: settlement_id.clone(),
            kind: iroha_data_model::isi::SettlementKind::Dvp,
            authority: from.clone(),
            plan: iroha_data_model::isi::SettlementPlan::default(),
            metadata: Metadata::default(),
            block_height: 1,
            block_hash: iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0; Hash::LENGTH]),
            ),
            executed_at_ms: 1,
            legs: vec![iroha_data_model::isi::SettlementLegSnapshot {
                role: iroha_data_model::isi::SettlementLegRole::Delivery,
                leg: iroha_data_model::isi::SettlementLeg::new(
                    asset_definition_id.clone(),
                    Numeric::new(1, 0),
                    from,
                    to,
                ),
                committed: true,
            }],
            outcome: iroha_data_model::isi::SettlementOutcomeRecord::Success(
                iroha_data_model::isi::SettlementSuccessRecord {
                    first_committed: true,
                    second_committed: true,
                    fx_window_ms: None,
                },
            ),
        });
        tx.world.settlement_ledgers.insert(settlement_id, ledger);

        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err(
                "asset definition with settlement ledger reference must not be unregistered",
            );
        let err_string = err.to_string();
        assert!(
            err_string.contains("settlement ledger state"),
            "error should explain settlement ledger conflict: {err_string}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "asset definition should remain after rejected unregister"
        );
    }

    #[test]
    fn unregister_asset_definition_removes_confidential_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "zk.guard".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_definition_id = AssetDefinitionId::new(domain_id, "shield".parse().unwrap());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.world.zk_assets.insert(
            asset_definition_id.clone(),
            crate::state::ZkAssetState::default(),
        );

        Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister asset definition");

        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_none(),
            "asset definition should be removed"
        );
        assert!(
            tx.world.zk_assets.get(&asset_definition_id).is_none(),
            "confidential state should be removed with asset definition"
        );
    }
}
