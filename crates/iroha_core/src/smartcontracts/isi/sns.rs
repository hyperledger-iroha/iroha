//! SNS-backed ownership query and lease instruction handlers.

use iroha_data_model::{
    account::AccountAddress,
    asset::AssetBalancePolicy,
    isi::{
        account_alias_lease::{AcquireAccountAliasLease, RenewAccountAliasLease},
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    nexus::DataSpaceId,
    query::{error::QueryExecutionFail as QueryError, sns::prelude::*},
    sns::{NameControllerV1, RegisterNameRequestV1, RenewNameRequestV1, SuffixId},
};
use iroha_primitives::numeric::Numeric;
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

fn xor_nanos_to_numeric(nanos: u64) -> Numeric {
    crate::sns::quote_charge_amount_to_numeric(nanos)
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
    if quote.charge_amount == 0
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

        if payer != *authority {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "AcquireAccountAliasLease payer must match the transaction authority".into(),
                ),
            ));
        }

        if owner != *authority
            && !authority_can_manage_account_alias(&state_transaction.world, authority, &alias)
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "transaction authority must own the alias or hold CanManageAccountAlias".into(),
            ));
        }

        crate::sns::sync_default_namespace_policy_payment_asset_in_transaction(state_transaction);
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
        let payment = charge_sns_quote(&quote, payer, authority, state_transaction)?;
        let controllers = vec![account_controller_for(&owner)?];
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

        crate::sns::sync_default_namespace_policy_payment_asset_in_transaction(state_transaction);
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

    Transfer::asset_numeric(
        AssetId::of(quote.payment_asset_definition_id.clone(), payer.clone()),
        xor_nanos_to_numeric(quote.charge_amount),
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
        ensure_payment_payer_is_authority(&request.payment.payer, authority, "RegisterSnsName")?;
        crate::sns::sync_default_namespace_policy_payment_asset_in_transaction(state_transaction);
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
        ensure_payment_payer_is_authority(&request.payment.payer, authority, "RenewSnsName")?;
        let namespace = namespace_from_suffix_id(self.suffix_id)?;
        crate::sns::sync_default_namespace_policy_payment_asset_in_transaction(state_transaction);
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
    }
}

impl Execute for iroha_data_model::isi::sns::TransferSnsName {
    #[metrics(+"transfer_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let namespace = namespace_from_suffix_id(self.suffix_id)?;
        let request = decode_sns_payload(&self.request, "TransferSnsName")?;
        ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
        crate::sns::transfer_name(state_transaction, namespace, &self.literal, request)
            .map(|_| ())
            .map_err(sns_mutation_instruction_error)
    }
}

impl Execute for iroha_data_model::isi::sns::UpdateSnsNameControllers {
    #[metrics(+"update_sns_name_controllers")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let namespace = namespace_from_suffix_id(self.suffix_id)?;
        let request = decode_sns_payload(&self.request, "UpdateSnsNameControllers")?;
        ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
        crate::sns::update_name_controllers(state_transaction, namespace, &self.literal, request)
            .map(|_| ())
            .map_err(sns_mutation_instruction_error)
    }
}

impl Execute for iroha_data_model::isi::sns::FreezeSnsName {
    #[metrics(+"freeze_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let namespace = namespace_from_suffix_id(self.suffix_id)?;
        let request = decode_sns_payload(&self.request, "FreezeSnsName")?;
        ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
        crate::sns::freeze_name(state_transaction, namespace, &self.literal, request)
            .map(|_| ())
            .map_err(sns_mutation_instruction_error)
    }
}

