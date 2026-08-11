#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[allow(clippy::module_inception)]
#[path = "../../common/mod.rs"]
mod common;

use std::{
    collections::HashSet,
    num::{NonZeroU16, NonZeroU64, NonZeroUsize},
    sync::Arc,
};

pub use common::*;
use iroha_core::{
    block::{BlockBuilder, CommittedBlock},
    governance::manifest::LaneManifestRegistry,
    prelude::*,
    query::store::LiveQueryStore,
    smartcontracts::{Execute, Registrable as _},
    state::{State, StateBlock, World},
    sumeragi::network_topology::Topology,
};
use iroha_data_model::{
    account::Account,
    asset::{AssetDefinition, AssetDefinitionId},
    domain::Domain,
    isi::{InstructionBox, Log},
    parameter::TransactionParameters,
    prelude::*,
    transaction::IvmBytecode,
};
use iroha_executor_data_model::permission::{
    account::CanUnregisterAccount, asset_definition::CanUnregisterAssetDefinition,
};

/// Create block
pub fn create_block(
    state: &mut StateBlock<'_>,
    instructions: Vec<InstructionBox>,
    account_id: AccountId,
    account_private_key: &PrivateKey,
    topology: &Topology,
    peer_private_key: &PrivateKey,
) -> CommittedBlock {
    let network_id = *state.network_id();

    let transaction = TransactionBuilder::new(
        network_id,
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(account_private_key);
    let (max_clock_drift, tx_limits) = {
        let params = state.world.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };

    let crypto_cfg = state.crypto();
    let block = BlockBuilder::new(vec![
        AcceptedTransaction::accept(
            transaction,
            &network_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .unwrap(),
    ])
    .chain(0, state)
    .sign(peer_private_key)
    .unpack(|_| {})
    .validate_and_record_transactions(state)
    .unpack(|_| {})
    .commit(topology)
    .unpack(|_| {})
    .unwrap();

    // Verify that transactions are valid
    for tx in block.as_ref().transactions() {
        assert_eq!(tx.error, None);
    }

    block
}

fn domain_for_index(
    domains: &[DomainId],
    total_items: usize,
    index: usize,
) -> Option<&DomainId> {
    if domains.is_empty() || total_items == 0 {
        return None;
    }
    let domain_index = index.saturating_mul(domains.len()) / total_items;
    domains.get(domain_index.min(domains.len() - 1))
}

fn generated_asset_definition_name(
    domain_count: usize,
    total_assets: usize,
    index: usize,
) -> String {
    assert!(domain_count > 0, "benchmark fixture needs at least one domain");
    assert!(total_assets > 0, "benchmark fixture needs at least one asset");
    assert_eq!(
        total_assets % domain_count,
        0,
        "benchmark assets must be partitioned evenly by domain"
    );
    let assets_per_domain = total_assets / domain_count;
    format!(
        "non_inlinable_asset_definition_name_{}",
        index % assets_per_domain
    )
}

pub fn populate_state(
    domains: &[DomainId],
    accounts: &[AccountId],
    asset_definitions: &[AssetDefinitionId],
    owner_id: &AccountId,
) -> Vec<InstructionBox> {
    let mut instructions: Vec<InstructionBox> = Vec::new();

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

    for (index, asset_definition_id) in asset_definitions.iter().enumerate() {
        let asset_definition = AssetDefinition::numeric(
            asset_definition_id.clone(),
            generated_asset_definition_name(domains.len(), asset_definitions.len(), index),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        );
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
        // Runtime domain re-registration is intentionally unavailable; churn the
        // domain's children while retaining its genesis-created parent.
        let delete_all_children = i % nth == 0;
        for (j, account_id) in accounts
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                domain_for_index(domains, accounts.len(), *index)
                    .is_some_and(|domain| domain == domain_id)
            })
            .map(|(_, account_id)| account_id)
            .enumerate()
        {
            if delete_all_children || j % nth == 0 {
                instructions.push(Unregister::account(account_id.clone()).into());
            }
        }
        for (k, asset_definition_id) in asset_definitions
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                domain_for_index(domains, asset_definitions.len(), *index)
                    .is_some_and(|domain| domain == domain_id)
            })
            .map(|(_, asset_definition_id)| asset_definition_id)
            .enumerate()
        {
            if delete_all_children || k % nth == 0 {
                instructions.push(Unregister::asset_definition(asset_definition_id.clone()).into());
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
        // Domains remain present so this restore batch uses only ordinary
        // post-genesis instructions.
        for (j, account_id) in accounts
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                domain_for_index(domains, accounts.len(), *index)
                    .is_some_and(|domain| domain == domain_id)
            })
            .map(|(_, account_id)| account_id)
            .enumerate()
        {
            if j % nth == 0 || i % nth == 0 {
                let account = Account::new(account_id.clone());
                instructions.push(Register::account(account).into());
            }
        }
        for (k, (asset_index, asset_definition_id)) in asset_definitions
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                domain_for_index(domains, asset_definitions.len(), *index)
                    .is_some_and(|domain| domain == domain_id)
            })
            .enumerate()
        {
            if k % nth == 0 || i % nth == 0 {
                let asset_definition = AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    generated_asset_definition_name(
                        domains.len(),
                        asset_definitions.len(),
                        asset_index,
                    ),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                );
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
    let domain = Domain::new(
        DomainId::try_new("bench", "universal").expect("valid benchmark domain id"),
    )
    .build(account_id);
    let state = State::try_new(
        World::with(
            [domain],
            [Account::new(account_id.clone()).build(account_id)],
            [],
        ),
        kura,
        query_handle,
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .expect("benchmark State startup must validate");
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));

    {
        let network_id = *state.network_id_ref();
        let transaction = TransactionBuilder::new(
            network_id,
            account_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "init".to_string())])
        .sign(account_private_key);
        let (max_clock_drift, tx_limits) = {
            let params = state.world.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let crypto_cfg = state.crypto();
        let unverified_block = BlockBuilder::new(vec![
            AcceptedTransaction::accept(
                transaction,
                &network_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            )
            .unwrap(),
        ])
        .chain(0, &state)
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
        state_block.world.parameters.executor.memory =
            NonZeroU64::new(iroha_data_model::parameter::system::IVM_HEAP_MAX_BYTES)
                .expect("ABI heap window is non-zero");

        let tx_hashes = unverified_block
            .transactions()
            .iter()
            .map(|tx| tx.as_ref().hash())
            .collect::<HashSet<_>>();
        let block_height: NonZeroUsize = unverified_block
            .header()
            .height()
            .try_into()
            .expect("block height should fit into usize");
        state_block
            .transactions
            .insert_block(tx_hashes, block_height);

        let mut state_transaction = state_block.transaction();
        let path_to_executor =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
        if let Ok(bytecode) = std::fs::read(&path_to_executor) {
            if !bytecode.is_empty() {
                let executor = Executor::new(IvmBytecode::from_compiled(bytecode));
                // Ignore upgrade failure and keep the default executor when bytecode is invalid
                let _ = Upgrade::new(executor).execute(account_id, &mut state_transaction);
            }
        }

        state_transaction.apply();
        state_block.commit().unwrap();
    }

    state
}

