//! ABI hash tests ensure the hash is stable for the same policy.

use ivm::syscalls::compute_abi_hash;

#[test]
fn abi_hash_is_stable() {
    let h1 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(h1, h2, "ABI hash must be stable for the same policy");
}

#[test]
fn abi_hash_matches_v1_golden() {
    let hash = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(
        hex::encode(hash),
        "7851f2523c084a1a96f5c9fa7eae5258550b38839c3d1475a826c748ed6fbaa3"
    );
}
