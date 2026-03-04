use ivm::{IVM, IVMHost, SyscallPolicy};

#[test]
fn unknown_syscalls_rejected_by_policy_function() {
    // Pick a number outside known ABI list
    let unknown = 0xFFFF_FFFFu32;
    assert!(!ivm::syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        unknown
    ));
}

#[test]
fn default_host_rejects_unknown_syscalls() {
    let mut vm = IVM::new(0);
    let mut host = ivm::host::DefaultHost::new();
    let res = host.syscall(0xFFFF, &mut vm);
    assert!(matches!(res, Err(ivm::VMError::UnknownSyscall(_))));
}
