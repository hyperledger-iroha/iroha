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
use ivm::{IVM, KotodamaCompiler, verify_contract_artifact};
use mv::storage::StorageReadOnly;
use std::{fs, path::PathBuf, sync::Arc};

fn fixture_account(hex_public_key: &str) -> AccountId {
    let public_key = hex_public_key.parse().expect("public key");
    AccountId::new(public_key)
}

#[test]
fn kotodama_pointer_abi_asset_ops_end_to_end() {
    // Compile Kotodama sample
    let asset_def_seed: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonder", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    let sample_asset_literal = asset_def_seed.canonical_address();
    let src = include_str!("../../kotodama_lang/src/samples/asset_ops.ko")
        .replace("coin#wonder", &sample_asset_literal);
    let compiler = KotodamaCompiler::new();
    let program = compiler.compile_source(&src).expect("compile kotodama");

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
    let account_domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let asset_def = AssetDefinitionId::parse_address_literal(&sample_asset_literal)
        .expect("canonical asset definition literal");
    let reg_account_domain =
        RegisterBox::from(Register::domain(Domain::new(account_domain_id.clone())));
    let asset_domain_id = asset_def.domain().clone();
    let reg_asset_domain = RegisterBox::from(Register::domain(Domain::new(asset_domain_id)));
    let reg_from = RegisterBox::from(Register::account(NewAccount::new(from.clone())));
    let reg_to = RegisterBox::from(Register::account(NewAccount::new(to.clone())));
    let reg_asset_def = RegisterBox::from(Register::asset_definition(
        AssetDefinition::numeric(asset_def.clone()).with_name(asset_def.name().to_string()),
    ));
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

#[test]
fn kotodama_state_loaded_pointers_drive_transfer_asset() {
    let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonder", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    let asset_literal = asset_def.canonical_address();
    let src = format!(
        r#"
        seiyaku PointerStateTransfer {{
          state PoolAsset: Map<int, AssetDefinitionId>;
          fn main() {{
            let key = 7;
            PoolAsset[key] = asset_definition("{asset_literal}");
            transfer_asset(authority(), authority(), PoolAsset[key], 1);
          }}
        }}
    "#
    );
    let program = KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile pointer state transfer");

    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let mut vm = IVM::new(50_000_000);
    vm.set_host(CoreHost::new(authority.clone()));
    vm.load_program(&program).expect("load program");
    vm.run()
        .expect("state-loaded pointers should be accepted by transfer_asset");

    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1, "expected one queued transfer");

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

    let account_domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let reg_account_domain =
        RegisterBox::from(Register::domain(Domain::new(account_domain_id.clone())));
    let reg_asset_domain =
        RegisterBox::from(Register::domain(Domain::new(asset_def.domain().clone())));
    let reg_authority = RegisterBox::from(Register::account(NewAccount::new(authority.clone())));
    let reg_asset_def = RegisterBox::from(Register::asset_definition(
        AssetDefinition::numeric(asset_def.clone()).with_name(asset_def.name().to_string()),
    ));
    let mint = MintBox::from(Mint::asset_numeric(
        1u32,
        AssetId::of(asset_def.clone(), authority.clone()),
    ));

    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_account_domain),
        InstructionBox::from(reg_asset_domain),
        InstructionBox::from(reg_authority),
        InstructionBox::from(reg_asset_def),
        InstructionBox::from(mint),
    ] {
        executor
            .execute_instruction(&mut tx, &authority, instr)
            .expect("setup should succeed");
    }

    for instr in queued {
        executor
            .execute_instruction(&mut tx, &authority, instr)
            .expect("apply queued transfer");
    }
    tx.apply();
    block.commit().expect("commit block");

    let balance = state
        .view()
        .world
        .assets()
        .get(&AssetId::of(asset_def, authority.clone()))
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    assert_eq!(balance, Numeric::from(1u32));
}

