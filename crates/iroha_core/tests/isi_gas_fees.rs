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

fn new_account_in_domain(account_id: &AccountId, domain: &str) -> Account {
    Account::new_in_domain(account_id.clone(), domain.parse().expect("domain")).build(account_id)
}

#[test]
fn non_vm_instructions_charge_fees() {
    // 1) Minimal world: domains, accounts, asset definition, payer balance, tech account
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
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
fn non_vm_instructions_can_charge_gas_to_fee_sponsor() {
    use iroha_core::smartcontracts::Execute;
    use iroha_executor_data_model::permission::nexus::CanUseFeeSponsor;

    // 1) Minimal world: domains, accounts, asset definition, sponsor balance, tech account
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let sponsor = new_account_in_domain(&sponsor_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
    let sponsor_asset = AssetId::of(asset_def_id.clone(), sponsor_id.clone());
    let init = 100_000u128;
    let sponsor_balance = Asset::new(sponsor_asset.clone(), Numeric::new(init, 0));
    let payer_balance = Asset::new(payer_asset.clone(), Numeric::new(0, 0));
    let world = World::with_assets(
        [dom_w, dom_i],
        [alice, sponsor, tech],
        [ad],
        [sponsor_balance, payer_balance],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let mut state = new_state(world, kura, query_handle);

    // 2) Configure pipeline gas policy + enable sponsorship
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
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.sponsorship_enabled = true;
    }

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

    // Metadata specifying gas asset + generous limit + fee sponsor
    let mut md = Metadata::default();
    md.insert(
        "gas_asset_id".parse().unwrap(),
        iroha_primitives::json::Json::new(asset_def_id.to_string()),
    );
    md.insert("gas_limit".parse().unwrap(), 1_000_000u64);
    md.insert(
        "fee_sponsor".parse().unwrap(),
        iroha_primitives::json::Json::new(sponsor_id.to_string()),
    );

    let chain: ChainId = "test-chain".parse().unwrap();
    let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(exec)
        .with_metadata(md)
        .sign(alice_kp.private_key());

    // 4) Execute via executor and verify sponsored fee transfer
    let executor = Executor::default();
    let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(block_header);
    let mut state_tx = block.transaction();

    // Grant the authority permission to charge fees to the sponsor.
    let permission = CanUseFeeSponsor {
        sponsor: sponsor_id.clone(),
    };
    Grant::account_permission(permission, alice_id.clone())
        .execute(&sponsor_id, &mut state_tx)
        .expect("grant fee sponsor permission");

    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    executor
        .execute_transaction(&mut state_tx, &alice_id, tx, &mut ivm_cache)
        .expect("execution");

    // Used gas is recorded for block-level accounting
    assert!(state_tx.last_tx_gas_used >= used);

    // Fee = used * rate (units_per_gas)
    let fee = u128::from(state_tx.last_tx_gas_used) * u128::from(rate);

    // Read balances and assert transfer took place from sponsor -> tech
    let payer_balance_after = state_tx
        .world
        .assets()
        .get(&payer_asset)
        .expect("payer asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    let sponsor_balance_after = state_tx
        .world
        .assets()
        .get(&sponsor_asset)
        .expect("sponsor asset exists")
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

    assert_eq!(payer_balance_after, 0);
    assert_eq!(sponsor_balance_after, init - fee);
    assert_eq!(payee_balance_after, fee);
}

#[test]
fn non_vm_instructions_can_charge_gas_to_fee_sponsor_via_overlay_pipeline() {
    use iroha_core::block::{BlockBuilder, ValidBlock};
    use iroha_executor_data_model::permission::nexus::CanUseFeeSponsor;

    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (sponsor_id, sponsor_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let sponsor = new_account_in_domain(&sponsor_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
    let sponsor_asset = AssetId::of(asset_def_id.clone(), sponsor_id.clone());
    let tech_asset = AssetId::of(asset_def_id.clone(), gas_id.clone());
    let init = 100_000u128;
    let sponsor_balance = Asset::new(sponsor_asset.clone(), Numeric::new(init, 0));
    let payer_balance = Asset::new(payer_asset.clone(), Numeric::new(0, 0));
    let world = World::with_assets(
        [dom_w, dom_i],
        [alice, sponsor, tech],
        [ad],
        [sponsor_balance, payer_balance],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let mut state = new_state(world, kura, query_handle);

    let permission = CanUseFeeSponsor {
        sponsor: sponsor_id.clone(),
    };
    let setup_instruction: InstructionBox =
        Grant::account_permission(permission, alice_id.clone()).into();
    let chain: ChainId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    let setup_tx = TransactionBuilder::new(chain.clone(), sponsor_id.clone())
        .with_executable(Executable::from(core::iter::once(setup_instruction)))
        .sign(sponsor_kp.private_key());
    let setup_accepted = AcceptedTransaction::new_unchecked(Cow::Owned(setup_tx));
    let setup_block = BlockBuilder::new(vec![setup_accepted])
        .chain(0, None)
        .sign(alice_kp.private_key())
        .unpack(|_| {});
    let setup_block_signed: SignedBlock = setup_block.clone().into();
    let mut setup_state_block = state.block(setup_block.header());
    let setup_valid =
        ValidBlock::validate_unchecked(setup_block.into(), &mut setup_state_block).unpack(|_| {});
    let setup_committed = setup_valid.commit_unchecked().unpack(|_| {});
    let _ = setup_state_block.apply_without_execution(&setup_committed, Vec::new());
    setup_state_block
        .commit()
        .expect("commit setup permission block");
    {
        let check_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut check_block = state.block(check_header);
        let mut check_tx = check_block.transaction();
        assert!(
            check_tx.can_use_fee_sponsor(&alice_id, &sponsor_id),
            "setup block must grant CanUseFeeSponsor permission"
        );
    }

    let mut pipeline = state.pipeline.clone();
    pipeline.gas.tech_account_id = gas_id.to_string();
    pipeline.gas.accepted_assets = vec![asset_def_id.to_string()];
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: 10,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);
    {
        let nexus = state.nexus.get_mut();
        nexus.fees.sponsorship_enabled = true;
    }

    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();
    let mut md = Metadata::default();
    md.insert(
        "gas_asset_id".parse().unwrap(),
        iroha_primitives::json::Json::new(asset_def_id.to_string()),
    );
    md.insert("gas_limit".parse().unwrap(), 1_000_000u64);
    md.insert(
        "fee_sponsor".parse().unwrap(),
        iroha_primitives::json::Json::new(sponsor_id.to_string()),
    );
    let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(Executable::from(core::iter::once(instruction)))
        .with_metadata(md)
        .sign(alice_kp.private_key());

    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    // Build a height>1 block so genesis fee bypass never applies in this test.
    let block = BlockBuilder::new(vec![accepted])
        .chain(0, Some(&setup_block_signed))
        .sign(alice_kp.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block.into(), &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block.commit().expect("commit block");

    let inspect_header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut inspect_block = state.block(inspect_header);
    let inspect_tx = inspect_block.transaction();
    let payer_balance_after = inspect_tx
        .world
        .assets()
        .get(&payer_asset)
        .expect("payer asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    let sponsor_balance_after = inspect_tx
        .world
        .assets()
        .get(&sponsor_asset)
        .expect("sponsor asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    let account_after = inspect_tx
        .world
        .accounts()
        .get(&alice_id)
        .expect("authority account exists");
    assert!(
        account_after
            .metadata()
            .get(&"k".parse::<Name>().expect("metadata key"))
            .is_some(),
        "overlay transaction must apply instruction effects"
    );
    assert_eq!(payer_balance_after, 0);
    assert!(
        sponsor_balance_after < init,
        "sponsor must pay fees in overlay pipeline path"
    );
    let payee_balance_after = inspect_tx
        .world
        .assets()
        .get(&tech_asset)
        .map(|asset| asset.0.try_mantissa_u128().unwrap())
        .unwrap_or(0);
    assert!(
        payee_balance_after > 0,
        "gas fee recipient must receive sponsored fee units"
    );
}

#[test]
fn genesis_overlay_pipeline_transactions_remain_fee_free() {
    use iroha_core::block::{BlockBuilder, ValidBlock};

    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (gas_id, _gas_kp) = gen_account_in("ivm");
    let dom_w: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let dom_i: Domain = Domain::new("ivm".parse().unwrap()).build(&gas_id);
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
    let tech_asset = AssetId::of(asset_def_id.clone(), gas_id.clone());
    let init = 100_000u128;
    let payer_balance = Asset::new(payer_asset.clone(), Numeric::new(init, 0));
    let tech_balance = Asset::new(tech_asset.clone(), Numeric::new(0, 0));
    let world = World::with_assets(
        [dom_w, dom_i],
        [alice, tech],
        [ad],
        [payer_balance, tech_balance],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = query::store::LiveQueryStore::start_test();
    let mut state = new_state(world, kura, query_handle);

    let mut pipeline = state.pipeline.clone();
    pipeline.gas.tech_account_id = gas_id.to_string();
    pipeline.gas.accepted_assets = vec![asset_def_id.to_string()];
    pipeline.gas.units_per_gas = vec![iroha_config::parameters::actual::GasRate {
        asset: asset_def_id.to_string(),
        units_per_gas: 10,
        twap_local_per_xor: Decimal::ONE,
        liquidity: GasLiquidity::Tier2,
        volatility: GasVolatility::Stable,
    }];
    state.set_pipeline(pipeline);

    let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
        alice_id.clone(),
        "k".parse().unwrap(),
        iroha_primitives::json::Json::new("v"),
    )
    .into();
    let mut md = Metadata::default();
    md.insert(
        "gas_asset_id".parse().unwrap(),
        iroha_primitives::json::Json::new(asset_def_id.to_string()),
    );
    md.insert("gas_limit".parse().unwrap(), 1_000_000u64);

    let chain: ChainId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    let tx = TransactionBuilder::new(chain, alice_id.clone())
        .with_executable(Executable::from(core::iter::once(instruction)))
        .with_metadata(md)
        .sign(alice_kp.private_key());

    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let block = BlockBuilder::new(vec![accepted])
        .chain(0, None)
        .sign(alice_kp.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block.into(), &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block.commit().expect("commit genesis block");

    let inspect_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut inspect_block = state.block(inspect_header);
    let inspect_tx = inspect_block.transaction();
    let account_after = inspect_tx
        .world
        .accounts()
        .get(&alice_id)
        .expect("authority account exists");
    assert!(
        account_after
            .metadata()
            .get(&"k".parse::<Name>().expect("metadata key"))
            .is_some(),
        "overlay transaction must apply instruction effects"
    );
    let payer_balance_after = inspect_tx
        .world
        .assets()
        .get(&payer_asset)
        .expect("payer asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    let payee_balance_after = inspect_tx
        .world
        .assets()
        .get(&tech_asset)
        .expect("tech account asset exists")
        .0
        .try_mantissa_u128()
        .unwrap();
    assert_eq!(payer_balance_after, init);
    assert_eq!(payee_balance_after, 0);
}

#[test]
fn non_vm_gas_limit_too_low_rejects() {
    // Minimal world: one domain/account/asset; no fee mapping needed for this negative test.
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
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
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
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
        u8::try_from(ivm_sys::SYSCALL_DEBUG_PRINT).expect("syscall id fits in u8"),
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

    let scall_cost = ivm::gas::cost_of(scall).expect("SCALL must have gas cost");
    assert_eq!(state_tx.last_tx_gas_used, scall_cost);

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
    let alice = new_account_in_domain(&alice_id, "wonderland");
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
    let alice = new_account_in_domain(&alice_id, "wonderland");
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
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
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
    let alice = new_account_in_domain(&alice_id, "wonderland");
    let tech = new_account_in_domain(&gas_id, "ivm");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
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
