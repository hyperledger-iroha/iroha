use ivm::{self, PointerType, SyscallPolicy};

#[test]
fn syscall_policy_allows_known_and_rejects_unknown_for_v1() {
    // Known allowed syscall in the canonical surface
    assert!(ivm::syscalls::abi_syscall_list().contains(&ivm::syscalls::SYSCALL_EXIT));
    assert!(ivm::syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        ivm::syscalls::SYSCALL_EXIT
    ));

    // A number not present in the canonical surface (0xFC) is disallowed
    let unknown = 0xFC;
    assert!(!ivm::syscalls::abi_syscall_list().contains(&unknown));
    assert!(!ivm::syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        unknown
    ));
}

#[test]
fn pointer_type_policy_allows_asset_id_under_abi_v1() {
    assert!(ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::AssetId
    ));
}
