//! SNS-backed ownership query and lease instruction handlers.

use iroha_data_model::{
    asset::AssetBalancePolicy,
    isi::{
        account_alias_lease::{AcquireAccountAliasLease, RenewAccountAliasLease},
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    nexus::DataSpaceId,
    query::{error::QueryExecutionFail as QueryError, sns::prelude::*},
    sns::{
        GovernanceHookV1, NameControllerV1, RegisterNameRequestV1, RenewNameRequestV1, SuffixId,
        TransferNameRequestV1,
    },
};
use iroha_telemetry::metrics;
use norito::codec::Decode;

use super::prelude::*;
use crate::{alias::authority_can_manage_account_alias, prelude::ValidSingularQuery};

impl ValidSingularQuery for FindDataspaceNameOwnerById {
    #[metrics(+"find_dataspace_name_owner_by_id")]
    fn execute(&self, state_ro: &impl StateReadOnly) -> Result<AccountId, QueryError> {
        let now_ms = state_ro.latest_block().map_or(0, |block| {
            u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX)
        });
        crate::sns::active_dataspace_owner_by_id(
            state_ro.world(),
            &state_ro.nexus().dataspace_catalog,
            self.dataspace_id(),
            now_ms,
        )
        .ok_or(QueryError::NotFound)
    }
}

fn alias_lease_instruction_error(err: crate::sns::SnsError) -> InstructionExecutionError {
    match err {
        crate::sns::SnsError::NotFound(message)
        | crate::sns::SnsError::BadRequest(message)
        | crate::sns::SnsError::Conflict(message)
        | crate::sns::SnsError::Internal(message) => {
            InstructionExecutionError::InvariantViolation(message.into())
        }
    }
}

fn sns_mutation_instruction_error(err: crate::sns::SnsError) -> InstructionExecutionError {
    match err {
        crate::sns::SnsError::BadRequest(message) => InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message.into()),
        ),
        crate::sns::SnsError::NotFound(message)
        | crate::sns::SnsError::Conflict(message)
        | crate::sns::SnsError::Internal(message) => {
            InstructionExecutionError::InvariantViolation(message.into())
        }
    }
}

fn namespace_from_suffix_id(
    suffix_id: SuffixId,
) -> Result<crate::sns::SnsNamespace, InstructionExecutionError> {
    crate::sns::SnsNamespace::from_suffix_id(suffix_id).map_err(sns_mutation_instruction_error)
}

fn ensure_configured_policy_payment_asset(
    state_transaction: &StateTransaction<'_, '_>,
    namespace: crate::sns::SnsNamespace,
) -> Result<(), InstructionExecutionError> {
    crate::sns::ensure_namespace_policy_payment_asset_matches_configured(
        state_transaction.world(),
        namespace,
        &state_transaction.nexus.fees.fee_asset_id,
    )
    .map_err(sns_mutation_instruction_error)
}

fn reject_generic_account_alias_mutation(
    namespace: crate::sns::SnsNamespace,
    instruction: &str,
) -> Result<(), InstructionExecutionError> {
    if namespace != crate::sns::SnsNamespace::AccountAlias {
        return Ok(());
    }
    Err(InstructionExecutionError::InvalidParameter(
        InvalidParameterError::SmartContract(
            format!(
                "{instruction} is unavailable for the account-alias namespace; use the dedicated account-alias lease/binding/rekey instructions"
            )
            .into(),
        ),
    ))
}

#[cfg(any(test, feature = "telemetry"))]
fn metric_namespace_from_suffix_id(suffix_id: SuffixId) -> String {
    crate::sns::SnsNamespace::from_suffix_id(suffix_id)
        .map(|namespace| namespace.as_path().to_owned())
        .unwrap_or_else(|_| suffix_id.to_string())
}

#[cfg(feature = "telemetry")]
fn record_sns_registrar_status<T, E>(
    state_transaction: &StateTransaction<'_, '_>,
    suffix_id: SuffixId,
    outcome: &Result<T, E>,
) {
    let result = if outcome.is_ok() { "ok" } else { "error" };
    let namespace = metric_namespace_from_suffix_id(suffix_id);
    state_transaction
        .telemetry
        .inc_sns_registrar_status(result, &namespace);
}

#[cfg(not(feature = "telemetry"))]
fn record_sns_registrar_status<T, E>(
    _state_transaction: &StateTransaction<'_, '_>,
    _suffix_id: SuffixId,
    _outcome: &Result<T, E>,
) {
}

fn decode_sns_payload<T: Decode>(bytes: &[u8], instruction_name: &str) -> Result<T, Error> {
    let mut cursor = bytes;
    let value = T::decode(&mut cursor).map_err(|err| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            format!("{instruction_name} payload failed to decode: {err}").into(),
        ))
    })?;
    if !cursor.is_empty() {
        return Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(
                format!("{instruction_name} payload has trailing bytes").into(),
            ),
        ));
    }
    Ok(value)
}

fn ensure_payment_payer_is_authority(
    payer: &AccountId,
    authority: &AccountId,
    instruction_name: &str,
) -> Result<(), InstructionExecutionError> {
    if payer == authority {
        return Ok(());
    }
    Err(InstructionExecutionError::InvalidParameter(
        InvalidParameterError::SmartContract(
            format!("{instruction_name} payment payer must match the transaction authority").into(),
        ),
    ))
}

fn ensure_name_owner_authority(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    namespace: crate::sns::SnsNamespace,
    literal: &str,
) -> Result<(), InstructionExecutionError> {
    let record = crate::sns::get_name_record(
        state_transaction.world(),
        &state_transaction.nexus.dataspace_catalog,
        namespace,
        literal,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(sns_mutation_instruction_error)?;
    if record.owner == *authority {
        return Ok(());
    }
    Err(InstructionExecutionError::InvariantViolation(
        "transaction authority must own the SNS name".into(),
    ))
}

fn account_controller_for(
    owner: &AccountId,
) -> Result<NameControllerV1, InstructionExecutionError> {
    AccountAddress::from_account_id(owner)
        .map(|address| NameControllerV1::account(&address))
        .map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to derive account-alias controller: {err}").into(),
            )
        })
}

