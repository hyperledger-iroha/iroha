//! WsvHost: INPUT_PUBLISH_TLV rejects invalid pointer-ABI envelopes.

use ivm::{
    IVM, VMError,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};

mod common;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&type_id.to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    v.extend_from_slice(&h);
    v
}

fn wsv_host() -> WsvHost {
    let wsv = MockWorldStateView::new();
    let caller: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .expect("caller id");
    WsvHost::new(wsv, caller, Default::default(), Default::default())
}

#[test]
fn wsv_host_input_publish_rejects_unknown_type_id() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());
    let tlv = make_tlv(0x00FF, b"oops");
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8]);
    vm.load_program(&prog).expect("load");
    vm.set_register(10, ptr);
    let err = vm
        .run()
        .expect_err("unknown pointer type should be rejected");
    assert!(matches!(err, VMError::NoritoInvalid));
}
