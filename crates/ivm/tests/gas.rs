use ivm::{IVM, VMError, encoding, instruction, kotodama::wide};
mod common;
use common::{assemble, assemble_zk};

const HALT: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();

#[test]
fn test_out_of_gas() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let mut prog = Vec::new();
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(0); // not enough for even the first instruction
    let res = vm.run();
    assert!(
        matches!(res, Err(VMError::OutOfGas)),
        "Expected OutOfGas error"
    );
}

#[test]
fn test_exact_gas_limit() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let mut prog = Vec::new();
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(2); // enough for ADD and NOP padding if any
    vm.run().expect("VM should succeed with exact gas limit");
    assert_eq!(vm.register(3), 3);
}

#[test]
fn test_gas_accounting_multiple_ops() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let mut prog = Vec::new();
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(5);
    vm.run().expect("execution failed");
    assert_eq!(vm.remaining_gas(), 3);
}

#[test]
fn test_getgas_instruction() {
    let mut vm = IVM::new(u64::MAX);
    let mut prog = Vec::new();
    prog.extend_from_slice(
        &encoding::wide::encode_rr(instruction::wide::system::GETGAS, 1, 0, 0).to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(10);
    vm.run().expect("execution failed");
    assert_eq!(vm.register(1), 10);
    assert_eq!(vm.remaining_gas(), 10); // HALT no longer consumes gas
}

#[test]
fn test_getgas_progress() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let mut prog = Vec::new();
    // add r3 = r1 + r2
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    // GETGAS r4
    prog.extend_from_slice(
        &encoding::wide::encode_rr(instruction::wide::system::GETGAS, 4, 0, 0).to_le_bytes(),
    );
    // add r5 = r1 + r2
    prog.extend_from_slice(&wide::encode_add(5, 1, 2).to_le_bytes());
    // GETGAS r6
    prog.extend_from_slice(
        &encoding::wide::encode_rr(instruction::wide::system::GETGAS, 6, 0, 0).to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(100);
    vm.run().expect("execution failed");
    assert_eq!(vm.register(4), 99);
    assert_eq!(vm.register(6), 98);
    assert_eq!(vm.remaining_gas(), 98);
}

#[test]
fn test_getgas_zero_remaining() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    let mut prog = Vec::new();
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(&wide::encode_add(3, 1, 2).to_le_bytes());
    prog.extend_from_slice(
        &encoding::wide::encode_rr(instruction::wide::system::GETGAS, 3, 0, 0).to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(2);
    vm.run().expect("execution failed");
    assert_eq!(vm.register(3), 0);
    assert_eq!(vm.remaining_gas(), 0);
}

struct ExtraCostHost;

impl ivm::IVMHost for ExtraCostHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        assert_eq!(number, 1);
        let a0 = vm.register(10);
        vm.set_register(10, a0.wrapping_add(1));
        Ok(2) // additional gas cost
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn downcast_extra_cost_host() {
    let mut host: Box<dyn ivm::IVMHost> = Box::new(ExtraCostHost);
    assert!(host.as_any().downcast_mut::<ExtraCostHost>().is_some());
}

#[test]
fn test_syscall_additional_gas() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(ExtraCostHost);
    vm.set_register(10, 5);
    let mut prog = Vec::new();
    prog.extend_from_slice(
        &encoding::wide::encode_sys(instruction::wide::system::SCALL, 1).to_le_bytes(),
    );
    prog.extend_from_slice(&HALT);
    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(7); // 5 base + 2 extra
    vm.run().expect("syscall should succeed");
    assert_eq!(vm.register(10), 6);
    assert_eq!(vm.remaining_gas(), 0);
}

#[test]
fn test_zk_padding_consumes_gas() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_max_cycles(5);
    let halt_prog: [u8; 4] = HALT;
    let prog = assemble_zk(&halt_prog, 5);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(10);
    vm.run().expect("execution failed");
    // One HALT (0 gas) plus four padding cycles at cost 1 each
    assert_eq!(vm.remaining_gas(), 6);
}

#[test]
fn test_zk_padding_out_of_gas() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_max_cycles(5);
    let halt_prog: [u8; 4] = HALT;
    let prog = assemble_zk(&halt_prog, 5);
    vm.load_program(&prog).unwrap();
    vm.set_gas_limit(3); // less than 4 padding cycles
    let res = vm.run();
    assert!(matches!(res, Err(VMError::OutOfGas)));
    // Padding attempts set the cycle count to MAX_CYCLES even on failure
    assert_eq!(vm.get_cycle_count(), 5);
}
