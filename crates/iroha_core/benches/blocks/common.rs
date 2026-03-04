#![allow(clippy::disallowed_types, clippy::items_after_test_module)]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{
    num::{NonZeroU16, NonZeroU64},
    sync::Arc,
};

use iroha_core::{
    block::{BlockBuilder, CommittedBlock},
    prelude::*,
    query::store::LiveQueryStore,
    smartcontracts::{Execute, Registrable as _},
    state::{State, StateBlock, World},
    sumeragi::network_topology::Topology,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::Account,
    asset::{AssetDefinition, AssetDefinitionId},
    domain::Domain,
    isi::{InstructionBox, Log},
    parameter::TransactionParameters,
    prelude::*,
    transaction::IvmBytecode,
};
use iroha_executor_data_model::permission::{
    account::CanUnregisterAccount,
    asset_definition::CanUnregisterAssetDefinition,
    domain::{CanRegisterDomain, CanUnregisterDomain},
};

/// Create block
pub fn create_block<'a>(
    state: &'a State,
    instructions: impl IntoIterator<Item = InstructionBox>,
    account_id: AccountId,
    account_private_key: &PrivateKey,
    topology: &Topology,
    peer_private_key: &PrivateKey,
) -> (CommittedBlock, StateBlock<'a>) {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let transaction = TransactionBuilder::new(chain_id.clone(), account_id)
        .with_instructions(instructions)
        .sign(account_private_key);
    let (max_clock_drift, tx_limits) = {
        let state_view = state.view();
        let params = state_view.world.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };

    let crypto_cfg = state.crypto();
    let unverified_block = BlockBuilder::new(vec![
        AcceptedTransaction::accept(
            transaction,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .unwrap(),
    ])
    .chain(0, state.view().latest_block().as_deref())
    .sign(peer_private_key)
    .unpack(|_| {});

    let mut state_block = state.block(unverified_block.header());
    let block = unverified_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {})
        .commit(topology)
        .unpack(|_| {})
        .unwrap();

    // Verify that transactions are valid (non-fatal in release benches)
    debug_assert_eq!(block.as_ref().errors().count(), 0);

    (block, state_block)
}

pub fn populate_state(
    domains: &[DomainId],
    accounts: &[AccountId],
    asset_definitions: &[AssetDefinitionId],
    owner_id: &AccountId,
) -> Vec<InstructionBox> {
    let mut instructions: Vec<InstructionBox> =
        vec![Grant::account_permission(CanRegisterDomain, owner_id.clone()).into()];

    for domain_id in domains {
        let domain = Domain::new(domain_id.clone());
        instructions.push(Register::domain(domain).into());
        let can_unregister_domain = Grant::account_permission(
            CanUnregisterDomain {
                domain: domain_id.clone(),
            },
            owner_id.clone(),
        );
        instructions.push(can_unregister_domain.into());
    }

    for account_id in accounts {
        let account = Account::new(account_id.clone());
        instructions.push(Register::account(account).into());
        let can_unregister_account = Grant::account_permission(
            CanUnregisterAccount {
                account: account_id.clone(),
            },
            owner_id.clone(),
        );
        instructions.push(can_unregister_account.into());
    }

    for asset_definition_id in asset_definitions {
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone());
        instructions.push(Register::asset_definition(asset_definition).into());
        let can_unregister_asset_definition = Grant::account_permission(
            CanUnregisterAssetDefinition {
                asset_definition: asset_definition_id.clone(),
            },
            owner_id.clone(),
        );
        instructions.push(can_unregister_asset_definition.into());
    }

    instructions
}

pub fn delete_every_nth(
    domains: &[DomainId],
    accounts: &[AccountId],
    asset_definitions: &[AssetDefinitionId],
    nth: usize,
) -> Vec<InstructionBox> {
    let mut instructions: Vec<InstructionBox> = Vec::new();
    for (i, domain_id) in domains.iter().enumerate() {
        if i % nth == 0 {
            instructions.push(Unregister::domain(domain_id.clone()).into());
        } else {
            for (j, account_id) in accounts
                .iter()
                .filter(|account_id| account_id.domain() == domain_id)
                .enumerate()
            {
                if j % nth == 0 {
                    instructions.push(Unregister::account(account_id.clone()).into());
                }
            }
            for (k, asset_definition_id) in asset_definitions
                .iter()
                .filter(|asset_definition_id| asset_definition_id.domain() == domain_id)
                .enumerate()
            {
                if k % nth == 0 {
                    instructions
                        .push(Unregister::asset_definition(asset_definition_id.clone()).into());
                }
            }
        }
    }
    instructions
}

pub fn restore_every_nth(
    domains: &[DomainId],
    accounts: &[AccountId],
    asset_definitions: &[AssetDefinitionId],
    nth: usize,
) -> Vec<InstructionBox> {
    let mut instructions: Vec<InstructionBox> = Vec::new();
    for (i, domain_id) in domains.iter().enumerate() {
        if i % nth == 0 {
            let domain = Domain::new(domain_id.clone());
            instructions.push(Register::domain(domain).into());
        }
        for (j, account_id) in accounts
            .iter()
            .filter(|account_id| account_id.domain() == domain_id)
            .enumerate()
        {
            if j % nth == 0 || i % nth == 0 {
                let account = Account::new(account_id.clone());
                instructions.push(Register::account(account).into());
            }
        }
        for (k, asset_definition_id) in asset_definitions
            .iter()
            .filter(|asset_definition_id| asset_definition_id.domain() == domain_id)
            .enumerate()
        {
            if k % nth == 0 || i % nth == 0 {
                let asset_definition = AssetDefinition::numeric(asset_definition_id.clone());
                instructions.push(Register::asset_definition(asset_definition).into());
            }
        }
    }
    instructions
}

