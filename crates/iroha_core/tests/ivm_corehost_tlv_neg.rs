//! Host-level negative tests for typed TLV decoding via `CoreHost`.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::smartcontracts::ivm::host::CoreHost;
use iroha_data_model::prelude::*;
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, IVMHost, Memory, PointerType, ProgramMetadata, syscalls};

fn build_tlv(type_id: u16, version: u8, payload: &[u8], corrupt_hash: bool) -> Vec<u8> {
    use iroha_crypto::Hash;
    let mut v = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    v.extend_from_slice(&type_id.to_be_bytes());
    v.push(version);
    v.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    v.extend_from_slice(payload);
    let h = Hash::new(payload);
    let mut hb = h.as_ref().to_vec();
    if corrupt_hash {
        hb[0] ^= 0xFF;
    }
    v.extend_from_slice(&hb);
    v
}

#[test]
fn mint_asset_rejects_assetid_tlv_instead_of_assetdefinitionid() {
    // Authority and host
    let authority: AccountId = ALICE_ID.clone();
    let mut host = CoreHost::new(authority.clone());
    // Minimal VM with header
    let mut vm = IVM::new(0);
    let header = ProgramMetadata::default().encode();
    vm.load_program(&header).unwrap();

    // Build valid AccountId TLV for r10
    let acct_payload = norito::to_bytes(&authority).expect("encode account");
    let acct_tlv = build_tlv(0x0001, 1, &acct_payload, false);
    vm.memory
        .preload_input(0, &acct_tlv)
        .expect("preload input");
    let p_acct = Memory::INPUT_START;

    // Build AssetId TLV for r11 where AssetDefinitionId is expected (type mismatch)
    let asset_id: AssetId = AssetId::of(
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        authority.clone(),
    );
    let asset_id_payload = norito::to_bytes(&asset_id).expect("encode asset id");
    let assetid_tlv = build_tlv(0x0007, 1, &asset_id_payload, false);
    let off = (acct_tlv.len() as u64 + 7) & !7; // align a bit
    vm.memory
        .preload_input(off, &assetid_tlv)
        .expect("preload input");
    let p_asset = Memory::INPUT_START + off;
    let amount_payload = norito::to_bytes(&Numeric::from(42_u64)).expect("encode amount");
    let amount_tlv = build_tlv(PointerType::NoritoBytes as u16, 1, &amount_payload, false);
    let off_amount = (off + assetid_tlv.len() as u64 + 7) & !7;
    vm.memory
        .preload_input(off_amount, &amount_tlv)
        .expect("preload input");
    let p_amount = Memory::INPUT_START + off_amount;

    // r12 = amount
    vm.set_register(10, p_acct);
    vm.set_register(11, p_asset);
    vm.set_register(12, p_amount);
    let res = host.syscall(syscalls::SYSCALL_MINT_ASSET, &mut vm);
    assert!(matches!(res, Err(ivm::VMError::NoritoInvalid)));
}

#[test]
fn mint_asset_rejects_corrupted_accountid_hash() {
    let authority: AccountId = ALICE_ID.clone();
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(0);
    let header = ProgramMetadata::default().encode();
    vm.load_program(&header).unwrap();

    // Corrupted AccountId TLV for r10
    let acct_payload = norito::to_bytes(&authority).expect("encode account");
    let acct_tlv_bad = build_tlv(0x0001, 1, &acct_payload, true);
    vm.memory
        .preload_input(0, &acct_tlv_bad)
        .expect("preload input");
    let p_acct = Memory::INPUT_START;

    // Valid AssetDefinitionId for r11
    let asset_def = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let assetdef_payload = norito::to_bytes(&asset_def).expect("encode asset definition");
    let assetdef_tlv = build_tlv(0x0002, 1, &assetdef_payload, false);
    let off = 64;
    vm.memory
        .preload_input(off, &assetdef_tlv)
        .expect("preload input");
    let p_assetdef = Memory::INPUT_START + off;
    let amount_payload = norito::to_bytes(&Numeric::from(100_u64)).expect("encode amount");
    let amount_tlv = build_tlv(PointerType::NoritoBytes as u16, 1, &amount_payload, false);
    let off_amount = (off + assetdef_tlv.len() as u64 + 7) & !7;
    vm.memory
        .preload_input(off_amount, &amount_tlv)
        .expect("preload input");
    let p_amount = Memory::INPUT_START + off_amount;

    vm.set_register(10, p_acct);
    vm.set_register(11, p_assetdef);
    vm.set_register(12, p_amount);
    let res = host.syscall(syscalls::SYSCALL_MINT_ASSET, &mut vm);
    assert!(matches!(res, Err(ivm::VMError::NoritoInvalid)));
}

#[test]
fn mint_asset_rejects_unknown_typeid() {
    let authority: AccountId = ALICE_ID.clone();
    let mut host = CoreHost::new(authority.clone());
    let mut vm = IVM::new(0);
    let header = ProgramMetadata::default().encode();
    vm.load_program(&header).unwrap();

    // Valid AccountId TLV for r10
    let acct_payload = norito::to_bytes(&authority).expect("encode account");
    let acct_tlv = build_tlv(0x0001, 1, &acct_payload, false);
    vm.memory
        .preload_input(0, &acct_tlv)
        .expect("preload input");
    let p_acct = Memory::INPUT_START;

    // Unknown type id (e.g., 0x00AA) for r11
    let payload = norito::to_bytes(&AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    ))
    .expect("encode asset definition");
    let bad_tlv = build_tlv(0x00AA, 1, &payload, false);
    let off = 128;
    vm.memory
        .preload_input(off, &bad_tlv)
        .expect("preload input");
    let p_bad = Memory::INPUT_START + off;
    let amount_payload = norito::to_bytes(&Numeric::from(1_u64)).expect("encode amount");
    let amount_tlv = build_tlv(PointerType::NoritoBytes as u16, 1, &amount_payload, false);
    let off_amount = (off + bad_tlv.len() as u64 + 7) & !7;
    vm.memory
        .preload_input(off_amount, &amount_tlv)
        .expect("preload input");
    let p_amount = Memory::INPUT_START + off_amount;

    vm.set_register(10, p_acct);
    vm.set_register(11, p_bad);
    vm.set_register(12, p_amount);
    let res = host.syscall(syscalls::SYSCALL_MINT_ASSET, &mut vm);
    assert!(matches!(res, Err(ivm::VMError::NoritoInvalid)));
}
