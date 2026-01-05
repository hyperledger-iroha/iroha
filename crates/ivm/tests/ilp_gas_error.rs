//! ILP execution checks for gas accounting and deterministic error ordering.

use ivm::{IVM, Instruction, VMError};

#[test]
fn ilp_charges_gas_in_program_order() {
    let mut vm = IVM::new(1);
    let block = [
        Instruction::AddImm {
            rd: 1,
            rs: 0,
            imm: 5,
        },
        Instruction::AddImm {
            rd: 2,
            rs: 0,
            imm: 7,
        },
    ];
    let err = vm
        .execute_block_parallel(&block)
        .expect_err("expected out-of-gas");
    assert_eq!(err, VMError::OutOfGas);
    assert_eq!(vm.register(1), 5);
    assert_eq!(vm.register(2), 0);
}

#[test]
fn ilp_reports_first_error_in_index_order() {
    let mut vm = IVM::new(u64::MAX);
    vm.zk_mode = true;
    vm.registers.set_tag(1, true);

    let block = [
        Instruction::Vadd {
            rd: 1,
            rs: 2,
            rt: 3,
        },
        Instruction::Add {
            rd: 4,
            rs: 1,
            rt: 2,
        },
    ];

    let err = vm
        .execute_block_parallel(&block)
        .expect_err("expected ilp error");
    assert_eq!(err, VMError::VectorExtensionDisabled);
}
