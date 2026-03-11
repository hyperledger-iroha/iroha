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
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
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

#[test]
fn apply_queued_isis_from_corehost_transfer_asset() {
    // Build a minimal IVM program that performs SCALL TRANSFER_ASSET and HALT
    let from =
        fixture_account("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let to =
        fixture_account("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    let asset_def: AssetDefinitionId = "coin#wonderland".parse().unwrap();
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
    let reg_from = RegisterBox::from(Register::account(NewAccount::new_in_domain(
        from.clone(),
        domain_id.clone(),
    )));
    let reg_to = RegisterBox::from(Register::account(NewAccount::new_in_domain(
        to.clone(),
        domain_id.clone(),
    )));
    let new_asset_def = AssetDefinition::numeric(asset_def.clone());
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
