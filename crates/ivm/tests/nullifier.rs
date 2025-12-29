use ivm::{IVM, VMError, host::DefaultHost, syscalls};
mod common;
use common::assemble_syscalls;

#[test]
fn test_use_nullifier_once() {
    let host = DefaultHost::new();
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    let prog = assemble_syscalls(&[syscalls::SYSCALL_USE_NULLIFIER as u8]);
    vm.load_program(&prog).unwrap();
    vm.set_register(10, 123);
    vm.run().unwrap();
}

#[test]
fn test_duplicate_nullifier_fails() {
    let host = DefaultHost::new();
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    let prog = assemble_syscalls(&[
        syscalls::SYSCALL_USE_NULLIFIER as u8,
        syscalls::SYSCALL_USE_NULLIFIER as u8,
    ]);
    vm.load_program(&prog).unwrap();
    vm.set_register(10, 42);
    let res = vm.run();
    assert!(matches!(res, Err(VMError::NullifierAlreadyUsed)));
}