fn dataspace_id_for_asset_alias_segment(
    catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    dataspace_alias: &str,
) -> Option<DataSpaceId> {
    if dataspace_alias.eq_ignore_ascii_case("universal") {
        return Some(DataSpaceId::UNIVERSAL);
    }
    catalog.by_alias(dataspace_alias).map(|entry| entry.id)
}

fn global_payment_asset_home_dataspace_id(
    state_transaction: &StateTransaction<'_, '_>,
    definition_id: &iroha_data_model::asset::AssetDefinitionId,
) -> Result<Option<DataSpaceId>, Error> {
    let definition = state_transaction
        .world
        .asset_definition(definition_id)
        .map_err(Error::from)?;
    if definition.balance_scope_policy() != AssetBalancePolicy::Global {
        return Ok(None);
    }

    let dataspace_alias = definition
        .alias()
        .as_ref()
        .map(|alias| alias.dataspace_segment().to_owned())
        .or_else(|| {
            definition
                .id()
                .try_domain()
                .map(|domain| domain.dataspace().as_ref().to_owned())
        });

    Ok(match dataspace_alias {
        Some(alias) => {
            dataspace_id_for_asset_alias_segment(&state_transaction.nexus.dataspace_catalog, &alias)
        }
        None => Some(DataSpaceId::UNIVERSAL),
    })
}

fn should_externally_settle_sns_quote(
    quote: &crate::sns::LeaseQuote,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<bool, Error> {
    if quote.charge_amount.is_zero()
        || !state_transaction.nexus.fees.sponsorship_enabled
        || !state_transaction.nexus.fees.external_settlement_enabled
    {
        return Ok(false);
    }

    let Some(configured_fee_asset_id) = crate::block::parse_asset_definition_literal_with_world(
        state_transaction.world(),
        &state_transaction.nexus.fees.fee_asset_id,
        state_transaction.block_unix_timestamp_ms(),
    ) else {
        return Ok(false);
    };
    if configured_fee_asset_id != quote.payment_asset_definition_id {
        return Ok(false);
    }

    let Some(route_dataspace) = state_transaction
        .current_dataspace_id
        .or(state_transaction.world.current_dataspace_id)
    else {
        return Ok(false);
    };
    let Some(home_dataspace) = global_payment_asset_home_dataspace_id(
        state_transaction,
        &quote.payment_asset_definition_id,
    )?
    else {
        return Ok(false);
    };

    Ok(route_dataspace != home_dataspace)
}

fn account_alias_lease_payer_matches_dataspace_sponsor(
    payer: &AccountId,
    quote: &crate::sns::LeaseQuote,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<bool, Error> {
    if !should_externally_settle_sns_quote(quote, state_transaction)? {
        return Ok(false);
    }
    let Some(route_dataspace) = state_transaction
        .current_dataspace_id
        .or(state_transaction.world.current_dataspace_id)
    else {
        return Ok(false);
    };
    Ok(crate::state::dataspace_fee_sponsor_matches(
        state_transaction.world(),
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.dataspace_fee_sponsors,
        route_dataspace,
        payer,
        state_transaction.block_unix_timestamp_ms(),
    ))
}

fn ensure_account_alias_lease_payer_allowed(
    payer: &AccountId,
    authority: &AccountId,
    quote: &crate::sns::LeaseQuote,
    instruction_name: &str,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if payer == authority
        || account_alias_lease_payer_matches_dataspace_sponsor(payer, quote, state_transaction)?
    {
        return Ok(());
    }

    Err(InstructionExecutionError::InvalidParameter(
        InvalidParameterError::SmartContract(
            format!(
                "{instruction_name} payment payer must match transaction authority or configured dataspace fee sponsor"
            )
            .into(),
        ),
    )
    .into())
}

impl Execute for AcquireAccountAliasLease {
    #[metrics(+"acquire_account_alias_lease")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Self {
            alias,
            owner,
            payer,
            term_years,
            pricing_class_hint,
        } = self;

        if owner != *authority
            && !authority_can_manage_account_alias(&state_transaction.world, authority, &alias)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "transaction authority must own the alias or hold CanManageAccountAlias".into(),
            ));
        }
        // Derive and validate the canonical owner controller before policy lookup or charging.
        let controllers = vec![account_controller_for(&owner)?];

        ensure_configured_policy_payment_asset(
            state_transaction,
            crate::sns::SnsNamespace::AccountAlias,
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms();
        let quote = crate::sns::quote_account_alias_registration(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            &alias,
            &owner,
            term_years,
            pricing_class_hint,
            now_ms,
        )
        .map_err(alias_lease_instruction_error)?;
        ensure_account_alias_lease_payer_allowed(
            &payer,
            authority,
            &quote,
            "AcquireAccountAliasLease",
            state_transaction,
        )?;
        let payment = charge_sns_quote(&quote, payer, authority, state_transaction)?;
        crate::sns::register_name(
            state_transaction,
            RegisterNameRequestV1 {
                selector: quote.selector,
                owner,
                controllers,
                term_years,
                pricing_class_hint: Some(quote.pricing_class),
                payment,
                governance: None,
                metadata: Metadata::default(),
            },
        )
        .map(|_| ())
        .map_err(alias_lease_instruction_error)
    }
}

