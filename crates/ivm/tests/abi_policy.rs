use ivm::{self, PointerType, SyscallPolicy};
#[test]
fn syscall_policy_allows_known_and_rejects_unknown_for_v1() {
    // Known allowed syscall in the canonical surface
    assert!(
        ivm::syscalls::abi_syscall_list()
            .contains(&ivm::syscalls::SYSCALL_SORACLOUD_READ_COMMITTED_STATE)
    );
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
fn first_release_compiler_runtime_syscalls_are_ungated_in_abi_v1() {
    for number in [
        ivm::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD,
        ivm::syscalls::SYSCALL_STATE_MAP_KEY_AT,
        ivm::syscalls::SYSCALL_STATE_VALUE_ENCODE,
        ivm::syscalls::SYSCALL_STATE_VALUE_DECODE,
        ivm::syscalls::SYSCALL_STATE_PATH_FROM_NAME,
        ivm::syscalls::SYSCALL_NORMALIZE_NORITO_BYTES,
    ] {
        assert!(
            ivm::syscalls::is_syscall_allowed(SyscallPolicy::AbiV1, number),
            "first-release syscall 0x{number:06x} must be available in ABI v1"
        );
    }
}
#[test]
fn pointer_type_policy_allows_soracloud_response_under_abi_v1() {
    assert!(ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::SoracloudResponse
    ));
}