pub fn build_state(
    rt: &tokio::runtime::Handle,
    account_id: &AccountId,
    account_private_key: &PrivateKey,
) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query_handle = {
        let _guard = rt.enter();
        LiveQueryStore::start_test()
    };
    let domain = Domain::new(account_id.domain().clone()).build(account_id);
    let state = State::new(
        World::with(
            [domain],
            [Account::new(account_id.clone()).build(account_id)],
            [],
        ),
        Arc::clone(&kura),
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    );

    {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let transaction = TransactionBuilder::new(chain_id.clone(), account_id.clone())
            .with_instructions([Log::new(Level::INFO, "init".to_string())])
            .sign(account_private_key);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.view();
            let params = state_view.world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto_cfg = state.crypto();
        let unverified_block = BlockBuilder::new(vec![
            AcceptedTransaction::accept(
                transaction,
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            )
            .unwrap(),
        ])
        .chain(0, state.view().latest_block().as_deref())
        .sign(account_private_key)
        .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header());

        state_block.world.parameters.transaction = TransactionParameters::with_max_signatures(
            NonZeroU64::MAX,
            NonZeroU64::MAX,
            NonZeroU64::MAX,
            NonZeroU64::MAX,
            NonZeroU64::MAX,
            NonZeroU16::new(u16::MAX).expect("u16::MAX is non-zero"),
        );
        state_block.world.parameters.executor.fuel = NonZeroU64::MAX;
        state_block.world.parameters.executor.memory = NonZeroU64::MAX;

        let mut state_transaction = state_block.transaction();
        let path_to_executor =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
        if let Ok(bytecode) = std::fs::read(&path_to_executor)
            && !bytecode.is_empty()
        {
            let executor = Executor::new(IvmBytecode::from_compiled(bytecode));
            // Ignore upgrade failure and keep the default executor when bytecode is invalid
            let _ = Upgrade::new(executor).execute(account_id, &mut state_transaction);
        }

        state_transaction.apply();
        let committed_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _ = state_block.apply_without_execution(&committed_block, Vec::new());
        state_block.commit().unwrap();

        let block_arc = Arc::new(committed_block.into());
        kura.store_block(block_arc)
            .expect("store block in bench setup");
    }

    state
}

fn construct_domain_id(i: usize) -> DomainId {
    format!("non_inlinable_domain_name_{i}").parse().unwrap()
}

fn generate_account_id(domain_id: DomainId, seed: u128) -> AccountId {
    let keypair = KeyPair::from_seed(seed.to_le_bytes().to_vec(), Algorithm::Ed25519);
    AccountId::new(domain_id, keypair.public_key().clone())
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use iroha_crypto::KeyPair;
    #[allow(unused_imports)]
    use iroha_data_model::prelude::AccountId;
    #[allow(unused_imports)]
    use tokio::runtime::Runtime;

    #[allow(unused_imports)]
    use super::build_state;

    #[test]
    fn build_state_succeeds_without_executor_bytecode() {
        let rt = Runtime::new().unwrap();
        let keypair = KeyPair::random();
        let account_id = AccountId::new(
            "test_domain".parse().expect("valid domain"),
            keypair.public_key().clone(),
        );

        // Should not panic even if executor bytecode is missing or invalid
        let state = build_state(rt.handle(), &account_id, keypair.private_key());

        let view = state.view();
        assert_eq!(view.height(), 1);
        assert!(view.latest_block().is_some());
    }
}

fn construct_asset_definition_id(i: usize, domain_id: DomainId) -> AssetDefinitionId {
    AssetDefinitionId::new(
        domain_id,
        format!("non_inlinable_asset_definition_name_{i}")
            .parse()
            .unwrap(),
    )
}

pub fn generate_ids(
    domains: usize,
    accounts_per_domain: usize,
    assets_per_domain: usize,
) -> (Vec<DomainId>, Vec<AccountId>, Vec<AssetDefinitionId>) {
    let mut domain_ids = Vec::new();
    let mut account_ids = Vec::new();
    let mut asset_definition_ids = Vec::new();

    for i in 0..domains {
        let domain_id = construct_domain_id(i);
        domain_ids.push(domain_id.clone());
        for account_idx in 0..accounts_per_domain {
            let seed = (i as u128) * accounts_per_domain as u128 + account_idx as u128;
            let account_id = generate_account_id(domain_id.clone(), seed);
            account_ids.push(account_id)
        }
        for k in 0..assets_per_domain {
            let asset_definition_id = construct_asset_definition_id(k, domain_id.clone());
            asset_definition_ids.push(asset_definition_id);
        }
    }

    (domain_ids, account_ids, asset_definition_ids)
}
