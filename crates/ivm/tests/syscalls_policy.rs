use ivm::{self, SyscallPolicy};

#[test]
fn baseline_policy_allows_known_surface_numbers() {
    use ivm::syscalls::*;
    // Spot-check a mix of core helpers and WSV-bridged calls that must remain available
    let allowed = [
        SYSCALL_ALLOC,
        SYSCALL_GROW_HEAP,
        SYSCALL_GET_PUBLIC_INPUT,
        SYSCALL_GET_PRIVATE_INPUT,
        SYSCALL_COMMIT_OUTPUT,
        SYSCALL_PROVE_EXECUTION,
        SYSCALL_VERIFY_PROOF,
        SYSCALL_USE_NULLIFIER,
        SYSCALL_GET_MERKLE_PATH,
        SYSCALL_SORACLOUD_READ_COMMITTED_STATE,
        SYSCALL_SORACLOUD_EGRESS_FETCH,
        // WSV helpers
        SYSCALL_SET_ACCOUNT_DETAIL,
        SYSCALL_MINT_ASSET,
        SYSCALL_TRANSFER_ASSET,
        SYSCALL_NFT_MINT_ASSET,
        SYSCALL_NFT_TRANSFER_ASSET,
    ];
    for &num in &allowed {
        assert!(
            ivm::syscalls::is_syscall_allowed(SyscallPolicy::AbiV1, num),
            "num=0x{num:x}"
        );
    }
}

#[test]
fn unknown_numbers_rejected() {
    let nums = [0x80u32, 0x98u32, 0xEEu32];
    for &n in &nums {
        assert!(!ivm::syscalls::is_syscall_allowed(SyscallPolicy::AbiV1, n));
    }
}

#[test]
fn abi_hash_is_stable_for_policy() {
    use ivm::syscalls::compute_abi_hash;
    let h1 = compute_abi_hash(SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(SyscallPolicy::AbiV1);
    assert_eq!(h1, h2, "hash must be stable across calls");
}