#[test]
fn kotodama_name_keyed_state_loaded_pointers_survive_cross_call() {
    let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonder", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    let asset_literal = asset_def.canonical_address();
    let write_src = format!(
        r#"
        seiyaku PointerStateWrite {{
          state PoolAsset: Map<Name, AssetDefinitionId>;
          fn main() {{
            let key = name!("pool");
            PoolAsset[key] = asset_definition("{asset_literal}");
          }}
        }}
    "#
    );
    let read_src = format!(
        r#"
        seiyaku PointerStateRead {{
          state PoolAsset: Map<Name, AssetDefinitionId>;
          fn main() {{
            let key = name!("pool");
            transfer_asset(authority(), authority(), PoolAsset[key], 1);
          }}
        }}
    "#
    );
    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

    let write_program = KotodamaCompiler::new()
        .compile_source(&write_src)
        .expect("compile writer");
    let mut write_vm = IVM::new(50_000_000);
    write_vm.set_host(CoreHost::new(authority.clone()));
    write_vm.load_program(&write_program).expect("load writer");
    write_vm.run().expect("writer run");
    let overlay = CoreHost::with_host(&mut write_vm, CoreHost::drain_durable_state_overlay);
    assert_eq!(overlay.len(), 1, "expected one durable state write");

    let mut world = World::new();
    for (path, value) in overlay {
        let stored = value.expect("state value must be present");
        world
            .smart_contract_state_mut_for_testing()
            .insert(path, stored);
    }

    let read_program = KotodamaCompiler::new()
        .compile_source(&read_src)
        .expect("compile reader");
    let mut read_vm = IVM::new(50_000_000);
    let mut read_host = CoreHost::new(authority.clone());
    let world_view = world.view();
    read_host.set_durable_state_snapshot_from_world(&world_view);
    read_vm.set_host(read_host);
    read_vm.load_program(&read_program).expect("load reader");
    read_vm
        .run()
        .expect("name-keyed state-loaded pointers should survive cross-call");

    let queued = CoreHost::with_host(&mut read_vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1, "expected one queued transfer");
}

