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
        "1daac4a4c98904194fb294638d2d62c9b35c658b31d319f0e92b10d0ce9b7883"
    );
}
