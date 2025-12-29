use ivm::{IVM, encoding, instruction};
mod common;
use common::assemble_zk;

#[test]
fn test_register_events_logged() {
    // Program: store 0x11 at heap, load it back
    let store = encoding::wide::encode_store(instruction::wide::memory::STORE64, 1, 2, 0);
    let load = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 3, 1, 0);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&store.to_le_bytes());
    bytes.extend_from_slice(&load.to_le_bytes());
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, ivm::Memory::HEAP_START);
    vm.set_register(2, 0x11);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    let log = vm.register_log();
    assert!(!log.is_empty());
    for e in log {
        match e {
            ivm::RegEvent::Read { path, root, .. } | ivm::RegEvent::Write { path, root, .. } => {
                assert!(!path.is_empty());
                assert_ne!(*root.as_ref(), [0u8; 32]);
            }
        }
    }
    // Step log should match cycle count
    let steps = vm.step_log();
    assert_eq!(steps.len() as u64, vm.get_cycle_count());
}
