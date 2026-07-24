//! ABI hash tests ensure the hash is stable for the same policy.

use ivm::syscalls::compute_abi_hash;

const ABI_V1_HASH_GOLDEN: &str = "38823fb30c83cbf4a47f8898de1a7d7ec153a14f2b76c3d0b3461c620083b6b3";

#[test]
fn abi_hash_is_stable() {
    let h1 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(h1, h2, "ABI hash must be stable for the same policy");
}

#[test]
fn abi_hash_matches_v1_golden() {
    let hash = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(hex::encode(hash), ABI_V1_HASH_GOLDEN);
}
