//! Verify IVM pointer ABI applies queued instructions correctly.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::too_many_lines)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{account::NewAccount, prelude::*};
use ivm::{
    IVM, PointerType, ProgramMetadata, encoding, instruction, kotodama::compiler::Compiler,
    syscalls as ivm_sys,
};
use mv::storage::StorageReadOnly;
use norito::NoritoSerialize;

fn fixture_account(hex_public_key: &str) -> AccountId {
    let public_key = hex_public_key.parse().expect("public key");
    AccountId::new(public_key)
}

fn tlv_envelope<T: NoritoSerialize>(type_id: PointerType, val: &T) -> Vec<u8> {
    let payload = norito::to_bytes(val).expect("encode payload");
    let mut blob = Vec::with_capacity(2 + 1 + 4 + payload.len() + iroha_crypto::Hash::LENGTH);
    blob.extend_from_slice(&(type_id as u16).to_be_bytes());
    blob.push(1u8); // version
    blob.extend_from_slice(&(u32::try_from(payload.len()).unwrap()).to_be_bytes());
    blob.extend_from_slice(&payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    blob.extend_from_slice(&hash);
    blob
}

fn opaque_asset_definition_literal(aid_bytes: [u8; 16]) -> String {
    AssetDefinitionId::from_uuid_bytes(aid_bytes)
        .expect("opaque asset definition id")
        .to_string()
}

#[test]
fn apply_queued_isis_from_corehost_transfer_asset() {
    // Build a minimal IVM program that performs SCALL TRANSFER_ASSET and HALT
    let from =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let to =
        fixture_account("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "coin".parse().unwrap(),
    );
    let from_bytes = tlv_envelope(PointerType::AccountId, &from);
    let to_bytes = tlv_envelope(PointerType::AccountId, &to);
    let asset_bytes = tlv_envelope(PointerType::AssetDefinitionId, &asset_def);
    let amount = Numeric::from(500_u64);
    let amount_bytes = tlv_envelope(PointerType::NoritoBytes, &amount);
    let align8 = |n: u64| (n + 7) & !7;
    let off_from = 0u64;
    let off_to = align8(off_from + from_bytes.len() as u64);
    let off_asset = align8(off_to + to_bytes.len() as u64);
    let off_amount = align8(off_asset + asset_bytes.len() as u64);
    let ptr_from = ivm::Memory::INPUT_START + off_from;
    let ptr_to = ivm::Memory::INPUT_START + off_to;
    let ptr_asset = ivm::Memory::INPUT_START + off_asset;
    let ptr_amount = ivm::Memory::INPUT_START + off_amount;

    let mut code = Vec::new();
    // SCALL transfer + HALT
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_TRANSFER_ASSET).unwrap(),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    // Full program (metadata + code)
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 10_000,
        abi_version: 1,
    };
    let mut program = meta.encode();
    program.extend_from_slice(&code);

    // Build VM with CoreHost and preload INPUT
    let mut vm = IVM::new(500_000);
    vm.set_host(CoreHost::new(from.clone()));
    vm.memory
        .preload_input(off_from, &from_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_to, &to_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_asset, &asset_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_amount, &amount_bytes)
        .expect("preload input");
    vm.load_program(&program).unwrap();
    vm.set_register(10, ptr_from);
    vm.set_register(11, ptr_to);
    vm.set_register(12, ptr_asset);
    vm.set_register(13, ptr_amount);
    vm.run().unwrap();

    // Build a minimal State and apply setup ISIs (register domain/accounts/asset, mint initial balance)
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
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

    // Setup: register domain 'wonderland', register accounts, asset definition, and mint 1_000 to 'from'
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let new_domain = Domain::new(domain_id.clone());
    let reg_domain = RegisterBox::from(Register::domain(new_domain));
    let reg_from = RegisterBox::from(Register::account(
        NewAccount::new(from.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_to = RegisterBox::from(Register::account(
        NewAccount::new(to.clone()).with_linked_domain(domain_id.clone()),
    ));
    let new_asset_def =
        AssetDefinition::numeric(asset_def.clone()).with_name(asset_def.name().to_string());
    let reg_asset_def = RegisterBox::from(Register::asset_definition(new_asset_def));
    let mint = MintBox::from(Mint::asset_numeric(
        1000u64,
        AssetId::of(asset_def.clone(), from.clone()),
    ));

    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_domain),
        InstructionBox::from(reg_from),
        InstructionBox::from(reg_to),
        InstructionBox::from(reg_asset_def),
        InstructionBox::from(mint),
    ] {
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("setup should succeed");
    }

    // Apply queued transfer via CoreHost bridge
    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1);
    let executor = tx.world.executor().clone();
    for instr in queued {
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("apply queued instruction");
    }
    tx.apply();
    block.commit().expect("commit block");

    // Assert balances updated: from decreased by 500, to increased by 500
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
    assert_eq!(from_bal, 500u32.into());
    assert_eq!(to_bal, 500u32.into());
}