impl Execute for RenewAccountAliasLease {
    #[metrics(+"renew_account_alias_lease")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Self {
            alias,
            payer,
            term_years,
        } = self;

        if payer != *authority {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "RenewAccountAliasLease payer must match the transaction authority".into(),
                ),
            ));
        }

        let literal = alias
            .to_literal(&state_transaction.nexus.dataspace_catalog)
            .map_err(|err| {
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    err.to_string().into(),
                ))
            })?;
        let record = crate::sns::get_name_record(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            crate::sns::SnsNamespace::AccountAlias,
            &literal,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(alias_lease_instruction_error)?;
        if record.owner != *authority
            && !authority_can_manage_account_alias(&state_transaction.world, authority, &alias)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "transaction authority must own the alias or hold CanManageAccountAlias".into(),
            ));
        }

        ensure_configured_policy_payment_asset(
            state_transaction,
            crate::sns::SnsNamespace::AccountAlias,
        )?;
        let quote = crate::sns::quote_account_alias_renewal(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            &alias,
            term_years,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(alias_lease_instruction_error)?;
        let payment = charge_sns_quote(&quote, payer, authority, state_transaction)?;
        crate::sns::renew_name(
            state_transaction,
            crate::sns::SnsNamespace::AccountAlias,
            &literal,
            RenewNameRequestV1 {
                term_years,
                payment,
            },
        )
        .map(|_| ())
        .map_err(alias_lease_instruction_error)
    }
}

