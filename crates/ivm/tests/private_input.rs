use ivm::{IVM, host::DefaultHost, syscalls};
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
