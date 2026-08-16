//! This module contains [`Domain`] structure and related implementations and trait implementations.
use super::super::isi::prelude::*;
use eyre::Result;
use iroha_data_model::{account::rekey::AccountRekeyRecord, prelude::*, query::error::FindError};
use iroha_telemetry::metrics;
/// ISI module contains all instructions related to domains:
/// - creating/changing assets
/// - registering/unregistering accounts
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use super::*;
    use crate::{
        alias::{
            authority_can_manage_account_alias, authority_can_manage_account_alias_scope,
            authority_can_manage_asset_definition_alias,
        },
        state::{
            WorldReadOnly as _, account_label_is_pii, public_lane_reward_record_matches_key,
            public_lane_stake_share_matches_key, public_lane_validator_record_matches_key,
        },
    };
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::{
        IntoKeyValue, NetworkId,
        account::{
            AccountController,
            curve::{CurveId, CurveRegistryError},
        },
        alias_setup::{AccountAliasRoleV1, AliasAccountIntentV1},
        asset::AssetBalancePolicy,
        asset::definition::{
            validate_asset_alias_against_names, validate_asset_description, validate_asset_name,
        },
        governance::types::ProposalKind,
        isi::error::{InstructionExecutionError, InvalidParameterError, RepetitionError},
        metadata::Metadata,
        name::Name,
        nexus::{DataSpaceCatalog, DataSpaceId, LaneVisibility},
        validation_fee::ValidationFeePlainElectorateRulesV1,
    };
    use iroha_logger::prelude::*;
    use std::{
        collections::{BTreeSet, btree_map::Entry},
        str::FromStr,
    };
    /// Alias grace window after lease expiry (369 hours).
    const ASSET_ALIAS_GRACE_MS: u64 = 369u64 * 60 * 60 * 1_000;
    fn retained_validation_fee_plain_electorate_rules(
        proposal: &crate::state::GovernanceProposalRecord,
    ) -> Option<&ValidationFeePlainElectorateRulesV1> {
        if !matches!(
            proposal.status,
            crate::state::GovernanceProposalStatus::Proposed
                | crate::state::GovernanceProposalStatus::Approved
        ) {
            return None;
        }
        match &proposal.kind {
            ProposalKind::ValidationFeePolicy(payload) => Some(&payload.plain_electorate_rules),
            ProposalKind::ValidationFeePayoutLifecycle(payload) => {
                Some(&payload.plain_electorate_rules)
            }
            ProposalKind::DeployContract(_)
            | ProposalKind::RuntimeUpgrade(_)
            | ProposalKind::SccpRouteGovernance(_)
            | ProposalKind::SorafsProviderGovernance(_)
            | ProposalKind::MusubiRegistryGovernance(_) => None,
        }
    }
    include!("domain/asset_alias_scope.rs");
    fn ensure_global_asset_definition_registered_on_authoritative_route(
        state_transaction: &StateTransaction<'_, '_>,
        definition: &AssetDefinition,
    ) -> Result<(), InstructionExecutionError> {
        if state_transaction.replay_compatibility {
            return Ok(());
        }
        ensure_global_asset_definition_home_is_public_or_universal(state_transaction, definition)?;
        let home_dataspace = asset_definition_home_dataspace(state_transaction, definition)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "asset definition {} owning domain has no active dataspace",
                        definition.id()
                    )
                    .into(),
                )
            })?;
        let route_dataspace = state_transaction
            .current_dataspace_id
            .or(state_transaction.world.current_dataspace_id);
        if let Some(route_dataspace) = route_dataspace
            && route_dataspace != home_dataspace
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "global asset definition {} must be registered on its authoritative dataspace {}; current route is {}",
                    definition.id(),
                    home_dataspace.as_u64(),
                    route_dataspace.as_u64()
                )
                .into(),
            ));
        }
        Ok(())
    }
    /// Restore or retarget the continuity record for an already validated alias binding.
    ///
    /// # Errors
    ///
    /// Returns an invariant violation when persisted continuity state is malformed.
    pub(crate) fn upsert_account_rekey_record(
        state_transaction: &mut StateTransaction<'_, '_>,
        label: &AccountAlias,
        account: &AccountId,
    ) -> Result<(), InstructionExecutionError> {
        let record = match state_transaction
            .world
            .account_rekey_records
            .get(label)
            .cloned()
        {
            Some(record) => {
                if &record.label != label {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "cannot reassign an account alias with a mismatched continuity record"
                            .into(),
                    ));
                }
                record
                    .reassign_alias_to_account(account.clone())
                    .map_err(|error| {
                        InstructionExecutionError::InvariantViolation(
                            format!("cannot reassign malformed account alias history: {error}")
                                .into(),
                        )
                    })?
            }
            None => AccountRekeyRecord::new(label.clone(), account.clone()),
        };
        state_transaction
            .world
            .account_rekey_records
            .insert(label.clone(), record);
        Ok(())
    }
    /// Restore only the missing binding/index state of an exact account-alias setup intent.
    ///
    /// The declarative classifier must run immediately before this helper. The checks here are
    /// repeated defensively so repair can never overwrite a different binding or primary alias.
    pub(crate) fn repair_account_alias_setup_state(
        state_transaction: &mut StateTransaction<'_, '_>,
        intent: &AliasAccountIntentV1,
    ) -> Result<(), InstructionExecutionError> {
        let alias = intent.alias.account_alias();
        if account_label_is_pii(&alias) {
            return Err(InstructionExecutionError::InvariantViolation(
                "Account alias looks like raw PII; use UAID/opaque identifiers instead"
                    .to_owned()
                    .into(),
            ));
        }
        let current_primary = state_transaction
            .world
            .account(&intent.target_account)?
            .label()
            .cloned();
        match (intent.role, current_primary.as_ref()) {
            (AccountAliasRoleV1::Primary, Some(existing)) if existing != &alias => {
                return Err(InstructionExecutionError::InvariantViolation(
                    "alias.primary.conflict: target account has a different primary alias"
                        .to_owned()
                        .into(),
                ));
            }
            (AccountAliasRoleV1::Additional, Some(existing)) if existing == &alias => {
                return Err(InstructionExecutionError::InvariantViolation(
                    "alias.primary.conflict: alias is primary but was requested as additional"
                        .to_owned()
                        .into(),
                ));
            }
            _ => {}
        }
        if let Some(existing) = state_transaction.world.account_aliases.get(&alias)
            && existing != &intent.target_account
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "alias.binding.conflict: account alias is already bound to another account"
                    .to_owned()
                    .into(),
            ));
        }
        if let Some(record) = state_transaction.world.account_rekey_records.get(&alias)
            && record.active_account_id != intent.target_account
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "alias.binding.conflict: account rekey record targets another account"
                    .to_owned()
                    .into(),
            ));
        }
        ensure_single_sbp_retail_fi_home(state_transaction, &intent.target_account, &alias)?;
        ensure_contract_alias_namespace_available(state_transaction, &alias)?;
        state_transaction
            .world
            .insert_account_alias_binding(alias.clone(), intent.target_account.clone());
        upsert_account_rekey_record(state_transaction, &alias, &intent.target_account)?;
        if matches!(intent.role, AccountAliasRoleV1::Primary) && current_primary.is_none() {
            state_transaction
                .world
                .account_mut(&intent.target_account)?
                .set_label(Some(alias));
        }
        Ok(())
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
    /// Reject a primary-alias change while recovery state still depends on the alias.
    ///
    /// # Errors
    ///
    /// Returns an invariant violation for a pending recovery request or policy.
    pub(crate) fn ensure_alias_can_change_recovery_binding(
        state_transaction: &StateTransaction<'_, '_>,
        label: &AccountAlias,
    ) -> Result<(), InstructionExecutionError> {
        if state_transaction
            .world
            .account_recovery_requests
            .get(label)
            .is_some_and(|request| request.is_pending())
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "cannot change primary account alias {label:?}: a recovery request is pending"
                )
                .into(),
            ));
        }
        if state_transaction
            .world
            .account_recovery_policies
            .get(label)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "cannot change primary account alias {label:?}: clear the account recovery policy first"
                )
                .into(),
            ));
        }
        Ok(())
    }
    fn ensure_account_alias_lease(
        state_transaction: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        label: &AccountAlias,
    ) -> Result<(), InstructionExecutionError> {
        crate::sns::ensure_account_alias_lease(state_transaction, owner, label)
            .map_err(|e| InstructionExecutionError::InvariantViolation(e.to_string().into()))
    }
    fn sbp_retail_fi_home_domain(
        alias: &AccountAlias,
        catalog: &DataSpaceCatalog,
    ) -> Option<DomainId> {
        let domain = alias.domain_id(catalog).ok().flatten()?;
        if domain.dataspace().as_ref() == "sbp" && matches!(domain.name().as_ref(), "hbl" | "ubl") {
            Some(domain)
        } else {
            None
        }
    }
    fn ensure_single_sbp_retail_fi_home(
        state_transaction: &StateTransaction<'_, '_>,
        account: &AccountId,
        requested_alias: &AccountAlias,
    ) -> Result<(), InstructionExecutionError> {
        let Some(requested_home) =
            sbp_retail_fi_home_domain(requested_alias, &state_transaction.nexus.dataspace_catalog)
        else {
            return Ok(());
        };
        for existing_alias in state_transaction.world.bound_account_aliases(account) {
            if existing_alias == *requested_alias
                || crate::sns::resolve_active_account_alias(
                    &state_transaction.world,
                    &state_transaction.nexus.dataspace_catalog,
                    &existing_alias,
                    state_transaction.block_unix_timestamp_ms(),
                )
                .as_ref()
                    != Some(account)
            {
                continue;
            }
            let Some(existing_home) = sbp_retail_fi_home_domain(
                &existing_alias,
                &state_transaction.nexus.dataspace_catalog,
            ) else {
                continue;
            };
            if existing_home != requested_home {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account already belongs to retail FI home domain `{existing_home}`; cross-FI alias binding to `{requested_home}` requires an explicit jointly-authorized migration"
                    )
                    .into(),
                ));
            }
        }
        Ok(())
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
    pub(crate) fn resolve_contract_alias_components(
        state_transaction: &StateTransaction<'_, '_>,
        alias: &ContractAlias,
    ) -> Result<(Name, Option<AccountAliasDomain>, DataSpaceId), InstructionExecutionError> {
        let label_name = alias.name_segment().parse::<Name>().map_err(|_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "contract alias name segment is invalid".into(),
            ))
        })?;
        let domain = alias
            .domain_segment()
            .map(str::parse::<AccountAliasDomain>)
            .map(|result| {
                result.map_err(|_| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "contract alias domain segment is invalid".into(),
                        ),
                    )
                })
            })
            .transpose()?;
        let dataspace =
            dataspace_id_for_alias_segment(state_transaction, alias.dataspace_segment())
                .ok_or_else(|| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            format!(
                                "unknown or inactive dataspace alias `{}` in contract alias",
                                alias.dataspace_segment()
                            )
                            .into(),
                        ),
                    )
                })?;
        Ok((label_name, domain, dataspace))
    }
    pub(crate) fn ensure_authority_can_manage_contract_alias(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        alias: &ContractAlias,
    ) -> Result<(), InstructionExecutionError> {
        if state_transaction.replay_compatibility {
            return Ok(());
        }
        let (label, domain, dataspace) =
            resolve_contract_alias_components(state_transaction, alias)?;
        let account_alias = AccountAlias::new_in_dataspace(label, domain, dataspace);
        if authority_can_manage_account_alias(&state_transaction.world, authority, &account_alias) {
            return Ok(());
        }
        Err(InstructionExecutionError::InvariantViolation(
            format!("authority is not permitted to manage contract alias `{alias}`").into(),
        ))
    }
    fn ensure_authority_can_manage_stale_contract_alias(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        contract_address: &ContractAddress,
        alias: &ContractAlias,
    ) -> Result<(), InstructionExecutionError> {
        if state_transaction.replay_compatibility {
            return Ok(());
        }
        let dataspace = contract_address.dataspace_id().map_err(|err| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                err.to_string().into(),
            ))
        })?;
        let domain = alias
            .domain_segment()
            .map(|name| DomainId::try_new(name, alias.dataspace_segment()))
            .transpose()
            .map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.to_string().into(),
                ))
            })?;
        if authority_can_manage_account_alias_scope(
            &state_transaction.world,
            authority,
            dataspace,
            domain.as_ref(),
        ) {
            return Ok(());
        }
        Err(InstructionExecutionError::InvariantViolation(
            format!("authority is not permitted to manage contract alias `{alias}`").into(),
        ))
    }
    fn account_alias_selector_for_contract_alias(
        alias: &ContractAlias,
    ) -> Result<iroha_data_model::sns::NameSelectorV1, InstructionExecutionError> {
        let account_alias_literal = alias.domain_segment().map_or_else(
            || format!("{}@{}", alias.name_segment(), alias.dataspace_segment()),
            |domain| {
                format!(
                    "{}@{}.{}",
                    alias.name_segment(),
                    domain,
                    alias.dataspace_segment()
                )
            },
        );
        Ok(iroha_data_model::sns::NameSelectorV1 {
            version: iroha_data_model::sns::NameSelectorV1::VERSION,
            suffix_id: crate::sns::ACCOUNT_ALIAS_SUFFIX_ID,
            label: account_alias_literal,
        })
    }
    pub(crate) fn ensure_account_alias_namespace_available_for_contract_alias(
        state_transaction: &StateTransaction<'_, '_>,
        alias: &ContractAlias,
    ) -> Result<(), InstructionExecutionError> {
        resolve_contract_alias_components(state_transaction, alias)?;
        let selector = account_alias_selector_for_contract_alias(alias)?;
        let storage_key = crate::sns::record_storage_key(&selector);
        let Some(bytes) = state_transaction
            .world
            .smart_contract_state
            .get(&storage_key)
        else {
            return Ok(());
        };
        let mut slice = bytes.as_slice();
        let record = NameRecordV1::decode(&mut slice).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to decode account alias SNS record: {err}").into(),
            )
        })?;
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
        validate_alias_for_asset_definition(asset_definition.alias().as_ref(), asset_definition)?;
        Ok(())
    }
    fn validate_asset_definition_alias_route(
        state_transaction: &StateTransaction<'_, '_>,
        alias: Option<&AssetDefinitionAlias>,
    ) -> Result<(), InstructionExecutionError> {
        let Some(alias) = alias else {
            return Ok(());
        };
        if dataspace_id_for_alias_segment(state_transaction, alias.dataspace_segment()).is_none() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition alias `{alias}` references an unknown or inactive dataspace"
                )
                .into(),
            ));
        }
        let Some(domain) = alias.domain_segment() else {
            return Ok(());
        };
        let domain_id = DomainId::try_new(domain, alias.dataspace_segment()).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("asset definition alias `{alias}` has invalid domain context: {err}")
                    .into(),
            )
        })?;
        if state_transaction.world.domains.get(&domain_id).is_none() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("asset definition alias `{alias}` references missing domain {domain_id}")
                    .into(),
            ));
        }
        Ok(())
    }
    fn ensure_authority_can_manage_asset_definition_alias(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        asset_definition_id: &AssetDefinitionId,
        alias: &AssetDefinitionAlias,
    ) -> Result<(), InstructionExecutionError> {
        // Genesis is the trusted namespace bootstrap. Every post-genesis mutation must carry the
        // independent asset-definition-alias capability; replay compatibility deliberately does
        // not bypass this first-release authorization boundary.
        if state_transaction._curr_block.is_genesis() {
            return Ok(());
        }
        let dataspace =
            dataspace_id_for_alias_segment(state_transaction, alias.dataspace_segment())
                .ok_or_else(|| {
                    InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition alias `{alias}` references an unknown or inactive dataspace"
                )
                .into(),
            )
                })?;
        let domain = alias
            .domain_segment()
            .map(|domain| DomainId::try_new(domain, alias.dataspace_segment()))
            .transpose()
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("asset definition alias `{alias}` has invalid domain context: {err}")
                        .into(),
                )
            })?;
        if authority_can_manage_asset_definition_alias(
            &state_transaction.world,
            authority,
            asset_definition_id,
            alias,
            dataspace,
            domain.as_ref(),
        ) {
            return Ok(());
        }
        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "authority {authority} lacks exact CanManageAssetDefinitionAlias scope for asset definition alias `{alias}`"
            )
            .into(),
        ))
    }
    fn ensure_asset_definition_domain_context(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        asset_definition: &AssetDefinition,
    ) -> Result<(), InstructionExecutionError> {
        let owning_domain = asset_definition.owning_domain().as_ref();
        if asset_definition.balance_scope_policy() == AssetBalancePolicy::DataspaceRestricted
            && owning_domain.is_none()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "dataspace-restricted asset definition {} requires an explicit owning domain",
                    asset_definition.id()
                )
                .into(),
            ));
        }
        let Some(domain_id) = owning_domain else {
            return Ok(());
        };
        let domain = state_transaction.world.domain(domain_id).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition {} references missing owning domain {domain_id}",
                    asset_definition.id(),
                )
                .into(),
            )
        })?;
        if domain.owned_by() != authority {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("authority {authority} does not own asset definition domain {domain_id}")
                    .into(),
            ));
        }
        Ok(())
    }
    /// Derive the deterministic offline escrow account for an asset definition.
    pub(crate) fn offline_escrow_account_id(
        network_id: &NetworkId,
        definition_id: &AssetDefinitionId,
    ) -> AccountId {
        iroha_data_model::offline::offline_escrow_account_id(network_id, definition_id)
    }
    pub(crate) fn ensure_offline_escrow_account(
        asset_definition: &AssetDefinition,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let definition_id = asset_definition.id();
        let derived = offline_escrow_account_id(state_transaction.network_id(), definition_id);
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
            iroha_executor_data_model::permission::nexus::CanManageFeeSponsorProgram::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.sponsor, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::nexus::CanEnrollFeeSponsorProgram::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.program_id.sponsor, account_id);
        }
        if let Ok(permission) =
            iroha_executor_data_model::permission::asset::CanMintAssetToAccount::try_from(
                permission,
            )
        {
            return account_subject_matches(&permission.account, account_id);
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
            iroha_executor_data_model::permission::query::CanReadAccountData::try_from(permission)
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
        if let Ok(permission) = iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionConfidentialPolicy::try_from(permission) {
            return &permission.asset_definition == asset_definition_id;
        }
        if let Ok(permission) = iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionAlias::try_from(permission)
            && let iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Alias(alias) = permission.scope
        {
            return &alias.asset_definition_id == asset_definition_id;
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
            iroha_executor_data_model::permission::asset::CanMintAssetToAccount::try_from(
                permission,
            )
        {
            return &permission.asset_definition == asset_definition_id;
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
        now_ms: u64,
    ) -> Result<AccountId, Error> {
        crate::block::parse_account_literal_with_world(world, dataspace_catalog, raw, now_ms)
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
        now_ms: u64,
    ) -> Result<bool, Error> {
        let configured =
            resolve_config_account_literal(world, dataspace_catalog, raw, field_path, now_ms)?;
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
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::AccountId(account_id),
                }
                .into());
            }
            if crate::smartcontracts::isi::asset::isi::is_sccp_custody_account(
                state_transaction,
                &account_id,
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot register account {account_id}: its identity is reserved for deterministic SCCP route protocol escrow"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::asset::isi::is_fx_corridor_escrow_account(
                state_transaction,
                &account_id,
            )? {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot register account {account_id}: its identity is reserved for deterministic FX corridor protocol escrow"
                    )
                    .into(),
                )
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
                ensure_account_alias_lease(state_transaction, &account_id, label)?;
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
            let created = AccountEvent::Created(AccountCreated::new(account));
            state_transaction.world.emit_events(Some(created));
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
            if let Some((program_id, _)) =
                state_transaction
                    .world
                    .fee_sponsor_programs
                    .iter()
                    .find(|(_, program)| {
                        program.payout_account == account_id
                            && program.lifecycle
                                != iroha_data_model::nexus::FeeSponsorProgramLifecycle::Closed
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is the immutable payout account for fee sponsor program {program_id}"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::asset::isi::is_sccp_custody_account(
                state_transaction,
                &account_id,
            ) || crate::smartcontracts::isi::asset::isi::is_sccp_custody_owner(
                state_transaction,
                &account_id,
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is retained SCCP protocol escrow or an immutable route custody owner"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::asset::isi::is_fx_corridor_escrow_account(
                state_transaction,
                &account_id,
            )? {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is retained FX protocol escrow"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::sorafs_reserve::is_reserve_custody_account(
                state_transaction.world(),
                &account_id,
            )? {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is active SoraFS reserve custody"
                    )
                    .into(),
                )
                    .into());
            }
            if crate::smartcontracts::isi::escrow::is_protocol_escrow_custody_account(
                state_transaction,
                &account_id,
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is retained native escrow or VPN lease custody"
                    )
                    .into(),
                )
                    .into());
            }
            if crate::smartcontracts::isi::vpn::is_active_vpn_client(
                &state_transaction.world,
                &account_id,
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it funds an active operator-signed VPN lease"
                    )
                    .into(),
                )
                .into());
            }
            let contract_deploy_nonce_key = Name::from_str(
                iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY,
            )
            .expect("contract deployment nonce metadata key must remain valid");
            if state_transaction
                .world
                .account(&account_id)?
                .metadata()
                .contains(&contract_deploy_nonce_key)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it has native contract deployment nonce state; retain the account to preserve deployment address monotonicity and audit history"
                    )
                    .into(),
                )
                .into());
            }
            let orchard_pool_references =
                crate::privacy_state::load_privacy_orchard_pool_references_v1(
                    &state_transaction.world.privacy_commitments,
                )
                .map_err(|message| {
                    InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister account {account_id}: persisted Orchard pool state is invalid: {message}"
                        )
                        .into(),
                    )
                })?;
            if let Some(reference) = orchard_pool_references
                .iter()
                .find(|reference| reference.reserve_account() == &account_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is the reserve account for governed Orchard pool {:?}",
                        reference.namespace()
                    )
                    .into(),
                )
                .into());
            }
            if let Some(primary_label) = state_transaction.world.account(&account_id)?.label() {
                ensure_alias_can_change_recovery_binding(state_transaction, primary_label)?;
            }
            if let Some(owned_domain_id) = state_transaction
                .world
                .domains_by_owner
                .get(&account_id)
                .and_then(|domains| domains.iter().next())
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it owns domain {owned_domain_id}; transfer ownership first"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(owned_definition_id) = state_transaction
                .world
                .asset_definitions_by_owner
                .get(&account_id)
                .and_then(|definitions| definitions.iter().next())
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
            if let Some((referendum_id, _)) =
                state_transaction
                    .world
                    .governance_locks
                    .iter()
                    .find(|(_, locks)| {
                        locks.locks.values().any(|record| {
                            record.custody.as_ref().is_some_and(|custody| {
                                custody.bond_escrow_account == account_id
                                    || custody.slash_receiver_account == account_id
                            })
                        })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is retained by immutable governance lock custody (referendum {referendum_id}); clear the governance locks first"
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
                    retained_validation_fee_plain_electorate_rules(proposal).is_some_and(|rules| {
                        rules.bond_escrow_account == account_id
                            || rules.slash_receiver_account == account_id
                    })
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is retained by immutable validation-fee proposal custody (proposal {proposal_id:?}); reject or supersede the proposal first"
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
                state_transaction.block_unix_timestamp_ms(),
            )?;
            let nexus_stake_escrow_matches = config_account_matches(
                &state_transaction.world,
                &state_transaction.nexus.dataspace_catalog,
                &state_transaction.nexus.staking.stake_escrow_account_id,
                &account_id,
                "nexus.staking.stake_escrow_account_id",
                state_transaction.block_unix_timestamp_ms(),
            )?;
            let nexus_slash_sink_matches = config_account_matches(
                &state_transaction.world,
                &state_transaction.nexus.dataspace_catalog,
                &state_transaction.nexus.staking.slash_sink_account_id,
                &account_id,
                "nexus.staking.slash_sink_account_id",
                state_transaction.block_unix_timestamp_ms(),
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
            for (definition_id, escrow_account) in
                &state_transaction.settlement.offline.escrow_accounts
            {
                if escrow_account != &account_id {
                    continue;
                }
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is the lazily derived offline escrow account for asset definition {definition_id}"
                    )
                    .into(),
                )
                .into());
            }
            let network_id = *state_transaction.network_id();
            if let Some(definition_id) = state_transaction
                .world
                .assets_in_account_iter(&account_id)
                .find_map(|asset| {
                    let definition_id = asset.id().definition();
                    (offline_escrow_account_id(&network_id, definition_id) == account_id)
                        .then(|| definition_id.clone())
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is the deterministic offline escrow account holding live assets for asset definition {definition_id}"
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
            if let Some(((lane_id, validator), _)) = state_transaction
                .world
                .public_lane_validators
                .iter()
                .find(|(key, record)| {
                    public_lane_validator_record_matches_key(key, record)
                        && (key.1 == account_id || record.stake_account == account_id)
                })
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
                .find(|(key, record)| {
                    public_lane_stake_share_matches_key(key, record)
                        && (key.1 == account_id
                            || key.2 == account_id
                            || record.validator == account_id
                            || record.staker == account_id)
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
                .find(|(key, record)| {
                    public_lane_reward_record_matches_key(key, record)
                        && (record.asset.account() == &account_id
                            || record
                                .shares
                                .iter()
                                .any(|share| share.account == account_id))
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
                        "cannot unregister account {account_id}: it is referenced by repo agreement state ({agreement_id}); retain account for settlement audit references"
                    )
                    .into(),
                )
                .into());
            }
            if let Some((settlement_id, _)) = state_transaction
                .world
                .settlement_receipts
                .iter()
                .find(|(_, receipt)| {
                    receipt.authority == account_id
                        || receipt
                            .legs
                            .iter()
                            .any(|leg| leg.leg.from == account_id || leg.leg.to == account_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister account {account_id}: it is referenced by committed settlement receipt {settlement_id}"
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
                .find(|(_, record)| record.intent.authorization.owner == account_id)
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
            let mut remove_nfts: BTreeSet<NftId> = state_transaction
                .world
                .nfts_in_account_iter(&account_id)
                .map(|nft| nft.id().clone())
                .collect();
            remove_nfts.extend(
                state_transaction
                    .world
                    .nfts
                    .iter()
                    .filter_map(|(nft_id, nft)| {
                        (nft.owned_by == account_id).then(|| nft_id.clone())
                    }),
            );
            for nft_id in remove_nfts {
                crate::smartcontracts::isi::nft::isi::remove_nft_associated_permissions(
                    state_transaction,
                    &nft_id,
                );
                state_transaction.world.remove_nft_entry(&nft_id);
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
            validate_asset_definition_alias_route(
                state_transaction,
                asset_definition.alias().as_ref(),
            )?;
            if let Some(alias) = asset_definition.alias().as_ref() {
                ensure_authority_can_manage_asset_definition_alias(
                    state_transaction,
                    authority,
                    asset_definition.id(),
                    alias,
                )?;
            }
            ensure_asset_definition_domain_context(
                state_transaction,
                authority,
                &asset_definition,
            )?;
            ensure_global_asset_definition_registered_on_authoritative_route(
                state_transaction,
                &asset_definition,
            )?;
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
                .insert_asset_definition_entry(asset_definition_id.clone(), stored_definition);
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
            state_transaction
                .world
                .emit_asset_definition_event(AssetDefinitionEvent::Created(asset_definition));
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
            if crate::smartcontracts::isi::asset::isi::is_sccp_settlement_asset_definition(
                state_transaction,
                &asset_definition_id,
            ) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is governed SCCP settlement backing"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::asset::isi::is_fx_corridor_asset_definition(
                state_transaction,
                &asset_definition_id,
            )? {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is retained native FX corridor backing"
                    )
                    .into(),
                )
                .into());
            }
            if crate::smartcontracts::isi::sorafs_reserve::is_reserve_asset_definition(
                state_transaction.world(),
                &asset_definition_id,
            )? {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it backs active SoraFS reserve custody"
                    )
                    .into(),
                )
                    .into());
            }
            let orchard_pool_references =
                crate::privacy_state::load_privacy_orchard_pool_references_v1(
                    &state_transaction.world.privacy_commitments,
                )
                .map_err(|message| {
                    InstructionExecutionError::InvariantViolation(
                        format!(
                            "cannot unregister asset definition {asset_definition_id}: persisted Orchard pool state is invalid: {message}"
                        )
                        .into(),
                    )
                })?;
            if let Some(reference) = orchard_pool_references
                .iter()
                .find(|reference| reference.asset_definition_id() == &asset_definition_id)
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it backs governed Orchard pool {:?}",
                        reference.namespace()
                    )
                    .into(),
                )
                .into());
            }
            if asset_definition_id == state_transaction.gov.voting_asset_id {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is configured as governance voting asset definition (`gov.voting_asset_id`); update governance config first"
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
                        locks.locks.values().any(|record| {
                            record.custody.as_ref().is_some_and(|custody| {
                                custody.asset_definition_id == asset_definition_id
                            })
                        })
                    })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is retained by immutable governance lock custody (referendum {referendum_id}); clear the governance locks first"
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
                    retained_validation_fee_plain_electorate_rules(proposal)
                        .is_some_and(|rules| rules.voting_asset_id == asset_definition_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is retained by immutable validation-fee proposal custody (proposal {proposal_id:?}); reject or supersede the proposal first"
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
                .settlement_receipts
                .iter()
                .find(|(_, receipt)| {
                    receipt
                        .legs
                        .iter()
                        .any(|leg| leg.leg.asset_definition_id() == &asset_definition_id)
                })
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "cannot unregister asset definition {asset_definition_id}: it is referenced by committed settlement receipt {settlement_id}"
                    )
                    .into(),
                )
                .into());
            }
            if let Some(((lane_id, epoch), _)) = state_transaction
                .world
                .public_lane_rewards
                .iter()
                .find(|(key, record)| {
                    public_lane_reward_record_matches_key(key, record)
                        && record.asset.definition() == &asset_definition_id
                })
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
            let domain = state_transaction
                .world
                .asset_definition_domains
                .get(&asset_definition_id)
                .cloned();
            let mut events = Vec::with_capacity(assets_to_remove.len() + 1);
            for asset_id in assets_to_remove {
                if state_transaction
                    .world
                    .remove_asset_and_metadata_with_total(&asset_id)?
                    .is_none()
                {
                    error!(%asset_id, "asset not found. This is a bug");
                }
                events.push(DataEvent::asset(
                    AssetEvent::Deleted(asset_id),
                    domain.clone(),
                ));
            }
            if state_transaction
                .world
                .remove_asset_definition_entry(&asset_definition_id)
                .is_none()
            {
                return Err(FindError::AssetDefinition(asset_definition_id).into());
            }
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
            events.push(DataEvent::asset_definition(
                AssetDefinitionEvent::Deleted(asset_definition_id),
                domain,
            ));
            state_transaction.world.emit_events(events);
            Ok(())
        }
    }
    impl Execute for SetAssetDefinitionAlias {
        fn execute(
            self,
            authority: &AccountId,
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
            // Ensure definition exists and validate the alias against the display label and any
            // explicit human-readable name carried by the stored definition.
            let definition = state_transaction
                .world
                .asset_definition(&asset_definition_id)
                .map_err(Error::from)?;
            if definition.owned_by() != authority {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "only asset-definition owner {} may change alias for {asset_definition_id}",
                        definition.owned_by()
                    )
                    .into(),
                )
                .into());
            }
            validate_alias_for_asset_definition(alias.as_ref(), &definition)?;
            validate_asset_definition_alias_route(state_transaction, alias.as_ref())?;
            let existing_alias = state_transaction
                .world
                .asset_definition_alias_bindings
                .get(&asset_definition_id)
                .map(|binding| binding.alias.clone());
            if let Some(alias) = alias.as_ref() {
                ensure_authority_can_manage_asset_definition_alias(
                    state_transaction,
                    authority,
                    &asset_definition_id,
                    alias,
                )?;
            }
            if let Some(existing_alias) = existing_alias.as_ref()
                && alias.as_ref() != Some(existing_alias)
            {
                ensure_authority_can_manage_asset_definition_alias(
                    state_transaction,
                    authority,
                    &asset_definition_id,
                    existing_alias,
                )?;
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
            authority: &AccountId,
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
            if let Some(alias) = alias {
                let contract_dataspace_id = contract_address.dataspace_id().map_err(|err| {
                    InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(err.to_string().into()),
                    )
                })?;
                let contract_deployed = state_transaction
                    .world
                    .contract_instances()
                    .get(&contract_address)
                    .is_some();
                if !contract_deployed
                    && state_transaction
                        .nexus
                        .dataspace_catalog
                        .by_id(contract_dataspace_id)
                        .is_none()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "contract address dataspace is unknown".to_owned().into(),
                    )
                    .into());
                }
                if !contract_deployed {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!("contract {contract_address} is not deployed").into(),
                    )
                    .into());
                }
                let (_, _, alias_dataspace_id) =
                    resolve_contract_alias_components(state_transaction, &alias)?;
                if alias_dataspace_id != contract_dataspace_id {
                    return Err(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "contract alias dataspace must match contract address dataspace".into(),
                        ),
                    )
                    .into());
                }
                ensure_authority_can_manage_contract_alias(state_transaction, authority, &alias)?;
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
                if let Some(binding) = state_transaction
                    .world
                    .contract_alias_bindings()
                    .get(&contract_address)
                {
                    if ensure_authority_can_manage_contract_alias(
                        state_transaction,
                        authority,
                        &binding.alias,
                    )
                    .is_err()
                    {
                        ensure_authority_can_manage_stale_contract_alias(
                            state_transaction,
                            authority,
                            &contract_address,
                            &binding.alias,
                        )?;
                    }
                }
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
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: asset_definition_id,
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
                .asset_definition_mut(&asset_definition_id)
                .map_err(Error::from)
                .map(|asset_definition| {
                    asset_definition
                        .metadata_mut()
                        .insert(key.clone(), value.clone())
                })?;
            state_transaction.world.emit_asset_definition_event(
                AssetDefinitionEvent::MetadataInserted(MetadataChanged {
                    target: asset_definition_id,
                    key,
                    value,
                }),
            );
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
            state_transaction.world.emit_asset_definition_event(
                AssetDefinitionEvent::MetadataRemoved(MetadataChanged {
                    target: asset_definition_id,
                    key: self.key().clone(),
                    value,
                }),
            );
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
            let next_musubi_owner_generation = (source != destination)
                .then(|| {
                    let current_generation = match state_transaction
                        .world
                        .musubi_domain_ownership_generations()
                        .get(&object)
                        .copied()
                    {
                        None => 1,
                        Some(generation) if generation >= 2 => generation,
                        Some(_) => {
                            return Err(Error::InvariantViolation(
                                "Musubi domain ownership generation is noncanonical".into(),
                            ));
                        }
                    };
                    current_generation.checked_add(1).ok_or_else(|| {
                        Error::InvariantViolation(
                            "Musubi domain ownership generation overflow".into(),
                        )
                    })
                })
                .transpose()?;
            {
                let domain = state_transaction.world.domain_mut(&object)?;
                if domain.owned_by() != &source {
                    return Err(Error::InvariantViolation(
                        format!("Can't transfer domain {domain} since {source} doesn't own it",)
                            .into(),
                    ));
                }
                domain.set_owned_by(destination.clone());
            }
            if let Some(next_generation) = next_musubi_owner_generation {
                state_transaction
                    .world
                    .musubi_domain_ownership_generations_mut()
                    .insert(object.clone(), next_generation);
            }
            state_transaction
                .world
                .replace_domain_owner_index(&object, &source, &destination);
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
        if let Some(disallowed) = first_disallowed_algorithm(controller, allowed_algorithms)? {
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
        match first_disallowed_curve(controller, allowed_curve_ids)? {
            Some(curve) => {
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
            None => {}
        }
        Ok(())
    }
    fn first_disallowed_algorithm(
        controller: &AccountController,
        allowed: &[Algorithm],
    ) -> Result<Option<Algorithm>, InstructionExecutionError> {
        match controller {
            AccountController::Single(signatory) => Ok(algorithm_if_disallowed(
                controller_algorithm(signatory)?,
                allowed,
            )),
            AccountController::Multisig(policy) => {
                for member in policy.members() {
                    if let Some(disallowed) =
                        algorithm_if_disallowed(controller_algorithm(member.public_key())?, allowed)
                    {
                        return Ok(Some(disallowed));
                    }
                }
                Ok(None)
            }
        }
    }
    fn controller_algorithm(
        public_key: &PublicKey,
    ) -> Result<Algorithm, InstructionExecutionError> {
        public_key.try_algorithm().map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("account controller public key is malformed: {err}").into(),
            )
        })
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
    ) -> Result<Option<CurveId>, InstructionExecutionError> {
        match controller {
            AccountController::Single(signatory) => {
                let algo = controller_algorithm(signatory)?;
                curve_if_disallowed(algo, allowed_curve_ids).map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!(
                            "account controller uses signing algorithm {} which is not registered in \
                             the account curve registry: {err}",
                            algo.as_static_str()
                        )
                        .into(),
                    )
                })
            }
            AccountController::Multisig(policy) => {
                for member in policy.members() {
                    let algo = controller_algorithm(member.public_key())?;
                    match curve_if_disallowed(algo, allowed_curve_ids) {
                        Ok(Some(curve)) => return Ok(Some(curve)),
                        Ok(None) => {}
                        Err(err) => {
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
    use super::*;
    use crate::{
        smartcontracts::{ValidQuery, ValidSingularQuery},
        state::{StateReadOnly, WorldReadOnly},
    };
    use iroha_data_model::{
        domain::Domain,
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail,
            json::{PredicateJson, predicate_json_candidate_plan_for_execution},
        },
    };
    use norito::json::Value;
    use std::collections::BTreeSet;
    #[derive(Debug, Default)]
    struct DomainPredicateView {
        ids: BTreeSet<DomainId>,
        owners: BTreeSet<AccountId>,
    }
    impl DomainPredicateView {
        fn from_predicate(predicate: &CompoundPredicate<Domain>) -> Self {
            let mut view = Self::default();
            let Some(raw) = predicate.json_payload() else {
                return view;
            };
            let Some(predicate) = predicate_json_candidate_plan_for_execution(raw) else {
                return view;
            };
            for condition in predicate.equals {
                view.push_field_value(&condition.field, &condition.value);
            }
            for membership in predicate.r#in {
                for value in membership.values {
                    view.push_field_value(&membership.field, &value);
                }
            }
            view
        }
        fn push_field_value(&mut self, field: &str, value: &Value) {
            let Value::String(raw) = value else {
                return;
            };
            match field {
                "id" | "domain" | "domain_id" => {
                    if let Some(domain_id) = parse_domain_predicate_value(raw) {
                        self.ids.insert(domain_id);
                    }
                }
                "owner" | "owned_by" | "account" | "account_id" => {
                    if let Ok(account_id) = AccountId::parse_encoded(raw)
                        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    {
                        self.owners.insert(account_id.subject_id());
                    }
                }
                _ => {}
            }
        }
        fn plan(&self) -> DomainQueryPlan {
            let mut ids = self.ids.iter().cloned().collect::<Vec<_>>();
            ids.sort();
            let mut owners = self.owners.iter().cloned().collect::<Vec<_>>();
            owners.sort();
            if !ids.is_empty() {
                return DomainQueryPlan::Ids(ids);
            }
            if !owners.is_empty() {
                return DomainQueryPlan::Owners(owners);
            }
            DomainQueryPlan::Full
        }
    }
    #[derive(Debug)]
    enum DomainQueryPlan {
        Ids(Vec<DomainId>),
        Owners(Vec<AccountId>),
        Full,
    }
    fn parse_domain_predicate_value(raw: &str) -> Option<DomainId> {
        DomainId::parse_fully_qualified(raw)
            .ok()
            .or_else(|| DomainId::try_new(raw, "universal").ok())
    }
    fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        if path.is_empty() {
            return None;
        }
        let mut current = value;
        for segment in path.split('.') {
            if segment.is_empty() {
                return None;
            }
            match current {
                Value::Object(map) => current = map.get(segment)?,
                _ => return None,
            }
        }
        Some(current)
    }
    fn predicate_value_equals_str(value: &Value, expected: &str) -> bool {
        matches!(value, Value::String(raw) if raw == expected)
    }
    fn predicate_values_contain_str(values: &[Value], expected: &str) -> bool {
        values
            .iter()
            .any(|value| matches!(value, Value::String(raw) if raw == expected))
    }
    fn domain_alias_values(domain: &Domain, field: &str) -> Vec<String> {
        match field {
            "id" | "domain" | "domain_id" => {
                let canonical = domain.id().to_string();
                let shorthand = domain.id().name().to_string();
                if canonical == shorthand {
                    vec![canonical]
                } else {
                    vec![canonical, shorthand]
                }
            }
            "owner" | "owned_by" | "account" | "account_id" => {
                vec![domain.owned_by().to_string()]
            }
            _ => Vec::new(),
        }
    }
    fn domain_json_value<'a>(cache: &'a mut Option<Value>, domain: &Domain) -> Option<&'a Value> {
        if cache.is_none() {
            *cache = crate::smartcontracts::isi::query::ordinary_predicate_json_value(domain);
        }
        cache.as_ref()
    }
    fn predicate_matches_domain(predicate: &PredicateJson, domain: &Domain) -> bool {
        let mut domain_json = None;
        for cond in &predicate.equals {
            let aliases = domain_alias_values(domain, &cond.field);
            if !aliases.is_empty() {
                if !aliases
                    .iter()
                    .any(|alias| predicate_value_equals_str(&cond.value, alias))
                {
                    return false;
                }
                continue;
            }
            let Some(value) = domain_json_value(&mut domain_json, domain) else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, &cond.field) else {
                return false;
            };
            if actual != &cond.value {
                return false;
            }
        }
        for cond in &predicate.r#in {
            let aliases = domain_alias_values(domain, &cond.field);
            if !aliases.is_empty() {
                if !aliases
                    .iter()
                    .any(|alias| predicate_values_contain_str(&cond.values, alias))
                {
                    return false;
                }
                continue;
            }
            let Some(value) = domain_json_value(&mut domain_json, domain) else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, &cond.field) else {
                return false;
            };
            if !cond.values.iter().any(|candidate| candidate == actual) {
                return false;
            }
        }
        for field in &predicate.exists {
            if !domain_alias_values(domain, field).is_empty() {
                continue;
            }
            let Some(value) = domain_json_value(&mut domain_json, domain) else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, field) else {
                return false;
            };
            if actual.is_null() {
                return false;
            }
        }
        true
    }
    impl ValidSingularQuery for FindDomainById {
        #[metrics(+"find_domain_by_id")]
        fn execute(
            &self,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<Domain, QueryExecutionFail> {
            let domain = state_ro
                .world()
                .domain(self.domain_id())
                .map_err(QueryExecutionFail::from)?;
            crate::smartcontracts::isi::query::own_singular_query_value(domain)
        }
    }
    impl ValidQuery for FindDomains {
        #[metrics(+"find_domains")]
        fn execute(
            self,
            filter: CompoundPredicate<Domain>,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<impl Iterator<Item = Domain>, QueryExecutionFail> {
            let world = state_ro.world();
            let predicate_view = DomainPredicateView::from_predicate(&filter);
            let predicate_json = filter
                .json_payload()
                .and_then(predicate_json_candidate_plan_for_execution);
            let iter: Box<dyn Iterator<Item = Domain> + '_> = match predicate_view.plan() {
                DomainQueryPlan::Ids(ids) => Box::new(
                    ids.into_iter()
                        .filter_map(move |domain_id| world.domain(&domain_id).ok().cloned()),
                ),
                DomainQueryPlan::Owners(owners) => {
                    Box::new(owners.into_iter().flat_map(move |owner| {
                        world
                            .domains_by_owner()
                            .get(&owner)
                            .cloned()
                            .into_iter()
                            .flatten()
                            .filter_map(move |domain_id| world.domain(&domain_id).ok().cloned())
                    }))
                }
                DomainQueryPlan::Full => Box::new(world.domains_iter().cloned()),
            };
            Ok(iter.filter(move |domain| {
                if let Some(predicate) = predicate_json.as_ref() {
                    predicate_matches_domain(predicate, domain)
                } else {
                    filter.applies(domain)
                }
            }))
        }
    }
    impl ValidQuery for FindDomainsByAccountId {
        #[metrics(+"find_domains_by_account_id")]
        fn execute(
            self,
            filter: CompoundPredicate<Domain>,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<impl Iterator<Item = Domain>, QueryExecutionFail> {
            let account_id = self.account_id().clone();
            state_ro.world().account(&account_id)?;
            let domains = state_ro
                .world()
                .domains_owned_by_iter(&account_id)
                .cloned()
                .collect::<Vec<_>>();
            Ok(domains
                .into_iter()
                .filter(move |domain| filter.applies(domain)))
        }
    }
}
#[cfg(test)]
mod tests {
    include!("domain_restricted_asset_definition_tests.rs");
    use super::isi::upsert_account_rekey_record;
    use super::*;
    use crate::{
        kura::Kura,
        nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
        prelude::World,
        query::store::LiveQueryStore,
        smartcontracts::{ValidQuery, ValidSingularQuery},
        state::{
            GovernanceLockCustody, GovernanceLockRecord, GovernanceLocksForReferendum,
            GovernanceParliamentSnapshot, GovernancePipeline, GovernanceProposalRecord,
            GovernanceProposalStatus, GovernanceReferendumMode, GovernanceReferendumRecord,
            GovernanceReferendumStatus, GovernanceStageApprovals, State, WorldReadOnly,
        },
    };
    use iroha_crypto::{
        Algorithm, Hash, KeyPair,
        blake2::{Blake2b512, digest::Digest as _},
    };
    use iroha_data_model::{
        ChainId, IntoKeyValue,
        account::{
            Account, AccountAddress, NewAccount, OpaqueAccountId,
            controller::{MultisigMember, MultisigPolicy},
            rekey::{AccountAlias, AccountRekeyRecord, AccountRekeyTransitionProvenance},
        },
        alias_setup::{
            AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
            AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1, ResolvedAccountAliasV1,
        },
        asset::{
            Asset, AssetDefinition, AssetDefinitionAlias, AssetDefinitionId, AssetId, Mintable,
            NewAssetDefinition, ResolvedAssetDefinitionAliasV1,
        },
        block::BlockHeader,
        events::data::space_directory::{
            SpaceDirectoryEvent, SpaceDirectoryManifestActivated, SpaceDirectoryManifestRevoked,
        },
        governance::types::{
            GovernanceFinalizationEvidence, ParliamentBodies, ParliamentBody, ParliamentRoster,
            ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
        },
        isi::{
            alias_setup::{CompareAndSetPrimaryAccountAlias, EnsureAlias, RebindAccountAlias},
            error::{InstructionExecutionError, InvalidParameterError, RepetitionError},
            governance::{CouncilDerivationKind, VotingMode},
        },
        metadata::Metadata,
        name::Name,
        nexus::{
            AssetPermissionManifest, DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
            LaneConfig, LaneId, LaneVisibility, ManifestVersion, UniversalAccountId,
        },
        nft::{Nft, NftId},
        permission::Permission,
        prelude::Domain,
        privacy::{
            PrivacyNamespaceScopeV1, PrivacyNamespaceV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1,
            PrivacyProtocolIdV1,
        },
        role::{Role, RoleId},
        smart_contract::ContractAddress,
        sns::{NameControllerV1, NameRecordV1},
        validation_fee::{
            VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
            VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS, VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            ValidationFeeChargingMode, ValidationFeePlainElectorateEligibilityRuleV1,
            ValidationFeePlainElectorateMemberV1, ValidationFeePlainElectorateRulesV1,
            ValidationFeePlainElectorateSnapshotV1, ValidationFeePolicyV1,
            ValidationFeeTreasuryPayoutBindingV1, ValidationFeeTreasuryPayoutRecipientV1,
        },
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount,
    };
    use iroha_executor_data_model::permission::asset_definition::{
        AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
    };
    use iroha_primitives::{
        json::Json,
        numeric::{NumericSpec, Quantity},
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;
    use std::sync::Arc;
    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }
    fn fixture_keypair(seed: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("fixture seed must derive a valid keypair")
    }
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("domain ISI fixture key generation should succeed")
    }
    fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("domain ISI fixture key generation for requested algorithm should succeed")
    }
    fn install_orchard_pool_dependency_guard(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
        asset_definition_id: AssetDefinitionId,
        reserve_account: AccountId,
    ) -> crate::privacy_state::PrivacyCommitmentKeyV1 {
        let namespace = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new([0xD1; 32]),
            }),
        );
        let bootstrap = iroha_data_model::privacy::PrivacyOrchardPoolBootstrapV1::new(
            PrivacyPoolIdV1::new([0xD1; 32]),
            asset_definition_id,
            iroha_data_model::asset::AssetBalanceScope::Global,
            reserve_account,
        )
        .expect("canonical Orchard dependency-guard bootstrap");
        let pool_state = crate::privacy_state::PrivacyOrchardPoolStateV1::bootstrap(bootstrap)
            .expect("canonical Orchard dependency-guard state");
        let key = crate::privacy_state::PrivacyCommitmentKeyV1::orchard_pool_state(namespace)
            .expect("canonical Orchard dependency-guard key");
        let record = crate::privacy_state::PrivacyStateItemRecordV1::orchard_pool_state(pool_state)
            .expect("canonical Orchard dependency-guard record");
        assert!(
            state_transaction
                .world
                .privacy_commitments
                .insert(key, record)
                .is_none()
        );
        key
    }
    #[derive(Clone, Copy, Debug)]
    enum ValidationFeeProposalFixtureKind {
        Policy,
        PayoutLifecycle,
    }
    #[derive(Clone, Debug)]
    struct ValidationFeeUnregisterTargets {
        domain_id: DomainId,
        voting_asset_id: AssetDefinitionId,
        bond_escrow_account: AccountId,
        slash_receiver_account: AccountId,
    }
    fn fixture_account(seed: u8) -> AccountId {
        AccountId::new(
            fixture_keypair(seed, Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }
    fn register_validation_fee_unregister_targets(
        authority: &AccountId,
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    ) -> ValidationFeeUnregisterTargets {
        let domain_id =
            DomainId::try_new("validation", "guard").expect("validation-fee guard domain");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(authority, state_transaction)
            .expect("register validation-fee guard domain");
        if state_transaction.world.accounts.get(authority).is_none() {
            Register::account(NewAccount::new(authority.clone()))
                .execute(authority, state_transaction)
                .expect("register validation-fee proposal operator");
        }
        let active_domain_id =
            DomainId::try_new("active", "guard").expect("active governance guard domain");
        Register::domain(Domain::new(active_domain_id.clone()))
            .execute(authority, state_transaction)
            .expect("register active governance guard domain");
        let active_voting_asset_id = AssetDefinitionId::derive_from_components(
            active_domain_id,
            "replacement".parse().expect("replacement asset name"),
        );
        Register::asset_definition(AssetDefinition::numeric(
            active_voting_asset_id.clone(),
            "replacement".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(authority, state_transaction)
        .expect("register replacement governance voting asset");
        let voting_asset_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "xor".parse().expect("asset name"),
        );
        Register::asset_definition(AssetDefinition::numeric(
            voting_asset_id.clone(),
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(authority, state_transaction)
        .expect("register retained validation-fee voting asset");
        let bond_escrow_account = fixture_account(0x91);
        let slash_receiver_account = fixture_account(0x92);
        let active_bond_escrow_account = fixture_account(0xB1);
        let active_slash_receiver_account = fixture_account(0xB2);
        for account_id in [
            &bond_escrow_account,
            &slash_receiver_account,
            &active_bond_escrow_account,
            &active_slash_receiver_account,
        ] {
            Register::account(NewAccount::new(account_id.clone()))
                .execute(authority, state_transaction)
                .expect("register validation-fee custody account");
        }
        ValidationFeeUnregisterTargets {
            domain_id,
            voting_asset_id,
            bond_escrow_account,
            slash_receiver_account,
        }
    }
    fn validation_fee_unregister_rules(
        targets: &ValidationFeeUnregisterTargets,
    ) -> ValidationFeePlainElectorateRulesV1 {
        let rules = ValidationFeePlainElectorateRulesV1 {
            voting_asset_id: targets.voting_asset_id.clone(),
            bond_escrow_account: targets.bond_escrow_account.clone(),
            slash_receiver_account: targets.slash_receiver_account.clone(),
            ballot_amount: Quantity::from(150_u32),
            ballot_duration_blocks: 3_600,
            citizenship_amount: Quantity::from(10_000_u32),
            max_members: VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
            conviction_step_blocks: 100,
            max_conviction: 6,
            min_turnout: 1,
            approval_threshold_numerator: 1,
            approval_threshold_denominator: 2,
            eligibility_rule: ValidationFeePlainElectorateEligibilityRuleV1::
                ProposalOperatorAtOrBeforeGateOthersAfterGate,
        };
        assert_eq!(
            rules.invariant_error(),
            None,
            "unregister guard rules must be structurally valid"
        );
        rules
    }
    fn validation_fee_guard_sbd_asset_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("sbd", "guard").expect("SBD guard domain"),
            "sbd".parse().expect("SBD asset name"),
        )
    }
    fn validation_fee_guard_network_id() -> iroha_data_model::NetworkId {
        "0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .expect("exact validation-fee guard network id")
    }
    fn validation_fee_guard_payout_binding(
        rules: &ValidationFeePlainElectorateRulesV1,
    ) -> ValidationFeeTreasuryPayoutBindingV1 {
        let controller = fixture_account(0xA0);
        let contract_address = ContractAddress::derive(
            &validation_fee_guard_network_id(),
            &controller,
            91,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive validation-fee payout contract address");
        let binding = ValidationFeeTreasuryPayoutBindingV1 {
            treasury_account_id: contract_address.subject_id(),
            contract_address,
            code_hash: [0xA5; 32],
            entrypoint: "autonomous_validation_fee_tick"
                .parse()
                .expect("payout entrypoint"),
            sbd_asset_id: validation_fee_guard_sbd_asset_id(),
            xor_asset_id: rules.voting_asset_id.clone(),
            pool_vault_account_id: fixture_account(0xA2),
            batch_sbd: iroha_data_model::validation_fee::validation_fee_payout_batch_sbd(),
            min_xor_out: iroha_data_model::validation_fee::validation_fee_payout_min_xor(),
            max_xor_out: iroha_data_model::validation_fee::validation_fee_payout_max_xor(),
            recipients: (0xA3..=0xA6)
                .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                    account_id: fixture_account(seed),
                    share: iroha_data_model::validation_fee::validation_fee_payout_recipient_share(
                    ),
                })
                .collect(),
        };
        assert_eq!(
            binding.invariant_error(),
            None,
            "unregister guard payout binding must be structurally valid"
        );
        binding
    }
    fn validation_fee_unregister_proposal_kind(
        fixture_kind: ValidationFeeProposalFixtureKind,
        rules: &ValidationFeePlainElectorateRulesV1,
    ) -> ProposalKind {
        match fixture_kind {
            ValidationFeeProposalFixtureKind::Policy => {
                let policy = ValidationFeePolicyV1 {
                    schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
                    network_id: validation_fee_guard_network_id(),
                    policy_version: 1,
                    previous_policy_hash: None,
                    ds_asset_id: validation_fee_guard_sbd_asset_id(),
                    ds_scale: VALIDATION_FEE_DS_SCALE,
                    fee: Quantity::zero(),
                    treasury_account_id: fixture_account(0xA7),
                    charging_mode: ValidationFeeChargingMode::Disabled,
                    effective_from_height: VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS + 3_601,
                    expires_after_height: None,
                    exemption_classes: Vec::new(),
                    treasury_payout_binding: None,
                };
                assert_eq!(
                    policy.policy_invariant_error(),
                    None,
                    "unregister guard policy must be structurally valid"
                );
                ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    policy,
                    payout_lifecycle_proposal_id: None,
                    plain_electorate_rules: rules.clone(),
                })
            }
            ValidationFeeProposalFixtureKind::PayoutLifecycle => {
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    payout_binding: validation_fee_guard_payout_binding(rules),
                    plain_electorate_rules: rules.clone(),
                })
            }
        }
    }
    const VALIDATION_FEE_PARLIAMENT_BODIES: [ParliamentBody; 7] = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ];
    fn validation_fee_unregister_parliament_snapshot() -> GovernanceParliamentSnapshot {
        let rosters = VALIDATION_FEE_PARLIAMENT_BODIES
            .into_iter()
            .map(|body| {
                (
                    body,
                    ParliamentRoster {
                        body,
                        epoch: 1,
                        members: vec![(*ALICE_ID).clone()],
                        alternates: Vec::new(),
                        candidate_count: 1,
                        derived_by: CouncilDerivationKind::Manual,
                    },
                )
            })
            .collect();
        let bodies = ParliamentBodies {
            selection_epoch: 1,
            rosters,
        };
        let encoded = norito::to_bytes(&bodies).expect("encode unregister guard Parliament bodies");
        let digest = Blake2b512::digest(encoded);
        let mut roster_root = [0_u8; 32];
        roster_root.copy_from_slice(&digest[..32]);
        GovernanceParliamentSnapshot {
            selection_epoch: 1,
            beacon: [0x55; 32],
            roster_root,
            bodies,
        }
    }
    fn insert_validation_fee_unregister_proposal(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
        fixture_kind: ValidationFeeProposalFixtureKind,
        status: GovernanceProposalStatus,
        rules: &ValidationFeePlainElectorateRulesV1,
    ) -> [u8; 32] {
        let kind = validation_fee_unregister_proposal_kind(fixture_kind, rules);
        let proposal_id = kind.fingerprint();
        let referendum_status = match status {
            GovernanceProposalStatus::Proposed => GovernanceReferendumStatus::Proposed,
            GovernanceProposalStatus::Approved => GovernanceReferendumStatus::Closed,
            GovernanceProposalStatus::Rejected
            | GovernanceProposalStatus::Enacted
            | GovernanceProposalStatus::Superseded => {
                panic!("unregister guard fixture only supports retained proposal statuses")
            }
        };
        let finalization_evidence = (status == GovernanceProposalStatus::Approved).then_some(
            GovernanceFinalizationEvidence {
                proposal_id,
                referendum_id: proposal_id,
                finalized_at_height: 3_601,
                mode: VotingMode::Plain,
                approve: 1,
                reject: 0,
                abstain: 0,
                min_turnout: rules.min_turnout,
                approval_threshold_numerator: rules.approval_threshold_numerator,
                approval_threshold_denominator: rules.approval_threshold_denominator,
                approved: true,
            },
        );
        state_transaction.world.put_governance_proposal(
            proposal_id,
            GovernanceProposalRecord {
                proposer: (*ALICE_ID).clone(),
                kind,
                created_height: 1,
                status,
                pipeline: GovernancePipeline::default(),
                parliament_snapshot: Some(validation_fee_unregister_parliament_snapshot()),
                finalization_evidence,
                enacted_at_height: None,
            },
        );
        let referendum_id = hex::encode(proposal_id);
        state_transaction.world.governance_referenda.insert(
            referendum_id.clone(),
            GovernanceReferendumRecord {
                h_start: 2,
                h_end: 3_601,
                status: referendum_status,
                mode: GovernanceReferendumMode::Plain,
            },
        );
        if status == GovernanceProposalStatus::Approved {
            let electorate = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
                proposal_id,
                (*ALICE_ID).clone(),
                2,
                1,
                vec![ValidationFeePlainElectorateMemberV1 {
                    account_id: (*ALICE_ID).clone(),
                    bonded_height: 1,
                    bonded_amount: rules.citizenship_amount.clone(),
                }],
            )
            .expect("build unregister guard frozen electorate");
            assert_eq!(
                electorate.context_error(proposal_id, &ALICE_ID, rules),
                None,
                "unregister guard electorate must be valid for its proposal"
            );
            let mut approvals = GovernanceStageApprovals::default();
            for body in VALIDATION_FEE_PARLIAMENT_BODIES {
                approvals
                    .ensure_stage(body, 1, 1, 10_000)
                    .record((*ALICE_ID).clone());
            }
            approvals.approval_gate_height = Some(1);
            approvals.validation_fee_plain_electorate_snapshot = Some(electorate);
            state_transaction
                .world
                .governance_stage_approvals
                .insert(referendum_id, approvals);
        }
        proposal_id
    }
    fn drift_validation_fee_governance_config(
        state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    ) {
        state_transaction.gov.voting_asset_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("active", "guard").expect("active guard domain"),
            "replacement".parse().expect("replacement asset name"),
        );
        state_transaction.gov.bond_escrow_account = fixture_account(0xB1);
        state_transaction.gov.slash_receiver_account = fixture_account(0xB2);
    }
    fn assert_validation_fee_governance_config_drift(
        state_transaction: &crate::state::StateTransaction<'_, '_>,
        rules: &ValidationFeePlainElectorateRulesV1,
    ) {
        assert_ne!(state_transaction.gov.voting_asset_id, rules.voting_asset_id);
        assert_ne!(
            state_transaction.gov.bond_escrow_account,
            rules.bond_escrow_account
        );
        assert_ne!(
            state_transaction.gov.slash_receiver_account,
            rules.slash_receiver_account
        );
    }
    fn instruction_error_contains(error: &InstructionExecutionError, expected: &str) -> bool {
        match error {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) => message.contains(expected),
            InstructionExecutionError::InvariantViolation(message) => message.contains(expected),
            _ => false,
        }
    }
    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithms() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        for algorithm in [Algorithm::Secp256k1, Algorithm::BlsNormal] {
            assert_eq!(
                checked_keypair_with_algorithm(algorithm).algorithm(),
                algorithm
            );
        }
    }
    fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(domain_id.clone(), domain);
        let mut domains = state
            .world
            .domains_by_owner
            .view()
            .get(owner)
            .cloned()
            .unwrap_or_default();
        domains.insert(domain_id.clone());
        state.world.domains_by_owner.insert(owner.clone(), domains);
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
    fn test_state_with_authority(authority: &AccountId) -> State {
        let mut state = test_state();
        let domain_id = DomainId::try_new("authority", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, authority);
        seed_account(&mut state, authority, &domain_id);
        state
    }
    #[test]
    fn unregister_account_rejects_governed_orchard_reserve_dependency_atomically() {
        let authority = (*ALICE_ID).clone();
        let mut state = test_state_with_authority(&authority);
        let domain_id =
            DomainId::try_new("orchard_account", "guard").expect("Orchard guard domain");
        seed_domain(&mut state, &domain_id, &authority);
        let reserve_account = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "coin".parse().expect("asset name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        Register::account(NewAccount::new(reserve_account.clone()))
            .execute(&authority, &mut transaction)
            .expect("register Orchard reserve account");
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(&authority, &mut transaction)
        .expect("register Orchard backing definition");
        let state_key = install_orchard_pool_dependency_guard(
            &mut transaction,
            asset_definition_id,
            reserve_account.clone(),
        );
        let error = Unregister::account(reserve_account.clone())
            .execute(&authority, &mut transaction)
            .expect_err("governed Orchard reserve account must remain registered");
        assert!(
            error
                .to_string()
                .contains("reserve account for governed Orchard pool"),
            "{error}"
        );
        assert!(transaction.world.accounts.get(&reserve_account).is_some());
        assert!(
            transaction
                .world
                .privacy_commitments
                .get(&state_key)
                .is_some()
        );
    }
    #[test]
    fn unregister_asset_definition_rejects_governed_orchard_dependency_atomically() {
        let authority = (*ALICE_ID).clone();
        let mut state = test_state_with_authority(&authority);
        let domain_id = DomainId::try_new("orchard_asset", "guard").expect("Orchard guard domain");
        seed_domain(&mut state, &domain_id, &authority);
        let reserve_account = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "coin".parse().expect("asset name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        Register::account(NewAccount::new(reserve_account.clone()))
            .execute(&authority, &mut transaction)
            .expect("register Orchard reserve account");
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(&authority, &mut transaction)
        .expect("register Orchard backing definition");
        let state_key = install_orchard_pool_dependency_guard(
            &mut transaction,
            asset_definition_id.clone(),
            reserve_account,
        );
        let error = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut transaction)
            .expect_err("governed Orchard backing definition must remain registered");
        assert!(
            error.to_string().contains("backs governed Orchard pool"),
            "{error}"
        );
        assert!(
            transaction
                .world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some()
        );
        assert!(
            transaction
                .world
                .privacy_commitments
                .get(&state_key)
                .is_some()
        );
    }
    #[test]
    fn unregister_domain_rejects_governed_orchard_asset_cascade_before_mutation() {
        let authority = (*ALICE_ID).clone();
        let mut state = test_state_with_authority(&authority);
        let domain_id = DomainId::try_new("orchard_domain", "guard").expect("Orchard guard domain");
        seed_domain(&mut state, &domain_id, &authority);
        let reserve_account = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "coin".parse().expect("asset name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        Register::account(NewAccount::new(reserve_account.clone()))
            .execute(&authority, &mut transaction)
            .expect("register Orchard reserve account");
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "coin".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(&authority, &mut transaction)
        .expect("register Orchard backing definition");
        let state_key = install_orchard_pool_dependency_guard(
            &mut transaction,
            asset_definition_id.clone(),
            reserve_account.clone(),
        );
        let error = Unregister::domain(domain_id.clone())
            .execute(&authority, &mut transaction)
            .expect_err("domain cascade must retain governed Orchard backing definition");
        assert!(
            error.to_string().contains("backs governed Orchard pool"),
            "{error}"
        );
        assert!(transaction.world.domains.get(&domain_id).is_some());
        assert!(
            transaction
                .world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some()
        );
        assert!(transaction.world.accounts.get(&reserve_account).is_some());
        assert!(
            transaction
                .world
                .privacy_commitments
                .get(&state_key)
                .is_some()
        );
    }
    #[test]
    fn domain_controller_capabilities_check_multisig_members_with_checked_algorithm_access() {
        let allowed = [Algorithm::Ed25519];
        let allowed_curve_ids =
            iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(&allowed);
        let ed25519 = fixture_keypair(0x51, Algorithm::Ed25519);
        let secp256k1 = fixture_keypair(0x52, Algorithm::Secp256k1);
        let allowed_policy = MultisigPolicy::new(
            1,
            vec![MultisigMember::new(ed25519.public_key().clone(), 1).expect("member")],
        )
        .expect("policy");
        super::isi::ensure_controller_capabilities(
            &AccountController::Multisig(allowed_policy),
            &allowed,
            &allowed_curve_ids,
        )
        .expect("Ed25519 multisig member should be accepted");
        let disallowed_policy = MultisigPolicy::new(
            1,
            vec![MultisigMember::new(secp256k1.public_key().clone(), 1).expect("member")],
        )
        .expect("policy");
        let err = super::isi::ensure_controller_capabilities(
            &AccountController::Multisig(disallowed_policy),
            &allowed,
            &allowed_curve_ids,
        )
        .expect_err("Secp256k1 multisig member should be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(_)
        ));
    }
    #[test]
    fn fixture_keypair_uses_checked_seed_derivation() {
        assert_eq!(
            fixture_keypair(0x53, Algorithm::Ed25519).algorithm(),
            Algorithm::Ed25519
        );
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn find_domain_by_id_returns_registered_domain() {
        let mut state = test_state();
        let domain_id = DomainId::try_new("banka", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &ALICE_ID);
        let view = state.view();
        let domain = FindDomainById::new(domain_id.clone())
            .execute(&view)
            .unwrap();
        assert_eq!(domain.id(), &domain_id);
        assert_eq!(domain.owned_by(), &*ALICE_ID);
    }
    #[test]
    fn find_domains_by_account_id_returns_owned_domains_only() {
        use std::collections::BTreeSet;
        let mut state = test_state();
        let owner_domain = DomainId::try_new("owner", "universal").expect("domain id");
        let alice_owned = DomainId::try_new("banka", "universal").expect("domain id");
        let bob_owned = DomainId::try_new("bankb", "universal").expect("domain id");
        let bob_id = AccountId::new(checked_keypair().public_key().clone());
        seed_domain(&mut state, &owner_domain, &ALICE_ID);
        seed_account(&mut state, &ALICE_ID, &owner_domain);
        seed_account(&mut state, &bob_id, &owner_domain);
        seed_domain(&mut state, &alice_owned, &ALICE_ID);
        seed_domain(&mut state, &bob_owned, &bob_id);
        let view = state.view();
        let domains: Vec<_> = FindDomainsByAccountId::new(ALICE_ID.clone())
            .execute(CompoundPredicate::PASS, &view)
            .unwrap()
            .map(|domain| domain.id().clone())
            .collect();
        assert_eq!(
            domains.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([owner_domain, alice_owned])
        );
    }
    #[test]
    fn find_domains_filters_owner_with_owner_index() {
        use std::collections::BTreeSet;
        let mut state = test_state();
        let alice_owned = DomainId::try_new("banka", "universal").expect("domain id");
        let alice_owned_two = DomainId::try_new("cards", "universal").expect("domain id");
        let bob_owned = DomainId::try_new("bankb", "universal").expect("domain id");
        let bob_id = AccountId::new(checked_keypair().public_key().clone());
        seed_domain(&mut state, &alice_owned, &ALICE_ID);
        seed_domain(&mut state, &alice_owned_two, &ALICE_ID);
        seed_domain(&mut state, &bob_owned, &bob_id);
        let view = state.view();
        assert_eq!(
            view.world()
                .domains_owned_by_iter(&ALICE_ID)
                .map(|domain| domain.id().clone())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([alice_owned.clone(), alice_owned_two.clone()]),
            "fixture should populate the owner index used by the query planner",
        );
        let predicate =
            CompoundPredicate::<Domain>::build(|p| p.equals("owned_by", ALICE_ID.to_string()));
        let domains: Vec<_> = FindDomains
            .execute(predicate, &view)
            .unwrap()
            .map(|domain| domain.id().clone())
            .collect();
        assert_eq!(
            domains.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([alice_owned, alice_owned_two])
        );
    }
    #[test]
    fn domain_owner_index_tracks_insert_transfer_and_remove() {
        let state = test_state();
        let owner = (*ALICE_ID).clone();
        let new_owner = AccountId::new(checked_keypair().public_key().clone());
        let domain_id = DomainId::try_new("indexed", "universal").expect("domain id");
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.insert_domain_entry(domain_id.clone(), domain);
        let owned = tx
            .world
            .domains_owned_by_iter(&owner)
            .map(|domain| domain.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(owned, vec![domain_id.clone()]);
        {
            let domain = tx.world.domain_mut(&domain_id).expect("domain exists");
            domain.set_owned_by(new_owner.clone());
        }
        tx.world
            .replace_domain_owner_index(&domain_id, &owner, &new_owner);
        assert!(
            tx.world.domains_owned_by_iter(&owner).next().is_none(),
            "owner index should remove transferred domain from previous owner",
        );
        let owned_by_new_owner = tx
            .world
            .domains_owned_by_iter(&new_owner)
            .map(|domain| domain.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(owned_by_new_owner, vec![domain_id.clone()]);
        tx.world.remove_domain_entry(&domain_id);
        assert!(
            tx.world.domains_owned_by_iter(&new_owner).next().is_none(),
            "owner index should remove unregistered domain",
        );
    }
    fn alias_domain(domain: &DomainId) -> AccountAliasDomain {
        AccountAliasDomain::new(domain.name().clone())
    }
    fn alias_in_domain(domain: &DomainId, label: Name) -> AccountAlias {
        AccountAlias::new(label, Some(alias_domain(domain)), DataSpaceId::UNIVERSAL)
    }
    fn alias_in_dataspace_domain(
        domain: &DomainId,
        dataspace: DataSpaceId,
        label: Name,
    ) -> AccountAlias {
        AccountAlias::new_in_dataspace(label, Some(alias_domain(domain)), dataspace)
    }
    fn resolved_account_alias(
        tx: &StateTransaction<'_, '_>,
        alias: &AccountAlias,
    ) -> ResolvedAccountAliasV1 {
        let literal = alias
            .to_literal(&tx.nexus.dataspace_catalog)
            .expect("test alias must resolve through the live catalog");
        ResolvedAccountAliasV1::new(
            literal
                .parse::<AccountAliasName>()
                .expect("test alias literal must be canonical"),
            alias.dataspace,
        )
    }
    fn repair_only_quote_guard() -> AliasQuoteGuardV1 {
        AliasQuoteGuardV1 {
            expected_policy_version: 0,
            expected_payment_asset: AssetDefinitionId::derive_from_components(
                DomainId::try_new("assets", "universal").expect("fixture asset domain"),
                "xor".parse().expect("fixture asset name"),
            ),
            max_amount: Quantity::zero(),
            valid_until_ms: 0,
        }
    }
    /// Test adapter for exercising declarative setup repair against pre-seeded leases.
    struct EnsureTestAccountAliasBinding {
        account: AccountId,
        alias: Option<AccountAlias>,
        lease_expiry_ms: Option<u64>,
    }
    impl EnsureTestAccountAliasBinding {
        fn bind(account: AccountId, alias: AccountAlias, lease_expiry_ms: Option<u64>) -> Self {
            Self {
                account,
                alias: Some(alias),
                lease_expiry_ms,
            }
        }
        fn clear(account: AccountId) -> Self {
            Self {
                account,
                alias: None,
                lease_expiry_ms: None,
            }
        }
    }
    impl Execute for EnsureTestAccountAliasBinding {
        fn execute(
            self,
            authority: &AccountId,
            tx: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.lease_expiry_ms.is_some() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "alias setup never accepts lease expiry; use RenewAliasLease".into(),
                    ),
                )
                .into());
            }
            let alias = self.alias.ok_or_else(|| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    "broad alias clearing was removed; use explicit lifecycle CAS operations"
                        .into(),
                ))
            })?;
            let resolved = resolved_account_alias(tx, &alias);
            EnsureAlias::new(
                AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                    alias: resolved,
                    target_account: self.account,
                    provision: AccountProvisionV1::Existing,
                    role: AccountAliasRoleV1::Additional,
                }),
                AliasLeaseAcquisitionV1::new(1, None),
                repair_only_quote_guard(),
            )
            .execute(authority, tx)
        }
    }
    /// Test adapter for exercising explicit primary-alias compare-and-set semantics.
    struct CasTestPrimaryAccountAlias {
        account: AccountId,
        alias: Option<AccountAlias>,
        lease_expiry_ms: Option<u64>,
    }
    impl CasTestPrimaryAccountAlias {
        fn bind(account: AccountId, alias: AccountAlias, lease_expiry_ms: Option<u64>) -> Self {
            Self {
                account,
                alias: Some(alias),
                lease_expiry_ms,
            }
        }
        fn clear(account: AccountId) -> Self {
            Self {
                account,
                alias: None,
                lease_expiry_ms: None,
            }
        }
    }
    impl Execute for CasTestPrimaryAccountAlias {
        fn execute(
            self,
            authority: &AccountId,
            tx: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.lease_expiry_ms.is_some() {
                return Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "primary-alias CAS never accepts lease expiry; use RenewAliasLease".into(),
                    ),
                )
                .into());
            }
            let current = tx.world.account(&self.account)?.label().cloned();
            if let Some(alias) = current.as_ref() {
                seed_account_alias_lease_record(tx, &self.account, alias);
            }
            let expected = current
                .as_ref()
                .map(|alias| resolved_account_alias(tx, alias));
            let new_alias = self
                .alias
                .as_ref()
                .map(|alias| resolved_account_alias(tx, alias));
            if let Some(alias) = self.alias
                && current.as_ref() != Some(&alias)
            {
                EnsureTestAccountAliasBinding::bind(self.account.clone(), alias, None)
                    .execute(authority, tx)?;
            }
            CompareAndSetPrimaryAccountAlias::new(self.account, expected, new_alias)
                .execute(authority, tx)
        }
    }
    fn install_retail_dataspace_catalog(
        tx: &mut StateTransaction<'_, '_>,
    ) -> (DataSpaceId, DataSpaceId) {
        let paynet = DataSpaceId::new(12);
        let cbuae = DataSpaceId::new(13);
        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: paynet,
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: cbuae,
                alias: "cbuae".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("retail dataspace catalog");
        tx.nexus.dataspace_catalog = catalog.clone();
        tx.world.dataspace_catalog = catalog;
        (paynet, cbuae)
    }
    fn install_dataspace_catalog_with_lane(
        tx: &mut StateTransaction<'_, '_>,
        dataspace: DataSpaceId,
        alias: &str,
        visibility: LaneVisibility,
    ) {
        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: dataspace,
                alias: alias.to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        tx.nexus.dataspace_catalog = catalog.clone();
        tx.world.dataspace_catalog = catalog;
        tx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: dataspace,
                    alias: alias.to_owned(),
                    visibility,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
    }
    fn seed_account_alias_manage_permissions(
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        alias: &AccountAlias,
    ) {
        if tx.world.account(authority).is_err() {
            let account = Account {
                id: authority.clone(),
                metadata: Metadata::default(),
                label: None,
                uaid: None,
                opaque_ids: Vec::new(),
            };
            let (account_id, account_value) = account.into_key_value();
            tx.world.accounts.insert(account_id, account_value);
        }
        tx.world.add_account_permission(
            authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(alias.dataspace),
            }),
        );
        if let Some(domain_id) = alias
            .domain_id(&tx.nexus.dataspace_catalog)
            .expect("alias domain id")
        {
            tx.world.add_account_permission(
                authority,
                Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain_id),
                }),
            );
        }
    }
    fn seed_contract_alias_manage_permissions(
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        alias: &ContractAlias,
    ) {
        let (label, domain, dataspace) = super::isi::resolve_contract_alias_components(tx, alias)
            .expect("contract alias components");
        seed_contract_alias_manage_permissions_in_dataspace(
            tx, authority, label, domain, dataspace,
        );
    }
    fn seed_contract_alias_manage_permissions_in_dataspace(
        tx: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        label: Name,
        domain: Option<AccountAliasDomain>,
        dataspace: DataSpaceId,
    ) {
        seed_account_alias_manage_permissions(
            tx,
            authority,
            &AccountAlias::new_in_dataspace(label, domain, dataspace),
        );
    }
    fn retail_account_aliases(paynet: DataSpaceId, cbuae: DataSpaceId) -> Vec<AccountAlias> {
        let hbl = DomainId::try_new("hbl", "paynet").expect("hbl domain");
        let ubl = DomainId::try_new("ubl", "paynet").expect("ubl domain");
        vec![
            AccountAlias::domainless("retailpaynet".parse::<Name>().expect("label"), paynet),
            alias_in_dataspace_domain(&hbl, paynet, "retailhbl".parse::<Name>().expect("label")),
            alias_in_dataspace_domain(&ubl, paynet, "retailubl".parse::<Name>().expect("label")),
            AccountAlias::domainless("retailcbuae".parse::<Name>().expect("label"), cbuae),
        ]
    }
    fn seed_account_alias_lease(
        tx: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        alias: &AccountAlias,
    ) {
        seed_account_alias_lease_record(tx, owner, alias);
        if tx.world.account(owner).is_err() {
            let account = Account {
                id: owner.clone(),
                metadata: Metadata::default(),
                label: None,
                uaid: None,
                opaque_ids: Vec::new(),
            };
            let (account_id, account_value) = account.into_key_value();
            tx.world.accounts.insert(account_id, account_value);
        }
        tx.world.add_account_permission(
            owner,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(alias.dataspace),
            }),
        );
        if let Some(domain_id) = alias
            .domain_id(&tx.nexus.dataspace_catalog)
            .expect("domain id")
        {
            tx.world.add_account_permission(
                owner,
                Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Domain(domain_id),
                }),
            );
        }
    }
    fn seed_account_alias_lease_record(
        tx: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        alias: &AccountAlias,
    ) {
        let dataspace_name = tx
            .nexus
            .dataspace_catalog
            .by_id(alias.dataspace)
            .expect("fixture alias dataspace must be catalogued")
            .alias
            .clone();
        let dataspace_selector =
            crate::sns::selector_for_dataspace_alias(&dataspace_name).expect("dataspace selector");
        let dataspace_key = crate::sns::record_storage_key(&dataspace_selector);
        if tx.world.smart_contract_state.get(&dataspace_key).is_none() {
            let address = AccountAddress::from_account_id(owner).expect("account address");
            let mut metadata = Metadata::default();
            metadata.insert(
                crate::sns::SNS_DATASPACE_ID_METADATA_KEY
                    .parse()
                    .expect("static dataspace metadata key"),
                Json::new(alias.dataspace.as_u64()),
            );
            let record = NameRecordV1::new(
                dataspace_selector,
                owner.clone(),
                vec![NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                metadata,
            );
            tx.world
                .smart_contract_state
                .insert(dataspace_key, norito::codec::Encode::encode(&record));
        }
        if let Some(domain_id) = alias
            .domain_id(&tx.nexus.dataspace_catalog)
            .expect("fixture alias domain")
        {
            let domain_owner = tx
                .world
                .domains
                .get(&domain_id)
                .map(|domain| domain.owned_by().clone())
                .unwrap_or_else(|| owner.clone());
            if tx.world.domains.get(&domain_id).is_none() {
                let domain = Domain::new(domain_id.clone()).build(&domain_owner);
                tx.world.insert_domain_entry(domain_id.clone(), domain);
                tx.world.track_domain_owner(&domain_id, &domain_owner);
            }
            let selector =
                crate::sns::selector_for_domain(&domain_id).expect("SNS domain selector");
            let storage_key = crate::sns::record_storage_key(&selector);
            if tx.world.smart_contract_state.get(&storage_key).is_none() {
                let address =
                    AccountAddress::from_account_id(&domain_owner).expect("domain owner address");
                let record = NameRecordV1::new(
                    selector,
                    domain_owner,
                    vec![NameControllerV1::account(&address)],
                    0,
                    0,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    Metadata::default(),
                );
                tx.world
                    .smart_contract_state
                    .insert(storage_key, norito::codec::Encode::encode(&record));
            }
        }
        let selector = crate::sns::selector_for_account_alias(alias, &tx.nexus.dataspace_catalog)
            .expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        tx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }
    fn seed_expired_account_alias_lease_record(
        tx: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        alias: &AccountAlias,
    ) {
        // Seed the active parent dataspace/domain records first, then replace only
        // the account-alias leaf with an expired lifecycle fixture.
        seed_account_alias_lease_record(tx, owner, alias);
        let selector = crate::sns::selector_for_account_alias(alias, &tx.nexus.dataspace_catalog)
            .expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            1,
            2,
            3,
            Metadata::default(),
        );
        tx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }
    fn seed_dataspace_alias_lease(
        tx: &mut StateTransaction<'_, '_>,
        owner: &AccountId,
        alias: &str,
    ) {
        let selector = crate::sns::selector_for_dataspace_alias(alias).expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
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
                scope: AccountAliasPermissionScope::Domain(domain.clone()),
            }),
        );
        tx.world.add_account_permission(
            authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
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
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let account_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(account_label.clone()));
        // Execute register with label.
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &account_label);
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
        let second_keypair = checked_keypair();
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
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let account_label = AccountAlias::domainless(
            "primary".parse::<Name>().expect("label"),
            DataSpaceId::UNIVERSAL,
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_manage_permissions(&mut tx, &authority, &account_label);
        seed_account_alias_lease_record(&mut tx, &account_id, &account_label);
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
    fn register_domainless_account_emits_direct_created_event() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register domainless account");
        let created = tx
            .world
            .internal_event_buf
            .iter()
            .find_map(|event| match event.as_ref() {
                DataEvent::Account(AccountEvent::Created(created))
                    if created.account.id() == &account_id =>
                {
                    Some(created.clone())
                }
                _ => None,
            })
            .expect("account created event");
        assert_eq!(created.account.id(), &account_id);
        assert!(tx.world.internal_event_buf.iter().all(|event| {
            !matches!(event.as_ref(), DataEvent::Domain(DomainEvent::Account(_)))
        }));
    }
    #[test]
    fn register_existing_plain_account_returns_repetition_error() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let error = Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect_err("explicit duplicate registration must fail");
        assert_eq!(
            error,
            InstructionExecutionError::Repetition(RepetitionError {
                instruction: InstructionType::Register,
                id: IdBox::AccountId(account_id),
            })
        );
    }
    #[test]
    fn register_account_with_label_still_emits_direct_created_event() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let account_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &account_label);
        Register::account(Account::new(account_id.clone()).with_label(Some(account_label)))
            .execute(&authority, &mut tx)
            .expect("register account with label");
        let created = tx
            .world
            .internal_event_buf
            .iter()
            .find_map(|event| match event.as_ref() {
                DataEvent::Account(AccountEvent::Created(created))
                    if created.account.id() == &account_id =>
                {
                    Some(created.clone())
                }
                _ => None,
            })
            .expect("account created event");
        assert_eq!(created.account.id(), &account_id);
        assert!(tx.world.internal_event_buf.iter().all(|event| {
            !matches!(event.as_ref(), DataEvent::Domain(DomainEvent::Account(_)))
        }));
    }
    #[test]
    fn register_account_with_label_requires_active_sns_lease() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        let err = Register::account(Account::new(account_id.clone()).with_label(Some(label)))
            .execute(&authority, &mut tx)
            .expect_err("alias lease should be required");
        assert!(
            instruction_error_contains(&err, "active SNS lease"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn register_account_with_label_rejects_lease_owned_by_another_account() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        let lease_owner = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease(&mut tx, &lease_owner, &label);
        let err =
            Register::account(Account::new(account_id.clone()).with_label(Some(label.clone())))
                .execute(&authority, &mut tx)
                .expect_err("another account's lease must not authorize registration");
        assert!(
            instruction_error_contains(&err, "owned by another account"),
            "unexpected error: {err}"
        );
        assert!(tx.world.account(&account_id).is_err());
        assert!(tx.world.account_aliases.get(&label).is_none());
    }
    #[test]
    fn register_account_with_retail_aliases_requires_active_sns_lease() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let (paynet, cbuae) = install_retail_dataspace_catalog(&mut tx);
        for alias in retail_account_aliases(paynet, cbuae) {
            let account_id = AccountId::new(checked_keypair().public_key().clone());
            seed_account_alias_manage_permissions(&mut tx, &authority, &alias);
            let err =
                Register::account(Account::new(account_id.clone()).with_label(Some(alias.clone())))
                    .execute(&authority, &mut tx)
                    .expect_err("retail aliases must require an SNS lease");
            assert!(
                instruction_error_contains(&err, "active SNS lease"),
                "unexpected error for {alias:?}: {err}"
            );
            assert!(tx.world.account_aliases.get(&alias).is_none());
        }
    }
    #[test]
    fn set_account_label_relabels_existing_single_key_account() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let old_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let new_label = alias_in_domain(&domain_id, "treasury".parse::<Name>().unwrap());
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(old_label.clone()));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_manage_permissions(&mut tx, &authority, &old_label);
        seed_account_alias_lease_record(&mut tx, &account_id, &old_label);
        seed_account_alias_lease_record(&mut tx, &account_id, &new_label);
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with initial label");
        CasTestPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(new_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("relabel existing account");
        assert_eq!(
            tx.world.account_aliases.get(&old_label),
            Some(&account_id),
            "primary CAS must retain the former alias as an additional binding"
        );
        assert!(
            tx.world.account_rekey_records.get(&old_label).is_some(),
            "primary CAS must retain former alias continuity"
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
    fn primary_alias_cas_allows_domainful_alias_without_domain_link() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let label = alias_in_domain(&domain_id, "treasury".parse::<Name>().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register domainless account");
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &label);
        CasTestPrimaryAccountAlias {
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
    fn primary_alias_setup_rejects_stale_non_empty_binding() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let stale_owner = AccountId::new(checked_keypair().public_key().clone());
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .account_aliases
            .insert(label.clone(), stale_owner.clone());
        tx.world.account_rekey_records.insert(
            label.clone(),
            AccountRekeyRecord::new(label.clone(), stale_owner.clone()),
        );
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &label);
        let error = CasTestPrimaryAccountAlias {
            account: account_id.clone(),
            alias: Some(label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("declarative setup must not reclaim a non-empty binding");
        assert!(instruction_error_contains(&error, "alias.binding.conflict"));
        assert_eq!(
            tx.world.account_aliases.get(&label),
            Some(&stale_owner),
            "conflicting setup must preserve the stale binding for explicit remediation"
        );
        assert_eq!(
            tx.world
                .account_rekey_records
                .get(&label)
                .expect("rekey record should exist")
                .active_account_id,
            stale_owner,
            "conflicting setup must preserve continuity state"
        );
    }
    #[test]
    fn bind_account_alias_requires_active_sns_lease() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let err = RebindAccountAlias::new(
            resolved_account_alias(&tx, &alias),
            account_id.clone(),
            account_id,
        )
        .execute(&authority, &mut tx)
        .expect_err("alias lease should be required");
        assert!(
            instruction_error_contains(&err, "not found"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn alias_binding_and_primary_alias_reject_lease_owned_by_another_account() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        let lease_owner = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &lease_owner, &alias);
        for err in [
            EnsureTestAccountAliasBinding {
                account: account_id.clone(),
                alias: Some(alias.clone()),
                lease_expiry_ms: None,
            }
            .execute(&authority, &mut tx)
            .expect_err("another account's lease must not authorize an alias binding"),
            CasTestPrimaryAccountAlias {
                account: account_id.clone(),
                alias: Some(alias.clone()),
                lease_expiry_ms: None,
            }
            .execute(&authority, &mut tx)
            .expect_err("another account's lease must not authorize a primary alias"),
        ] {
            assert!(
                instruction_error_contains(&err, "alias.owner.conflict"),
                "unexpected error: {err}"
            );
        }
        assert!(tx.world.account_aliases.get(&alias).is_none());
        assert!(
            tx.world
                .account(&account_id)
                .expect("account")
                .label()
                .is_none()
        );
    }
    #[test]
    fn account_alias_mutations_reject_expired_leases_even_during_replay() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let existing_id = AccountId::new(checked_keypair().public_key().clone());
        let registration_id = AccountId::new(checked_keypair().public_key().clone());
        let registration_alias =
            alias_in_domain(&domain_id, "register_expired".parse::<Name>().unwrap());
        let binding_alias = alias_in_domain(&domain_id, "binding_expired".parse::<Name>().unwrap());
        let primary_alias = alias_in_domain(&domain_id, "primary_expired".parse::<Name>().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.replay_compatibility = true;
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(existing_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register existing account");
        seed_expired_account_alias_lease_record(&mut tx, &registration_id, &registration_alias);
        seed_expired_account_alias_lease_record(&mut tx, &existing_id, &binding_alias);
        seed_expired_account_alias_lease_record(&mut tx, &existing_id, &primary_alias);
        let registration_err = Register::account(
            Account::new(registration_id.clone()).with_label(Some(registration_alias.clone())),
        )
        .execute(&authority, &mut tx)
        .expect_err("replay must not bypass an expired registration lease");
        assert!(
            instruction_error_contains(&registration_err, "active SNS lease"),
            "unexpected registration error: {registration_err}"
        );
        let binding_err =
            EnsureTestAccountAliasBinding::bind(existing_id.clone(), binding_alias.clone(), None)
                .execute(&authority, &mut tx)
                .expect_err("replay must not bypass an expired binding lease");
        let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            binding_message,
        )) = &binding_err
        else {
            panic!("unexpected expired-binding error: {binding_err:?}");
        };
        assert!(
            binding_message.contains("alias.lifecycle.conflict"),
            "unexpected expired-binding payload: {binding_message}"
        );
        let primary_err =
            CasTestPrimaryAccountAlias::bind(existing_id.clone(), primary_alias.clone(), None)
                .execute(&authority, &mut tx)
                .expect_err("replay must not bypass an expired primary-alias lease");
        let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            primary_message,
        )) = &primary_err
        else {
            panic!("unexpected expired-primary error: {primary_err:?}");
        };
        assert!(
            primary_message.contains("alias.lifecycle.conflict"),
            "unexpected expired-primary payload: {primary_message}"
        );
        assert!(tx.world.account(&registration_id).is_err());
        assert!(tx.world.account_aliases.get(&registration_alias).is_none());
        assert!(tx.world.account_aliases.get(&binding_alias).is_none());
        assert!(tx.world.account_aliases.get(&primary_alias).is_none());
        assert!(
            tx.world
                .account(&existing_id)
                .expect("existing account")
                .label()
                .is_none()
        );
    }
    #[test]
    fn account_alias_binding_instructions_cannot_bypass_paid_sns_renewal() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let alias = alias_in_domain(&domain_id, "renewal".parse::<Name>().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_lease_record(&mut tx, &account_id, &alias);
        for err in [
            EnsureTestAccountAliasBinding::bind(account_id.clone(), alias.clone(), Some(20))
                .execute(&authority, &mut tx)
                .expect_err("binding must not renew an SNS lease"),
            CasTestPrimaryAccountAlias::bind(account_id.clone(), alias.clone(), Some(20))
                .execute(&authority, &mut tx)
                .expect_err("primary binding must not renew an SNS lease"),
            EnsureTestAccountAliasBinding {
                account: account_id.clone(),
                alias: None,
                lease_expiry_ms: Some(20),
            }
            .execute(&authority, &mut tx)
            .expect_err("clear plus lease expiry must be rejected"),
            CasTestPrimaryAccountAlias {
                account: account_id.clone(),
                alias: None,
                lease_expiry_ms: Some(20),
            }
            .execute(&authority, &mut tx)
            .expect_err("primary clear plus lease expiry must be rejected"),
        ] {
            let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                message,
            )) = err
            else {
                panic!("unexpected error: {err:?}");
            };
            assert!(
                message.contains("RenewAliasLease") || message.contains("requires alias binding"),
                "unexpected error: {message}"
            );
        }
        assert!(tx.world.account_aliases.get(&alias).is_none());
        assert!(
            tx.world
                .account(&account_id)
                .expect("account")
                .label()
                .is_none()
        );
        let selector = crate::sns::selector_for_account_alias(&alias, &tx.nexus.dataspace_catalog)
            .expect("selector");
        let bytes = tx
            .world
            .smart_contract_state
            .get(&crate::sns::record_storage_key(&selector))
            .expect("lease record");
        let mut slice = bytes.as_slice();
        let record = NameRecordV1::decode(&mut slice).expect("decode lease record");
        assert_eq!(record.expires_at_ms, u64::MAX);
    }
    #[test]
    fn bind_account_alias_in_retail_namespace_requires_active_sns_lease() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let (paynet, _) = install_retail_dataspace_catalog(&mut tx);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let alias = AccountAlias::domainless("bindretail".parse::<Name>().expect("label"), paynet);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_manage_permissions(&mut tx, &authority, &alias);
        let err = RebindAccountAlias::new(
            resolved_account_alias(&tx, &alias),
            account_id.clone(),
            account_id.clone(),
        )
        .execute(&authority, &mut tx)
        .expect_err("retail alias binding must require an SNS lease");
        assert!(
            instruction_error_contains(&err, "not found"),
            "unexpected error: {err}"
        );
        assert!(tx.world.account_aliases.get(&alias).is_none());
    }
    #[test]
    fn retail_account_alias_rejects_asset_alias_permission() {
        let state = test_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let (paynet, _) = install_retail_dataspace_catalog(&mut tx);
        let hbl = DomainId::try_new("hbl", "paynet").expect("hbl domain");
        let alias = alias_in_dataspace_domain(&hbl, paynet, "cbdc".parse::<Name>().expect("label"));
        let current_account_id = AccountId::new(checked_keypair().public_key().clone());
        let replacement_account_id = AccountId::new(checked_keypair().public_key().clone());
        for account_id in [&current_account_id, &replacement_account_id] {
            let account = Account {
                id: account_id.clone(),
                metadata: Metadata::default(),
                label: None,
                uaid: None,
                opaque_ids: Vec::new(),
            };
            let (account_id, account_value) = account.into_key_value();
            tx.world.accounts.insert(account_id, account_value);
        }
        tx.world
            .insert_account_alias_binding(alias.clone(), current_account_id.clone());
        tx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), current_account_id.clone()),
        );
        seed_account_alias_lease_record(&mut tx, &replacement_account_id, &alias);
        tx.world.add_account_permission(
            &current_account_id,
            Permission::from(CanManageAssetDefinitionAlias {
                scope: AssetDefinitionAliasPermissionScope::Domain(hbl.clone()),
            }),
        );
        let err = RebindAccountAlias::new(
            resolved_account_alias(&tx, &alias),
            current_account_id.clone(),
            replacement_account_id.clone(),
        )
        .execute(&current_account_id, &mut tx)
        .expect_err("asset-definition alias permission must not authorize account aliases");
        assert!(
            instruction_error_contains(&err, "exact management permission"),
            "unexpected error: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&current_account_id),
            "rejected mutation must preserve the original alias binding"
        );
        tx.world.add_account_permission(
            &current_account_id,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(hbl),
            }),
        );
        RebindAccountAlias::new(
            resolved_account_alias(&tx, &alias),
            current_account_id.clone(),
            replacement_account_id.clone(),
        )
        .execute(&current_account_id, &mut tx)
        .expect("exact domain permission should authorize the alias mutation");
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&replacement_account_id)
        );
        assert_eq!(
            tx.world
                .account_rekey_records
                .get(&alias)
                .expect("rekey record should follow alias")
                .active_account_id,
            replacement_account_id
        );
    }
    #[test]
    fn cbdc_fi_account_alias_management_isolated_by_exact_domain() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let retail = DataSpaceId::new(10);
        install_dataspace_catalog_with_lane(&mut tx, retail, "sbp", LaneVisibility::Public);
        let hbl = DomainId::try_new("hbl", "sbp").expect("HBL alias domain");
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL alias domain");
        let hbl_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "customerhbl".parse::<Name>().expect("HBL alias label"),
        );
        let ubl_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "customerubl".parse::<Name>().expect("UBL alias label"),
        );
        let hbl_secondary_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "secondaryhbl"
                .parse::<Name>()
                .expect("secondary HBL alias label"),
        );
        let domainless_alias =
            AccountAlias::domainless("retailroot".parse::<Name>().expect("root alias"), retail);
        let target = AccountId::new(checked_keypair().public_key().clone());
        let replacement = AccountId::new(checked_keypair().public_key().clone());
        for account in [&target, &replacement] {
            Register::account(Account::new(account.clone()))
                .execute(&authority, &mut tx)
                .expect("register alias target");
        }
        for alias in [
            &hbl_alias,
            &ubl_alias,
            &hbl_secondary_alias,
            &domainless_alias,
        ] {
            seed_account_alias_lease_record(&mut tx, &target, alias);
        }
        let hbl_permission: Permission = CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(hbl.clone()),
        }
        .into();
        tx.world
            .add_account_permission(&authority, hbl_permission.clone());
        EnsureTestAccountAliasBinding {
            account: target.clone(),
            alias: Some(hbl_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("exact HBL domain permission must authorize an HBL alias");
        for forbidden_alias in [&ubl_alias, &domainless_alias] {
            let err = EnsureTestAccountAliasBinding {
                account: target.clone(),
                alias: Some(forbidden_alias.clone()),
                lease_expiry_ms: None,
            }
            .execute(&authority, &mut tx)
            .expect_err("HBL domain permission must not authorize another alias scope");
            assert!(
                instruction_error_contains(&err, "alias.setup.authority_forbidden"),
                "unexpected cross-scope error: {err}"
            );
            assert!(
                tx.world.account_aliases.get(forbidden_alias).is_none(),
                "rejected cross-scope alias must remain unbound"
            );
        }
        tx.world
            .insert_account_alias_binding(ubl_alias.clone(), target.clone());
        tx.world.account_rekey_records.insert(
            ubl_alias.clone(),
            AccountRekeyRecord::new(ubl_alias.clone(), target.clone()),
        );
        let err = RebindAccountAlias::new(
            resolved_account_alias(&tx, &ubl_alias),
            target.clone(),
            replacement,
        )
        .execute(&authority, &mut tx)
        .expect_err("HBL authority must not rebind a UBL secondary alias");
        assert!(
            instruction_error_contains(&err, "exact management permission"),
            "unexpected secondary-rebind error: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&hbl_alias),
            Some(&target),
            "secondary rebind must preflight every alias before mutating"
        );
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&target));
        tx.world.remove_account_alias_binding(&ubl_alias);
        tx.world.account_rekey_records.remove(ubl_alias.clone());
        assert!(
            tx.world
                .remove_account_permission(&authority, &hbl_permission),
            "HBL permission fixture should be removable"
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(retail),
            }),
        );
        for forbidden_alias in [&hbl_secondary_alias, &ubl_alias] {
            let err = EnsureTestAccountAliasBinding {
                account: target.clone(),
                alias: Some(forbidden_alias.clone()),
                lease_expiry_ms: None,
            }
            .execute(&authority, &mut tx)
            .expect_err("dataspace permission must not authorize a domainful FI alias");
            assert!(
                instruction_error_contains(&err, "alias.setup.authority_forbidden"),
                "unexpected domainful-alias error: {err}"
            );
        }
        EnsureTestAccountAliasBinding {
            account: target.clone(),
            alias: Some(domainless_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("dataspace permission must authorize a domainless alias");
        assert_eq!(
            tx.world.account_aliases.get(&domainless_alias),
            Some(&target)
        );
        assert_eq!(tx.world.account_aliases.get(&hbl_alias), Some(&target));
        assert!(tx.world.account_aliases.get(&hbl_secondary_alias).is_none());
        assert!(tx.world.account_aliases.get(&ubl_alias).is_none());
    }
    #[test]
    fn cbdc_retail_account_rejects_cross_fi_secondary_alias_and_repoint() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let retail = DataSpaceId::new(10);
        install_dataspace_catalog_with_lane(&mut tx, retail, "sbp", LaneVisibility::Public);
        let hbl = DomainId::try_new("hbl", "sbp").expect("HBL alias domain");
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL alias domain");
        let hbl_home_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "homehbl".parse::<Name>().expect("HBL home alias label"),
        );
        let hbl_secondary_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "secondaryhbl"
                .parse::<Name>()
                .expect("HBL secondary alias label"),
        );
        let hbl_foreign_target_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "foreignhbl"
                .parse::<Name>()
                .expect("HBL foreign-target alias label"),
        );
        let ubl_home_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "homeubl".parse::<Name>().expect("UBL home alias label"),
        );
        let hbl_account = AccountId::new(checked_keypair().public_key().clone());
        let ubl_account = AccountId::new(checked_keypair().public_key().clone());
        let unhomed_account = AccountId::new(checked_keypair().public_key().clone());
        for account in [&hbl_account, &ubl_account, &unhomed_account] {
            Register::account(Account::new(account.clone()))
                .execute(&authority, &mut tx)
                .expect("register retail account");
        }
        for alias in [&hbl_home_alias, &hbl_secondary_alias] {
            seed_account_alias_lease_record(&mut tx, &hbl_account, alias);
        }
        seed_account_alias_lease_record(&mut tx, &ubl_account, &hbl_foreign_target_alias);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(hbl),
            }),
        );
        EnsureTestAccountAliasBinding {
            account: hbl_account.clone(),
            alias: Some(hbl_home_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind HBL home alias");
        EnsureTestAccountAliasBinding {
            account: hbl_account.clone(),
            alias: Some(hbl_secondary_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("same-FI secondary aliases remain supported");
        tx.world
            .insert_account_alias_binding(ubl_home_alias.clone(), ubl_account.clone());
        tx.world.account_rekey_records.insert(
            ubl_home_alias.clone(),
            AccountRekeyRecord::new(ubl_home_alias.clone(), ubl_account.clone()),
        );
        seed_account_alias_lease_record(&mut tx, &ubl_account, &ubl_home_alias);
        let err = EnsureTestAccountAliasBinding {
            account: ubl_account.clone(),
            alias: Some(hbl_foreign_target_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("HBL must not add a secondary alias to a UBL-home account");
        assert!(
            instruction_error_contains(&err, "jointly-authorized migration"),
            "unexpected cross-FI secondary-binding error: {err}"
        );
        assert!(
            tx.world
                .account_aliases
                .get(&hbl_foreign_target_alias)
                .is_none(),
            "rejected foreign-home secondary alias must remain unbound"
        );
        let err = EnsureTestAccountAliasBinding {
            account: ubl_account.clone(),
            alias: Some(hbl_home_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("HBL must not repoint an existing alias to a UBL-home account");
        assert!(
            instruction_error_contains(&err, "alias.owner.conflict"),
            "unexpected cross-FI repoint error: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&hbl_home_alias),
            Some(&hbl_account),
            "rejected cross-FI repoint must preserve the HBL binding"
        );
        assert_eq!(
            tx.world.account_aliases.get(&ubl_home_alias),
            Some(&ubl_account),
            "rejected cross-FI repoint must preserve the UBL home"
        );
        tx.world.remove_account_alias_binding(&hbl_secondary_alias);
        tx.world
            .account_rekey_records
            .remove(hbl_secondary_alias.clone());
        let err = EnsureTestAccountAliasBinding {
            account: unhomed_account,
            alias: Some(hbl_home_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("repointing must not strand the previous account without an FI home");
        assert!(
            instruction_error_contains(&err, "alias.owner.conflict"),
            "unexpected last-home repoint error: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&hbl_home_alias),
            Some(&hbl_account),
            "rejected last-home repoint must preserve its source binding"
        );
    }
    #[test]
    fn clearing_primary_status_keeps_retail_home_binding_and_blocks_cross_fi_setup() {
        let state = test_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let retail = DataSpaceId::new(10);
        install_dataspace_catalog_with_lane(&mut tx, retail, "sbp", LaneVisibility::Public);
        let hbl = DomainId::try_new("hbl", "sbp").expect("HBL alias domain");
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL alias domain");
        let hbl_manager = AccountId::new(checked_keypair().public_key().clone());
        let ubl_manager = AccountId::new(checked_keypair().public_key().clone());
        let customer = AccountId::new(checked_keypair().public_key().clone());
        for account in [&hbl_manager, &ubl_manager, &customer] {
            Register::account(Account::new(account.clone()))
                .execute(&hbl_manager, &mut tx)
                .expect("register CBDC alias fixture account");
        }
        let hbl_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "switchhbl".parse::<Name>().expect("HBL alias label"),
        );
        let ubl_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "homeubl".parse::<Name>().expect("UBL alias label"),
        );
        tx.world
            .account_mut(&customer)
            .expect("customer account")
            .set_label(Some(ubl_alias.clone()));
        tx.world
            .insert_account_alias_binding(ubl_alias.clone(), customer.clone());
        tx.world.account_rekey_records.insert(
            ubl_alias.clone(),
            AccountRekeyRecord::new(ubl_alias.clone(), customer.clone()),
        );
        tx.world.add_account_permission(
            &ubl_manager,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(ubl),
            }),
        );
        tx.world.add_account_permission(
            &hbl_manager,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(hbl),
            }),
        );
        seed_account_alias_lease_record(&mut tx, &customer, &hbl_alias);
        CasTestPrimaryAccountAlias::clear(customer.clone())
            .execute(&ubl_manager, &mut tx)
            .expect("explicit primary CAS may clear status without unbinding the FI home");
        assert_eq!(tx.world.account(&customer).expect("customer").label(), None);
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&customer));
        let bind_err = EnsureTestAccountAliasBinding {
            account: customer.clone(),
            alias: Some(hbl_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&hbl_manager, &mut tx)
        .expect_err("HBL manager must not bind while the UBL home remains bound");
        assert!(
            bind_err
                .to_string()
                .contains("jointly-authorized migration"),
            "unexpected cross-FI bind error: {bind_err}"
        );
        assert!(tx.world.account_aliases.get(&hbl_alias).is_none());
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&customer));
    }
    #[test]
    fn cbdc_retail_same_fi_primary_rotation_remains_supported() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let retail = DataSpaceId::new(10);
        install_dataspace_catalog_with_lane(&mut tx, retail, "sbp", LaneVisibility::Public);
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL alias domain");
        let old_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "oldubl".parse::<Name>().expect("old UBL alias label"),
        );
        let new_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "newubl".parse::<Name>().expect("new UBL alias label"),
        );
        let customer = AccountId::new(checked_keypair().public_key().clone());
        Register::account(Account::new(customer.clone()))
            .execute(&authority, &mut tx)
            .expect("register UBL customer");
        tx.world
            .account_mut(&customer)
            .expect("customer account")
            .set_label(Some(old_alias.clone()));
        tx.world
            .insert_account_alias_binding(old_alias.clone(), customer.clone());
        tx.world.account_rekey_records.insert(
            old_alias.clone(),
            AccountRekeyRecord::new(old_alias.clone(), customer.clone()),
        );
        seed_account_alias_lease_record(&mut tx, &customer, &new_alias);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(ubl),
            }),
        );
        EnsureTestAccountAliasBinding {
            account: customer.clone(),
            alias: Some(new_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("add a same-FI secondary alias before rotation");
        CasTestPrimaryAccountAlias {
            account: customer.clone(),
            alias: Some(new_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("atomically rotate the primary alias within UBL");
        assert_eq!(
            tx.world.account(&customer).expect("customer").label(),
            Some(&new_alias)
        );
        assert_eq!(tx.world.account_aliases.get(&old_alias), Some(&customer));
        assert_eq!(tx.world.account_aliases.get(&new_alias), Some(&customer));
    }
    #[test]
    fn cleared_primary_status_does_not_enable_cross_fi_replacement() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let retail = DataSpaceId::new(10);
        install_dataspace_catalog_with_lane(&mut tx, retail, "sbp", LaneVisibility::Public);
        let hbl = DomainId::try_new("hbl", "sbp").expect("HBL alias domain");
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL alias domain");
        let hbl_alias = alias_in_dataspace_domain(
            &hbl,
            retail,
            "replacementhbl".parse::<Name>().expect("HBL alias label"),
        );
        let ubl_alias = alias_in_dataspace_domain(
            &ubl,
            retail,
            "primaryubl".parse::<Name>().expect("UBL alias label"),
        );
        let target = AccountId::new(checked_keypair().public_key().clone());
        Register::account(Account::new(target.clone()))
            .execute(&authority, &mut tx)
            .expect("register alias target");
        tx.world
            .account_mut(&target)
            .expect("alias target")
            .set_label(Some(ubl_alias.clone()));
        tx.world
            .insert_account_alias_binding(ubl_alias.clone(), target.clone());
        tx.world.account_rekey_records.insert(
            ubl_alias.clone(),
            AccountRekeyRecord::new(ubl_alias.clone(), target.clone()),
        );
        seed_account_alias_lease_record(&mut tx, &target, &hbl_alias);
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(hbl),
            }),
        );
        let err = CasTestPrimaryAccountAlias {
            account: target.clone(),
            alias: Some(hbl_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("HBL permission must not remove an existing UBL primary alias");
        assert!(
            instruction_error_contains(&err, "jointly-authorized migration"),
            "unexpected primary-replacement error: {err}"
        );
        assert_eq!(
            tx.world.account(&target).expect("alias target").label(),
            Some(&ubl_alias)
        );
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&target));
        assert!(tx.world.account_aliases.get(&hbl_alias).is_none());
        let err = CasTestPrimaryAccountAlias {
            account: target.clone(),
            alias: None,
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("HBL permission must not clear an existing UBL primary alias");
        assert!(
            instruction_error_contains(&err, "exact management permission"),
            "unexpected primary-clear error: {err}"
        );
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&target));
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(ubl),
            }),
        );
        CasTestPrimaryAccountAlias::clear(target.clone())
            .execute(&authority, &mut tx)
            .expect("primary status may clear while the UBL home binding remains");
        let err = CasTestPrimaryAccountAlias {
            account: target.clone(),
            alias: Some(hbl_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("cross-FI primary migration requires an explicit joint instruction");
        assert!(
            instruction_error_contains(&err, "jointly-authorized migration"),
            "unexpected cross-FI primary migration error: {err}"
        );
        assert_eq!(
            tx.world.account(&target).expect("alias target").label(),
            None
        );
        assert!(tx.world.account_aliases.get(&hbl_alias).is_none());
        assert_eq!(tx.world.account_aliases.get(&ubl_alias), Some(&target));
    }
    #[test]
    fn primary_alias_cas_in_retail_namespace_requires_active_sns_lease() {
        let authority = (*ALICE_ID).clone();
        let state = test_state_with_authority(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let (_, cbuae) = install_retail_dataspace_catalog(&mut tx);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let alias =
            AccountAlias::domainless("primaryretail".parse::<Name>().expect("label"), cbuae);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_account_alias_manage_permissions(&mut tx, &authority, &alias);
        let err = CompareAndSetPrimaryAccountAlias::new(
            account_id.clone(),
            None,
            Some(resolved_account_alias(&tx, &alias)),
        )
        .execute(&authority, &mut tx)
        .expect_err("retail primary alias must require an SNS lease");
        assert!(
            instruction_error_contains(&err, "not found"),
            "unexpected error: {err}"
        );
        assert!(tx.world.account_aliases.get(&alias).is_none());
    }
    #[test]
    fn set_account_label_binds_existing_multisig_account_with_rekey_record() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let member_a = MultisigMember::new(checked_keypair().public_key().clone(), 1)
            .expect("multisig member");
        let member_b = MultisigMember::new(checked_keypair().public_key().clone(), 1)
            .expect("multisig member");
        let policy = MultisigPolicy::new(2, vec![member_a, member_b]).expect("multisig policy");
        let account_id = AccountId::new_multisig(policy);
        let account_label = alias_in_domain(&domain_id, "cbdc".parse::<Name>().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register unlabeled multisig account");
        seed_account_alias_lease(&mut tx, &account_id, &account_label);
        CasTestPrimaryAccountAlias {
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
    fn account_rekey_upsert_records_alias_reassignment_provenance() {
        let state = test_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let alias = AccountAlias::domainless(
            "direct-reassignment".parse().expect("alias label"),
            DataSpaceId::UNIVERSAL,
        );
        let first = AccountId::new(checked_keypair().public_key().clone());
        let second = AccountId::new(checked_keypair().public_key().clone());
        upsert_account_rekey_record(&mut tx, &alias, &first).expect("initial continuity record");
        upsert_account_rekey_record(&mut tx, &alias, &second).expect("alias reassignment");
        let record = tx
            .world
            .account_rekey_records
            .get(&alias)
            .expect("updated continuity record");
        assert_eq!(record.active_account_id, second);
        assert_eq!(record.previous_account_ids, vec![first]);
        assert_eq!(
            record.transition_provenance,
            vec![AccountRekeyTransitionProvenance::AliasReassignment]
        );
        let active_account_id = record.active_account_id.clone();
        let mismatched_alias = AccountAlias::domainless(
            "mismatched".parse().expect("alias label"),
            DataSpaceId::UNIVERSAL,
        );
        tx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(mismatched_alias, active_account_id),
        );
        let replacement = AccountId::new(checked_keypair().public_key().clone());
        let error = upsert_account_rekey_record(&mut tx, &alias, &replacement)
            .expect_err("mismatched embedded alias must fail closed");
        assert!(error.to_string().contains("mismatched continuity record"));
    }
    #[test]
    fn set_account_label_rejects_account_registrar_repointing_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        seed_account_alias_manage_permissions(&mut tx, &domain_owner, &alias);
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &first_id, &alias);
        CasTestPrimaryAccountAlias {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");
        seed_account_alias_lease(&mut tx, &second_id, &alias);
        let error = CasTestPrimaryAccountAlias {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect_err("registrar must use an explicit CAS rebind operation");
        assert!(
            instruction_error_contains(&error, "alias.binding.conflict"),
            "unexpected error: {error}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "conflicting setup must preserve the existing binding"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            Some(&alias),
            "conflicting setup must preserve the existing primary alias"
        );
        assert_eq!(
            tx.world
                .account(&second_id)
                .expect("second account should exist")
                .label(),
            None,
            "conflicting setup must not modify the requested target"
        );
        let rekey_record = tx
            .world
            .account_rekey_records
            .get(&alias)
            .expect("existing binding should retain its continuity record");
        assert_eq!(
            rekey_record.active_account_id, first_id,
            "rekey record must retain the existing account"
        );
        assert!(rekey_record.previous_account_ids.is_empty());
        assert!(rekey_record.transition_provenance.is_empty());
        assert_eq!(
            rekey_record.active_signatory,
            Some(first_keypair.public_key().clone()),
            "rekey record must retain the existing controller"
        );
    }
    #[test]
    fn set_account_label_rejects_global_registrar_repointing_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        seed_account_alias_manage_permissions(&mut tx, &domain_owner, &alias);
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &first_id, &alias);
        CasTestPrimaryAccountAlias {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");
        seed_account_alias_lease(&mut tx, &second_id, &alias);
        let error = CasTestPrimaryAccountAlias {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect_err("global registrar must use an explicit CAS rebind operation");
        assert!(instruction_error_contains(&error, "alias.binding.conflict"));
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "conflicting setup must preserve the existing binding"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            Some(&alias),
            "conflicting setup must preserve the existing primary alias"
        );
        assert_eq!(
            tx.world
                .account(&second_id)
                .expect("second account should exist")
                .label(),
            None,
            "conflicting setup must not modify the requested target"
        );
    }
    #[test]
    fn bind_account_alias_adds_multiple_aliases_to_existing_multisig_account() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let member_a = MultisigMember::new(checked_keypair().public_key().clone(), 1)
            .expect("multisig member");
        let member_b = MultisigMember::new(checked_keypair().public_key().clone(), 1)
            .expect("multisig member");
        let policy = MultisigPolicy::new(2, vec![member_a, member_b]).expect("multisig policy");
        let account_id = AccountId::new_multisig(policy);
        let banking_label = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let issuance_label = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register unlabeled multisig account");
        seed_account_alias_lease(&mut tx, &account_id, &banking_label);
        seed_account_alias_lease(&mut tx, &account_id, &issuance_label);
        EnsureTestAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(banking_label.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind banking alias");
        EnsureTestAccountAliasBinding {
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
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let primary_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let bound_label =
            AccountAlias::domainless("public".parse::<Name>().unwrap(), DataSpaceId::UNIVERSAL);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &primary_label);
        seed_account_alias_lease_record(&mut tx, &account_id, &bound_label);
        Register::account(Account::new(account_id.clone()).with_label(Some(primary_label.clone())))
            .execute(&authority, &mut tx)
            .expect("register account with primary label");
        EnsureTestAccountAliasBinding {
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
    fn broad_account_alias_binding_clear_is_rejected_without_mutation() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        seed_account(&mut state, &authority, &domain_id);
        let primary_label = alias_in_domain(&domain_id, "primary".parse::<Name>().unwrap());
        let root_alias =
            AccountAlias::domainless("public".parse::<Name>().unwrap(), DataSpaceId::UNIVERSAL);
        let domain_alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &primary_label);
        seed_account_alias_lease_record(&mut tx, &account_id, &root_alias);
        seed_account_alias_lease_record(&mut tx, &account_id, &domain_alias);
        Register::account(Account::new(account_id.clone()).with_label(Some(primary_label.clone())))
            .execute(&authority, &mut tx)
            .expect("register account with primary label");
        EnsureTestAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(root_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind root alias");
        EnsureTestAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(domain_alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect("bind domain alias");
        let error = EnsureTestAccountAliasBinding::clear(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("broad secondary-alias clearing is not a lifecycle CAS operation");
        assert!(instruction_error_contains(
            &error,
            "broad alias clearing was removed"
        ));
        assert_eq!(
            tx.world.account_aliases.get(&primary_label),
            Some(&account_id),
            "primary alias binding must remain"
        );
        assert_eq!(tx.world.account_aliases.get(&root_alias), Some(&account_id));
        assert_eq!(
            tx.world.account_aliases.get(&domain_alias),
            Some(&account_id)
        );
        assert!(tx.world.account_rekey_records.get(&root_alias).is_some());
        assert!(tx.world.account_rekey_records.get(&domain_alias).is_some());
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
            .expect("reverse index should remain intact");
        assert_eq!(remaining_aliases.len(), 3);
        assert!(remaining_aliases.contains(&primary_label));
        assert!(remaining_aliases.contains(&root_alias));
        assert!(remaining_aliases.contains(&domain_alias));
    }
    #[test]
    fn alias_setup_rejects_stale_non_empty_binding() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let stale_owner = AccountId::new(checked_keypair().public_key().clone());
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .account_aliases
            .insert(alias.clone(), stale_owner.clone());
        tx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), stale_owner.clone()),
        );
        Register::account(Account::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        seed_domainful_alias_manage_permissions(&mut tx, &authority, &domain_id);
        seed_account_alias_lease_record(&mut tx, &account_id, &alias);
        let error = EnsureTestAccountAliasBinding {
            account: account_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("declarative setup must reject stale non-empty binding drift");
        assert!(instruction_error_contains(&error, "alias.binding.conflict"));
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&stale_owner),
            "conflicting setup must preserve the existing binding"
        );
        assert_eq!(
            tx.world
                .account_rekey_records
                .get(&alias)
                .expect("rekey record should exist")
                .active_account_id,
            stale_owner,
            "conflicting setup must preserve continuity state"
        );
    }
    #[test]
    fn bind_account_alias_allows_account_registrar_for_domain() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &account_id, &alias);
        EnsureTestAccountAliasBinding {
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
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        Register::account(Account::new(account_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register account");
        seed_account_alias_lease(&mut tx, &account_id, &alias);
        EnsureTestAccountAliasBinding {
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
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let unauthorized = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &unauthorized, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = checked_keypair();
        let second_id = AccountId::new(second_keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_account_alias_manage_permissions(&mut tx, &domain_owner, &alias);
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &first_id, &alias);
        EnsureTestAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("bind alias to first account");
        let err = EnsureTestAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&unauthorized, &mut tx)
        .expect_err("alias collision should be rejected");
        assert!(
            instruction_error_contains(&err, "alias.owner.conflict"),
            "error should preserve the existing lease-owner conflict: {err}"
        );
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "existing alias binding must remain unchanged"
        );
    }
    #[test]
    fn bind_account_alias_rejects_account_registrar_repointing_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "banking".parse::<Name>().unwrap());
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        seed_account_alias_manage_permissions(&mut tx, &domain_owner, &alias);
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &first_id, &alias);
        EnsureTestAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");
        seed_account_alias_lease(&mut tx, &second_id, &alias);
        let error = EnsureTestAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect_err("registrar must use an explicit CAS rebind operation");
        assert!(instruction_error_contains(&error, "alias.binding.conflict"));
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "conflicting setup must preserve the existing binding"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "an additional alias conflict must not alter primary state"
        );
    }
    #[test]
    fn bind_account_alias_rejects_global_registrar_repointing_existing_alias() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let domain_owner = (*ALICE_ID).clone();
        let registrar = (*BOB_ID).clone();
        seed_domain(&mut state, &domain_id, &domain_owner);
        seed_account(&mut state, &registrar, &domain_id);
        let alias = alias_in_domain(&domain_id, "issuance".parse::<Name>().unwrap());
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let second_keypair = checked_keypair();
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
        seed_domainful_alias_manage_permissions(&mut tx, &registrar, &domain_id);
        seed_account_alias_manage_permissions(&mut tx, &domain_owner, &alias);
        Register::account(Account::new(first_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register first account");
        Register::account(Account::new(second_id.clone()))
            .execute(&domain_owner, &mut tx)
            .expect("register second account");
        seed_account_alias_lease(&mut tx, &first_id, &alias);
        EnsureTestAccountAliasBinding {
            account: first_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&domain_owner, &mut tx)
        .expect("seed alias on first account");
        seed_account_alias_lease(&mut tx, &second_id, &alias);
        let error = EnsureTestAccountAliasBinding {
            account: second_id.clone(),
            alias: Some(alias.clone()),
            lease_expiry_ms: None,
        }
        .execute(&registrar, &mut tx)
        .expect_err("global registrar must use an explicit CAS rebind operation");
        assert!(instruction_error_contains(&error, "alias.binding.conflict"));
        assert_eq!(
            tx.world.account_aliases.get(&alias),
            Some(&first_id),
            "conflicting setup must preserve the existing binding"
        );
        assert_eq!(
            tx.world
                .account(&first_id)
                .expect("first account should exist")
                .label(),
            None,
            "an additional alias conflict must not alter primary state"
        );
    }
    #[test]
    fn register_account_rejects_phone_like_label() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("label", "universal").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let account_label = alias_in_domain(&domain_id, "+819398553445".parse::<Name>().unwrap());
        let keypair = checked_keypair();
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
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let transferred_domain_id: DomainId =
            DomainId::try_new("foo", "universal").expect("foo domain id");
        let user1 = AccountId::new(checked_keypair().public_key().clone());
        let user2 = AccountId::new(checked_keypair().public_key().clone());
        let authority_domain: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
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
    fn transfer_domain_rejects_noncanonical_musubi_generation_before_owner_mutation() {
        for generation in [0, 1] {
            let mut state = test_state();
            let source = (*ALICE_ID).clone();
            let destination = (*BOB_ID).clone();
            let domain_id = DomainId::try_new("generation_guard", "universal").expect("domain id");
            seed_domain(&mut state, &domain_id, &source);
            seed_account(&mut state, &source, &domain_id);
            seed_account(&mut state, &destination, &domain_id);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .musubi_domain_ownership_generations_mut()
                .insert(domain_id.clone(), generation);
            let error = Transfer::domain(source.clone(), domain_id.clone(), destination.clone())
                .execute(&source, &mut transaction)
                .expect_err("a noncanonical stored generation must fail before transfer");
            assert!(
                error
                    .to_string()
                    .contains("Musubi domain ownership generation is noncanonical"),
                "unexpected generation guard error: {error}"
            );
            assert_eq!(
                transaction
                    .world
                    .domain(&domain_id)
                    .expect("domain remains registered")
                    .owned_by(),
                &source
            );
            assert_eq!(
                transaction
                    .world
                    .musubi_domain_ownership_generations()
                    .get(&domain_id),
                Some(&generation)
            );
        }
    }
    #[test]
    fn register_account_rejects_opaque_ids_without_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("opaque", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
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
        let domain_id: DomainId = DomainId::try_new("opaque", "dupes").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
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
        let domain_id: DomainId = DomainId::try_new("opaque", "collide").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::collide"));
        let first_id = AccountId::new(checked_keypair().public_key().clone());
        let second_id = AccountId::new(checked_keypair().public_key().clone());
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
        let domain_id: DomainId = DomainId::try_new("disallowed", "curves").expect("domain id");
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
        let secp_pair = checked_keypair_with_algorithm(Algorithm::Secp256k1);
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
        let domain_id: DomainId = DomainId::try_new("bls", "allowed").expect("domain id");
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
        let bls_pair = checked_keypair_with_algorithm(Algorithm::BlsNormal);
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
        let domain_id: DomainId = DomainId::try_new("restricted", "curves").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_curve_ids.clear();
            *guard = Arc::new(cfg);
        }
        let keypair = checked_keypair();
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
        let domain_id: DomainId = DomainId::try_new("spaces", "bindings").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::register_bindings"));
        let dataspace = DataSpaceId::new(17);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(5);
        });
        let keypair = checked_keypair();
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
        let domain_id: DomainId = DomainId::try_new("uaid", "duplicates").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::duplicate"));
        let first_keypair = checked_keypair();
        let first_id = AccountId::new(first_keypair.public_key().clone());
        let first_account = NewAccount::new(first_id.clone()).with_uaid(Some(uaid));
        let second_keypair = checked_keypair();
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
        let domain_id: DomainId = DomainId::try_new("spaces", "cleanup").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unregister_bindings"));
        let dataspace = DataSpaceId::new(21);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(3);
        });
        let keypair = checked_keypair();
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
        let domain_id: DomainId = DomainId::try_new("cleanup", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let other_domain_id: DomainId = DomainId::try_new("other", "world").expect("domain id");
        seed_domain(&mut state, &other_domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "rose".parse().unwrap());
        Register::asset_definition({
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "rose".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
        let quantity = Quantity::from(5_u32);
        let asset = Asset::new(asset_id.clone(), quantity.clone());
        let (asset_id, asset_value) = asset.into_key_value();
        tx.world.assets.insert(asset_id.clone(), asset_value);
        tx.world.track_asset_holder(&asset_id);
        tx.world
            .increase_asset_total_amount(&asset_def_id, &quantity)
            .expect("fixture asset total must match the inserted balance");
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
        tx.world.insert_nft_entry(nft_id.clone(), nft_value);
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
        let domain_id: DomainId = DomainId::try_new("cleanup", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let foreign_domain_id: DomainId = DomainId::try_new("foreign", "world").expect("domain id");
        seed_domain(&mut state, &foreign_domain_id, &authority);
        let holder_domain: DomainId = DomainId::try_new("holders", "world").expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(checked_keypair().public_key().clone());
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
        tx.world.insert_nft_entry(nft_id.clone(), nft_value);
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
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let external_domain: DomainId = DomainId::try_new("external", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
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
    fn unregister_account_rejects_immutable_governance_lock_custody_after_config_change() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let custody_account = AccountId::new(checked_keypair().public_key().clone());
        let owner = (*BOB_ID).clone();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(custody_account.clone()))
            .execute(&authority, &mut tx)
            .expect("register retained custody account");
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            owner.clone(),
            GovernanceLockRecord {
                owner,
                amount: Quantity::from(150_u32),
                slashed: Quantity::zero(),
                expiry_height: 10,
                direction: 0,
                duration_blocks: 3_600,
                custody: Some(GovernanceLockCustody {
                    escrowed: true,
                    asset_definition_id: tx.gov.voting_asset_id.clone(),
                    bond_escrow_account: custody_account.clone(),
                    slash_receiver_account: custody_account.clone(),
                }),
            },
        );
        tx.world
            .put_governance_locks("retained-account-custody".to_owned(), locks);
        tx.gov.bond_escrow_account = authority.clone();
        tx.gov.slash_receiver_account = authority.clone();
        let err = Unregister::account(custody_account.clone())
            .execute(&authority, &mut tx)
            .expect_err("immutable lock custody account must remain registered");
        assert!(
            err.to_string()
                .contains("retained by immutable governance lock custody"),
            "error should identify retained lock custody: {err}"
        );
        assert!(
            tx.world.accounts.get(&custody_account).is_some(),
            "custody account must remain after rejected unregister"
        );
    }
    #[test]
    fn unregister_account_rejects_zero_lock_validation_fee_proposal_custody_after_config_change() {
        for fixture_kind in [
            ValidationFeeProposalFixtureKind::Policy,
            ValidationFeeProposalFixtureKind::PayoutLifecycle,
        ] {
            for status in [
                GovernanceProposalStatus::Proposed,
                GovernanceProposalStatus::Approved,
            ] {
                let state = test_state();
                let authority = (*ALICE_ID).clone();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut tx = block.transaction();
                let targets = register_validation_fee_unregister_targets(&authority, &mut tx);
                let rules = validation_fee_unregister_rules(&targets);
                let proposal_id = insert_validation_fee_unregister_proposal(
                    &mut tx,
                    fixture_kind,
                    status,
                    &rules,
                );
                assert!(
                    tx.world.governance_locks.iter().next().is_none(),
                    "zero-lock regression fixture must not rely on ballot custody"
                );
                drift_validation_fee_governance_config(&mut tx);
                assert_validation_fee_governance_config_drift(&tx, &rules);
                for (reference, account_id) in [
                    ("bond escrow", &targets.bond_escrow_account),
                    ("slash receiver", &targets.slash_receiver_account),
                ] {
                    let err = Unregister::account(account_id.clone())
                        .execute(&authority, &mut tx)
                        .expect_err(
                            "retained validation-fee proposal custody must block account removal",
                        );
                    assert!(
                        err.to_string()
                            .contains("retained by immutable validation-fee proposal custody"),
                        "{fixture_kind:?} {status:?} proposal {proposal_id:?} must retain its \
                         {reference} account without locks: {err}"
                    );
                    assert!(
                        tx.world.accounts.get(account_id).is_some(),
                        "{reference} account must remain after rejected unregister"
                    );
                }
            }
        }
    }
    #[test]
    fn unregister_asset_definition_rejects_zero_lock_validation_fee_proposal_rules_after_config_change()
     {
        for fixture_kind in [
            ValidationFeeProposalFixtureKind::Policy,
            ValidationFeeProposalFixtureKind::PayoutLifecycle,
        ] {
            for status in [
                GovernanceProposalStatus::Proposed,
                GovernanceProposalStatus::Approved,
            ] {
                let state = test_state();
                let authority = (*ALICE_ID).clone();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut tx = block.transaction();
                let targets = register_validation_fee_unregister_targets(&authority, &mut tx);
                let rules = validation_fee_unregister_rules(&targets);
                let proposal_id = insert_validation_fee_unregister_proposal(
                    &mut tx,
                    fixture_kind,
                    status,
                    &rules,
                );
                assert!(
                    tx.world.governance_locks.iter().next().is_none(),
                    "zero-lock regression fixture must not rely on ballot custody"
                );
                drift_validation_fee_governance_config(&mut tx);
                assert_validation_fee_governance_config_drift(&tx, &rules);
                let err = Unregister::asset_definition(targets.voting_asset_id.clone())
                    .execute(&authority, &mut tx)
                    .expect_err(
                        "retained validation-fee proposal rules must block asset-definition removal",
                    );
                assert!(
                    err.to_string()
                        .contains("retained by immutable validation-fee proposal custody"),
                    "{fixture_kind:?} {status:?} proposal {proposal_id:?} must retain its voting \
                     asset without locks: {err}"
                );
                assert!(
                    tx.world
                        .asset_definitions
                        .get(&targets.voting_asset_id)
                        .is_some(),
                    "voting asset definition must remain after rejected unregister"
                );
            }
        }
    }
    #[test]
    fn unregister_domain_rejects_zero_lock_validation_fee_proposal_rules_after_config_change() {
        for fixture_kind in [
            ValidationFeeProposalFixtureKind::Policy,
            ValidationFeeProposalFixtureKind::PayoutLifecycle,
        ] {
            for status in [
                GovernanceProposalStatus::Proposed,
                GovernanceProposalStatus::Approved,
            ] {
                let state = test_state();
                let authority = (*ALICE_ID).clone();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut tx = block.transaction();
                let targets = register_validation_fee_unregister_targets(&authority, &mut tx);
                let rules = validation_fee_unregister_rules(&targets);
                let proposal_id = insert_validation_fee_unregister_proposal(
                    &mut tx,
                    fixture_kind,
                    status,
                    &rules,
                );
                assert!(
                    tx.world.governance_locks.iter().next().is_none(),
                    "zero-lock regression fixture must not rely on ballot custody"
                );
                drift_validation_fee_governance_config(&mut tx);
                assert_validation_fee_governance_config_drift(&tx, &rules);
                let err = Unregister::domain(targets.domain_id.clone())
                    .execute(&authority, &mut tx)
                    .expect_err(
                        "domain removal must retain validation-fee proposal rule references",
                    );
                assert!(
                    err.to_string()
                        .contains("retained by immutable validation-fee proposal custody"),
                    "{fixture_kind:?} {status:?} proposal {proposal_id:?} must retain its voting \
                     asset domain without locks: {err}"
                );
                assert!(
                    tx.world.domains.get(&targets.domain_id).is_some(),
                    "referenced domain must remain after rejected unregister"
                );
                assert!(
                    tx.world
                        .asset_definitions
                        .get(&targets.voting_asset_id)
                        .is_some(),
                    "referenced asset definition must remain after rejected domain unregister"
                );
            }
        }
    }
    #[test]
    fn unregister_account_removes_account_read_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("cleanup", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let holder_domain: DomainId = DomainId::try_new("holders", "world").expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(checked_keypair().public_key().clone());
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
            iroha_executor_data_model::permission::query::CanReadAccountData {
                account: account_id.clone(),
            }
            .into();
        assert!(
            iroha_executor_data_model::permission::query::CanReadAccountData::try_from(&permission)
                .is_ok(),
            "permission should decode as CanReadAccountData"
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
        let domain_id: DomainId = DomainId::try_new("cleanup", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let holder_domain: DomainId = DomainId::try_new("holders", "world").expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(checked_keypair().public_key().clone());
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
        let domain_id: DomainId = DomainId::try_new("cleanup", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let retained_domain: DomainId = DomainId::try_new("retained", "world").expect("domain id");
        seed_domain(&mut state, &retained_domain, &authority);
        let holder_domain: DomainId = DomainId::try_new("holders", "world").expect("domain id");
        seed_domain(&mut state, &holder_domain, &authority);
        let keypair = checked_keypair();
        let target_id = AccountId::new(keypair.public_key().clone());
        let holder_id = AccountId::new(checked_keypair().public_key().clone());
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
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "bond".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        Register::asset_definition({
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "bond".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.world
            .asset_definition_mut(&asset_def_id)
            .expect("definition exists")
            .set_owned_by(account_id.clone());
        tx.world
            .replace_asset_definition_owner_index(&asset_def_id, &authority, &account_id);
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
    fn unregister_account_rejects_when_account_has_contract_deployment_nonce_state() {
        let mut state = test_state();
        let domain_id: DomainId =
            DomainId::try_new("contract_deployer", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let nonce_key: Name = iroha_data_model::smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY
            .parse()
            .expect("contract deployment nonce key");
        tx.world
            .account_mut(&account_id)
            .expect("registered account")
            .insert(nonce_key.clone(), Json::new(1_u64));
        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("contract deployer identity must retain its monotonic nonce state");
        let err_string = err.to_string();
        assert!(
            err_string.contains("contract deployment nonce state")
                && err_string.contains("address monotonicity"),
            "error should explain the deployment replay invariant: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account and nonce must remain after rejected unregister"
        );
        assert_eq!(
            tx.world
                .account(&account_id)
                .expect("account remains")
                .metadata()
                .get(&nonce_key),
            Some(&Json::new(1_u64)),
            "deployment nonce must remain unchanged"
        );
    }
    fn assert_account_unregister_guard(
        configure: impl FnOnce(
            &mut crate::state::StateTransaction<'_, '_>,
            &DomainId,
            &AccountId,
            &AccountId,
        ),
        reject_message: &str,
        error_fragment: &str,
        error_diagnostic: &str,
    ) {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        configure(&mut tx, &domain_id, &authority, &account_id);
        let err = Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect_err(reject_message);
        let err_string = err.to_string();
        assert!(
            err_string.contains(error_fragment),
            "{error_diagnostic}: {err_string}"
        );
        assert!(
            tx.world.accounts.get(&account_id).is_some(),
            "account should remain after rejected unregister"
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_governance_bond_escrow_account() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.gov.bond_escrow_account = account_id.clone();
            },
            "governance bond escrow account must not be unregistered",
            "governance bond escrow account",
            "error should explain governance bond escrow conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_governance_viral_incentive_pool_account() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.gov.viral_incentives.incentive_pool_account = account_id.clone();
            },
            "governance viral incentive pool account must not be unregistered",
            "governance viral incentive pool account",
            "error should explain governance viral incentive pool conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_oracle_reward_pool() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.oracle.economics.reward_pool = account_id.clone();
            },
            "oracle reward pool account must not be unregistered",
            "oracle reward pool account",
            "error should explain oracle reward-pool conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_nexus_fee_sink_account() {
        assert_account_unregister_guard(
            |tx, _domain_id, authority, account_id| {
                let helper_account_id = AccountId::new(checked_keypair().public_key().clone());
                Register::account(NewAccount::new(helper_account_id.clone()))
                    .execute(authority, tx)
                    .expect("register helper account");
                tx.nexus.fees.fee_sink_account_id = account_id.to_string();
                tx.nexus.staking.stake_escrow_account_id = helper_account_id.to_string();
                tx.nexus.staking.slash_sink_account_id = helper_account_id.to_string();
            },
            "nexus fee sink account must not be unregistered",
            "nexus fee sink account",
            "error should explain nexus fee-sink conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_nexus_fee_sink_account_is_configured() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let sink_domain: DomainId = DomainId::try_new("sink", "world").expect("domain id");
        let remove_domain: DomainId = DomainId::try_new("remove", "world").expect("domain id");
        seed_domain(&mut state, &sink_domain, &authority);
        seed_domain(&mut state, &remove_domain, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
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
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, _account_id| {
                tx.nexus.fees.fee_sink_account_id = "not-an-account-literal".to_owned();
            },
            "invalid nexus fee sink literal must fail closed",
            "invalid nexus.fees.fee_sink_account_id account literal",
            "error should explain invalid nexus fee-sink literal",
        );
    }
    #[test]
    fn unregister_account_allows_unrelated_account_when_fee_sink_is_different_account() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let remove_domain: DomainId = DomainId::try_new("remove", "world").expect("domain id");
        let sink_domain: DomainId = DomainId::try_new("sink", "world").expect("domain id");
        seed_domain(&mut state, &remove_domain, &authority);
        seed_domain(&mut state, &sink_domain, &authority);
        let remove_account_id = AccountId::new(checked_keypair().public_key().clone());
        let sink_account_id = AccountId::new(checked_keypair().public_key().clone());
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
        assert_account_unregister_guard(
            |tx, _domain_id, authority, account_id| {
                let helper_account_id = AccountId::new(checked_keypair().public_key().clone());
                Register::account(NewAccount::new(helper_account_id.clone()))
                    .execute(authority, tx)
                    .expect("register helper account");
                tx.nexus.fees.fee_sink_account_id = helper_account_id.to_string();
                tx.nexus.staking.stake_escrow_account_id = account_id.to_string();
                tx.nexus.staking.slash_sink_account_id = helper_account_id.to_string();
            },
            "nexus staking escrow account must not be unregistered",
            "nexus staking escrow account",
            "error should explain nexus staking-escrow conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_nexus_staking_slash_sink_account() {
        assert_account_unregister_guard(
            |tx, _domain_id, authority, account_id| {
                let helper_account_id = AccountId::new(checked_keypair().public_key().clone());
                Register::account(NewAccount::new(helper_account_id.clone()))
                    .execute(authority, tx)
                    .expect("register helper account");
                tx.nexus.fees.fee_sink_account_id = helper_account_id.to_string();
                tx.nexus.staking.stake_escrow_account_id = helper_account_id.to_string();
                tx.nexus.staking.slash_sink_account_id = account_id.to_string();
            },
            "nexus staking slash sink account must not be unregistered",
            "nexus staking slash sink account",
            "error should explain nexus staking slash-sink conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_offline_escrow_account() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(domain_id.clone(), "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
    fn unregister_account_rejects_live_offline_escrow_after_transaction_boundary() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "usd".parse().expect("asset definition name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let escrow_account_id;
        let escrow_asset_id;
        {
            let mut first_tx = block.transaction();
            Register::asset_definition({
                let asset_definition_id = asset_definition_id.clone();
                AssetDefinition::numeric(
                    asset_definition_id,
                    "usd".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            })
            .execute(&authority, &mut first_tx)
            .expect("register asset definition");
            let asset_definition = first_tx
                .world
                .asset_definition(&asset_definition_id)
                .expect("registered asset definition");
            super::isi::ensure_offline_escrow_account(&asset_definition, &authority, &mut first_tx)
                .expect("materialize deterministic offline escrow account");
            escrow_account_id =
                super::isi::offline_escrow_account_id(first_tx.network_id(), &asset_definition_id);
            escrow_asset_id = AssetId::new(asset_definition_id.clone(), escrow_account_id.clone());
            Mint::asset_quantity(5_u32, escrow_asset_id.clone())
                .execute(&authority, &mut first_tx)
                .expect("mint live offline escrow backing");
            first_tx.apply();
        }
        let mut second_tx = block.transaction();
        assert!(
            second_tx.settlement.offline.escrow_accounts.is_empty(),
            "transaction-local escrow bindings must not be required for protection"
        );
        let err = Unregister::account(escrow_account_id.clone())
            .execute(&authority, &mut second_tx)
            .expect_err("live offline escrow backing must survive account unregistration");
        let err_string = err.to_string();
        assert!(
            err_string.contains("offline escrow account"),
            "error should explain the live offline escrow conflict: {err_string}"
        );
        assert!(
            second_tx.world.accounts.get(&escrow_account_id).is_some(),
            "offline escrow account should remain after rejected unregister"
        );
        assert_eq!(
            second_tx
                .world
                .asset(&escrow_asset_id)
                .expect("offline escrow backing should remain")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(5_u32),
        );
    }
    #[test]
    fn ordinary_metadata_does_not_reserve_an_offline_escrow_account() {
        let chain_id = ChainId::from("offline-escrow-testnet");
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"offline-escrow-test-network"),
        ));
        let domain_id: DomainId = DomainId::try_new("offline", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "usd".parse().expect("asset definition name"),
        );
        let escrow_account_id =
            iroha_data_model::offline::offline_escrow_account_id(&network_id, &asset_definition_id);
        let mut metadata = Metadata::default();
        metadata.insert(
            "offline.enabled".parse().expect("legacy metadata key"),
            Json::new(true),
        );
        let mut asset_definition = AssetDefinition::numeric(
            asset_definition_id.clone(),
            "usd".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority);
        asset_definition.metadata = metadata;
        let world = World::with_assets(
            [Domain::new(domain_id).build(&authority)],
            [Account::new(escrow_account_id.clone()).build(&authority)],
            [asset_definition],
            [],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_with_chain_and_network_id_for_testing(
            world, kura, query, chain_id, network_id,
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        assert!(
            tx.settlement.offline.escrow_accounts.is_empty(),
            "ordinary asset metadata must not create an escrow binding"
        );
        Unregister::account(escrow_account_id.clone())
            .execute(&authority, &mut tx)
            .expect("legacy-looking metadata must have no offline semantics");
        assert!(
            tx.world.accounts.get(&escrow_account_id).is_none(),
            "ordinary unbound account should be removable"
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_content_publish_allow_account() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.content.publish_allow_accounts = vec![account_id.clone()];
            },
            "content publish allow-list account must not be unregistered",
            "content publish allow-list account",
            "error should explain content publish-allow conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_sorafs_telemetry_submitter() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.gov.sorafs_telemetry.submitters = vec![account_id.clone()];
            },
            "SoraFS telemetry submitter account must not be unregistered",
            "SoraFS telemetry submitter",
            "error should explain telemetry-submitter conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_configured_sorafs_provider_owner() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let provider_id = iroha_data_model::sorafs::capacity::ProviderId::new([0xD4; 32]);
                tx.gov
                    .sorafs_provider_owners
                    .insert(provider_id, account_id.clone());
            },
            "configured SoraFS provider-owner account must not be unregistered",
            "configured as SoraFS provider owner",
            "error should explain configured provider-owner conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_owns_sorafs_provider() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
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
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.world.citizens.insert(
                    account_id.clone(),
                    crate::state::CitizenshipRecord::new(account_id.clone(), 100_u64.into(), 1),
                );
            },
            "account with citizenship record must not be unregistered",
            "active citizenship record",
            "error should explain citizenship conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_public_lane_validator_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                tx.world.public_lane_validators.insert(
                    (LaneId::SINGLE, account_id.clone()),
                    iroha_data_model::nexus::PublicLaneValidatorRecord {
                        lane_id: LaneId::SINGLE,
                        validator: account_id.clone(),
                        peer_id: PeerId::from(account_id.expect_single_signatory().clone()),
                        stake_account: account_id.clone(),
                        total_stake: Quantity::from(1_u32),
                        self_stake: Quantity::from(1_u32),
                        metadata: Metadata::default(),
                        status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                        activation_epoch: Some(1),
                        activation_height: Some(1),
                        last_reward_epoch: None,
                    },
                );
            },
            "account with validator stake state must not be unregistered",
            "public-lane validator stake state",
            "error should explain staking conflict",
        );
    }
    #[test]
    fn unregister_account_ignores_mismatched_public_lane_validator_row() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
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
                lane_id: LaneId::new(1),
                validator: account_id.clone(),
                peer_id: PeerId::from(account_id.expect_single_signatory().clone()),
                stake_account: account_id.clone(),
                total_stake: Quantity::from(1_u32),
                self_stake: Quantity::from(1_u32),
                metadata: Metadata::default(),
                status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                activation_epoch: Some(1),
                activation_height: Some(1),
                last_reward_epoch: None,
            },
        );
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("mismatched validator row must not block account unregister");
        assert!(
            tx.world.accounts.get(&account_id).is_none(),
            "account should be unregistered when only malformed validator state references it"
        );
        let record = tx
            .world
            .public_lane_validators
            .get(&(LaneId::SINGLE, account_id))
            .expect("malformed validator row remains as stored");
        assert_eq!(record.lane_id, LaneId::new(1));
        assert!(matches!(
            record.status,
            iroha_data_model::nexus::PublicLaneValidatorStatus::Active
        ));
    }
    #[test]
    fn unregister_account_rejects_when_account_has_public_lane_reward_record_state() {
        assert_account_unregister_guard(
            |tx, domain_id, _authority, account_id| {
                tx.world.public_lane_rewards.insert(
                    (LaneId::SINGLE, 1),
                    iroha_data_model::nexus::PublicLaneRewardRecord {
                        lane_id: LaneId::SINGLE,
                        epoch: 1,
                        asset: AssetId::new(
                            AssetDefinitionId::derive_from_components(
                                domain_id.clone(),
                                "fee".parse().unwrap(),
                            ),
                            account_id.clone(),
                        ),
                        total_reward: Quantity::from(1_u32),
                        shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                            account: account_id.clone(),
                            role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                            amount: Quantity::from(1_u32),
                        }],
                        metadata: Metadata::default(),
                    },
                );
            },
            "account with public-lane reward state must not be unregistered",
            "public-lane reward ledger state",
            "error should explain reward-state conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_is_reward_claim_asset_owner() {
        assert_account_unregister_guard(
            |tx, domain_id, authority, account_id| {
                tx.world.public_lane_reward_claims.insert(
                    (
                        LaneId::SINGLE,
                        authority.clone(),
                        AssetId::new(
                            AssetDefinitionId::derive_from_components(
                                domain_id.clone(),
                                "fee".parse().unwrap(),
                            ),
                            account_id.clone(),
                        ),
                    ),
                    1,
                );
            },
            "account referenced by reward-claim asset owner must not be unregistered",
            "public-lane reward claim state",
            "error should explain reward-claim conflict",
        );
    }
    #[test]
    fn unregister_account_ignores_mismatched_public_lane_economic_rows() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.public_lane_stake_shares.insert(
            (LaneId::SINGLE, account_id.clone(), authority.clone()),
            iroha_data_model::nexus::PublicLaneStakeShare {
                lane_id: LaneId::new(1),
                validator: account_id.clone(),
                staker: authority.clone(),
                bonded: Quantity::from(1_u32),
                pending_unbonds: std::collections::BTreeMap::new(),
                metadata: Metadata::default(),
            },
        );
        tx.world.public_lane_rewards.insert(
            (LaneId::SINGLE, 1),
            iroha_data_model::nexus::PublicLaneRewardRecord {
                lane_id: LaneId::new(1),
                epoch: 1,
                asset: AssetId::new(
                    AssetDefinitionId::derive_from_components(
                        domain_id.clone(),
                        "fee".parse().unwrap(),
                    ),
                    account_id.clone(),
                ),
                total_reward: Quantity::from(1_u32),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: account_id.clone(),
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: Quantity::from(1_u32),
                }],
                metadata: Metadata::default(),
            },
        );
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("mismatched public-lane economic rows must not block account unregister");
        assert!(
            tx.world.accounts.get(&account_id).is_none(),
            "account should be unregistered when only malformed economic rows reference it"
        );
        assert!(
            tx.world
                .public_lane_stake_shares
                .get(&(LaneId::SINGLE, account_id.clone(), authority))
                .is_some(),
            "malformed stake-share row remains as stored"
        );
        assert!(
            tx.world
                .public_lane_rewards
                .get(&(LaneId::SINGLE, 1))
                .is_some(),
            "malformed reward row remains as stored"
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_repo_agreement_state() {
        assert_account_unregister_guard(
            |tx, domain_id, authority, account_id| {
                let repo_id: iroha_data_model::repo::RepoAgreementId =
                    "repoguard".parse().expect("repo agreement id");
                let agreement = iroha_data_model::repo::RepoAgreement::new(
                    repo_id.clone(),
                    account_id.clone(),
                    authority.clone(),
                    iroha_data_model::repo::RepoCashLeg {
                        asset_definition_id: AssetDefinitionId::derive_from_components(
                            domain_id.clone(),
                            "usd".parse().unwrap(),
                        ),
                        quantity: Quantity::from(10_u32),
                    },
                    AssetId::new(
                        AssetDefinitionId::derive_from_components(
                            domain_id.clone(),
                            "usd".parse().unwrap(),
                        ),
                        authority.clone(),
                    ),
                    iroha_data_model::repo::RepoCollateralLeg::new(
                        AssetDefinitionId::derive_from_components(
                            domain_id.clone(),
                            "bond".parse().unwrap(),
                        ),
                        Quantity::from(12_u32),
                    ),
                    AssetId::new(
                        AssetDefinitionId::derive_from_components(
                            domain_id.clone(),
                            "bond".parse().unwrap(),
                        ),
                        authority.clone(),
                    ),
                    250,
                    1000,
                    1,
                    iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
                    None,
                );
                tx.world.insert_repo_agreement_entry(agreement);
            },
            "account with repo agreement state must not be unregistered",
            "repo agreement state",
            "error should explain repo-state conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_committed_settlement_receipt() {
        assert_account_unregister_guard(
            |tx, domain_id, authority, account_id| {
                let settlement_id: iroha_data_model::isi::SettlementId =
                    "settleguard".parse().expect("settlement id");
                let receipt = iroha_data_model::isi::SettlementReceipt {
                    kind: iroha_data_model::isi::SettlementKind::Dvp,
                    authority: account_id.clone(),
                    plan: iroha_data_model::isi::SettlementPlan::default(),
                    metadata: Metadata::default(),
                    block_height: 1,
                    block_hash: iroha_crypto::HashOf::<
                        iroha_data_model::block::BlockHeader,
                    >::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
                    executed_at_ms: 1,
                    legs: [
                        iroha_data_model::isi::SettlementLegSnapshot {
                            role: iroha_data_model::isi::SettlementLegRole::Delivery,
                            leg: iroha_data_model::isi::SettlementLeg::new(
                                AssetDefinitionId::derive_from_components(
                                    domain_id.clone(),
                                    "usd".parse().unwrap(),
                                ),
                                Quantity::one(),
                                account_id.clone(),
                                authority.clone(),
                            ),
                        },
                        iroha_data_model::isi::SettlementLegSnapshot {
                            role: iroha_data_model::isi::SettlementLegRole::Payment,
                            leg: iroha_data_model::isi::SettlementLeg::new(
                                AssetDefinitionId::derive_from_components(
                                    domain_id.clone(),
                                    "eur".parse().unwrap(),
                                ),
                                Quantity::one(),
                                authority.clone(),
                                account_id.clone(),
                            ),
                        },
                    ],
                    fx_corridor: None,
                };
                tx.world.settlement_receipts.insert(settlement_id, receipt);
            },
            "account with a committed settlement receipt must not be unregistered",
            "committed settlement receipt",
            "error should explain settlement-state conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_oracle_feed_provider_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let mut feed = iroha_data_model::oracle::kits::price_xor_usd().feed_config;
                feed.providers = vec![account_id.clone()];
                tx.world.oracle_feeds.insert(feed.feed_id.clone(), feed);
            },
            "account with oracle provider state must not be unregistered",
            "active oracle feed provider state",
            "error should explain oracle-state conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_oracle_feed_history_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let kit = iroha_data_model::oracle::kits::price_xor_usd();
                let feed = kit.feed_config;
                let feed_id = feed.feed_id.clone();
                let request_hash = kit.connector_request.request_hash();
                tx.world.oracle_history.insert(
                    feed_id.clone(),
                    vec![iroha_data_model::events::data::oracle::FeedEventRecord {
                        event: iroha_data_model::oracle::FeedEvent {
                            feed_id: feed_id.clone(),
                            feed_config_version: feed.feed_config_version,
                            slot: 1,
                            request_hash,
                            outcome: iroha_data_model::oracle::FeedEventOutcome::Success(
                                iroha_data_model::oracle::FeedSuccess {
                                    value: iroha_data_model::oracle::ObservationValue::new(
                                        1_000, 2,
                                    ),
                                    entries: vec![iroha_data_model::oracle::ReportEntry {
                                        oracle_id: account_id.clone(),
                                        observation_hash:
                                            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                                                b"oracle-history-account-guard",
                                            )),
                                        value: iroha_data_model::oracle::ObservationValue::new(
                                            1_000, 2,
                                        ),
                                        outlier: false,
                                    }],
                                },
                            ),
                        },
                        recorded_at_ms: 1,
                        evidence_hashes: Vec::new(),
                    }],
                );
            },
            "account with oracle history state must not be unregistered",
            "active oracle feed history state",
            "error should explain oracle-history conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_governance_proposal_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let proposal_id = [0xA5; 32];
                let kind = iroha_data_model::governance::types::ProposalKind::DeployContract(
                    iroha_data_model::governance::types::DeployContractProposal {
                        contract_address:
                            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                                .parse()
                                .expect("contract address"),
                        code_hash_hex: iroha_data_model::governance::types::ContractCodeHash::new(
                            [0x11; 32],
                        ),
                        abi_hash_hex: iroha_data_model::governance::types::ContractAbiHash::new(
                            [0x22; 32],
                        ),
                        abi_version: iroha_data_model::governance::types::AbiVersion::new(1),
                        manifest_provenance: None,
                    },
                );
                tx.world.put_governance_proposal(
                    proposal_id,
                    crate::state::GovernanceProposalRecord {
                        proposer: account_id.clone(),
                        kind,
                        created_height: 1,
                        status: crate::state::GovernanceProposalStatus::Proposed,
                        pipeline: crate::state::GovernancePipeline::default(),
                        parliament_snapshot: None,
                        finalization_evidence: None,
                        enacted_at_height: None,
                    },
                );
            },
            "account with governance proposal state must not be unregistered",
            "active governance proposal state",
            "error should explain governance proposal conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_content_bundle_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let bundle_id = Hash::new(b"content-bundle-account-guard");
                let stripe_layout = iroha_data_model::da::prelude::DaStripeLayout::default();
                let manifest = iroha_data_model::content::ContentBundleManifest {
                    bundle_id,
                    index_hash: [0x44; 32],
                    dataspace: DataSpaceId::UNIVERSAL,
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
            },
            "account with content bundle state must not be unregistered",
            "content bundle state",
            "error should explain content-bundle conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_runtime_upgrade_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
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
            },
            "account with runtime upgrade state must not be unregistered",
            "active runtime upgrade proposal state",
            "error should explain runtime-upgrade conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_viral_escrow_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let binding_digest = Hash::new(b"viral-escrow-account-guard");
                tx.world.viral_escrows.insert(
                    binding_digest,
                    iroha_data_model::social::ViralEscrowRecord {
                        binding_hash: iroha_data_model::oracle::KeyedHash {
                            pepper_id: "pepper".to_string(),
                            digest: binding_digest,
                        },
                        sender: account_id.clone(),
                        amount: iroha_primitives::numeric::Quantity::from(1_u32),
                        created_at_ms: 1,
                    },
                );
            },
            "account with viral escrow state must not be unregistered",
            "active viral escrow state",
            "error should explain viral-escrow conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_sorafs_pin_manifest_state() {
        assert_account_unregister_guard(
            |tx, _domain_id, _authority, account_id| {
                let digest =
                    iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xAB; 32]);
                tx.world.pin_manifests.insert(
                    digest,
                    iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
                        digest,
                        iroha_data_model::sorafs::pin_registry::ManifestRootCid::from_blake3_digest(
                            [0xBC; 32],
                        )
                        .expect("canonical manifest root CID"),
                        iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
                            profile_id: 1,
                            namespace: "sorafs".to_string(),
                            name: "sf1".to_string(),
                            semver: "1.0.0".to_string(),
                            multihash_code: 0x1E,
                        },
                        [0xCD; 32],
                        [0; 32],
                        0,
                        iroha_data_model::sorafs::pin_registry::PinPolicy::default(),
                        account_id.clone(),
                        1,
                        None,
                        None,
                        Metadata::default(),
                    ),
                );
            },
            "account with SoraFS pin manifest state must not be unregistered",
            "active SoraFS pin manifest state",
            "error should explain SoraFS pin-manifest conflict",
        );
    }
    #[test]
    fn unregister_account_rejects_when_account_has_da_pin_intent_owner_state() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let network_id = *state.network_id_ref();
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
                    authorization: crate::da::signed_test_ingest_authorization(
                        network_id,
                        &keypair,
                        LaneId::new(1),
                        1,
                        1,
                        1,
                    ),
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
        let domain_id: DomainId = DomainId::try_new("owner", "world").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = checked_keypair();
        let account_id = AccountId::new(keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");
        let peer = PeerId::new(
            checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
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
        assert_account_unregister_guard(
            |tx, _domain_id, authority, account_id| {
                let proposal_id = [0xC5; 32];
                let kind = iroha_data_model::governance::types::ProposalKind::DeployContract(
                    iroha_data_model::governance::types::DeployContractProposal {
                        contract_address:
                            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                                .parse()
                                .expect("contract address"),
                        code_hash_hex: iroha_data_model::governance::types::ContractCodeHash::new(
                            [0x51; 32],
                        ),
                        abi_hash_hex: iroha_data_model::governance::types::ContractAbiHash::new(
                            [0x61; 32],
                        ),
                        abi_version: iroha_data_model::governance::types::AbiVersion::new(1),
                        manifest_provenance: None,
                    },
                );
                let roster = iroha_data_model::governance::types::ParliamentRoster {
                    body: iroha_data_model::governance::types::ParliamentBody::AgendaCouncil,
                    epoch: 1,
                    members: vec![account_id.clone()],
                    alternates: Vec::new(),
                    candidate_count: 0,
                    derived_by: Default::default(),
                };
                tx.world.put_governance_proposal(
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
                        finalization_evidence: None,
                        enacted_at_height: None,
                    },
                );
            },
            "account in governance parliament snapshot must not be unregistered",
            "governance proposal parliament snapshot state",
            "error should explain governance parliament snapshot conflict",
        );
    }
    #[test]
    fn space_directory_events_drive_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("spaces", "events").expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::events"));
        let dataspace = DataSpaceId::new(33);
        let manifest_hash = seed_manifest_record(&mut state.world, uaid, dataspace, |_| {});
        let keypair = checked_keypair();
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
    fn asset_registration_is_independent_of_legacy_offline_metadata() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("offline", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let asset_name: Name = "usd".parse().expect("asset name");
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id.clone(), asset_name);
        let mut metadata = Metadata::default();
        metadata.insert(
            "offline.enabled".parse().expect("legacy metadata key"),
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
            owning_domain: None,
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
            "ordinary registration must not materialize offline state"
        );
    }
    #[test]
    fn register_asset_definition_defers_offline_state_until_offline_use() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("offline2", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let asset_name: Name = "eur".parse().expect("asset name");
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id.clone(), asset_name);
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
            owning_domain: None,
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
    fn register_global_asset_definition_rejects_restricted_dataspace_home() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("private-unit", "paynet").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "unit".parse().expect("asset definition name"),
        );
        let new_definition = NewAssetDefinition {
            id: definition_id,
            name: "Private Unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: paynet,
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        tx.nexus.dataspace_catalog = dataspace_catalog.clone();
        tx.world.dataspace_catalog = dataspace_catalog;
        tx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    dataspace_id: paynet,
                    alias: "paynet".to_owned(),
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        tx.current_dataspace_id = Some(paynet);
        tx.world.current_dataspace_id = Some(paynet);
        let err = Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect_err("restricted dataspaces must not register global asset definitions");
        match err {
            InstructionExecutionError::InvariantViolation(message) => {
                assert!(
                    message.contains("restricted dataspace"),
                    "unexpected invariant message: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn replay_allows_legacy_global_asset_definition_in_restricted_dataspace() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("private-unit", "paynet").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "unit".parse().expect("asset definition name"),
        );
        let new_definition = NewAssetDefinition {
            id: definition_id,
            name: "Private Unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        tx.current_dataspace_id = Some(paynet);
        tx.world.current_dataspace_id = Some(paynet);
        tx.replay_compatibility = true;
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("replay must preserve legacy committed registration");
    }
    #[test]
    fn register_global_asset_definition_allows_public_alias_home_on_authoritative_route() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("private-unit", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("alias");
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: Some(alias.clone()),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Public);
        tx.current_dataspace_id = Some(paynet);
        tx.world.current_dataspace_id = Some(paynet);
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("public dataspace may home a global asset");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn register_global_asset_definition_rejects_public_alias_home_on_wrong_route() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("private-unit", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("alias");
        let new_definition = NewAssetDefinition {
            id: definition_id,
            name: "unit".to_owned(),
            description: None,
            alias: Some(alias),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Public);
        tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let err = Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect_err("global definition must be registered on its alias home route");
        assert!(
            err.to_string().contains("authoritative dataspace"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn asset_home_extra_coverage_register_global_allows_universal_alias_home() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("private-unit", "paynet").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let alias: AssetDefinitionAlias = "unit#universal".parse().expect("alias");
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: Some(alias.clone()),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: Some(domain_id),
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        tx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        tx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("universal alias may home a global asset");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn asset_home_more_coverage_register_restricted_policy_allows_restricted_alias_home() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("restricted-unit", "paynet").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let alias: AssetDefinitionAlias = "unit#restricted-unit.paynet".parse().expect("alias");
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: Some(alias.clone()),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            owning_domain: Some(domain_id),
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        tx.current_dataspace_id = Some(paynet);
        tx.world.current_dataspace_id = Some(paynet);
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("restricted-policy definition may use a restricted alias home");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
        assert_eq!(
            tx.world
                .asset_definition(&definition_id)
                .expect("definition exists")
                .balance_scope_policy(),
            iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted
        );
    }
    #[test]
    fn register_asset_definition_rejects_missing_explicit_name() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId =
            DomainId::try_new("missing-name", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "usd".parse().expect("asset definition name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::asset_definition(AssetDefinition::numeric(
            definition_id,
            "   ".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
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
        let domain_id: DomainId = DomainId::try_new("alias-test", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let id1 = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "usd1".parse().expect("asset name"),
        );
        let id2 = AssetDefinitionId::derive_from_components(
            domain_id,
            "usd2".parse().expect("asset name"),
        );
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
            owning_domain: None,
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
            owning_domain: None,
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
    fn asset_alias_requires_asset_owner_and_independent_domain_namespace_scope() {
        let attacker = (*ALICE_ID).clone();
        let namespace_owner = (*BOB_ID).clone();
        let mut state = test_state_with_authority(&attacker);
        let issuer_domain =
            DomainId::try_new("attacker-issuer", "universal").expect("issuer domain");
        let victim_domain =
            DomainId::try_new("victim", "universal").expect("victim namespace domain");
        seed_domain(&mut state, &issuer_domain, &attacker);
        seed_domain(&mut state, &victim_domain, &namespace_owner);
        seed_account(&mut state, &namespace_owner, &victim_domain);
        let definition_id = AssetDefinitionId::derive_from_components(
            issuer_domain.clone(),
            "usd".parse().expect("asset name"),
        );
        let other_definition_id = AssetDefinitionId::derive_from_components(
            issuer_domain,
            "usd_shadow".parse().expect("asset name"),
        );
        let alias: AssetDefinitionAlias = "usd#victim.universal".parse().expect("alias");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.add_account_permission(
            &attacker,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(victim_domain.clone()),
            }),
        );
        let error = Register::asset_definition(
            AssetDefinition::numeric(
                definition_id.clone(),
                "usd".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .with_alias(Some(alias.clone())),
        )
        .execute(&attacker, &mut tx)
        .expect_err("account-alias permission must not confer an asset alias");
        assert!(error.to_string().contains("CanManageAssetDefinitionAlias"));
        assert!(tx.world.asset_definitions.get(&definition_id).is_none());
        assert!(tx.world.asset_definition_aliases.get(&alias).is_none());
        Register::asset_definition(AssetDefinition::numeric(
            definition_id.clone(),
            "usd".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&attacker, &mut tx)
        .expect("register unaliased attacker-owned definition");
        Register::asset_definition(AssetDefinition::numeric(
            other_definition_id.clone(),
            "usd".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&attacker, &mut tx)
        .expect("register second unaliased attacker-owned definition");
        let error = SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&attacker, &mut tx)
            .expect_err("asset owner still needs exact victim-domain namespace scope");
        assert!(error.to_string().contains("CanManageAssetDefinitionAlias"));
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none()
        );
        tx.world.add_account_permission(
            &namespace_owner,
            Permission::from(CanManageAssetDefinitionAlias {
                scope: AssetDefinitionAliasPermissionScope::Domain(victim_domain.clone()),
            }),
        );
        let error = SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&namespace_owner, &mut tx)
            .expect_err("namespace authority must not replace asset-owner authority");
        assert!(error.to_string().contains("only asset-definition owner"));
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none()
        );
        let domain_asset_alias_permission = Permission::from(CanManageAssetDefinitionAlias {
            scope: AssetDefinitionAliasPermissionScope::Domain(victim_domain),
        });
        tx.world
            .add_account_permission(&attacker, domain_asset_alias_permission.clone());
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&attacker, &mut tx)
            .expect("delegated exact domain scope and asset ownership authorize binding");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
        assert!(
            tx.world
                .remove_account_permission(&attacker, &domain_asset_alias_permission),
            "domain-scoped asset alias permission must be removable"
        );
        tx.world.add_account_permission(
            &attacker,
            Permission::from(CanManageAssetDefinitionAlias {
                scope: AssetDefinitionAliasPermissionScope::Alias(
                    ResolvedAssetDefinitionAliasV1::new(
                        alias.clone(),
                        DataSpaceId::UNIVERSAL,
                        definition_id.clone(),
                    ),
                ),
            }),
        );
        SetAssetDefinitionAlias::clear(definition_id.clone())
            .execute(&attacker, &mut tx)
            .expect("exact asset alias permission authorizes clearing its binding");
        SetAssetDefinitionAlias::bind(other_definition_id, alias.clone(), None)
            .execute(&attacker, &mut tx)
            .expect_err("an exact alias capability must not migrate to another definition");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&attacker, &mut tx)
            .expect("exact asset alias permission authorizes restoring its binding");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn domainless_asset_alias_requires_exact_dataspace_namespace_scope() {
        let authority = (*ALICE_ID).clone();
        let mut state = test_state_with_authority(&authority);
        let issuer_domain =
            DomainId::try_new("dataspace-issuer", "universal").expect("issuer domain");
        seed_domain(&mut state, &issuer_domain, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            issuer_domain,
            "cash".parse().expect("asset name"),
        );
        let alias: AssetDefinitionAlias = "cash#paynet".parse().expect("dataspace-root alias");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let paynet = DataSpaceId::new(7);
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Public);
        Register::asset_definition(AssetDefinition::numeric(
            definition_id.clone(),
            "cash".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&authority, &mut tx)
        .expect("register unaliased definition");
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAssetDefinitionAlias {
                scope: AssetDefinitionAliasPermissionScope::Domain(
                    DomainId::try_new("unrelated", "paynet").expect("unrelated domain scope"),
                ),
            }),
        );
        let error = SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect_err("domain permission must not authorize a dataspace-root alias");
        assert!(error.to_string().contains("CanManageAssetDefinitionAlias"));
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none()
        );
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(paynet),
            }),
        );
        let error = SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect_err("account-alias dataspace permission must not authorize asset aliases");
        assert!(error.to_string().contains("CanManageAssetDefinitionAlias"));
        tx.world.add_account_permission(
            &authority,
            Permission::from(CanManageAssetDefinitionAlias {
                scope: AssetDefinitionAliasPermissionScope::Dataspace(paynet),
            }),
        );
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("delegated exact dataspace scope authorizes domainless binding");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn set_asset_definition_alias_updates_world_indexes() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId =
            DomainId::try_new("alias-update", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "usd".parse().expect("name"));
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
            owning_domain: None,
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
        let domain_id: DomainId = DomainId::try_new("alias-clear", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "usd".parse().expect("name"));
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
            owning_domain: None,
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
    fn set_asset_definition_alias_rejects_global_move_to_restricted_dataspace() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("alias-global", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let paynet = DataSpaceId::new(7);
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register global definition");
        let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("alias");
        let err = SetAssetDefinitionAlias::bind(definition_id, alias, None)
            .execute(&authority, &mut tx)
            .expect_err("global alias must not move home to restricted dataspace");
        assert!(
            err.to_string().contains("restricted dataspace"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn set_asset_definition_alias_allows_global_move_to_public_dataspace() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("alias-public", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "unit".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let paynet = DataSpaceId::new(7);
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Public);
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register global definition");
        let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("public dataspace may home a global asset alias");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn asset_home_extra_coverage_set_alias_allows_global_move_to_universal() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("alias-universal", "paynet").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            "unit".parse().expect("name"),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let paynet = DataSpaceId::new(7);
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        tx.world.insert_asset_definition_entry(
            definition_id.clone(),
            AssetDefinition::numeric(
                definition_id.clone(),
                "unit".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(domain_id),
            )
            .build(&authority),
        );
        let alias: AssetDefinitionAlias = "unit#universal".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("universal dataspace may home a global asset alias");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn asset_home_extra_coverage_clear_alias_keeps_global_home_universal() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId =
            DomainId::try_new("alias-clear-universal", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "unit".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register global definition");
        let alias: AssetDefinitionAlias = "unit#universal".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("bind universal alias");
        SetAssetDefinitionAlias::clear(definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("clearing alias should leave universal domain fallback");
        assert!(tx.world.asset_definition_aliases.get(&alias).is_none());
        assert!(
            tx.world
                .asset_definition_alias_bindings
                .get(&definition_id)
                .is_none()
        );
    }
    #[test]
    fn set_asset_definition_alias_clear_rejects_restricted_domain_fallback_for_global_asset() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let paynet = DataSpaceId::new(7);
        let domain_id: DomainId = DomainId::try_new("cash", "paynet").expect("domain");
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "unit".parse().expect("name"));
        let definition = AssetDefinition::numeric(
            definition_id.clone(),
            "unit".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        tx.world
            .asset_definitions
            .insert(definition_id.clone(), definition);
        tx.world
            .bind_asset_definition_alias(
                &definition_id,
                "unit#universal".parse().expect("alias"),
                None,
                None,
                10_000,
            )
            .expect("seed public alias");
        let err = SetAssetDefinitionAlias::clear(definition_id)
            .execute(&authority, &mut tx)
            .expect_err("clearing alias would expose restricted domain fallback");
        assert!(
            err.to_string().contains("restricted dataspace"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn set_asset_definition_alias_allows_restricted_policy_in_restricted_dataspace() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId =
            DomainId::try_new("alias-restricted", "universal").expect("domain");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "unit".parse().expect("name"));
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "unit".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let paynet = DataSpaceId::new(7);
        install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register restricted definition");
        let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("alias");
        SetAssetDefinitionAlias::bind(definition_id.clone(), alias.clone(), None)
            .execute(&authority, &mut tx)
            .expect("restricted asset alias may use restricted dataspace");
        assert_eq!(
            tx.world.asset_definition_aliases.get(&alias),
            Some(&definition_id)
        );
    }
    #[test]
    fn set_contract_alias_updates_world_indexes() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .contract_instances
            .insert(contract_address.clone(), Hash::new("contract-alias"));
        let alias: ContractAlias = "router::universal".parse().expect("alias");
        seed_contract_alias_manage_permissions(&mut tx, &authority, &alias);
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
    fn unprivileged_authority_cannot_bind_privileged_benefit_alias() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let benefit_dataspace = DataSpaceId::new(42);
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            benefit_dataspace,
        )
        .expect("address");
        let alias: ContractAlias = "benefit::benefit".parse().expect("benefit alias");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        install_dataspace_catalog_with_lane(
            &mut tx,
            benefit_dataspace,
            "benefit",
            LaneVisibility::Public,
        );
        tx.world.contract_instances.insert(
            contract_address.clone(),
            Hash::new("malicious-benefit-lookalike"),
        );
        let error = SetContractAlias::bind(contract_address, alias, None)
            .execute(&authority, &mut tx)
            .expect_err("unprivileged alias binding must fail closed");
        assert!(
            error
                .to_string()
                .contains("not permitted to manage contract alias"),
            "unexpected alias authorization error: {error}"
        );
    }
    #[test]
    fn set_contract_alias_allows_active_dynamic_sns_dataspace() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let dynamic_dataspace =
            crate::sns::dataspace_id_for_sns_alias("is").expect("dynamic dataspace id");
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            dynamic_dataspace,
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        seed_dataspace_alias_lease(&mut tx, &authority, "is");
        tx.world.contract_instances.insert(
            contract_address.clone(),
            Hash::new("dynamic-contract-alias"),
        );
        let alias: ContractAlias = "router::is".parse().expect("alias");
        seed_contract_alias_manage_permissions(&mut tx, &authority, &alias);
        SetContractAlias::bind(contract_address.clone(), alias.clone(), Some(11_000))
            .execute(&authority, &mut tx)
            .expect("bind dynamic dataspace contract alias");
        assert_eq!(
            tx.world.contract_aliases.get(&alias),
            Some(&contract_address)
        );
        assert_eq!(
            tx.world
                .contract_alias_bindings
                .get(&contract_address)
                .map(|record| record.alias.clone()),
            Some(alias)
        );
    }
    #[test]
    fn set_contract_alias_rejects_unknown_dynamic_dataspace_alias() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let dynamic_dataspace = DataSpaceId::new(4_242);
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            dynamic_dataspace,
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world.contract_instances.insert(
            contract_address.clone(),
            Hash::new("dynamic-contract-alias"),
        );
        let err = SetContractAlias::bind(
            contract_address,
            "router::missing".parse().expect("alias"),
            Some(11_000),
        )
        .execute(&authority, &mut tx)
        .expect_err("unknown dynamic dataspace must fail closed");
        let err_debug = format!("{err:?}");
        assert!(
            err_debug.contains("unknown or inactive dataspace alias `missing`"),
            "unexpected error: {err_debug}"
        );
    }
    #[test]
    fn set_contract_alias_clear_allows_stale_undeployed_binding() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .bind_contract_alias(
                &contract_address,
                "router::universal".parse().expect("alias"),
                None,
                None,
                10_000,
            )
            .expect("seed stale contract alias");
        seed_contract_alias_manage_permissions(
            &mut tx,
            &authority,
            &"router::universal".parse().expect("alias"),
        );
        SetContractAlias::clear(contract_address.clone())
            .execute(&authority, &mut tx)
            .expect("clear should tolerate undeployed stale alias");
        assert!(
            tx.world
                .contract_alias_bindings
                .get(&contract_address)
                .is_none(),
            "binding index should be removed"
        );
        assert!(
            tx.world
                .contract_aliases
                .get(&"router::universal".parse::<ContractAlias>().expect("alias"))
                .is_none(),
            "alias index should be removed"
        );
    }
    #[test]
    fn set_contract_alias_clear_allows_stale_dynamic_dataspace_binding() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let dynamic_dataspace =
            crate::sns::dataspace_id_for_sns_alias("is").expect("dynamic dataspace id");
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            dynamic_dataspace,
        )
        .expect("address");
        let alias: ContractAlias = "router::is".parse().expect("alias");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .bind_contract_alias(&contract_address, alias.clone(), None, None, 10_000)
            .expect("seed stale dynamic contract alias");
        seed_contract_alias_manage_permissions_in_dataspace(
            &mut tx,
            &authority,
            alias.name_segment().parse().expect("alias label"),
            alias
                .domain_segment()
                .map(|name| AccountAliasDomain::new(name.parse().expect("alias domain"))),
            dynamic_dataspace,
        );
        SetContractAlias::clear(contract_address.clone())
            .execute(&authority, &mut tx)
            .expect("clear should tolerate undeployed dynamic alias");
        assert!(
            tx.world
                .contract_alias_bindings
                .get(&contract_address)
                .is_none(),
            "binding index should be removed"
        );
        assert!(
            tx.world.contract_aliases.get(&alias).is_none(),
            "alias index should be removed"
        );
    }
    #[test]
    fn set_contract_alias_clear_allows_unknown_dynamic_dataspace_without_binding() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::new(4_242),
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        SetContractAlias::clear(contract_address.clone())
            .execute(&authority, &mut tx)
            .expect("clear should be a no-op for unknown undeployed dynamic address");
        assert!(
            tx.world
                .contract_alias_bindings
                .get(&contract_address)
                .is_none(),
            "clear should not create a binding"
        );
    }
    #[test]
    fn set_contract_alias_clear_rejects_lease_without_alias() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::new(4_242),
        )
        .expect("address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = SetContractAlias {
            contract_address,
            alias: None,
            lease_expiry_ms: Some(11_000),
        }
        .execute(&authority, &mut tx)
        .expect_err("lease metadata without alias must fail");
        let err_debug = format!("{err:?}");
        assert!(
            err_debug.contains("lease_expiry_ms requires alias binding"),
            "unexpected error: {err_debug}"
        );
    }
    #[test]
    fn set_contract_alias_rejects_account_alias_collision() {
        let state = test_state();
        let authority = (*ALICE_ID).clone();
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("address");
        let label =
            AccountAlias::domainless("router".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 10_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .contract_instances
            .insert(contract_address.clone(), Hash::new("contract-alias"));
        seed_account_alias_lease(&mut tx, &authority, &label);
        seed_contract_alias_manage_permissions(
            &mut tx,
            &authority,
            &"router::universal".parse().expect("alias"),
        );
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
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("address");
        let label =
            AccountAlias::domainless("router".parse().expect("label"), DataSpaceId::UNIVERSAL);
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
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }),
        );
        seed_account_alias_lease(&mut tx, &authority, &label);
        tx.world
            .contract_instances
            .insert(contract_address.clone(), Hash::new("contract-alias"));
        tx.world
            .bind_contract_alias(
                &contract_address,
                "router::universal".parse().expect("alias"),
                Some(11_000),
                Some(11_000 + 369u64 * 60 * 60 * 1_000),
                10_000,
            )
            .expect("seed contract alias");
        let err = EnsureTestAccountAliasBinding {
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
        let domain_id: DomainId = DomainId::try_new("alias-grace", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "usd".parse().expect("name"));
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
            owning_domain: None,
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
        let domain_id: DomainId =
            DomainId::try_new("alias-past-expiry", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "usd".parse().expect("name"));
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
            owning_domain: None,
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
    fn legacy_offline_metadata_is_ordinary_metadata() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("offline3", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let asset_name: Name = "gbp".parse().expect("asset name");
        let definition_id =
            AssetDefinitionId::derive_from_components(domain_id.clone(), asset_name);
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
            owning_domain: None,
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
            "offline.enabled".parse().expect("legacy metadata key"),
            Json::new(true),
        )
        .execute(&authority, &mut tx)
        .expect("set ordinary metadata");
        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&definition_id)
                .is_none(),
            "metadata must not create offline runtime state"
        );
    }
    #[test]
    fn legacy_offline_false_metadata_does_not_change_runtime_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id = DomainId::try_new("offline-disable", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "cash".parse().expect("asset name"),
        );
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "Offline cash".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");
        let metadata_key: Name = "offline.enabled".parse().expect("legacy metadata key");
        SetKeyValue::asset_definition(
            definition_id.clone(),
            metadata_key.clone(),
            Json::new(false),
        )
        .execute(&authority, &mut tx)
        .expect("store ordinary metadata");
        assert_eq!(
            tx.world
                .asset_definition(&definition_id)
                .expect("asset definition remains registered")
                .metadata()
                .get(&metadata_key),
            Some(&Json::new(false))
        );
        assert!(
            tx.settlement.offline.escrow_accounts.is_empty(),
            "legacy-looking metadata must not materialize offline state"
        );
    }
    #[test]
    fn removing_legacy_offline_metadata_does_not_change_runtime_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id = DomainId::try_new("offline-remove", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "cash".parse().expect("asset name"),
        );
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "Offline cash".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");
        let metadata_key: Name = "offline.enabled".parse().expect("legacy metadata key");
        SetKeyValue::asset_definition(definition_id.clone(), metadata_key.clone(), Json::new(true))
            .execute(&authority, &mut tx)
            .expect("store ordinary metadata");
        RemoveKeyValue::asset_definition(definition_id.clone(), metadata_key.clone())
            .execute(&authority, &mut tx)
            .expect("remove offline opt-in metadata");
        assert!(
            tx.world
                .asset_definition(&definition_id)
                .expect("asset definition remains registered")
                .metadata()
                .get(&metadata_key)
                .is_none(),
            "metadata must be removed"
        );
        assert!(
            tx.settlement.offline.escrow_accounts.is_empty(),
            "metadata removal must not materialize offline state"
        );
    }
    #[test]
    fn legacy_offline_true_metadata_does_not_change_runtime_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId =
            DomainId::try_new("offline-metadata-disabled", "universal").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let definition_id = AssetDefinitionId::derive_from_components(
            domain_id,
            "gbp".parse().expect("asset name"),
        );
        let definition = NewAssetDefinition {
            id: definition_id.clone(),
            name: "GBP".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: iroha_data_model::asset::AssetBalancePolicy::Global,
            owning_domain: None,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(definition)
            .execute(&authority, &mut tx)
            .expect("register baseline asset definition");
        let metadata_key: Name = "offline.enabled".parse().expect("legacy metadata key");
        SetKeyValue::asset_definition(definition_id.clone(), metadata_key.clone(), Json::new(true))
            .execute(&authority, &mut tx)
            .expect("store ordinary metadata");
        assert_eq!(
            tx.world
                .asset_definition(&definition_id)
                .expect("baseline definition remains registered")
                .metadata()
                .get(&metadata_key),
            Some(&Json::new(true)),
            "metadata should be stored unchanged"
        );
        assert!(
            tx.settlement.offline.escrow_accounts.is_empty(),
            "legacy-looking metadata must not materialize offline state"
        );
    }
    #[test]
    fn unregister_asset_definition_rejects_when_definition_has_repo_agreement_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        let counterparty_domain: DomainId =
            DomainId::try_new("counter", "guard").expect("counterparty domain");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &counterparty_domain, &authority);
        let initiator = AccountId::new(checked_keypair().public_key().clone());
        let counterparty = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain.clone(), "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        let repo_id: iroha_data_model::repo::RepoAgreementId =
            "repo_asset_guard".parse().expect("repo agreement id");
        tx.world
            .insert_repo_agreement_entry(iroha_data_model::repo::RepoAgreement::new(
                repo_id,
                initiator.clone(),
                counterparty.clone(),
                iroha_data_model::repo::RepoCashLeg {
                    asset_definition_id: asset_definition_id.clone(),
                    quantity: Quantity::from(10_u32),
                },
                AssetId::new(asset_definition_id.clone(), counterparty.clone()),
                iroha_data_model::repo::RepoCollateralLeg::new(
                    AssetDefinitionId::derive_from_components(
                        asset_domain.clone(),
                        "bond".parse().unwrap(),
                    ),
                    Quantity::from(12_u32),
                ),
                AssetId::new(
                    AssetDefinitionId::derive_from_components(
                        asset_domain,
                        "bond".parse().unwrap(),
                    ),
                    counterparty,
                ),
                250,
                1_000,
                1,
                iroha_data_model::repo::RepoGovernance::with_defaults(1_000, 60),
                None,
            ));
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
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
    fn unregister_asset_definition_rejects_immutable_governance_lock_custody_after_config_change() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            asset_domain.clone(),
            "locked".parse().unwrap(),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "locked".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register retained custody asset definition");
        let owner = (*BOB_ID).clone();
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            owner.clone(),
            GovernanceLockRecord {
                owner,
                amount: Quantity::from(150_u32),
                slashed: Quantity::zero(),
                expiry_height: 10,
                direction: 0,
                duration_blocks: 3_600,
                custody: Some(GovernanceLockCustody {
                    escrowed: true,
                    asset_definition_id: asset_definition_id.clone(),
                    bond_escrow_account: authority.clone(),
                    slash_receiver_account: authority.clone(),
                }),
            },
        );
        tx.world
            .put_governance_locks("retained-asset-custody".to_owned(), locks);
        tx.gov.voting_asset_id =
            AssetDefinitionId::derive_from_components(asset_domain, "replacement".parse().unwrap());
        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err("immutable lock custody asset definition must remain registered");
        assert!(
            err.to_string()
                .contains("retained by immutable governance lock custody"),
            "error should identify retained lock custody: {err}"
        );
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_some(),
            "custody asset definition must remain after rejected unregister"
        );
    }
    #[test]
    fn unregister_asset_definition_rejects_when_definition_is_governance_viral_reward_asset() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
    fn unregister_asset_definition_removes_associated_permissions_from_accounts_and_roles() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        let holder_domain: DomainId =
            DomainId::try_new("holder", "guard").expect("holder domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &holder_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain.clone(), "usd".parse().unwrap());
        let asset_account = AccountId::new(checked_keypair().public_key().clone());
        let holder_id = AccountId::new(checked_keypair().public_key().clone());
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
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        let alias: AssetDefinitionAlias = "usd#universal".parse().expect("asset alias");
        let bound_at_ms = tx.block_unix_timestamp_ms();
        tx.world
            .bind_asset_definition_alias(
                &asset_definition_id,
                alias.clone(),
                None,
                None,
                bound_at_ms,
            )
            .expect("bind exact-permission fixture alias");
        let permission_with_definition: Permission = iroha_executor_data_model::permission::asset_definition::CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        }
        .into();
        let permission_with_confidential_policy: Permission = iroha_executor_data_model::permission::asset_definition::CanManageAssetDefinitionConfidentialPolicy {
            asset_definition: asset_definition_id.clone(),
        }
        .into();
        let permission_with_exact_alias: Permission = CanManageAssetDefinitionAlias {
            scope: AssetDefinitionAliasPermissionScope::Alias(ResolvedAssetDefinitionAliasV1::new(
                alias,
                DataSpaceId::UNIVERSAL,
                asset_definition_id.clone(),
            )),
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
        Grant::account_permission(
            permission_with_confidential_policy.clone(),
            holder_id.clone(),
        )
        .execute(&authority, &mut tx)
        .expect("grant confidential policy permission to holder");
        Grant::account_permission(permission_with_asset.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant asset permission to holder");
        Grant::account_permission(permission_with_exact_alias.clone(), holder_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant exact alias permission to holder");
        let role_id: RoleId = "ASSET_CLEANUP".parse().expect("role id");
        Register::role(Role::new(role_id.clone(), holder_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register role");
        Grant::role_permission(permission_with_definition.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant definition permission to role");
        Grant::role_permission(permission_with_confidential_policy.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant confidential policy permission to role");
        Grant::role_permission(permission_with_asset.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant asset permission to role");
        Grant::role_permission(permission_with_exact_alias.clone(), role_id.clone())
            .execute(&authority, &mut tx)
            .expect("grant exact alias permission to role");
        assert!(
            tx.world
                .account_permissions
                .get(&holder_id)
                .is_some_and(|perms| {
                    perms.contains(&permission_with_definition)
                        && perms.contains(&permission_with_confidential_policy)
                        && perms.contains(&permission_with_asset)
                        && perms.contains(&permission_with_exact_alias)
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
                .any(|perm| perm == &permission_with_confidential_policy),
            "role should include confidential policy permission before unregister"
        );
        assert!(
            role.permissions()
                .any(|perm| perm == &permission_with_asset),
            "role should include asset permission before unregister"
        );
        assert!(
            role.permissions()
                .any(|perm| perm == &permission_with_exact_alias),
            "role should include exact alias permission before unregister"
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
                        || perms.contains(&permission_with_confidential_policy)
                        || perms.contains(&permission_with_asset)
                        || perms.contains(&permission_with_exact_alias)
                }),
            "holder permissions should be removed"
        );
        let role = tx.world.roles.get(&role_id).expect("role should exist");
        assert!(
            !role
                .permissions()
                .any(|perm| perm == &permission_with_confidential_policy),
            "role confidential policy permission should be removed"
        );
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
                .permissions()
                .any(|perm| perm == &permission_with_exact_alias),
            "role exact alias permission should be removed"
        );
        assert!(
            !role
                .permission_epochs()
                .contains_key(&permission_with_confidential_policy),
            "confidential policy permission epoch should be pruned"
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
        assert!(
            !role
                .permission_epochs()
                .contains_key(&permission_with_exact_alias),
            "exact alias permission epoch should be pruned"
        );
    }
    #[test]
    fn unregister_asset_definition_removes_offline_escrow_mapping() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &asset_domain, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
    fn unregister_asset_definition_rejects_when_definition_has_committed_settlement_receipt() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let asset_domain: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        let counterparty_domain: DomainId =
            DomainId::try_new("counter", "guard").expect("counterparty domain");
        seed_domain(&mut state, &asset_domain, &authority);
        seed_domain(&mut state, &counterparty_domain, &authority);
        let from = AccountId::new(checked_keypair().public_key().clone());
        let to = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(asset_domain, "usd".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "usd".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        let settlement_id: iroha_data_model::isi::SettlementId =
            "settle_asset_guard".parse().expect("settlement id");
        let receipt = iroha_data_model::isi::SettlementReceipt {
            kind: iroha_data_model::isi::SettlementKind::Dvp,
            authority: from.clone(),
            plan: iroha_data_model::isi::SettlementPlan::default(),
            metadata: Metadata::default(),
            block_height: 1,
            block_hash:
                iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0; Hash::LENGTH]),
                ),
            executed_at_ms: 1,
            legs: [
                iroha_data_model::isi::SettlementLegSnapshot {
                    role: iroha_data_model::isi::SettlementLegRole::Delivery,
                    leg: iroha_data_model::isi::SettlementLeg::new(
                        asset_definition_id.clone(),
                        Quantity::one(),
                        from.clone(),
                        to.clone(),
                    ),
                },
                iroha_data_model::isi::SettlementLegSnapshot {
                    role: iroha_data_model::isi::SettlementLegRole::Payment,
                    leg: iroha_data_model::isi::SettlementLeg::new(
                        AssetDefinitionId::derive_from_components(
                            counterparty_domain,
                            "eur".parse().expect("asset name"),
                        ),
                        Quantity::one(),
                        to,
                        from,
                    ),
                },
            ],
            fx_corridor: None,
        };
        tx.world.settlement_receipts.insert(settlement_id, receipt);
        let err = Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect_err(
                "asset definition with a committed settlement receipt must not be unregistered",
            );
        let err_string = err.to_string();
        assert!(
            err_string.contains("committed settlement receipt"),
            "error should explain settlement receipt conflict: {err_string}"
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
    fn unregister_asset_definition_ignores_mismatched_public_lane_reward_record() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("asset", "guard").expect("asset domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let account_id = AccountId::new(checked_keypair().public_key().clone());
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "fee".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "fee".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
        })
        .execute(&authority, &mut tx)
        .expect("register asset definition");
        tx.world.public_lane_rewards.insert(
            (LaneId::SINGLE, 1),
            iroha_data_model::nexus::PublicLaneRewardRecord {
                lane_id: LaneId::new(1),
                epoch: 1,
                asset: AssetId::new(asset_definition_id.clone(), account_id.clone()),
                total_reward: Quantity::from(1_u32),
                shares: vec![iroha_data_model::nexus::PublicLaneRewardShare {
                    account: account_id,
                    role: iroha_data_model::nexus::PublicLaneRewardRole::Validator,
                    amount: Quantity::from(1_u32),
                }],
                metadata: Metadata::default(),
            },
        );
        Unregister::asset_definition(asset_definition_id.clone())
            .execute(&authority, &mut tx)
            .expect("mismatched public-lane reward row must not block asset definition unregister");
        assert!(
            tx.world
                .asset_definitions
                .get(&asset_definition_id)
                .is_none(),
            "asset definition should be removed when only malformed rewards reference it"
        );
        assert!(
            tx.world
                .public_lane_rewards
                .get(&(LaneId::SINGLE, 1))
                .is_some(),
            "malformed reward row remains as stored"
        );
    }
    #[test]
    fn unregister_asset_definition_removes_confidential_state() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = DomainId::try_new("zk", "guard").expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let asset_definition_id =
            AssetDefinitionId::derive_from_components(domain_id, "shield".parse().unwrap());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition({
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "shield".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
