use ivm::{IVM, VMError, host::DefaultHost, syscalls};
mod common;
use common::assemble_syscalls;

#[test]
fn test_private_input_syscall() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.set_register(10, 0); // index 0
    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_PRIVATE_INPUT as u8]);
    vm.load_program(&prog).unwrap();
    vm.run().expect("syscall failed");
    assert_eq!(vm.register(10), 42);
}

#[test]
fn private_input_index_rejects_high_bits_without_usize_truncation() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    // This aliases index zero if a protocol register is narrowed to a 32-bit
    // usize with `as`.
    vm.set_register(10, u64::from(u32::MAX) + 1);
    let prog = assemble_syscalls(&[syscalls::SYSCALL_GET_PRIVATE_INPUT as u8]);
    vm.load_program(&prog).expect("load program");

    assert_eq!(vm.run(), Err(VMError::PermissionDenied));
}
