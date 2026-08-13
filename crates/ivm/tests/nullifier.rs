//! Admission regression for the retired invocation-local scalar nullifier helper.
mod common;
use common::assemble_syscalls;
use ivm::{
    IVM, SyscallPolicy, VMError,
    host::{DefaultHost, IVMHost, registered_host_syscall_gas_formula},
    syscalls,
};
#[test]
fn invocation_local_u64_nullifier_is_rejected_by_abi_v1() {
    assert!(!syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        syscalls::SYSCALL_USE_NULLIFIER
    ));
    assert_eq!(
        syscalls::registered_syscall_access(syscalls::SYSCALL_USE_NULLIFIER),
        None
    );
    assert_eq!(
        registered_host_syscall_gas_formula(syscalls::SYSCALL_USE_NULLIFIER),
        None
    );
    assert_eq!(
        syscalls::syscall_name(syscalls::SYSCALL_USE_NULLIFIER),
        None
    );
    let mut direct_host = DefaultHost::new();
    let mut direct_vm = IVM::new(u64::MAX);
    assert_eq!(
        direct_host.prepare_syscall(syscalls::SYSCALL_USE_NULLIFIER, &direct_vm),
        Err(VMError::UnknownSyscall(syscalls::SYSCALL_USE_NULLIFIER))
    );
    assert_eq!(
        direct_host.syscall(syscalls::SYSCALL_USE_NULLIFIER, &mut direct_vm),
        Err(VMError::UnknownSyscall(syscalls::SYSCALL_USE_NULLIFIER))
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(DefaultHost::new());
    let program = assemble_syscalls(&[syscalls::SYSCALL_USE_NULLIFIER as u8]);
    if let Err(error) = vm.load_program(&program) {
        assert_eq!(
            error,
            VMError::UnknownSyscall(syscalls::SYSCALL_USE_NULLIFIER)
        );
        return;
    }
    vm.set_register(10, 123);
    assert_eq!(
        vm.run(),
        Err(VMError::UnknownSyscall(syscalls::SYSCALL_USE_NULLIFIER))
    );
}
