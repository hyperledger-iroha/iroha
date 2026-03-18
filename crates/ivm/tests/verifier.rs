use ivm::{IVM, encoding, instruction, zk::verify_trace};
mod common;
use common::assemble_zk;

#[test]
fn test_verify_trace_pass() {
    // Program: ASSERT x1==0; HALT
    let assert_inst = encoding::wide::encode_rr(instruction::wide::zk::ASSERT, 0, 1, 0);
    let halt_inst = encoding::wide::encode_halt();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&assert_inst.to_le_bytes());
    bytes.extend_from_slice(&halt_inst.to_le_bytes());
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 0);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    let trace = vm.register_trace();
    verify_trace(&trace, vm.constraints(), vm.memory_log(), vm.register_log()).unwrap();
}

#[test]
fn test_verify_trace_fail() {
    // ASSERT on non-zero register should fail verification
    let assert_inst = encoding::wide::encode_rr(instruction::wide::zk::ASSERT, 0, 1, 0);
    let halt_inst = encoding::wide::encode_halt();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&assert_inst.to_le_bytes());
    bytes.extend_from_slice(&halt_inst.to_le_bytes());
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1); // will trigger assertion
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(res.is_err());
    let trace = vm.register_trace();
    let verify = verify_trace(&trace, vm.constraints(), vm.memory_log(), vm.register_log());
    assert!(verify.is_err());
}
