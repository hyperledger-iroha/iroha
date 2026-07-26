use ivm::{IVM, VMError, encoding};

mod common;
use common::assemble;

#[test]
fn classic_opcode_is_rejected() {
    // Classic RISC-V ADD encoding (rd=x0, rs1=x0, rs2=x0, funct3=0, funct7=0, opcode=0x33).
    let add_rv = 0x0000_0033u32;
    let halt = encoding::wide::encode_halt();
    let mut bytes = Vec::new();
    for word in [add_rv, halt] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    let program = assemble(&bytes);

    let mut vm = IVM::new(1_000);
    let err = vm
        .load_program(&program)
        .expect_err("classic opcode must be rejected");
    assert!(matches!(err, VMError::InvalidOpcode(0x0033)));
}

#[test]
fn classic_opcode_runtime_rejected_after_manual_load() {
    // Write a classic ADD instruction directly into memory to bypass loader validation.
    let add_rv = 0x0000_0033u32;
    let halt = encoding::wide::encode_halt();
    let mut bytes = Vec::new();
    for word in [add_rv, halt] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }

    let mut vm = IVM::new(1_000);
    vm.memory.load_code(&bytes);
    vm.pc = 0;
    assert_eq!(vm.memory.load_u32(0).expect("load"), add_rv);

    let err = vm
        .run()
        .expect_err("classic opcode must trap during execution");
    assert!(matches!(err, VMError::InvalidOpcode(0x0033)));
}
