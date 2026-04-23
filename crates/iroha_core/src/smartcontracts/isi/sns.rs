//! SNS-backed ownership query and lease instruction handlers.

use iroha_data_model::{
    account::AccountAddress,
    isi::{
        account_alias_lease::{AcquireAccountAliasLease, RenewAccountAliasLease},
        error::{InstructionExecutionError, InvalidParameterError},
    },
    metadata::Metadata,
    query::{error::QueryExecutionFail as QueryError, sns::prelude::*},
    sns::{NameControllerV1, RegisterNameRequestV1, RenewNameRequestV1},
};
use iroha_telemetry::metrics;

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
        Transfer::asset_numeric(
            AssetId::of(quote.payment_asset_definition_id.clone(), payer.clone()),
            quote.charge_amount,
            quote.collector_account.clone(),
        )
        .execute(authority, state_transaction)?;
        let payment = crate::sns::payment_proof_for_quote(&quote, payer);
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

        let quote = crate::sns::quote_account_alias_renewal(
            state_transaction.world(),
            &state_transaction.nexus.dataspace_catalog,
            &alias,
            term_years,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(alias_lease_instruction_error)?;
        Transfer::asset_numeric(
            AssetId::of(quote.payment_asset_definition_id.clone(), payer.clone()),
            quote.charge_amount,
            quote.collector_account.clone(),
        )
        .execute(authority, state_transaction)?;
        let payment = crate::sns::payment_proof_for_quote(&quote, payer);
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
        sns::{NameControllerV1, NameRecordV1},
    };
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        sns::{SnsNamespace, get_name_record, seed_default_namespace_policies},
        state::{State, World},
    };

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
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
}
