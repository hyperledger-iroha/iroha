//! ZK syscall policy tests: ensure ABI surface includes ZK numbers and rejects unknown.

#[test]
fn zk_syscalls_present_and_unknown_rejected() {
    use ivm::{SyscallPolicy, syscalls};

    // Ensure ZK syscalls are part of the canonical ABI surface
    let abi = ivm::syscalls::abi_syscall_list();
    for &n in &[
        syscalls::SYSCALL_ZK_VERIFY_TRANSFER,
        syscalls::SYSCALL_ZK_VERIFY_UNSHIELD,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT,
        syscalls::SYSCALL_ZK_VOTE_VERIFY_TALLY,
        syscalls::SYSCALL_ZK_ROOTS_GET,
        syscalls::SYSCALL_ZK_VOTE_GET_TALLY,
    ] {
        assert!(abi.contains(&n), "missing ZK syscall 0x{n:02x} in ABI list");
        assert!(ivm::syscalls::is_syscall_allowed(SyscallPolicy::AbiV1, n));
    }

    // Unknown number should not be allowed by policy (and host must return UnknownSyscall)
    let unknown = 0xABu32;
    assert!(
        !ivm::syscalls::is_syscall_allowed(SyscallPolicy::AbiV1, unknown),
        "unknown syscall must be disallowed by policy"
    );
}
