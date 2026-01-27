use std::collections::HashMap;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AccountId, AssetDefinitionId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};
use norito::to_bytes;

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_numeric_tlv(amount: impl Into<Numeric>) -> Vec<u8> {
    let buf = to_bytes(&amount.into()).expect("encode numeric into Norito");
    make_tlv(PointerType::NoritoBytes as u16, &buf)
}

fn alloc_numeric(vm: &mut IVM, amount: u64) -> u64 {
    let tlv = make_numeric_tlv(amount);
    vm.alloc_input_tlv(&tlv).expect("alloc numeric tlv")
}

#[test]
fn unregister_flow_with_dependencies() {
    let alice: AccountId =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@domain"
            .parse()
            .unwrap();
    let bob: AccountId =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4@wonder"
            .parse()
            .unwrap();
    let rose: AssetDefinitionId = "rose#wonder".parse().unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    // Grant required permissions
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    wsv.grant_permission(&alice, PermissionToken::RegisterAccount);
    wsv.grant_permission(&alice, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&alice, PermissionToken::MintAsset(rose.clone()));
    wsv.grant_permission(&alice, PermissionToken::BurnAsset(rose.clone()));

    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Register domain wonder
    let dom = make_tlv(PointerType::DomainId as u16, b"wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_dom = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_DOMAIN as u8]);
    vm.load_program(&prog_dom).unwrap();
    vm.run().expect("register domain");

    // Register account bob@wonder
    let acc = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    vm.memory.preload_input(0, &acc).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_acc = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ACCOUNT as u8]);
    vm.load_program(&prog_acc).unwrap();
    vm.run().expect("register account");

    // Register asset def rose#wonder
    let ad = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &ad).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_ad = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ASSET as u8]);
    vm.load_program(&prog_ad).unwrap();
    vm.run().expect("register asset def");

    // Mint 7 units of rose to bob
    let tlv_bob = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    let tlv_rose = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &tlv_bob).expect("preload input");
    vm.memory
        .preload_input(tlv_bob.len() as u64 + 8, &tlv_rose)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_bob.len() as u64 + 8);
    let mint_amount = alloc_numeric(&mut vm, 7);
    vm.set_register(12, mint_amount);
    let prog_mint = assemble_syscalls(&[syscalls::SYSCALL_MINT_ASSET as u8]);
    vm.load_program(&prog_mint).unwrap();
    vm.run().expect("mint asset");

    // Attempt to unregister domain now -> should fail due to dependencies
    let dom = make_tlv(PointerType::DomainId as u16, b"wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_udom = assemble_syscalls(&[syscalls::SYSCALL_UNREGISTER_DOMAIN as u8]);
    vm.load_program(&prog_udom).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));

    // Attempt to unregister asset def -> should fail (balances exist)
    let ad = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &ad).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_uad = assemble_syscalls(&[syscalls::SYSCALL_UNREGISTER_ASSET as u8]);
    vm.load_program(&prog_uad).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));

    // Burn 7 units from bob to clear balances
    let tlv_bob = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    let tlv_rose = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &tlv_bob).expect("preload input");
    vm.memory
        .preload_input(tlv_bob.len() as u64 + 8, &tlv_rose)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_bob.len() as u64 + 8);
    let burn_amount = alloc_numeric(&mut vm, 7);
    vm.set_register(12, burn_amount);
    let prog_burn = assemble_syscalls(&[syscalls::SYSCALL_BURN_ASSET as u8]);
    vm.load_program(&prog_burn).unwrap();
    vm.run().expect("burn asset");

    // Now unregister asset def -> success
    let ad = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &ad).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.load_program(&prog_uad).unwrap();
    vm.run().expect("unregister asset def");

    // Unregister account bob -> success
    let acc = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    vm.memory.preload_input(0, &acc).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_uacc = assemble_syscalls(&[syscalls::SYSCALL_UNREGISTER_ACCOUNT as u8]);
    vm.load_program(&prog_uacc).unwrap();
    vm.run().expect("unregister account");

    // Finally, unregister domain -> success
    let dom = make_tlv(PointerType::DomainId as u16, b"wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.load_program(&prog_udom).unwrap();
    vm.run().expect("unregister domain");
}
