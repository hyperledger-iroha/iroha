use ivm::{IVM, encoding, field, instruction};
mod common;
use common::assemble_zk;

const HALT: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();

#[test]
fn test_field_arithmetic() {
    let mut vm = IVM::new(u64::MAX);
    let base = u64::MAX - 2;
    vm.set_register(1, base); // large value
    vm.set_register(2, 5);
    let mut prog = Vec::new();
    let fadd = encoding::wide::encode_rr(instruction::wide::zk::FADD, 3, 1, 2);
    prog.extend_from_slice(&fadd.to_le_bytes());
    let fsub = encoding::wide::encode_rr(instruction::wide::zk::FSUB, 4, 3, 2);
    prog.extend_from_slice(&fsub.to_le_bytes());
    let fmul = encoding::wide::encode_rr(instruction::wide::zk::FMUL, 5, 4, 2);
    prog.extend_from_slice(&fmul.to_le_bytes());
    let finv = encoding::wide::encode_rr(instruction::wide::zk::FINV, 6, 2, 0);
    prog.extend_from_slice(&finv.to_le_bytes());
    prog.extend_from_slice(&HALT); // HALT
    let prog = assemble_zk(&prog, 32);
    vm.load_program(&prog).unwrap();
    vm.run().expect("field ops");
    let expected_add = field::add(base, 5);
    assert_eq!(vm.register(3), expected_add);
    let expected_sub = field::sub(expected_add, 5);
    assert_eq!(vm.register(4), expected_sub);
    let expected_mul = field::mul(expected_sub, 5);
    assert_eq!(vm.register(5), expected_mul);
    let inv = field::inv(5).unwrap();
    assert_eq!(vm.register(6), inv);
}

#[test]
fn test_assert_range() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 15);
    // ASSERT_RANGE x1, 4  -> should pass
    let instr = encoding::wide::encode_ri(instruction::wide::zk::ASSERT_RANGE, 0, 1, 4);
    let mut prog = Vec::new();
    prog.extend_from_slice(&instr.to_le_bytes());
    prog.extend_from_slice(&HALT);
    let prog = assemble_zk(&prog, 16);
    vm.load_program(&prog).unwrap();
    vm.run().expect("range pass");
    // failing case
    let mut vm2 = IVM::new(u64::MAX);
    vm2.set_register(1, 16);
    let mut prog2 = Vec::new();
    prog2.extend_from_slice(&instr.to_le_bytes());
    prog2.extend_from_slice(&HALT);
    let prog2 = assemble_zk(&prog2, 16);
    vm2.load_program(&prog2).unwrap();
    let res = vm2.run();
    assert!(matches!(res, Err(ivm::VMError::AssertionFailed)));
}