fn construct_domain_id(i: usize) -> DomainId {
    DomainId::try_new(format!("non_inlinable_domain_name_{i}"), "universal").unwrap()
}

fn generate_account_id(domain_id: DomainId) -> AccountId {
    AccountId::new(KeyPair::random().into_parts().0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use tokio::runtime::Runtime;

    #[test]
    fn build_state_succeeds_without_executor_bytecode() {
        let rt = Runtime::new().unwrap();
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        build_state(rt.handle(), &account_id, keypair.private_key());
    }

    #[test]
    fn build_state_records_init_transaction() {
        let rt = Runtime::new().unwrap();
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());

        let state = build_state(rt.handle(), &account_id, keypair.private_key());

        let network_id = *state.network_id_ref();
        let tx = TransactionBuilder::new(
            network_id,
            account_id.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "init".to_string())])
        .sign(keypair.private_key());

        assert!(state.transactions.view().get(&tx.hash()).is_some());
    }
}

fn construct_asset_definition_id(i: usize, domain_id: DomainId) -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
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
        for _ in 0..accounts_per_domain {
            let account_id = generate_account_id(domain_id.clone());
            account_ids.push(account_id)
        }
        for k in 0..assets_per_domain {
            let asset_definition_id = construct_asset_definition_id(k, domain_id.clone());
            asset_definition_ids.push(asset_definition_id);
        }
    }

    (domain_ids, account_ids, asset_definition_ids)
}
