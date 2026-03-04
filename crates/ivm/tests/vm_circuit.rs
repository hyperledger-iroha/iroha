#![cfg(feature = "ivm_zk_tests")]
use ivm::{IVM, encoding, halo2::VMExecutionCircuit, instruction, kotodama::wide as kwide};
mod common;
use common::assemble_zk;

#[test]
fn vm_execution_circuit_pass() {
    let addi_r1 = kwide::encode_addi(1, 0, 5).to_le_bytes();
    let beq = kwide::encode_branch_checked(instruction::wide::control::BEQ, 1, 1, 2)
        .expect("branch")
        .to_le_bytes();
    let addi_r2 = kwide::encode_addi(2, 0, 2).to_le_bytes(); // skipped
    let add_r3 = kwide::encode_addi(3, 1, 5).to_le_bytes(); // r3 = 10
    let halt = encoding::wide::encode_halt().to_le_bytes();

    let mut code = Vec::new();
    code.extend_from_slice(&addi_r1);
    code.extend_from_slice(&beq);
    code.extend_from_slice(&addi_r2);
    code.extend_from_slice(&add_r3);
    code.extend_from_slice(&halt);
    let prog = assemble_zk(&code, 10);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    let trace = vm.register_trace();
    let circuit = VMExecutionCircuit::new(&prog, &trace, vm.constraints());
    let res = circuit.verify();
    assert!(res.is_ok(), "{:?}", res);
}

#[test]
fn vm_execution_circuit_fail() {
    let addi_r1 = kwide::encode_addi(1, 0, 5).to_le_bytes();
    let beq = kwide::encode_branch_checked(instruction::wide::control::BEQ, 1, 1, 2)
        .expect("branch")
        .to_le_bytes();
    let addi_r2 = kwide::encode_addi(2, 0, 2).to_le_bytes();
    let add_r3 = kwide::encode_addi(3, 1, 5).to_le_bytes();
    let halt = encoding::wide::encode_halt().to_le_bytes();

    let mut code = Vec::new();
    code.extend_from_slice(&addi_r1);
    code.extend_from_slice(&beq);
    code.extend_from_slice(&addi_r2);
    code.extend_from_slice(&add_r3);
    code.extend_from_slice(&halt);
    let prog = assemble_zk(&code, 10);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let mut trace = vm.register_trace();
    trace[2].gpr[2] = trace[2].gpr[2].wrapping_add(1); // corrupt
    let circuit = VMExecutionCircuit::new(&prog, &trace, vm.constraints());
    assert!(circuit.verify().is_err());
}
