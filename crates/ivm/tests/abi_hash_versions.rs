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
        "1e4acdf5a13da87857a721c7a259562ec63e0295de1ea7e76793a2ca2fb0c6ef"
    );
}
