use std::collections::HashMap;

use iroha_crypto::Hash;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AccountId, AssetDefinitionId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};

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

#[test]
fn delete_role_with_assignees_fails() {
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
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    wsv.grant_permission(&alice, PermissionToken::RegisterAccount);
    wsv.grant_permission(&alice, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Register domain, account, and asset definition
    let dom = make_tlv(PointerType::DomainId as u16, b"wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_dom = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_DOMAIN as u8]);
    vm.load_program(&prog_dom).unwrap();
    vm.run().expect("register domain");

    let acc = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    vm.memory.preload_input(0, &acc).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_acc = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ACCOUNT as u8]);
    vm.load_program(&prog_acc).unwrap();
    vm.run().expect("register account");

    let ad = make_tlv(
        PointerType::AssetDefinitionId as u16,
        rose.to_string().as_bytes(),
    );
    vm.memory.preload_input(0, &ad).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_ad = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ASSET as u8]);
    vm.load_program(&prog_ad).unwrap();
    vm.run().expect("register asset def");

    // Create role and grant to alice
    let role = make_tlv(PointerType::Name as u16, b"minter");
    let json = format!("{{\"perms\":[\"mint_asset:{rose}\"]}}");
    let perms = make_tlv(PointerType::Json as u16, json.as_bytes());
    vm.memory.preload_input(0, &role).expect("preload input");
    vm.memory
        .preload_input(role.len() as u64 + 8, &perms)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + role.len() as u64 + 8);
    let prog_crole = assemble_syscalls(&[syscalls::SYSCALL_CREATE_ROLE as u8]);
    vm.load_program(&prog_crole).unwrap();
    vm.run().expect("create role");

    let tlv_alice = make_tlv(PointerType::AccountId as u16, alice.to_string().as_bytes());
    let rname = make_tlv(PointerType::Name as u16, b"minter");
    vm.memory
        .preload_input(0, &tlv_alice)
        .expect("preload input");
    vm.memory
        .preload_input(tlv_alice.len() as u64 + 8, &rname)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_alice.len() as u64 + 8);
    let prog_grole = assemble_syscalls(&[syscalls::SYSCALL_GRANT_ROLE as u8]);
    vm.load_program(&prog_grole).unwrap();
    vm.run().expect("grant role");

    // Attempt to delete role while assigned -> fail
    vm.memory.preload_input(0, &rname).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_drole = assemble_syscalls(&[syscalls::SYSCALL_DELETE_ROLE as u8]);
    vm.load_program(&prog_drole).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));
}

#[test]
fn grant_nonexistent_role_fails() {
    let alice: AccountId =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@domain"
            .parse()
            .unwrap();
    let bob: AccountId =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4@wonder"
            .parse()
            .unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    wsv.grant_permission(&alice, PermissionToken::RegisterAccount);
    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Register domain and bob
    let dom = make_tlv(PointerType::DomainId as u16, b"wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_dom = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_DOMAIN as u8]);
    vm.load_program(&prog_dom).unwrap();
    vm.run().expect("register domain");

    let acc = make_tlv(PointerType::AccountId as u16, bob.to_string().as_bytes());
    vm.memory.preload_input(0, &acc).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let prog_acc = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ACCOUNT as u8]);
    vm.load_program(&prog_acc).unwrap();
    vm.run().expect("register account");

    // Grant a role that does not exist
    let tlv_alice = make_tlv(PointerType::AccountId as u16, alice.to_string().as_bytes());
    let rname = make_tlv(PointerType::Name as u16, b"ghost");
    vm.memory
        .preload_input(0, &tlv_alice)
        .expect("preload input");
    vm.memory
        .preload_input(tlv_alice.len() as u64 + 8, &rname)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + tlv_alice.len() as u64 + 8);
    let prog_grole = assemble_syscalls(&[syscalls::SYSCALL_GRANT_ROLE as u8]);
    vm.load_program(&prog_grole).unwrap();
    assert!(matches!(vm.run(), Err(ivm::VMError::PermissionDenied)));
}
