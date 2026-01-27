//! CoreHost: decode Norito-framed i64 from NoritoBytes via SYSCALL_DECODE_INT.

use ivm::{CoreHost, IVM, PointerType, encoding, syscalls};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    v.extend_from_slice(&h);
    v
}

#[test]
fn core_host_decode_int_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    assert!(vm.host_mut_any().expect("host configured").is::<CoreHost>());
    let payload = norito::to_bytes(&12345_i64).expect("encode i64");
    let tlv = make_tlv(PointerType::NoritoBytes, &payload);
    let p = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    // Program: SCALL DECODE_INT; HALT
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_DECODE_INT as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = common::assemble(&code);
    vm.load_program(&prog).expect("load");
    vm.set_register(10, p);
    vm.run().expect("run");
    assert_eq!(vm.register(10), 12345);
}

#[test]
fn core_host_decode_int_rejects_blob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let tlv = make_tlv(PointerType::Blob, b"-42");
    let p = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            syscalls::SYSCALL_DECODE_INT as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = common::assemble(&code);
    vm.load_program(&prog).expect("load");
    vm.set_register(10, p);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}
