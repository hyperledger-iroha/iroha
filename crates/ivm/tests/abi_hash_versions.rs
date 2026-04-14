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
        "919d5cdca5efe4c668165b8f2c737c136db0bf399ef7c33e24a95808b1c692bb"
    );
}
