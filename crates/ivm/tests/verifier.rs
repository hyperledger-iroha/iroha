//! Retained diagnostic trace checks over a real ZK-mode execution.
use crate::common;
use common::assemble_zk;
use ivm::{IVM, encoding, instruction, zk::check_diagnostic_trace};
#[test]
fn test_diagnostic_trace_check_passes() {
    // Program: ADDI x1 = x0 + 7; HALT
    let addi_inst = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 0, 7);
    let halt_inst = encoding::wide::encode_halt();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&addi_inst.to_le_bytes());
    bytes.extend_from_slice(&halt_inst.to_le_bytes());
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    vm.set_zk_trace_enabled(true);
    let res = vm.run();
    assert!(res.is_ok());
    let snapshot = common::diagnostic_snapshot(&vm);
    assert!(
        snapshot.register_event_count() > 0,
        "the register write must leave an authenticated event to check"
    );
    check_diagnostic_trace(&snapshot).unwrap();
}