impl Execute for iroha_data_model::isi::sns::UnfreezeSnsName {
    #[metrics(+"unfreeze_sns_name")]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let namespace = namespace_from_suffix_id(self.suffix_id)?;
        let governance = decode_sns_payload(&self.governance, "UnfreezeSnsName")?;
        ensure_name_owner_authority(state_transaction, authority, namespace, &self.literal)?;
        crate::sns::unfreeze_name(state_transaction, namespace, &self.literal, governance)
            .map(|_| ())
            .map_err(sns_mutation_instruction_error)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_data_model::{
        account::{Account, AccountAddress, rekey::AccountAlias},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::Mint,
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        query::sns::prelude::FindDataspaceNameOwnerById,
        sns::{
            NameControllerV1, NameRecordV1, PaymentProofV1, RegisterNameRequestV1,
            RenewNameRequestV1,
        },
    };
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        sns::{
            ACCOUNT_ALIAS_SUFFIX_ID, SnsNamespace, get_name_record, policy_by_id,
            seed_default_namespace_policies,
        },
        state::{State, World, WorldReadOnly},
    };

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
    }

    fn another_owner() -> AccountId {
        let public_key = "ed0120C70416DC2D60D9AB2F0C6CED829837F1006DDED2DE794E9D5091A60663FA8C11"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
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
            gross_amount: 0,
            net_amount: 0,
            settlement_tx: iroha_primitives::json::Json::from("self-asserted"),
            payer: payer.clone(),
            signature: iroha_primitives::json::Json::from("self-asserted"),
        }
    }

    fn asset_balance(
        state: &State,
        payment_asset_definition_id: &AssetDefinitionId,
        account: &AccountId,
    ) -> Numeric {
        let view = state.view();
        view.world()
            .asset(&AssetId::of(
                payment_asset_definition_id.clone(),
                account.clone(),
            ))
            .map(|asset| asset.value().clone().into_inner())
            .unwrap_or_else(|_| Numeric::zero())
    }

    fn register_paid_alias(
        state: &State,
        payer: &AccountId,
        payment_asset_definition_id: &AssetDefinitionId,
        label: &str,
    ) -> u64 {
        let alias = AccountAlias::domainless(label.parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = {
            let view = state.view();
            crate::sns::selector_for_account_alias(&alias, &view.nexus.dataspace_catalog)
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
            iroha_data_model::isi::sns::RegisterSnsName::new(request)
                .execute(payer, &mut stx)
                .expect("register SNS alias");
            stx.apply();
            block.commit().expect("register block commits");
        }

        let literal = alias
            .to_literal(&state.nexus_snapshot().dataspace_catalog)
            .expect("literal");
        let view = state.view();
        get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
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
            Mint::asset_numeric(
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
    fn register_sns_name_charges_authoritative_quote_before_persisting() {
        let (state, payer, collector, payment_asset_definition_id) = sns_payment_payer_state();
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_numeric(
                2_u64,
                AssetId::of(payment_asset_definition_id.clone(), payer.clone()),
            )
            .execute(&collector, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }

        let initial_expiry =
            register_paid_alias(&state, &payer, &payment_asset_definition_id, "paid");

        assert!(initial_expiry > 0);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &payer),
            Numeric::new(1_500_000_000_i128, 9)
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Numeric::new(500_000_000_i128, 9)
        );
    }

    #[test]
    fn renew_sns_name_charges_authoritative_quote_before_extending() {
        let (state, payer, collector, payment_asset_definition_id) = sns_payment_payer_state();
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            Mint::asset_numeric(
                2_u64,
                AssetId::of(payment_asset_definition_id.clone(), payer.clone()),
            )
            .execute(&collector, &mut stx)
            .expect("mint payment balance");
            stx.apply();
            block.commit().expect("mint block commits");
        }
        let initial_expiry =
            register_paid_alias(&state, &payer, &payment_asset_definition_id, "renewed");

        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            iroha_data_model::isi::sns::RenewSnsName::new(
                ACCOUNT_ALIAS_SUFFIX_ID,
                "renewed@universal",
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
            SnsNamespace::AccountAlias,
            "renewed@universal",
            0,
        )
        .expect("renewed alias");
        assert!(renewed.expires_at_ms > initial_expiry);
        drop(view);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &payer),
            Numeric::new(1_000_000_000_i128, 9)
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Numeric::new(1_000_000_000_i128, 9)
        );
    }

    #[test]
    fn register_sns_name_without_balance_does_not_persist_record() {
        let (state, payer, _collector, payment_asset_definition_id) = sns_payment_payer_state();
        let alias =
            AccountAlias::domainless("free".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = {
            let view = state.view();
            crate::sns::selector_for_account_alias(&alias, &view.nexus.dataspace_catalog)
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
                SnsNamespace::AccountAlias,
                "free@universal",
                0,
            )
            .is_err(),
            "failed payment must not persist SNS record"
        );
    }

    #[test]
    fn acquire_account_alias_lease_syncs_stale_default_payment_asset() {
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
            Mint::asset_numeric(
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
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            AcquireAccountAliasLease::new(alias, authority.clone(), authority.clone(), 1, None)
                .execute(&authority, &mut stx)
                .expect("acquire lease with deployment payment asset");
            stx.apply();
            block.commit().expect("acquire block commits");
        }

        let view = state.view();
        let policy = policy_by_id(view.world(), ACCOUNT_ALIAS_SUFFIX_ID).expect("policy");
        assert_eq!(
            policy.payment_asset_id,
            payment_asset_definition_id.to_string()
        );
        let acquired = get_name_record(
            view.world(),
            &view.nexus.dataspace_catalog,
            SnsNamespace::AccountAlias,
            "retail@universal",
            0,
        )
        .expect("acquired alias lease");
        assert_eq!(acquired.owner, authority);
    }

    #[test]
    fn acquire_account_alias_lease_uses_external_settlement_on_non_authoritative_route() {
        let authority = owner();
        let collector = another_owner();
        let bpng = DataSpaceId::new(10);
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
                    id: bpng,
                    alias: "bpng".to_owned(),
                    description: None,
                    fault_tolerance: 1,
                },
            ])
            .expect("dataspace catalog");
        }

        let alias = AccountAlias::domainless("retail".parse().expect("label"), bpng);
        {
            let mut block = state.block(next_header(&state));
            let mut stx = block.transaction();
            stx.current_dataspace_id = Some(bpng);
            stx.world.current_dataspace_id = Some(bpng);
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
            "retail@bpng",
            0,
        )
        .expect("acquired alias lease");
        assert_eq!(acquired.owner, authority);
        drop(view);
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &authority),
            Numeric::zero()
        );
        assert_eq!(
            asset_balance(&state, &payment_asset_definition_id, &collector),
            Numeric::zero()
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
        let err = RenewAccountAliasLease::new(alias, authority.clone(), 1)
            .execute(&authority, &mut stx)
            .expect_err("non-owner without permission must fail");

        assert!(
            matches!(err, InstructionExecutionError::InvariantViolation(_)),
            "unexpected error: {err:?}"
        );
    }
}
