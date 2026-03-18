use ivm::{IVM, encoding};
mod common;
use common::assemble_zk;

#[test]
fn test_register_trace_length() {
    // Program consisting of a single HALT
    let prog_bytes: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();
    let prog = assemble_zk(&prog_bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    // cycles should equal max_cycles (8)
    assert_eq!(vm.get_cycle_count(), 8);
    // trace should contain one entry per cycle
    assert_eq!(vm.register_trace().len() as u64, vm.get_cycle_count());
}
