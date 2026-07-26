//! Ensure every syscall in the ABI list has a symbolic name mapping.

#[test]
fn every_abi_syscall_has_a_name() {
    let list = ivm::syscalls::abi_syscall_list();
    for &n in list {
        assert!(
            ivm::syscalls::syscall_name(n).is_some(),
            "missing name mapping for syscall 0x{n:02x}"
        );
    }
}