#[test]
#[ignore = "debug helper for live encoded account ids and opaque asset definitions"]
fn apply_queued_isis_from_corehost_transfer_asset_with_env_encoded_ids() {
    let from_raw =
        std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");
    let to_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");
    let asset_raw = std::env::var("IVM_DEBUG_ASSET_DEFINITION")
        .expect("IVM_DEBUG_ASSET_DEFINITION must be set");
    let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "sbp".to_owned());

    let from = iroha_data_model::account::AccountId::parse_encoded(&from_raw)
        .expect("valid encoded source account")
        .into_account_id();
    let to = iroha_data_model::account::AccountId::parse_encoded(&to_raw)
        .expect("valid encoded destination account")
        .into_account_id();
    let asset_def: AssetDefinitionId = asset_raw.parse().expect("valid asset definition");
    let amount = Numeric::from(500_u64);

    let from_bytes = tlv_envelope(PointerType::AccountId, &from);
    let to_bytes = tlv_envelope(PointerType::AccountId, &to);
    let asset_bytes = tlv_envelope(PointerType::AssetDefinitionId, &asset_def);
    let amount_bytes = tlv_envelope(PointerType::NoritoBytes, &amount);
    let align8 = |n: u64| (n + 7) & !7;
    let off_from = 0u64;
    let off_to = align8(off_from + from_bytes.len() as u64);
    let off_asset = align8(off_to + to_bytes.len() as u64);
    let off_amount = align8(off_asset + asset_bytes.len() as u64);
    let ptr_from = ivm::Memory::INPUT_START + off_from;
    let ptr_to = ivm::Memory::INPUT_START + off_to;
    let ptr_asset = ivm::Memory::INPUT_START + off_asset;
    let ptr_amount = ivm::Memory::INPUT_START + off_amount;

    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_TRANSFER_ASSET).unwrap(),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 10_000,
        abi_version: 1,
    };
    let mut program = meta.encode();
    program.extend_from_slice(&code);

    let mut vm = IVM::new(500_000);
    vm.set_host(CoreHost::new(from.clone()));
    vm.memory
        .preload_input(off_from, &from_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_to, &to_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_asset, &asset_bytes)
        .expect("preload input");
    vm.memory
        .preload_input(off_amount, &amount_bytes)
        .expect("preload input");
    vm.load_program(&program).unwrap();
    vm.set_register(10, ptr_from);
    vm.set_register(11, ptr_to);
    vm.set_register(12, ptr_asset);
    vm.set_register(13, ptr_amount);
    vm.run().unwrap();

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
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

    let domain_id: DomainId = domain_raw.parse().expect("domain id");
    let new_domain = Domain::new(domain_id.clone());
    let reg_domain = RegisterBox::from(Register::domain(new_domain));
    let reg_from = RegisterBox::from(Register::account(
        NewAccount::new(from.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_to = RegisterBox::from(Register::account(
        NewAccount::new(to.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_asset_def = RegisterBox::from(Register::asset_definition(
        AssetDefinition::numeric(asset_def.clone()).with_name(asset_def.to_string()),
    ));
    let mint = MintBox::from(Mint::asset_numeric(
        1000u64,
        AssetId::of(asset_def.clone(), from.clone()),
    ));

    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_domain),
        InstructionBox::from(reg_from),
        InstructionBox::from(reg_to),
        InstructionBox::from(reg_asset_def),
        InstructionBox::from(mint),
    ] {
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("setup should succeed");
    }

    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 1);
    let executor = tx.world.executor().clone();
    for instr in queued {
        executor
            .execute_instruction(&mut tx, &from, instr)
            .expect("apply queued instruction");
    }
}

#[test]
#[ignore = "debug helper for live trigger-style JSON-driven transfers"]
fn apply_queued_isis_from_compiled_json_driven_double_transfer() {
    let reserve_raw =
        std::env::var("IVM_DEBUG_FROM_ACCOUNT").expect("IVM_DEBUG_FROM_ACCOUNT must be set");
    let dst_raw = std::env::var("IVM_DEBUG_TO_ACCOUNT").expect("IVM_DEBUG_TO_ACCOUNT must be set");
    let aed_asset_raw = std::env::var("IVM_DEBUG_AED_ASSET_DEFINITION").unwrap_or_else(|_| {
        opaque_asset_definition_literal([
            0x68, 0x72, 0x45, 0x4e, 0x9c, 0x04, 0x46, 0x41, 0xaa, 0x58, 0x1e, 0xc5, 0xf3, 0x80,
            0x16, 0x19,
        ])
    });
    let pkr_asset_raw = std::env::var("IVM_DEBUG_PKR_ASSET_DEFINITION").unwrap_or_else(|_| {
        opaque_asset_definition_literal([
            0x2e, 0x3d, 0x34, 0xbe, 0xb8, 0xa8, 0x42, 0x39, 0xb3, 0xd9, 0x59, 0x07, 0x70, 0xf1,
            0x18, 0x9e,
        ])
    });
    let domain_raw = std::env::var("IVM_DEBUG_DOMAIN").unwrap_or_else(|_| "sbp".to_owned());
    let ratio_raw = std::env::var("IVM_DEBUG_RATIO").unwrap_or_else(|_| "76".to_owned());

    let reserve = iroha_data_model::account::AccountId::parse_encoded(&reserve_raw)
        .expect("valid encoded reserve account")
        .into_account_id();
    let dst = iroha_data_model::account::AccountId::parse_encoded(&dst_raw)
        .expect("valid encoded destination account")
        .into_account_id();
    let aed_asset_def: AssetDefinitionId = aed_asset_raw.parse().expect("valid AED asset");
    let pkr_asset_def: AssetDefinitionId = pkr_asset_raw.parse().expect("valid PKR asset");
    let domain_id: DomainId = domain_raw.parse().expect("valid domain");
    let ratio: u64 = ratio_raw.parse().expect("valid ratio");

    let authority =
        fixture_account("ed0120CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC");
    let compiler = Compiler::new();
    let src = format!(
        r#"
        fn main() {{
            let ev = json("{{\"kind\":\"asset_change\",\"op\":\"added\",\"asset_definition_id\":\"{aed}\",\"account_domain\":\"{domain}\",\"account_id\":\"{dst}\",\"amount_i64\":1}}");
            let recipient = json_get_account_id(ev, name("account_id"));
            let amount = json_get_int(ev, name("amount_i64"));
            transfer_asset(recipient, account_id("{reserve}"), asset_definition("{aed}"), amount);
            transfer_asset(account_id("{reserve}"), recipient, asset_definition("{pkr}"), amount * {ratio});
        }}
        "#,
        aed = aed_asset_raw,
        domain = domain_raw,
        dst = dst_raw,
        reserve = reserve_raw,
        pkr = pkr_asset_raw,
        ratio = ratio,
    );
    let program = compiler.compile_source(&src).expect("compile");

    let mut vm = IVM::new(500_000);
    vm.set_host(CoreHost::new(authority.clone()));
    vm.load_program(&program).expect("load");
    vm.run().expect("run");

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
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

    let reg_domain = RegisterBox::from(Register::domain(Domain::new(domain_id.clone())));
    let reg_authority = RegisterBox::from(Register::account(
        NewAccount::new(authority.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_reserve = RegisterBox::from(Register::account(
        NewAccount::new(reserve.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_dst = RegisterBox::from(Register::account(
        NewAccount::new(dst.clone()).with_linked_domain(domain_id.clone()),
    ));
    let reg_aed = RegisterBox::from(Register::asset_definition(
        AssetDefinition::numeric(aed_asset_def.clone()).with_name("aed".to_owned()),
    ));
    let reg_pkr = RegisterBox::from(Register::asset_definition(
        AssetDefinition::numeric(pkr_asset_def.clone()).with_name("pkr".to_owned()),
    ));
    let mint_aed = MintBox::from(Mint::asset_numeric(
        1u64,
        AssetId::of(aed_asset_def.clone(), dst.clone()),
    ));
    let mint_pkr = MintBox::from(Mint::asset_numeric(
        ratio * 2,
        AssetId::of(pkr_asset_def.clone(), reserve.clone()),
    ));

    let executor = tx.world.executor().clone();
    for instr in [
        InstructionBox::from(reg_domain),
        InstructionBox::from(reg_authority),
        InstructionBox::from(reg_reserve),
        InstructionBox::from(reg_dst),
        InstructionBox::from(reg_aed),
        InstructionBox::from(reg_pkr),
        InstructionBox::from(mint_aed),
        InstructionBox::from(mint_pkr),
    ] {
        executor
            .execute_instruction(&mut tx, &authority, instr)
            .expect("setup should succeed");
    }

    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions);
    assert_eq!(queued.len(), 2);
    for instr in queued {
        executor
            .execute_instruction(&mut tx, &authority, instr)
            .expect("apply queued instruction");
    }

    let dst_aed_bal = tx
        .world
        .assets()
        .get(&AssetId::of(aed_asset_def, dst.clone()))
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    let dst_pkr_bal = tx
        .world
        .assets()
        .get(&AssetId::of(pkr_asset_def, dst))
        .map_or_else(|| Numeric::from(0u32), |v| v.clone().into_inner());
    assert_eq!(dst_aed_bal, 0u32.into());
    assert_eq!(dst_pkr_bal, Numeric::from(ratio));
}
