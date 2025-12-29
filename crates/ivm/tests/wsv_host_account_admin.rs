use std::collections::HashMap;

use iroha_crypto::Hash;
use ivm::mock_wsv::DomainId;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AccountId, MockWorldStateView, PermissionToken, WsvHost},
    syscalls,
};
use norito::codec::Encode as NoritoEncode;

mod common;
use common::assemble_syscalls;

fn make_tlv(type_id: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&(type_id as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_domain_tlv(name: &str) -> Vec<u8> {
    let domain: DomainId = name.parse().expect("valid domain id");
    let payload = domain.encode();
    make_tlv(PointerType::DomainId, &payload)
}

fn make_account_tlv(account: &AccountId) -> Vec<u8> {
    let payload = account.encode();
    make_tlv(PointerType::AccountId, &payload)
}

fn load_and_run(vm: &mut IVM, program: &[u8]) -> Result<(), ivm::VMError> {
    vm.load_program(program)?;
    vm.run()
}

#[test]
fn add_and_remove_signatory_for_own_account() {
    let alice: AccountId =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@domain"
            .parse()
            .unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    wsv.grant_permission(&alice, PermissionToken::RegisterAccount);

    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(256);
    vm.set_host(host);

    // Register domain and account for alice
    let dom = make_domain_tlv("domain");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let reg_domain = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_DOMAIN as u8]);
    load_and_run(&mut vm, &reg_domain).expect("register domain");

    let acct = make_account_tlv(&alice);
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let reg_account = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ACCOUNT as u8]);
    load_and_run(&mut vm, &reg_account).expect("register account");

    // Add a signatory via JSON payload
    let sig_json = make_tlv(PointerType::Json, br#"{"public_key":"ed0120CAFEBABE"}"#);
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.memory
        .preload_input(acct.len() as u64 + 8, &sig_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acct.len() as u64 + 8);
    let add_sig = assemble_syscalls(&[syscalls::SYSCALL_ADD_SIGNATORY as u8]);
    load_and_run(&mut vm, &add_sig).expect("add signatory");

    // Adding the same signatory twice should fail
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.memory
        .preload_input(acct.len() as u64 + 8, &sig_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acct.len() as u64 + 8);
    assert!(matches!(
        load_and_run(&mut vm, &add_sig),
        Err(ivm::VMError::PermissionDenied)
    ));

    // Remove the signatory
    let remove_sig = assemble_syscalls(&[syscalls::SYSCALL_REMOVE_SIGNATORY as u8]);
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.memory
        .preload_input(acct.len() as u64 + 8, &sig_json)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acct.len() as u64 + 8);
    load_and_run(&mut vm, &remove_sig).expect("remove signatory");

    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_mut::<WsvHost>().expect("WsvHost");
    let signs = host
        .wsv
        .account_signatories(&alice)
        .expect("account present");
    assert!(signs.is_empty());
}

#[test]
fn set_quorum_and_detail_with_permissions() {
    let alice: AccountId =
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774@domain"
            .parse()
            .unwrap();
    let bob: AccountId =
        "ed0120C6C6F575510FB87360CB773FAF2665C9BD0FBD00320684A966569A2C0217F063@wonder"
            .parse()
            .unwrap();

    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    wsv.grant_permission(&alice, PermissionToken::RegisterAccount);

    let host = WsvHost::new(wsv, alice.clone(), HashMap::new(), HashMap::new());
    let mut vm = IVM::new(256);
    vm.set_host(host);

    // Register domain wonder and bob account
    let dom = make_domain_tlv("wonder");
    vm.memory.preload_input(0, &dom).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let reg_domain = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_DOMAIN as u8]);
    load_and_run(&mut vm, &reg_domain).expect("register domain");

    let acct = make_account_tlv(&bob);
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    let reg_account = assemble_syscalls(&[syscalls::SYSCALL_REGISTER_ACCOUNT as u8]);
    load_and_run(&mut vm, &reg_account).expect("register account");

    // Try setting quorum without extra permission → should fail
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, 3);
    let set_quorum = assemble_syscalls(&[syscalls::SYSCALL_SET_ACCOUNT_QUORUM as u8]);
    assert!(matches!(
        load_and_run(&mut vm, &set_quorum),
        Err(ivm::VMError::PermissionDenied)
    ));

    // Grant permissions and retry
    {
        let host_any = vm.host_mut_any().unwrap();
        let host = host_any.downcast_mut::<WsvHost>().expect("WsvHost");
        host.wsv
            .grant_permission(&alice, PermissionToken::SetAccountQuorum(bob.clone()));
        host.wsv
            .grant_permission(&alice, PermissionToken::SetAccountDetail(bob.clone()));
    }

    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, 4);
    load_and_run(&mut vm, &set_quorum).expect("set quorum");

    // Set account detail with JSON value (whitespace should be stripped)
    let key = make_tlv(PointerType::Name, b"greeting");
    let detail = make_tlv(
        PointerType::Json,
        br#"{ "msg" : "hello" }"#, // intentionally non-minified
    );
    vm.memory.preload_input(0, &acct).expect("preload input");
    vm.memory
        .preload_input(acct.len() as u64 + 8, &key)
        .expect("preload input");
    vm.memory
        .preload_input(acct.len() as u64 + key.len() as u64 + 16, &detail)
        .expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    vm.set_register(11, Memory::INPUT_START + acct.len() as u64 + 8);
    vm.set_register(
        12,
        Memory::INPUT_START + acct.len() as u64 + key.len() as u64 + 16,
    );
    let set_detail = assemble_syscalls(&[syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8]);
    load_and_run(&mut vm, &set_detail).expect("set detail");

    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_mut::<WsvHost>().expect("WsvHost");
    assert_eq!(host.wsv.account_quorum(&bob), Some(4));
    let stored = host
        .wsv
        .account_detail_value(&bob, "greeting")
        .expect("detail stored");
    assert_eq!(stored, br#"{"msg":"hello"}"#);
}
