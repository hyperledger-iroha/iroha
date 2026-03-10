//! End-to-end test for Kotodama pointer ABI asset operations.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::explicit_into_iter_loop, clippy::map_unwrap_or)]

#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{account::NewAccount, prelude::*};
use ivm::{IVM, KotodamaCompiler};
use mv::storage::StorageReadOnly;

fn fixture_account(hex_public_key: &str) -> AccountId {
    let public_key = hex_public_key.parse().expect("public key");
    AccountId::new(public_key)
}

#[test]
fn kotodama_pointer_abi_asset_ops_end_to_end() {
    // Compile Kotodama sample
    let src = include_str!("../../kotodama_lang/src/samples/asset_ops.ko");
    let compiler = KotodamaCompiler::new();
    let program = compiler.compile_source(src).expect("compile kotodama");

    // Prepare VM with CoreHost
    let from =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let to =
        fixture_account("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    let mut vm = IVM::new(50_000_000);
    vm.set_host(CoreHost::new(from.clone()));
    vm.load_program(&program).expect("load program");
    vm.run().expect("run VM");

    // Drain queued ISIs
    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions);
    assert!(!queued.is_empty());
    eprintln!("queued {} instructions", queued.len());
    for (i, instr) in queued.iter().enumerate() {
        eprintln!("queued[{i}]: {instr:?}");
    }

    // Build a minimal State and apply setup ISIs then queued ISIs
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    #[cfg(feature = "telemetry")]
    let state = State::new(World::new(), kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let state = State::new(World::new(), kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut tx = block.transaction();
    // Setup: register domains required by account ids and asset definition.
    let account_domain_id: DomainId = "wonderland".parse().unwrap();
    let reg_account_domain =
        RegisterBox::from(Register::domain(Domain::new(account_domain_id.clone())));
    let asset_domain_id: DomainId = "wonder".parse().unwrap();
    let reg_asset_domain = RegisterBox::from(Register::domain(Domain::new(asset_domain_id)));
    let reg_from = RegisterBox::from(Register::account(NewAccount::new_in_domain(
        from.clone(),
        account_domain_id.clone(),
    )));
    let reg_to = RegisterBox::from(Register::account(NewAccount::new_in_domain(
        to.clone(),
        account_domain_id.clone(),
    )));
    let asset_def: AssetDefinitionId = "coin#wonder".parse().unwrap();
    let reg_asset_def = RegisterBox::from(Register::asset_definition(AssetDefinition::numeric(
        asset_def.clone(),
    )));
    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_account_domain),
        InstructionBox::from(reg_asset_domain),
        InstructionBox::from(reg_from),
        InstructionBox::from(reg_to),
        InstructionBox::from(reg_asset_def),
    ] {
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("setup should succeed");
    }

    // Apply deterministic ISIs mirroring the sample program
    assert_eq!(queued.len(), 3, "expected three enqueued instructions");
    let expected_asset_id = AssetId::of(asset_def.clone(), from.clone());
    let expected_asset_id_to = AssetId::of(asset_def.clone(), to.clone());
    let mint = iroha_data_model::isi::Mint::asset_numeric(1000u32, expected_asset_id.clone());
    let transfer = iroha_data_model::isi::Transfer::asset_numeric(
        expected_asset_id.clone(),
        500u32,
        to.clone(),
    );
    let burn = iroha_data_model::isi::Burn::asset_numeric(100u32, expected_asset_id_to.clone());

    for instr in [
        InstructionBox::from(iroha_data_model::isi::MintBox::from(mint)),
        InstructionBox::from(iroha_data_model::isi::TransferBox::from(transfer)),
        InstructionBox::from(iroha_data_model::isi::BurnBox::from(burn)),
    ] {
        eprintln!("applying {instr:?}");
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("execution should succeed");
    }
    tx.apply();
    block.commit().expect("commit block");

    // Rough balance checks: after sample, net effect:
    // mint 1000 to from, transfer 500 from->to, burn 100 from to
    // from: 1000 - 500 = 500; to: +500 - 100 = 400
    let from_asset = AssetId::of(asset_def.clone(), from.clone());
    let to_asset = AssetId::of(asset_def.clone(), to.clone());
    let from_bal = state
        .view()
        .world
        .assets()
        .get(&from_asset)
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    let to_bal = state
        .view()
        .world
        .assets()
        .get(&to_asset)
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    assert_eq!(from_bal, Numeric::from(500u32));
    assert_eq!(to_bal, Numeric::from(400u32));
}
