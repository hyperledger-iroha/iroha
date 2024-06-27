use std::str::FromStr as _;

use iroha_core::{
    block::{BlockBuilder, CommittedBlock},
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
    isi::InstructionBox,
    prelude::*,
    transaction::TransactionLimits,
    ChainId,
};
use iroha_primitives::{json::JsonString, unique_vec::UniqueVec};
use serde_json::json;

/// Create block
pub fn create_block(
    state: &mut StateBlock<'_>,
    instructions: Vec<InstructionBox>,
    account_id: AccountId,
    account_private_key: &PrivateKey,
    topology: &Topology,
    peer_private_key: &PrivateKey,
) -> CommittedBlock {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let transaction = TransactionBuilder::new(chain_id.clone(), account_id)
        .with_instructions(instructions)
        .sign(account_private_key);
    let limits = state.transaction_executor().transaction_limits;

    let block = BlockBuilder::new(
        vec![AcceptedTransaction::accept(transaction, &chain_id, limits).unwrap()],
        topology.clone(),
        Vec::new(),
    )
    .chain(0, state)
    .sign(peer_private_key)
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

pub fn populate_state(
    domains: &[DomainId],
    accounts: &[AccountId],
    asset_definitions: &[AssetDefinitionId],
    owner_id: &AccountId,
) -> Vec<InstructionBox> {
    let mut instructions: Vec<InstructionBox> = Vec::new();

    for domain_id in domains {
        let domain = Domain::new(domain_id.clone());
        instructions.push(Register::domain(domain).into());
        let can_unregister_domain = Grant::permission(
            Permission::new(
                "CanUnregisterDomain".parse().unwrap(),
                JsonString::from(&json!({ "domain": domain_id.clone() })),
            ),
            owner_id.clone(),
        );
        instructions.push(can_unregister_domain.into());
    }

    for account_id in accounts {
        let account = Account::new(account_id.clone());
        instructions.push(Register::account(account).into());
        let can_unregister_account = Grant::permission(
            Permission::new(
                "CanUnregisterAccount".parse().unwrap(),
                JsonString::from(&json!({ "account": account_id.clone() })),
            ),
            owner_id.clone(),
        );
        instructions.push(can_unregister_account.into());
    }

    for asset_definition_id in asset_definitions {
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone());
        instructions.push(Register::asset_definition(asset_definition).into());
        let can_unregister_asset_definition = Grant::permission(
            Permission::new(
                "CanUnregisterAssetDefinition".parse().unwrap(),
                JsonString::from(&json!({ "asset_definition": asset_definition_id })),
            ),
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

pub fn build_state(rt: &tokio::runtime::Handle, account_id: &AccountId) -> State {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query_handle = {
        let _guard = rt.enter();
        LiveQueryStore::test().start()
    };
    let domain = Domain::new(account_id.domain().clone()).build(account_id);
    let state = State::new(
        World::with(
            [domain],
            [Account::new(account_id.clone()).build(account_id)],
            UniqueVec::new(),
        ),
        kura,
        query_handle,
    );

    {
        let mut state_block = state.block();

        state_block.config.transaction_limits = TransactionLimits::new(u64::MAX, u64::MAX);
        state_block.config.executor_runtime.fuel_limit = u64::MAX;
        state_block.config.executor_runtime.max_memory = u32::MAX.into();

        let mut state_transaction = state_block.transaction();
        let path_to_executor = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../configs/swarm/executor.wasm");
        let wasm = std::fs::read(&path_to_executor)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", path_to_executor.display()));
        let executor = Executor::new(WasmSmartContract::from_compiled(wasm));
        Upgrade::new(executor)
            .execute(account_id, &mut state_transaction)
            .expect("Failed to load executor");

        state_transaction.apply();
        state_block.commit();
    }

    state
}

fn construct_domain_id(i: usize) -> DomainId {
    DomainId::from_str(&format!("non_inlinable_domain_name_{i}")).unwrap()
}

fn generate_account_id(domain_id: DomainId) -> AccountId {
    AccountId::new(domain_id, KeyPair::random().into_parts().0)
}

fn construct_asset_definition_id(i: usize, domain_id: DomainId) -> AssetDefinitionId {
    AssetDefinitionId::new(
        domain_id,
        Name::from_str(&format!("non_inlinable_asset_definition_name_{i}")).unwrap(),
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