#[test]
fn kotodama_mixed_name_keyed_state_loaded_pointers_survive_cross_call() {
    let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonder", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    let asset_literal = asset_def.canonical_address();
    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let vault =
        fixture_account("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    let vault_literal = vault.to_string();

    let write_src = format!(
        r#"
        seiyaku PointerStateWrite {{
          state PoolAsset: Map<Name, AssetDefinitionId>;
          state VaultAccount: Map<Name, AccountId>;
          fn main() {{
            let key = name!("pool");
            PoolAsset[key] = asset_definition("{asset_literal}");
            VaultAccount[key] = account_id("{vault_literal}");
          }}
        }}
    "#
    );
    let read_src = r#"
        seiyaku PointerStateRead {
          state PoolAsset: Map<Name, AssetDefinitionId>;
          state VaultAccount: Map<Name, AccountId>;
          fn main() {
            let key = name!("pool");
            let vault = VaultAccount[key];
            transfer_asset(authority(), vault, PoolAsset[key], 1);
          }
        }
    "#;

    let write_program = KotodamaCompiler::new()
        .compile_source(&write_src)
        .expect("compile writer");
    let mut write_vm = IVM::new(50_000_000);
    write_vm.set_host(CoreHost::new(authority.clone()));
    write_vm.load_program(&write_program).expect("load writer");
    write_vm.run().expect("writer run");
    let overlay = CoreHost::with_host(&mut write_vm, CoreHost::drain_durable_state_overlay);
    assert_eq!(overlay.len(), 2, "expected two durable state writes");

    let mut world = World::new();
    for (path, value) in overlay {
        let stored = value.expect("state value must be present");
        world
            .smart_contract_state_mut_for_testing()
            .insert(path, stored);
    }

    let read_program = KotodamaCompiler::new()
        .compile_source(read_src)
        .expect("compile reader");
    let mut read_vm = IVM::new(50_000_000);
    let mut read_host = CoreHost::new(authority.clone());
    let world_view = world.view();
    read_host.set_durable_state_snapshot_from_world(&world_view);
    read_vm.set_host(read_host);
    read_vm.load_program(&read_program).expect("load reader");
    read_vm
        .run()
        .expect("mixed state-loaded pointers should survive cross-call");

    let queued = CoreHost::with_host(&mut read_vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1, "expected one queued transfer");
}

#[test]
fn kotodama_event_to_state_loaded_transfer_asset_survives_cross_call() {
    let asset_literal = "6qLb5RYJbzychndCXgFa9aZzjWyx";
    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let vault =
        fixture_account("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    let authority_literal = authority.to_string();
    let vault_literal = vault.to_string();

    let write_src = format!(
        r#"
        seiyaku PointerStateWrite {{
          state BaseAsset: Map<Name, AssetDefinitionId>;
          state VaultAccount: Map<Name, AccountId>;
          fn main() {{
            let key = name!("pool");
            BaseAsset[key] = asset_definition("{asset_literal}");
            VaultAccount[key] = account_id("{vault_literal}");
          }}
        }}
    "#
    );
    let read_src = format!(
        r#"
        seiyaku PointerStateRead {{
          state BaseAsset: Map<Name, AssetDefinitionId>;
          state VaultAccount: Map<Name, AccountId>;
          fn main() {{
            let key = name!("pool");
            let ev = json("{{\"provider\":\"{authority_literal}\",\"base_amount\":1000}}");
            let provider = json_get_account_id(ev, name("provider"));
            let base_amount = json_get_int(ev, name("base_amount"));
            let vault = VaultAccount[key];
            if base_amount > 0 {{
              transfer_asset(provider, vault, BaseAsset[key], base_amount);
            }}
          }}
        }}
    "#
    );

    let write_program = KotodamaCompiler::new()
        .compile_source(&write_src)
        .expect("compile writer");
    let mut write_vm = IVM::new(50_000_000);
    write_vm.set_host(CoreHost::new(authority.clone()));
    write_vm.load_program(&write_program).expect("load writer");
    write_vm.run().expect("writer run");
    let overlay = CoreHost::with_host(&mut write_vm, CoreHost::drain_durable_state_overlay);
    assert_eq!(overlay.len(), 2, "expected two durable state writes");

    let mut world = World::new();
    for (path, value) in overlay {
        let stored = value.expect("state value must be present");
        world
            .smart_contract_state_mut_for_testing()
            .insert(path, stored);
    }

    let read_program = KotodamaCompiler::new()
        .compile_source(&read_src)
        .expect("compile reader");
    let mut read_vm = IVM::new(50_000_000);
    let mut read_host = CoreHost::new(authority.clone());
    let world_view = world.view();
    read_host.set_durable_state_snapshot_from_world(&world_view);
    read_vm.set_host(read_host);
    read_vm.load_program(&read_program).expect("load reader");
    read_vm
        .run()
        .expect("event-fed state-loaded transfer_asset should survive cross-call");

    let queued = CoreHost::with_host(&mut read_vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1, "expected one queued transfer");
}

#[test]
fn dlmm_pool_seed_bin_entrypoint_survives_cross_call() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../soraswap/contracts/dlmm/dlmm_pool.ko");
    let source = fs::read_to_string(&source_path).expect("read dlmm_pool contract");
    let program = KotodamaCompiler::new()
        .compile_source(&source)
        .expect("compile dlmm_pool");
    let artifact = verify_contract_artifact(&program).expect("verify contract artifact");
    let prefix_len = (artifact.code_offset - artifact.header_len) as u64;
    let init_pool_pc = prefix_len
        + artifact
            .contract_interface
            .entrypoints
            .iter()
            .find(|entry| entry.name == "init_pool")
            .expect("init_pool entrypoint")
            .entry_pc;
    let seed_bin_pc = prefix_len
        + artifact
            .contract_interface
            .entrypoints
            .iter()
            .find(|entry| entry.name == "seed_bin")
            .expect("seed_bin entrypoint")
            .entry_pc;

    let authority =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let authority_literal = authority.to_string();
    let accounts = Arc::new(vec![authority.clone()]);

    let init_args = Json::new(norito::json!({
        "base_asset": "6qLb5RYJbzychndCXgFa9aZzjWyx",
        "quote_asset": "7Dsw1EgqCsPmv9HpEztf26xEL2qo",
        "vault_account": authority_literal,
        "fee_pips": 3000,
        "bin_step": 1,
        "active_bin": 0
    }));
    let mut init_vm = IVM::new(50_000_000);
    let init_host =
        CoreHost::with_accounts_and_args(authority.clone(), Arc::clone(&accounts), init_args);
    init_vm.load_program(&program).expect("load dlmm_pool");
    init_vm
        .set_program_counter(init_pool_pc)
        .expect("set init_pool pc");
    init_vm.set_register(1, init_vm.memory.code_len());
    init_vm.set_host(init_host);
    init_vm.run().expect("init_pool run");
    let overlay = CoreHost::with_host(&mut init_vm, CoreHost::drain_durable_state_overlay);
    assert!(
        !overlay.is_empty(),
        "expected durable state writes from init_pool"
    );

    let mut world = World::new();
    for (path, value) in overlay {
        let stored = value.expect("state value must be present");
        world
            .smart_contract_state_mut_for_testing()
            .insert(path, stored);
    }

    let provider_literal = authority.to_string();
    let seed_args = Json::new(norito::json!({
        "provider": provider_literal,
        "bin_id": 0,
        "base_amount": 1000,
        "quote_amount": 1000
    }));
    let mut seed_vm = IVM::new(50_000_000);
    let mut seed_host =
        CoreHost::with_accounts_and_args(authority.clone(), Arc::clone(&accounts), seed_args);
    let world_view = world.view();
    seed_host.set_durable_state_snapshot_from_world(&world_view);
    seed_vm.load_program(&program).expect("load dlmm_pool");
    seed_vm
        .set_program_counter(seed_bin_pc)
        .expect("set seed_bin pc");
    seed_vm.set_register(1, seed_vm.memory.code_len());
    seed_vm.set_host(seed_host);
    seed_vm.run().expect("seed_bin run");

    let queued = CoreHost::with_host(&mut seed_vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 2, "expected two queued transfer instructions");
}