fn charge_sns_quote(
    quote: &crate::sns::LeaseQuote,
    payer: AccountId,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<iroha_data_model::sns::PaymentProofV1, Error> {
    if should_externally_settle_sns_quote(quote, state_transaction)? {
        return Ok(crate::sns::payment_proof_for_quote(quote, payer));
    }

    let charge = quote.charge_amount.clone();
    Transfer::asset_quantity(
        AssetId::of(quote.payment_asset_definition_id.clone(), payer.clone()),
        charge,
        quote.collector_account.clone(),
    )
    .execute(authority, state_transaction)?;
    Ok(crate::sns::payment_proof_for_quote(quote, payer))
}

impl Execute for iroha_data_model::isi::sns::RegisterSnsName {
    #[metrics(+"register_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut request: RegisterNameRequestV1 =
            decode_sns_payload(&self.request, "RegisterSnsName")?;
        let suffix_id = request.selector.suffix_id;
        let result = (|| {
            ensure_payment_payer_is_authority(
                &request.payment.payer,
                authority,
                "RegisterSnsName",
            )?;
            let namespace = crate::sns::SnsNamespace::from_suffix_id(request.selector.suffix_id)
                .map_err(sns_mutation_instruction_error)?;
            reject_generic_account_alias_mutation(namespace, "RegisterSnsName")?;
            crate::sns::validate_name_controllers(&request.controllers)
                .map_err(sns_mutation_instruction_error)?;
            ensure_configured_policy_payment_asset(state_transaction, namespace)?;
            let quote = crate::sns::quote_name_registration(
                state_transaction.world(),
                &state_transaction.nexus.dataspace_catalog,
                request.selector.clone(),
                &request.owner,
                request.term_years,
                request.pricing_class_hint,
                state_transaction.block_unix_timestamp_ms(),
            )
            .map_err(sns_mutation_instruction_error)?;
            request.payment = charge_sns_quote(
                &quote,
                request.payment.payer.clone(),
                authority,
                state_transaction,
            )?;
            crate::sns::register_name(state_transaction, request)
                .map(|_| ())
                .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

impl Execute for iroha_data_model::isi::sns::RenewSnsName {
    #[metrics(+"renew_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut request: RenewNameRequestV1 = decode_sns_payload(&self.request, "RenewSnsName")?;
        let suffix_id = self.suffix_id;
        let result = (|| {
            ensure_payment_payer_is_authority(&request.payment.payer, authority, "RenewSnsName")?;
            let namespace = namespace_from_suffix_id(suffix_id)?;
            reject_generic_account_alias_mutation(namespace, "RenewSnsName")?;
            ensure_configured_policy_payment_asset(state_transaction, namespace)?;
            let quote = crate::sns::quote_name_renewal(
                state_transaction.world(),
                &state_transaction.nexus.dataspace_catalog,
                namespace,
                &self.literal,
                request.term_years,
                state_transaction.block_unix_timestamp_ms(),
            )
            .map_err(sns_mutation_instruction_error)?;
            request.payment = charge_sns_quote(
                &quote,
                request.payment.payer.clone(),
                authority,
                state_transaction,
            )?;
            crate::sns::renew_name(state_transaction, namespace, &self.literal, request)
                .map(|_| ())
                .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

impl Execute for iroha_data_model::isi::sns::TransferSnsName {
    #[metrics(+"transfer_sns_name")]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let suffix_id = self.suffix_id;
        let result = (|| {
            let namespace = namespace_from_suffix_id(suffix_id)?;
            let request: TransferNameRequestV1 =
                decode_sns_payload(&self.request, "TransferSnsName")?;
            reject_generic_account_alias_mutation(namespace, "TransferSnsName")?;
            crate::sns::transfer_name(state_transaction, namespace, &self.literal, request)
                .map(|_| ())
                .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

impl Execute for iroha_data_model::isi::sns::UpdateSnsNameControllers {
    #[metrics(+"update_sns_name_controllers")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let suffix_id = self.suffix_id;
        let result = (|| {
            let namespace = namespace_from_suffix_id(suffix_id)?;
            reject_generic_account_alias_mutation(namespace, "UpdateSnsNameControllers")?;
            let request: iroha_data_model::sns::UpdateControllersRequestV1 =
                decode_sns_payload(&self.request, "UpdateSnsNameControllers")?;
            crate::sns::validate_name_controllers(&request.controllers)
                .map_err(sns_mutation_instruction_error)?;
            ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
            crate::sns::update_name_controllers(
                state_transaction,
                namespace,
                &self.literal,
                request,
            )
            .map(|_| ())
            .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

impl Execute for iroha_data_model::isi::sns::FreezeSnsName {
    #[metrics(+"freeze_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let suffix_id = self.suffix_id;
        let result = (|| {
            let namespace = namespace_from_suffix_id(suffix_id)?;
            reject_generic_account_alias_mutation(namespace, "FreezeSnsName")?;
            let request = decode_sns_payload(&self.request, "FreezeSnsName")?;
            ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
            crate::sns::freeze_name(state_transaction, namespace, &self.literal, request)
                .map(|_| ())
                .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

impl Execute for iroha_data_model::isi::sns::UnfreezeSnsName {
    #[metrics(+"unfreeze_sns_name")]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let suffix_id = self.suffix_id;
        let result = (|| {
            let namespace = namespace_from_suffix_id(suffix_id)?;
            let governance: GovernanceHookV1 =
                decode_sns_payload(&self.governance, "UnfreezeSnsName")?;
            reject_generic_account_alias_mutation(namespace, "UnfreezeSnsName")?;
            crate::sns::unfreeze_name(state_transaction, namespace, &self.literal, governance)
                .map(|_| ())
                .map_err(sns_mutation_instruction_error)
        })();
        record_sns_registrar_status(state_transaction, suffix_id, &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{
            Account, AccountAddress,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{Mint, Register, domain_link::SetAccountAliasBinding},
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        permission::Permission,
        query::sns::prelude::FindDataspaceNameOwnerById,
        sns::{
            NameControllerV1, NameRecordV1, PaymentProofV1, RegisterNameRequestV1,
            RenewNameRequestV1, UpdateControllersRequestV1,
        },
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias,
    };
    use iroha_primitives::numeric::{Numeric, Quantity};
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        sns::{
            ACCOUNT_ALIAS_SUFFIX_ID, SnsNamespace, get_name_record, policy_by_id,
            seed_default_namespace_policies,
        },
        state::{State, StateTransaction, World, WorldReadOnly},
    };

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
    }

    #[test]
    fn registrar_metric_namespace_uses_stable_sns_paths() {
        assert_eq!(
            metric_namespace_from_suffix_id(ACCOUNT_ALIAS_SUFFIX_ID),
            "account-alias"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(crate::sns::DOMAIN_NAME_SUFFIX_ID),
            "domain"
        );
        assert_eq!(
            metric_namespace_from_suffix_id(crate::sns::DATASPACE_ALIAS_SUFFIX_ID),
            "dataspace"
        );
        assert_eq!(metric_namespace_from_suffix_id(65_535), "65535");
    }

    #[test]
    fn generic_account_alias_lifecycle_mutations_are_all_rejected() {
        let owner = owner();
        let other = another_owner();
        let owner_account = Account::new(owner.clone()).build(&owner);
        let other_account = Account::new(other.clone()).build(&owner);
        let mut world = World::with([], [owner_account, other_account], []);
        let alias =
            AccountAlias::domainless("canonical".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = crate::sns::selector_for_account_alias(&alias, &DataSpaceCatalog::default())
            .expect("selector");
        let owner_address = AccountAddress::from_account_id(&owner).expect("owner address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&owner_address)],
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
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let literal = "canonical@universal";
        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        stx.world
            .insert_account_alias_binding(alias.clone(), owner.clone());

        let payment_asset_definition_id: AssetDefinitionId =
            "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().expect("asset id");
        let register_err =
            iroha_data_model::isi::sns::RegisterSnsName::new(RegisterNameRequestV1 {
                selector: selector.clone(),
                owner: owner.clone(),
                controllers: vec![NameControllerV1::account(&owner_address)],
                term_years: 1,
                pricing_class_hint: None,
                payment: sns_payment(&owner, &payment_asset_definition_id),
                governance: None,
                metadata: Metadata::default(),
            })
            .execute(&owner, &mut stx)
            .expect_err("generic registration must not mutate account-alias leases");
        let renew_err = iroha_data_model::isi::sns::RenewSnsName::new(
            ACCOUNT_ALIAS_SUFFIX_ID,
            literal,
            RenewNameRequestV1 {
                term_years: 1,
                payment: sns_payment(&owner, &payment_asset_definition_id),
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("generic renewal must not mutate account-alias leases");
        let transfer_err = iroha_data_model::isi::sns::TransferSnsName::new(
            ACCOUNT_ALIAS_SUFFIX_ID,
            literal,
            iroha_data_model::sns::TransferNameRequestV1 {
                new_owner: other.clone(),
                governance: iroha_data_model::sns::GovernanceHookV1 {
                    proposal_id: "must-not-split-alias-state".to_owned(),
                    council_vote_hash: iroha_primitives::json::Json::from("council"),
                    dao_vote_hash: iroha_primitives::json::Json::from("dao"),
                    steward_ack: iroha_primitives::json::Json::from("steward"),
                    guardian_clearance: None,
                },
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("account-alias lease ownership cannot move independently");
        let other_address = AccountAddress::from_account_id(&other).expect("other address");
        let update_err = iroha_data_model::isi::sns::UpdateSnsNameControllers::new(
            ACCOUNT_ALIAS_SUFFIX_ID,
            literal,
            UpdateControllersRequestV1 {
                controllers: vec![NameControllerV1::account(&other_address)],
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("generic controller update must not mutate account-alias leases");
        let freeze_err = iroha_data_model::isi::sns::FreezeSnsName::new(
            ACCOUNT_ALIAS_SUFFIX_ID,
            literal,
            iroha_data_model::sns::FreezeNameRequestV1 {
                reason: "fabricated hold".to_owned(),
                until_ms: u64::MAX,
                guardian_ticket: iroha_primitives::json::Json::from("fabricated"),
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("generic freeze must not mutate account-alias leases");
        let unfreeze_err = iroha_data_model::isi::sns::UnfreezeSnsName::new(
            ACCOUNT_ALIAS_SUFFIX_ID,
            literal,
            iroha_data_model::sns::GovernanceHookV1 {
                proposal_id: "fabricated-unfreeze".to_owned(),
                council_vote_hash: iroha_primitives::json::Json::from("council"),
                dao_vote_hash: iroha_primitives::json::Json::from("dao"),
                steward_ack: iroha_primitives::json::Json::from("steward"),
                guardian_clearance: None,
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("generic unfreeze must not mutate account-alias leases");

        for err in [
            register_err,
            renew_err,
            transfer_err,
            update_err,
            freeze_err,
            unfreeze_err,
        ] {
            assert!(
                err.to_string().contains("dedicated account-alias"),
                "unexpected error: {err}"
            );
        }

        let unchanged = crate::sns::get_name_record(
            stx.world(),
            &stx.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            literal,
            0,
        )
        .expect("unchanged account alias record");
        assert_eq!(unchanged.owner, owner);
        assert_eq!(
            unchanged.controllers,
            vec![NameControllerV1::account(&owner_address)]
        );
        assert_eq!(stx.world.account_aliases.get(&alias), Some(&owner));
    }

    #[test]
    fn fabricated_governance_cannot_transfer_or_unfreeze_names() {
        let owner = owner();
        let other = another_owner();
        let owner_account = Account::new(owner.clone()).build(&owner);
        let mut world = World::with([], [owner_account], []);
        let selector = crate::sns::selector_for_namespace_literal(
            SnsNamespace::Domain,
            "governed.universal",
            &DataSpaceCatalog::default(),
        )
        .expect("selector");
        let owner_address = AccountAddress::from_account_id(&owner).expect("owner address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&owner_address)],
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
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let governance = iroha_data_model::sns::GovernanceHookV1 {
            proposal_id: "fabricated".to_owned(),
            council_vote_hash: iroha_primitives::json::Json::from("council"),
            dao_vote_hash: iroha_primitives::json::Json::from("dao"),
            steward_ack: iroha_primitives::json::Json::from("steward"),
            guardian_clearance: Some(iroha_primitives::json::Json::from("guardian")),
        };
        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();

        let transfer_err = iroha_data_model::isi::sns::TransferSnsName::new(
            iroha_data_model::sns::DOMAIN_NAME_SUFFIX_ID,
            "governed.universal",
            iroha_data_model::sns::TransferNameRequestV1 {
                new_owner: other,
                governance: governance.clone(),
            },
        )
        .execute(&owner, &mut stx)
        .expect_err("unverified transfer evidence must fail closed");
        let unfreeze_err = iroha_data_model::isi::sns::UnfreezeSnsName::new(
            iroha_data_model::sns::DOMAIN_NAME_SUFFIX_ID,
            "governed.universal",
            governance,
        )
        .execute(&owner, &mut stx)
        .expect_err("unverified unfreeze evidence must fail closed");
        for err in [transfer_err, unfreeze_err] {
            assert!(
                err.to_string().contains("governance evidence verification"),
                "unexpected error: {err}"
            );
        }
        assert_eq!(
            crate::sns::record_by_selector(stx.world(), &selector),
            Some(record),
            "governance rejection must not change the record"
        );
    }

    fn another_owner() -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn next_header(state: &State) -> BlockHeader {
        let height = u64::try_from(state.view().height())
            .unwrap_or(0)
            .saturating_add(1);
        BlockHeader::new(
            NonZeroU64::new(height).expect("height > 0"),
            None,
            None,
            None,
            0,
            0,
        )
    }

    fn seed_test_call_hash(state_transaction: &mut StateTransaction<'_, '_>, byte: u8) {
        state_transaction.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
    }

    fn sns_payment_payer_state() -> (State, AccountId, AccountId, AssetDefinitionId) {
        let payer = owner();
        let collector = another_owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&collector);
        let payer_account = Account::new(payer.clone()).build(&collector);
        let collector_account = Account::new(collector.clone()).build(&collector);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&collector);
        let mut world = World::with(
            vec![genesis_domain],
            vec![payer_account, collector_account],
            vec![payment_definition],
        );
        seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        (state, payer, collector, payment_asset_definition_id)
    }

    fn sns_payment(
        payer: &AccountId,
        payment_asset_definition_id: &AssetDefinitionId,
    ) -> PaymentProofV1 {
        PaymentProofV1 {
            asset_id: payment_asset_definition_id.to_string(),
            gross_amount: Quantity::zero(),
            net_amount: Quantity::zero(),
            settlement_tx: iroha_primitives::json::Json::from("self-asserted"),
            payer: payer.clone(),
            signature: iroha_primitives::json::Json::from("self-asserted"),
        }
    }

    fn asset_balance(
        state: &State,
        payment_asset_definition_id: &AssetDefinitionId,
        account: &AccountId,
    ) -> Quantity {
        let view = state.view();
        view.world()
            .asset(&AssetId::of(
                payment_asset_definition_id.clone(),
                account.clone(),
            ))
            .map(|asset| asset.value().clone().into_inner())
            .unwrap_or_else(|_| Quantity::zero())
    }

    #[test]
    fn generic_registration_guards_do_not_charge_or_persist() {
        let (state, payer, collector, payment_asset_definition_id) = sns_payment_payer_state();
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                2_u64,
                AssetId::of(payment_asset_definition_id.clone(), payer.clone()),
            )
            .execute(&collector, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let alias =
            AccountAlias::domainless("guarded".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let (alias_selector, domain_selector) = {
            let view = state.view();
            (
                crate::sns::selector_for_account_alias(&alias, &view.nexus.dataspace_catalog)
                    .expect("alias selector"),
                crate::sns::selector_for_namespace_literal(
                    SnsNamespace::Domain,
                    "empty.universal",
                    &view.nexus.dataspace_catalog,
                )
                .expect("domain selector"),
            )
        };
        let payer_asset = AssetId::of(payment_asset_definition_id.clone(), payer.clone());
        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xD1);
        let balance_before = stx
            .world
            .asset(&payer_asset)
            .expect("payer asset")
            .value()
            .clone();

        let alias_err = iroha_data_model::isi::sns::RegisterSnsName::new(RegisterNameRequestV1 {
            selector: alias_selector.clone(),
            owner: payer.clone(),
            controllers: vec![account_controller_for(&payer).expect("controller")],
            term_years: 1,
            pricing_class_hint: None,
            payment: sns_payment(&payer, &payment_asset_definition_id),
            governance: None,
            metadata: Metadata::default(),
        })
        .execute(&payer, &mut stx)
        .expect_err("generic account-alias registration is unavailable");
        assert!(alias_err.to_string().contains("dedicated account-alias"));

        let controller_err =
            iroha_data_model::isi::sns::RegisterSnsName::new(RegisterNameRequestV1 {
                selector: domain_selector.clone(),
                owner: payer.clone(),
                controllers: Vec::new(),
                term_years: 1,
                pricing_class_hint: None,
                payment: sns_payment(&payer, &payment_asset_definition_id),
                governance: None,
                metadata: Metadata::default(),
            })
            .execute(&payer, &mut stx)
            .expect_err("empty controllers must fail before charging");
        assert!(
            controller_err
                .to_string()
                .contains("at least one controller")
        );

        assert_eq!(
            stx.world.asset(&payer_asset).expect("payer asset").value(),
            &balance_before,
            "rejected registration must not debit the payer"
        );
        assert!(
            crate::sns::record_by_selector(stx.world(), &alias_selector).is_none(),
            "generic account-alias rejection must not create a record"
        );
        assert!(
            crate::sns::record_by_selector(stx.world(), &domain_selector).is_none(),
            "controller validation failure must not create a record"
        );
    }

    fn register_paid_domain_name(
        state: &State,
        payer: &AccountId,
        payment_asset_definition_id: &AssetDefinitionId,
        label: &str,
    ) -> u64 {
        let literal = format!("{label}.universal");
        let selector = {
            let view = state.view();
            crate::sns::selector_for_namespace_literal(
                SnsNamespace::Domain,
                &literal,
                &view.nexus.dataspace_catalog,
            )
            .expect("selector")
        };
        let request = RegisterNameRequestV1 {
            selector,
            owner: payer.clone(),
            controllers: vec![account_controller_for(payer).expect("controller")],
            term_years: 1,
            pricing_class_hint: None,
            payment: sns_payment(payer, payment_asset_definition_id),
            governance: None,
            metadata: Metadata::default(),
        };
        {
            let mut block = state.block(next_header(state));
            let mut stx = block.transaction();
            seed_test_call_hash(&mut stx, 0xC1);
            iroha_data_model::isi::sns::RegisterSnsName::new(request)
                .execute(payer, &mut stx)
                .expect("register SNS domain name");
            stx.apply();
            block.commit().expect("register block commits");
        }

        let view = state.view();
        get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::Domain,
            &literal,
            0,
        )
        .expect("registered alias")
        .expires_at_ms
    }

    #[test]
    fn find_dataspace_name_owner_by_id_returns_active_owner() {
        let mut state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "trade".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog");

        let selector = crate::sns::selector_for_dataspace_alias("trade").expect("selector");
        let owner = owner();
        let address = AccountAddress::from_account_id(&owner).expect("address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            4_000_000_000_000,
            4_100_000_000_000,
            4_200_000_000_000,
            Metadata::default(),
        );
        state.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );

        let view = state.view();
        let key = crate::sns::record_storage_key(&selector);
        assert!(
            view.world().smart_contract_state().get(&key).is_some(),
            "seeded SNS record must be present in raw state storage"
        );
        assert_eq!(
            crate::sns::active_dataspace_owner_by_id(
                view.world(),
                &view.nexus.dataspace_catalog,
                DataSpaceId::new(9),
                0,
            ),
            Some(owner.clone()),
            "SNS helper should resolve the active owner from the state view"
        );
        let resolved = FindDataspaceNameOwnerById::new(DataSpaceId::new(9))
            .execute(&view)
            .expect("query succeeds");
        assert_eq!(resolved, owner);
    }

    #[test]
    fn acquire_and_renew_account_alias_lease_round_trip() {
        let authority = owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let mut world = World::with([genesis_domain], [authority_account], [payment_definition]);
        seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                1_000_u64,
                AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
            )
            .execute(&authority, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let alias =
            AccountAlias::domainless("merchant".parse().expect("label"), DataSpaceId::UNIVERSAL);
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            seed_test_call_hash(&mut stx, 0xC2);
            AcquireAccountAliasLease::new(
                alias.clone(),
                authority.clone(),
                authority.clone(),
                1,
                None,
            )
            .execute(&authority, &mut stx)
            .expect("acquire lease");
            stx.apply();
            block.commit().expect("acquire block commits");
        }

        let view = state.view();
        let acquired = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "merchant@universal",
            0,
        )
        .expect("acquired alias lease");
        let initial_expiry = acquired.expires_at_ms;
        assert_eq!(acquired.owner, authority);
        drop(view);

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            seed_test_call_hash(&mut stx, 0xC3);
            RenewAccountAliasLease::new(alias, authority.clone(), 1)
                .execute(&authority, &mut stx)
                .expect("renew lease");
            stx.apply();
            block.commit().expect("renew block commits");
        }

        let view = state.view();
        let renewed = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "merchant@universal",
            0,
        )
        .expect("renewed alias lease");
        assert!(
            renewed.expires_at_ms > initial_expiry,
            "renewal must extend the alias expiry"
        );
    }

    #[test]
    fn register_account_acquire_alias_lease_and_bind_alias_in_one_transaction() {
        let authority = owner();
        let retail_account = another_owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let mut world = World::with([genesis_domain], [authority_account], [payment_definition]);
        seed_default_namespace_policies(&mut world);
        let mut permissions = world
            .account_permissions
            .view()
            .get(&authority)
            .cloned()
            .unwrap_or_default();
        permissions.insert(Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        }));
        world
            .account_permissions
            .insert(authority.clone(), permissions);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                1_000_u64,
                AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
            )
            .execute(&authority, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let alias = AccountAlias::domainless(
            "clearorbit3941".parse().expect("label"),
            DataSpaceId::UNIVERSAL,
        );
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Register::account(Account::new(retail_account.clone()))
                .execute(&authority, &mut stx)
                .expect("register retail account");
            seed_test_call_hash(&mut stx, 0xD1);
            AcquireAccountAliasLease::new(
                alias.clone(),
                retail_account.clone(),
                authority.clone(),
                1,
                None,
            )
            .execute(&authority, &mut stx)
            .expect("acquire alias lease for newly registered account");
            SetAccountAliasBinding::bind(retail_account.clone(), alias.clone(), None)
                .execute(&authority, &mut stx)
                .expect("bind alias for newly registered account");
            stx.apply();
            block.commit().expect("registration batch commits");
        }

        let view = state.view();
        assert!(
            view.world().account(&retail_account).is_ok(),
            "retail account should be visible after the batch"
        );
        let lease = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "clearorbit3941@universal",
            0,
        )
        .expect("alias lease should be active after the batch");
        assert_eq!(lease.owner, retail_account);
        assert_eq!(
            view.world().account_aliases().get(&alias),
            Some(&retail_account),
            "alias binding should be visible after the batch"
        );
    }

    #[test]
    fn register_account_acquire_fi_alias_lease_and_bind_alias_in_one_transaction() {
        let authority = owner();
        let retail_account = another_owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let hbl_domain = Domain::new(DomainId::try_new("hbl", "sbp").expect("hbl.sbp domain id"))
            .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let mut world = World::with(
            [genesis_domain, hbl_domain],
            [authority_account],
            [payment_definition],
        );
        seed_default_namespace_policies(&mut world);
        let mut permissions = world
            .account_permissions
            .view()
            .get(&authority)
            .cloned()
            .unwrap_or_default();
        let sbp = DataSpaceId::new(10);
        permissions.insert(Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(sbp),
        }));
        permissions.insert(Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(
                DomainId::try_new("hbl", "sbp").expect("hbl.sbp domain id"),
            ),
        }));
        world
            .account_permissions
            .insert(authority.clone(), permissions);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: sbp,
                alias: "sbp".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("sbp dataspace catalog");

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                1_000_u64,
                AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
            )
            .execute(&authority, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let alias = AccountAlias::new(
            "clear-orbit-3941".parse().expect("label"),
            Some(AccountAliasDomain::new("hbl".parse().expect("domain"))),
            sbp,
        );
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Register::account(Account::new(retail_account.clone()))
                .execute(&authority, &mut stx)
                .expect("register retail account");
            seed_test_call_hash(&mut stx, 0xD2);
            AcquireAccountAliasLease::new(
                alias.clone(),
                retail_account.clone(),
                authority.clone(),
                1,
                None,
            )
            .execute(&authority, &mut stx)
            .expect("acquire FI alias lease for newly registered account");
            SetAccountAliasBinding::bind(retail_account.clone(), alias.clone(), None)
                .execute(&authority, &mut stx)
                .expect("bind FI alias for newly registered account");
            stx.apply();
            block.commit().expect("FI registration batch commits");
        }

        let view = state.view();
        let lease = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "clear-orbit-3941@hbl.sbp",
            0,
        )
        .expect("FI alias lease should be active after the batch");
        assert_eq!(lease.owner, retail_account);
        assert_eq!(
            view.world().account_aliases().get(&alias),
            Some(&retail_account),
            "FI alias binding should be visible after the batch"
        );
    }

    #[test]
    fn register_sns_name_charges_authoritative_quote_before_persisting() {
        let (state, payer, collector, payment_asset_definition_id) = sns_payment_payer_state();
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                2_u64,
                AssetId::of(payment_asset_definition_id.clone(), payer.clone()),
            )
            .execute(&collector, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let initial_expiry =
            register_paid_domain_name(&state, &payer, &payment_asset_definition_id, "paid");

        assert!(initial_expiry > 0);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &payer),
            Quantity::try_from(Numeric::new(1_500_000_000_i128, 9))
                .expect("payer balance must be a non-negative quantity")
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Quantity::try_from(Numeric::new(500_000_000_i128, 9))
                .expect("collector balance must be a non-negative quantity")
        );
    }

    #[test]
    fn renew_sns_name_charges_authoritative_quote_before_extending() {
        let (state, payer, collector, payment_asset_definition_id) = sns_payment_payer_state();
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                2_u64,
                AssetId::of(payment_asset_definition_id.clone(), payer.clone()),
            )
            .execute(&collector, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }
        let initial_expiry =
            register_paid_domain_name(&state, &payer, &payment_asset_definition_id, "renewed");

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            seed_test_call_hash(&mut stx, 0xC4);
            iroha_data_model::isi::sns::RenewSnsName::new(
                iroha_data_model::sns::DOMAIN_NAME_SUFFIX_ID,
                "renewed.universal",
                RenewNameRequestV1 {
                    term_years: 1,
                    payment: sns_payment(&payer, &payment_asset_definition_id),
                },
            )
            .execute(&payer, &mut stx)
            .expect("renew SNS alias");
            stx.apply();
            block.commit().expect("renew block commits");
        }

        let view = state.view();
        let renewed = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::Domain,
            "renewed.universal",
            0,
        )
        .expect("renewed alias");
        assert!(renewed.expires_at_ms > initial_expiry);
        drop(view);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &payer),
            Quantity::try_from(Numeric::new(1_000_000_000_i128, 9))
                .expect("payer balance must be a non-negative quantity")
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Quantity::try_from(Numeric::new(1_000_000_000_i128, 9))
                .expect("collector balance must be a non-negative quantity")
        );
    }

    #[test]
    fn register_sns_name_without_balance_does_not_persist_record() {
        let (state, payer, _collector, payment_asset_definition_id) = sns_payment_payer_state();
        let selector = {
            let view = state.view();
            crate::sns::selector_for_namespace_literal(
                SnsNamespace::Domain,
                "free.universal",
                &view.nexus.dataspace_catalog,
            )
            .expect("selector")
        };
        let request = RegisterNameRequestV1 {
            selector,
            owner: payer.clone(),
            controllers: vec![account_controller_for(&payer).expect("controller")],
            term_years: 1,
            pricing_class_hint: None,
            payment: sns_payment(&payer, &payment_asset_definition_id),
            governance: None,
            metadata: Metadata::default(),
        };

        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xC5; Hash::LENGTH]));
        iroha_data_model::isi::sns::RegisterSnsName::new(request)
            .execute(&payer, &mut stx)
            .expect_err("unfunded payer must not register SNS name");
        drop(stx);
        drop(block);

        let view = state.view();
        assert!(
            get_name_record(
                view.world(),
                &view.nexus.dataspace_catalog,
                SnsNamespace::Domain,
                "free.universal",
                0,
            )
            .is_err(),
            "failed payment must not persist SNS record"
        );
    }

    #[test]
    fn acquire_account_alias_lease_rejects_stale_policy_without_mutating_it() {
        let authority = owner();
        let payment_asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("deployment payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority);
        let mut world = World::with([genesis_domain], [authority_account], [payment_definition]);
        seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().fees.fee_asset_id = payment_asset_definition_id.to_string();

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_quantity(
                1_000_u64,
                AssetId::of(payment_asset_definition_id.clone(), authority.clone()),
            )
            .execute(&authority, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let alias =
            AccountAlias::domainless("retail".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xC6);
        let err =
            AcquireAccountAliasLease::new(alias, authority.clone(), authority.clone(), 1, None)
                .execute(&authority, &mut stx)
                .expect_err("lease mutation must reject a stale SNS payment policy");
        assert!(
            err.to_string()
                .contains("does not match configured Nexus fee asset"),
            "unexpected error: {err}"
        );
        drop(stx);
        drop(block);

        let view = state.view();
        let policy = policy_by_id(view.world(), ACCOUNT_ALIAS_SUFFIX_ID).expect("policy");
        assert_eq!(
            policy.payment_asset_id, "61CtjvNd9T3THAR65GsMVHr82Bjc",
            "rejected mutation must not silently converge policy state"
        );
        assert!(
            get_name_record(
                view.world(),
                &view.nexus.dataspace_catalog,
                SnsNamespace::AccountAlias,
                "retail@universal",
                0,
            )
            .is_err(),
            "rejected mutation must not persist an alias lease"
        );
    }

    #[test]
    fn acquire_account_alias_lease_uses_external_settlement_on_non_authoritative_route() {
        let authority = owner();
        let collector = another_owner();
        let paynet = DataSpaceId::new(10);
        let payment_asset_definition_id: AssetDefinitionId = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
            .parse()
            .expect("deployment payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&collector);
        let authority_account = Account::new(authority.clone()).build(&collector);
        let collector_account = Account::new(collector.clone()).build(&collector);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&collector);
        let mut world = World::with(
            vec![genesis_domain],
            vec![authority_account, collector_account],
            vec![payment_definition],
        );
        seed_default_namespace_policies(&mut world);
        assert!(crate::sns::sync_default_namespace_policy_payment_asset(
            &mut world,
            &payment_asset_definition_id.to_string(),
        ));
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        {
            let mut nexus = state.nexus.write();
            nexus.fees.fee_asset_id = payment_asset_definition_id.to_string();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.external_settlement_enabled = true;
            nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                DataSpaceMetadata::default(),
                DataSpaceMetadata {
                    id: paynet,
                    alias: "paynet".to_owned(),
                    description: None,
                    fault_tolerance: 1,
                },
            ])
            .expect("dataspace catalog");
        }

        let alias = AccountAlias::domainless("retail".parse().expect("label"), paynet);
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            stx.current_dataspace_id = Some(paynet);
            stx.world.current_dataspace_id = Some(paynet);
            stx.tx_call_hash = Some(Hash::prehashed([0xC7; Hash::LENGTH]));
            AcquireAccountAliasLease::new(alias, authority.clone(), authority.clone(), 1, None)
                .execute(&authority, &mut stx)
                .expect("external settlement skips local global asset transfer");
            stx.apply();
            block.commit().expect("acquire block commits");
        }

        let view = state.view();
        let acquired = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "retail@paynet",
            0,
        )
        .expect("acquired alias lease");
        assert_eq!(acquired.owner, authority);
        drop(view);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &authority),
            Quantity::zero()
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Quantity::zero()
        );
    }

    #[test]
    fn acquire_account_alias_lease_rejects_mismatched_payer() {
        let authority = owner();
        let payer = another_owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let payer_account = Account::new(payer.clone()).build(&authority);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id)
            .with_name("xor".to_owned())
            .build(&authority);
        let mut world = World::with(
            vec![genesis_domain],
            vec![authority_account, payer_account],
            vec![payment_definition],
        );
        seed_default_namespace_policies(&mut world);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xC8; Hash::LENGTH]));
        let err = AcquireAccountAliasLease::new(
            AccountAlias::domainless("merchant".parse().expect("label"), DataSpaceId::UNIVERSAL),
            authority.clone(),
            payer,
            1,
            None,
        )
        .execute(&authority, &mut stx)
        .expect_err("mismatched payer must fail");

        assert!(
            matches!(
                err,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                    _
                ))
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn renew_account_alias_lease_rejects_non_owner_without_permission() {
        let owner = owner();
        let authority = another_owner();
        let payment_asset_definition_id: AssetDefinitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
            .parse()
            .expect("payment asset definition id");
        let genesis_domain =
            Domain::new(DomainId::try_new("genesis", "universal").expect("genesis domain id"))
                .build(&owner);
        let owner_account = Account::new(owner.clone()).build(&owner);
        let authority_account = Account::new(authority.clone()).build(&owner);
        let payment_definition = AssetDefinition::numeric(payment_asset_definition_id)
            .with_name("xor".to_owned())
            .build(&owner);
        let mut world = World::with(
            vec![genesis_domain],
            vec![owner_account, authority_account],
            vec![payment_definition],
        );
        seed_default_namespace_policies(&mut world);
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        let alias =
            AccountAlias::domainless("merchant".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = {
            let view = state.view();
            crate::sns::selector_for_account_alias(&alias, &view.nexus.dataspace_catalog)
                .expect("selector")
        };
        let address = AccountAddress::from_account_id(&owner).expect("address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            1,
            5_000,
            5_000 + (30 * 86_400_000),
            5_000 + (90 * 86_400_000),
            Metadata::default(),
        );
        state.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );

        let mut block = state.block(next_header(&state));
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xC9; Hash::LENGTH]));
        let err = RenewAccountAliasLease::new(alias, authority.clone(), 1)
            .execute(&authority, &mut stx)
            .expect_err("non-owner without permission must fail");

        assert!(
            matches!(err, InstructionExecutionError::InvariantViolation(_)),
            "unexpected error: {err:?}"
        );
    }
}
