//! CoreHost: build path from base Name and int key via SYSCALL_BUILD_PATH_MAP_KEY.

use iroha_data_model::prelude::Name;
use ivm::{CoreHost, IVM, PointerType, encoding, syscalls};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

#[test]
fn core_host_build_path_map_key() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let base_tlv = make_tlv(PointerType::Name, b"M");
    let p_base = vm.alloc_input_tlv(&base_tlv).expect("alloc base");
    // Program: publish base; move key to r11; SCALL BUILD_PATH_MAP_KEY; HALT
    let mut code = Vec::new();
    // publish r10
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
        )
        .to_le_bytes(),
    );
    // jal-like: not needed here
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_BUILD_PATH_MAP_KEY as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = common::assemble(&code);
    vm.set_register(10, p_base);
    vm.set_register(11, 42);
    vm.load_program(&prog).expect("load");
    vm.run().expect("run");
    let p_path = vm.register(10);
    let tlv = vm.memory.validate_tlv(p_path).expect("validate");
    assert_eq!(tlv.type_id, PointerType::Name);
    let name: Name = norito::decode_from_bytes(tlv.payload).expect("decode name");
    assert_eq!(name.as_ref(), "M/42");
}
