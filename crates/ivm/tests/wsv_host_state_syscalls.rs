//! WsvHost durable state syscalls: STATE_GET/SET/DEL with pointer-ABI.

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_crypto::PublicKey;
use ivm::{
    IVM, Memory, PointerType,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};
mod common;
use common::assemble_syscalls;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn account(_domain: &str, public_key: &str) -> AccountId {
    let public_key: PublicKey = public_key.parse().expect("public key");
    AccountId::new(public_key)
}

#[test]
fn wsv_host_state_set_get_del_roundtrip() {
    let wsv = MockWorldStateView::new();
    let caller = account(
        "wonderland",
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let mut vm = IVM::new(u64::MAX);
    let host = WsvHost::new_with_subject(wsv, caller.clone(), Default::default());
    vm.set_host(host);

    let path_tlv = make_tlv(PointerType::Name, b"bar");
    let val1 = vec![9u8, 8, 7];
    let val1_tlv = make_tlv(PointerType::NoritoBytes, &val1);
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let p_val1 = vm.alloc_input_tlv(&val1_tlv).expect("alloc val");

    // SET
    let set_prog = assemble_syscalls(&[syscalls::SYSCALL_STATE_SET as u8]);
    vm.set_register(10, p_path);
    vm.set_register(11, p_val1);
    vm.load_program(&set_prog).expect("load set");
    vm.run().expect("state set");

    // GET
    let get_prog = assemble_syscalls(&[syscalls::SYSCALL_STATE_GET as u8]);
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get");
    vm.run().expect("state get");
    let p_out = vm.register(10);
    assert!(p_out >= Memory::INPUT_START);
    let tlv = vm.memory.validate_tlv(p_out).expect("validate out");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(tlv.payload, &val1[..]);

    // DEL
    let del_prog = assemble_syscalls(&[syscalls::SYSCALL_STATE_DEL as u8]);
    vm.set_register(10, p_path);
    vm.load_program(&del_prog).expect("load del");
    vm.run().expect("state del");

    // GET -> 0
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get again");
    vm.run().expect("state get again");
    assert_eq!(vm.register(10), 0);
}

#[test]
fn durable_state_overlay_persists_and_restores() {
    let base = std::env::temp_dir().join(format!(
        "ivm_state_overlay_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_millis()
    ));
    fs::create_dir_all(&base).expect("create temp dir");
    let store_path = base.join("state.json");

    let mut wsv = MockWorldStateView::with_state_store(store_path.clone()).expect("persisted wsv");
    let tlv = make_tlv(PointerType::NoritoBytes, b"abc");
    wsv.sc_set("counter", tlv.clone()).expect("set state");
    assert_eq!(wsv.sc_get("counter"), Some(tlv.clone()));

    drop(wsv);
    let wsv_reloaded =
        MockWorldStateView::with_state_store(store_path.clone()).expect("reload persisted");
    assert_eq!(wsv_reloaded.sc_get("counter"), Some(tlv.clone()));

    let mut wsv_mut = wsv_reloaded;
    let snap = wsv_mut.sc_snapshot();
    let tlv_new = make_tlv(PointerType::NoritoBytes, b"new");
    wsv_mut
        .sc_set("counter", tlv_new.clone())
        .expect("set newer value");
    assert_eq!(wsv_mut.sc_get("counter"), Some(tlv_new.clone()));
    wsv_mut.sc_restore(&snap).expect("restore snapshot");
    assert_eq!(wsv_mut.sc_get("counter"), Some(tlv.clone()));
    drop(wsv_mut);

    let persisted =
        MockWorldStateView::with_state_store(store_path.clone()).expect("reload after restore");
    assert_eq!(persisted.sc_get("counter"), Some(tlv));
    let _ = fs::remove_dir_all(base);
}
