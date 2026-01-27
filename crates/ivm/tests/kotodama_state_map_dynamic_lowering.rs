//! Verify dynamic durable lowering for state Map<int,int>: uses path_map_key and encode_int.

use std::str::FromStr;

use iroha_data_model::prelude::Name;
use ivm::{
    CoreHost, IVM, PointerType, encoding, instruction,
    kotodama::compiler::Compiler as KotodamaCompiler, syscalls,
};
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
fn dynamic_map_set_uses_durable_state() {
    let src = r#"
        seiyaku C {
            state Map<int, int> M;
            fn main() {
                let k = 2;
                let v = 5;
                M[k] = v;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    vm.run().expect("run");

    // Verify durable state via STATE_GET("M/2") == "5"
    let path = Name::from_str("M/2").expect("valid path");
    let path_tlv = make_tlv(PointerType::Name, path.as_ref().as_bytes());
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let mut get_prog = Vec::with_capacity(8);
    get_prog.extend_from_slice(
        &encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            syscalls::SYSCALL_STATE_GET as u8,
        )
        .to_le_bytes(),
    );
    get_prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let get_prog = common::assemble(&get_prog);
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get");
    vm.run().expect("state get");
    let p_out = vm.register(10);
    let tlv = vm.memory.validate_tlv(p_out).expect("validate out");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(tlv.payload, b"5");
}
