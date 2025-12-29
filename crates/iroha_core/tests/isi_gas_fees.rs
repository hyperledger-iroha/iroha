//! Integration-style test: non-VM (native ISI) transaction gas metering and fee transfer.
#![allow(clippy::similar_names)]
use std::sync::Arc;

use iroha_config::parameters::actual::{GasLiquidity, GasVolatility};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    executor::Executor,
    gas as isi_gas,
    kura::Kura,
    query,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use rust_decimal::Decimal;

#[cfg(feature = "telemetry")]
fn new_state(
    world: World,
    kura: Arc<Kura>,
    query_handle: query::store::LiveQueryStoreHandle,
) -> State {
    State::new(world, kura, query_handle, StateTelemetry::default())
}

#[cfg(not(feature = "telemetry"))]
fn new_state(
    world: World,
    kura: Arc<Kura>,
    query_handle: query::store::LiveQueryStoreHandle,
) -> State {
    State::new(world, kura, query_handle)
}

#[test]
fn non_vm_instructions_charge_fees() {
    // 1) Minimal world: domains, accounts, asset definition, payer balance, tech account
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
    let tech: Account = Account::new(gas_id.clone()).build(&gas_id);
    let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
    let init = 100_000u128;
    let payer_balance = Asset::new(payer_asset.clone(), Numeric::new(init, 0));
    let world = World::with_assets([dom_w, dom_i], [alice, tech], [ad], [payer_balance], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let mut state = new_state(world, kura, query_handle);

    // 2) Configure pipeline gas policy
    let mut pipeline = state.pipeline.clone();
    pipeline.gas.tech_account_id = gas_id.to_string();
    pipeline.gas.accepted_assets = vec![asset_def_id.to_string()];
    let rate: u64 = 10; // minimal units per one gas
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: rate,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);

    // 3) Build a simple native ISI transaction (SetKeyValue<Account>)
    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();

    let exec = Executable::from(core::iter::once(instruction.clone()));
    let used = isi_gas::meter_instructions(match &exec {
        Executable::Instructions(v) => v.as_ref(),
        _ => unreachable!(),
    });
    assert!(used > 0);

    // Metadata specifying gas asset + generous limit
    let mut md = Metadata::default();
    md.insert(
        "gas_asset_id".parse().unwrap(),
        iroha_primitives::json::Json::new(asset_def_id.to_string()),
    );
    md.insert("gas_limit".parse().unwrap(), 1_000_000u64);

    let chain: ChainId = "test-chain".parse().unwrap();
    let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(exec)
        .with_metadata(md)
        .sign(alice_kp.private_key());

    // 4) Execute via executor and verify fee transfer
    let executor = Executor::default();
    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    let mut state_tx = block.transaction();
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    executor
        .execute_transaction(&mut state_tx, &alice_id, tx, &mut ivm_cache)
        .expect("execution");

    // Used gas is recorded for block-level accounting
    assert!(state_tx.last_tx_gas_used >= used);

    // Fee = used * rate (units_per_gas)
    let fee = u128::from(state_tx.last_tx_gas_used) * u128::from(rate);

    // Read balances and assert transfer took place
    let payer_balance_after = state_tx
        .world
        .assets()
        .get(&payer_asset)
        .expect("payer asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    let payee_balance_after = state_tx
        .world
        .assets()
        .get(&AssetId::of(asset_def_id.clone(), gas_id.clone()))
        .expect("tech account asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();

    assert_eq!(payer_balance_after, init - fee);
    assert_eq!(payee_balance_after, fee);
}

#[test]
fn non_vm_gas_limit_too_low_rejects() {
    // Minimal world: one domain/account/asset; no fee mapping needed for this negative test.
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
    let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let world = World::with([dom], [alice], [ad]);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = new_state(world, kura, query_handle);

    // Single SetKeyValue<Account> instruction
    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();
    let exec = Executable::from(core::iter::once(instruction));
    let used = isi_gas::meter_instructions(match &exec {
        Executable::Instructions(v) => v.as_ref(),
        _ => unreachable!(),
    });
    assert!(used > 0);

    // Provide a gas_limit smaller than metered usage
    let mut md = Metadata::default();
    md.insert("gas_limit".parse().unwrap(), used.saturating_sub(1));

    let chain: ChainId = "test-chain".parse().unwrap();
    let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(exec)
        .with_metadata(md)
        .sign(alice_kp.private_key());

    let executor = Executor::default();
    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    let mut state_tx = block.transaction();
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let res = executor.execute_transaction(&mut state_tx, &alice_id, tx, &mut ivm_cache);
    assert!(matches!(res, Err(ValidationFail::NotPermitted(_))));
}
