//! CoreHost roundtrip harness for IVM syscalls.

use ivm::{CoreHost, IVM, Memory, PointerType, syscalls};

mod common;

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

#[test]
fn host_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let path_tlv = make_tlv(PointerType::Name, b"roundtrip_key");
    let value = vec![0xA5, 0x5A, 0x01];
    let value_tlv = make_tlv(PointerType::NoritoBytes, &value);
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let p_val = vm.alloc_input_tlv(&value_tlv).expect("alloc value");

    let set_prog = common::assemble_syscalls(&[syscalls::SYSCALL_STATE_SET as u8]);
    vm.set_register(10, p_path);
    vm.set_register(11, p_val);
    vm.load_program(&set_prog).expect("load set");
    vm.run().expect("state set");

    let get_prog = common::assemble_syscalls(&[syscalls::SYSCALL_STATE_GET as u8]);
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get");
    vm.run().expect("state get");
    let p_out = vm.register(10);
    assert!((Memory::INPUT_START..Memory::INPUT_START + Memory::INPUT_SIZE).contains(&p_out));
    let tlv = vm.memory.validate_tlv(p_out).expect("validate output");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(tlv.payload, &value[..]);
}
