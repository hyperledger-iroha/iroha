//! Type-mismatch validation for `CoreHost` pointer-ABI decoding.

use iroha_core::smartcontracts::ivm::host::CoreHost;
use iroha_data_model::prelude::*;
use ivm::{IVM, Memory, PointerType, ProgramMetadata, encoding, instruction, syscalls as ivm_sys};
use norito::codec::Encode as NoritoEncode;

fn program_scall(sys: u32) -> Vec<u8> {
    let mut code = Vec::new();
    let scall = instruction::wide::system::SCALL;
    let sys_u8 = u8::try_from(sys).unwrap();
    code.extend_from_slice(&encoding::wide::encode_sys(scall, sys_u8).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 10_000,
        abi_version: 1,
    };
    let mut out = meta.encode();
    out.extend_from_slice(&code);
    out
}

fn tlv_envelope(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    blob.extend_from_slice(&type_id.to_be_bytes());
    blob.push(1u8);
    blob.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    blob.extend_from_slice(payload);
    let h = iroha_crypto::Hash::new(&blob[7..7 + payload.len()]);
    blob.extend_from_slice(h.as_ref());
    blob
}

#[test]
fn wrong_type_for_asset_def_rejected() {
    // Transfer asset expects (&AccountId, &AccountId, &AssetDefinitionId, amount)
    let program = program_scall(ivm_sys::SYSCALL_TRANSFER_ASSET);
    let mut vm = IVM::new(u64::MAX);
    let authority: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonder"
            .parse()
            .unwrap();
    vm.set_host(CoreHost::new(authority.clone()));

    let from = authority.encode();
    let to: AccountId =
        "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonder"
            .parse()
            .unwrap();
    let to = to.encode();
    // Wrong type: Name TLV instead of AssetDefinitionId
    let wrong: Name = "not_an_asset".parse().unwrap();
    let wrong = wrong.encode();
    let tlv_from = tlv_envelope(PointerType::AccountId as u16, &from);
    let tlv_to = tlv_envelope(PointerType::AccountId as u16, &to);
    let tlv_wrong = tlv_envelope(PointerType::Name as u16, &wrong);

    let align8 = |n: u64| (n + 7) & !7;
    let off_from = 0u64;
    let off_to = align8(off_from + tlv_from.len() as u64);
    let off_wrong = align8(off_to + tlv_to.len() as u64);

    vm.memory
        .preload_input(off_from, &tlv_from)
        .expect("preload input");
    vm.memory
        .preload_input(off_to, &tlv_to)
        .expect("preload input");
    vm.memory
        .preload_input(off_wrong, &tlv_wrong)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START + off_from);
    vm.set_register(11, Memory::INPUT_START + off_to);
    vm.set_register(12, Memory::INPUT_START + off_wrong);
    vm.set_register(13, 1);
    vm.load_program(&program).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn wrong_type_for_set_account_detail_value_rejected() {
    let program = program_scall(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL);
    let mut vm = IVM::new(u64::MAX);
    let authority: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonder"
            .parse()
            .unwrap();
    vm.set_host(CoreHost::new(authority.clone()));
    let acc = tlv_envelope(PointerType::AccountId as u16, &authority.encode());
    let key: Name = "cursor".parse().unwrap();
    let key = tlv_envelope(PointerType::Name as u16, &key.encode());
    // Wrong type: AssetDefinitionId TLV where Json is expected
    let wrong = tlv_envelope(
        PointerType::AssetDefinitionId as u16,
        &"coin#wonder".parse::<AssetDefinitionId>().unwrap().encode(),
    );
    let align8 = |n: u64| (n + 7) & !7;
    let off_acc = 0u64;
    let off_key = align8(off_acc + acc.len() as u64);
    let off_wrong = align8(off_key + key.len() as u64);
    vm.memory
        .preload_input(off_acc, &acc)
        .expect("preload input");
    vm.memory
        .preload_input(off_key, &key)
        .expect("preload input");
    vm.memory
        .preload_input(off_wrong, &wrong)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START + off_acc);
    vm.set_register(11, Memory::INPUT_START + off_key);
    vm.set_register(12, Memory::INPUT_START + off_wrong);
    vm.load_program(&program).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}
