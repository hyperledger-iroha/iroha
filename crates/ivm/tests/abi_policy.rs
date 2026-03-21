use ivm::{self, PointerType, SyscallPolicy};

#[test]
fn syscall_policy_allows_known_and_rejects_unknown_for_v1() {
    // Known allowed syscall in the canonical surface
    assert!(ivm::syscalls::abi_syscall_list().contains(
        &ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE
    ));
    assert!(ivm::syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE
    ));

    // Pick a number not present in the canonical surface.
    let list = ivm::syscalls::abi_syscall_list();
    let unknown = list
        .windows(2)
        .find_map(|w| {
            let candidate = w[0].saturating_add(1);
            if candidate < w[1] {
                Some(candidate)
            } else {
                None
            }
        })
        .unwrap_or_else(|| list.last().copied().unwrap_or(0).saturating_add(1));
    assert!(!ivm::syscalls::abi_syscall_list().contains(&unknown));
    assert!(!ivm::syscalls::is_syscall_allowed(
        SyscallPolicy::AbiV1,
        unknown
    ));
}

#[test]
fn pointer_type_policy_allows_soracloud_response_under_abi_v1() {
    assert!(ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::SoracloudResponse
    ));
}
