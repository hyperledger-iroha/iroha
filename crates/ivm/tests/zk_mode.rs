use crate::common;
use common::{assemble, assemble_zk};
use ivm::{IVM, VMError, encoding, zk::MAX_CYCLES};
#[test]
fn test_zk_mode_padding_and_abort() {
    // Program that fails the run through the ABORT syscall.
    let abort_instr = encoding::wide::encode_scall(ivm::syscalls::SYSCALL_ABORT);
    let halt = encoding::wide::encode_halt().to_le_bytes();
    let mut abort_prog = Vec::with_capacity(8);
    abort_prog.extend_from_slice(&abort_instr.to_le_bytes());
    abort_prog.extend_from_slice(&halt);
    // Case 1: Normal mode (zk_mode = false) -> the abort stops the run without padding.
    let mut vm = IVM::new(u64::MAX);
    let prog = assemble(&abort_prog);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(
        matches!(res, Err(VMError::AssertionFailed)),
        "Abort should fail the run in normal mode"
    );
    // Only the aborting syscall executed.
    assert_eq!(vm.get_cycle_count(), 1);
    // Case 2: ZK mode (zk_mode = true) -> should pad to MAX_CYCLES and then error
    let mut vm2 = IVM::new(u64::MAX);
    let prog = assemble_zk(&abort_prog, MAX_CYCLES);
    vm2.load_program(&prog).unwrap();
    let res2 = vm2.run();
    assert!(
        matches!(res2, Err(VMError::AssertionFailed)),
        "Abort should still produce error in ZK mode"
    );
    // Execution should be padded to MAX_CYCLES cycles
    assert_eq!(vm2.get_cycle_count(), MAX_CYCLES);
    // Case 3: ZK mode with no abort (normal halt) -> should still pad cycles and return Ok
    let mut vm3 = IVM::new(u64::MAX);
    let halt_prog: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();
    let prog = assemble_zk(&halt_prog, MAX_CYCLES);
    vm3.load_program(&prog).unwrap();
    let res3 = vm3.run();
    assert!(res3.is_ok(), "Normal halt should succeed in ZK mode");
    assert_eq!(
        vm3.get_cycle_count(),
        MAX_CYCLES,
        "Cycles should be padded to MAX_CYCLES on normal halt in ZK mode"
    );
}
#[test]
fn ordinary_execution_is_bounded_by_gas_not_cycles() {
    // Outside ZK mode there is no cycle bound: an endless loop ends when gas runs out.
    let prog: [u8; 4] =
        encoding::wide::encode_offset24(ivm::instruction::wide::control::JMP, 0).to_le_bytes();
    let mut vm = IVM::new(1_000);
    vm.load_program(&assemble(&prog)).unwrap();
    assert!(matches!(vm.run(), Err(VMError::OutOfGas)));
    // Each JMP costs two gas.
    assert_eq!(vm.get_cycle_count(), 500);
}
#[test]
fn test_exceed_max_cycles_error() {
    // Infinite loop using JMP with zero offset
    let prog: [u8; 4] =
        encoding::wide::encode_offset24(ivm::instruction::wide::control::JMP, 0).to_le_bytes();
    let mut vm = IVM::new(u64::MAX);
    vm.set_max_cycles(10);
    let prog = assemble_zk(&prog, 10);
    vm.load_program(&prog).unwrap();
    // Provide ample gas for the configured cycle limit and any padding
    vm.set_gas_limit(MAX_CYCLES + 10);
    let res = vm.run();
    assert!(matches!(res, Err(VMError::ExceededMaxCycles)));
    assert_eq!(vm.get_cycle_count(), 10);
}
#[test]
fn test_zero_max_cycles_means_no_padding() {
    // Program header sets max_cycles=0 which disables ZK padding.
    let halt_prog: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();
    let prog = assemble(&halt_prog);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(10);
    vm.run().expect("execution failed");
    // Without a cycle limit there should be no padding.
    assert_eq!(vm.get_cycle_count(), 1);
}
#[test]
fn test_program_exceeding_old_cycle_limit() {
    // Loop for >65k cycles but <MAX_CYCLES to confirm higher limit works.
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 0);
    let iterations: u64 = 35_000; // ~70k cycles with two instructions per loop
    vm.set_register(2, iterations);
    let addi =
        encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 1, 1, 1).to_le_bytes();
    let blt =
        encoding::wide::encode_branch(ivm::instruction::wide::control::BLT, 1, 2, -1).to_le_bytes();
    let mut code = Vec::new();
    code.extend_from_slice(&addi);
    code.extend_from_slice(&blt);
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = assemble_zk(&code, MAX_CYCLES);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(MAX_CYCLES + 10);
    vm.run().expect("execution failed");
    assert_eq!(vm.register(1), iterations);
    assert_eq!(vm.get_cycle_count(), MAX_CYCLES);
}
