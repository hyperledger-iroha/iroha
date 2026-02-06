//! Integration-style test: non-VM (native ISI) transaction gas metering and fee transfer.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::similar_names)]
use std::{borrow::Cow, sync::Arc};

use iroha_config::parameters::actual::{GasLiquidity, GasVolatility};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    executor::Executor,
    gas as isi_gas,
    kura::Kura,
    query,
    state::{State, World, WorldReadOnly},
    tx::{AcceptedTransaction, TransactionRejectionReason},
};
use iroha_data_model::prelude::*;
use iroha_data_model::query::parameters::ForwardCursor;
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::gen_account_in;
use ivm::{ProgramMetadata, encoding, instruction, kotodama::wide as kwide, syscalls as ivm_sys};
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

#[test]
fn ivm_syscall_charges_fees() {
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

    let mut pipeline = state.pipeline.clone();
    pipeline.gas.tech_account_id = gas_id.to_string();
    pipeline.gas.accepted_assets = vec![asset_def_id.to_string()];
    let rate: u64 = 10;
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: rate,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);

    let scall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        u8::try_from(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL).expect("syscall id fits in u8"),
    );
    let mut program = ProgramMetadata {
        max_cycles: 1_000,
        ..ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&scall.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let exec = Executable::Ivm(IvmBytecode::from_compiled(program));

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

    let executor = Executor::default();
    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    let mut state_tx = block.transaction();
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    executor
        .execute_transaction(&mut state_tx, &alice_id, tx, &mut ivm_cache)
        .expect("execution");

    let fc = ForwardCursor {
        query: "sc_dummy".to_string(),
        cursor: nonzero!(1_u64),
        gas_budget: None,
    };
    let isi = SetKeyValue::account(alice_id.clone(), "cursor".parse().unwrap(), Json::new(fc));
    let expected_extra =
        isi_gas::meter_instruction(&InstructionBox::from(SetKeyValueBox::from(isi)));
    let scall_cost = ivm::gas::cost_of(scall);
    let expected_used = scall_cost.saturating_add(expected_extra);
    assert_eq!(state_tx.last_tx_gas_used, expected_used);

    let fee = u128::from(state_tx.last_tx_gas_used) * u128::from(rate);
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
fn gas_limit_metadata_invalid_rejects() {
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
    let world = World::with([dom], [alice], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = new_state(world, kura, query_handle);

    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();
    let exec = Executable::from(core::iter::once(instruction));

    let mut md = Metadata::default();
    md.insert(
        "gas_limit".parse().unwrap(),
        iroha_primitives::json::Json::new("not-a-number"),
    );

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
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("invalid gas_limit metadata"),
            "unexpected rejection: {msg}"
        ),
        other => panic!("expected invalid gas_limit rejection, got {other:?}"),
    }
}

#[test]
fn gas_limit_metadata_zero_rejects() {
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
    let world = World::with([dom], [alice], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let state = new_state(world, kura, query_handle);

    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();
    let exec = Executable::from(core::iter::once(instruction));

    let mut md = Metadata::default();
    md.insert(
        "gas_limit".parse().unwrap(),
        iroha_primitives::json::Json::new(0_u64),
    );

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
    match res {
        Err(ValidationFail::NotPermitted(msg)) => assert!(
            msg.contains("gas_limit must be positive"),
            "unexpected rejection: {msg}"
        ),
        other => panic!("expected invalid gas_limit rejection, got {other:?}"),
    }
}

#[test]
fn ivm_gas_fees_record_settlement_receipt() {
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
    let rate: u64 = 7;
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: rate,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);

    // 3) Build a minimal IVM program that consumes gas
    let mut code = Vec::new();
    code.extend_from_slice(&kwide::encode_add(1, 0, 0).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut program = meta.encode();
    program.extend_from_slice(&code);

    // Metadata specifying gas asset + generous limit
    let mut md = Metadata::default();
    md.insert(
        "gas_asset_id".parse().unwrap(),
        iroha_primitives::json::Json::new(asset_def_id.to_string()),
    );
    md.insert("gas_limit".parse().unwrap(), 1_000_000u64);

    let chain: ChainId = "test-chain".parse().unwrap();
    let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .with_metadata(md)
        .sign(alice_kp.private_key());
    let tx_hash = tx.hash();

    // 4) Execute via executor and verify settlement receipt is recorded
    let executor = Executor::default();
    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    let mut state_tx = block.transaction();
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    executor
        .execute_transaction(&mut state_tx, &alice_id, tx, &mut ivm_cache)
        .expect("execution");

    assert!(state_tx.last_tx_gas_used > 0);
    let fee = u128::from(state_tx.last_tx_gas_used) * u128::from(rate);

    let mut receipts = state_tx.drain_settlement_records();
    let record = receipts
        .remove(&tx_hash)
        .expect("settlement receipt recorded");
    assert_eq!(record.asset_definition_id, asset_def_id);
    assert_eq!(record.local_amount_micro, fee);
}

#[test]
fn rejected_tx_does_not_record_settlement_receipt_when_block_gas_limit_exceeded() {
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
    let tech: Account = Account::new(gas_id.clone()).build(&gas_id);
    let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);

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

    let rate: u64 = 10;
    let fee = u128::from(used) * u128::from(rate);
    let init = fee.saturating_add(100);
    let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
    let payer_balance = Asset::new(payer_asset, Numeric::new(init, 0));
    let world = World::with_assets([dom_w, dom_i], [alice, tech], [ad], [payer_balance], []);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let mut state = new_state(world, kura, query_handle);

    let mut pipeline = state.pipeline.clone();
    pipeline.gas.tech_account_id = gas_id.to_string();
    pipeline.gas.accepted_assets = vec![asset_def_id.to_string()];
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: rate,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);

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

    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    block.gas_limit_per_block = used.saturating_sub(1);

    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let (_hash, res) = block.validate_transaction(accepted, &mut ivm_cache);
    assert!(matches!(
        res,
        Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(_)
        ))
    ));

    let receipts = block.drain_settlement_records();
    assert!(receipts.is_empty(), "rejected tx must not emit receipts");
}
