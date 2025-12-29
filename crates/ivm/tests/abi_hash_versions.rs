//! ABI hash versioning tests ensure that different ABI policies produce
//! different hashes and that the hash is stable for the same policy.

use ivm::syscalls::compute_abi_hash;

#[test]
fn abi_hash_differs_between_stable_and_experimental() {
    let stable = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let experimental = compute_abi_hash(ivm::SyscallPolicy::Experimental(1));
    assert_ne!(
        stable, experimental,
        "stable and experimental ABI hashes must differ"
    );
}

// No V2 in first release

#[test]
fn abi_hash_is_stable() {
    let h1 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(h1, h2, "ABI hash must be stable for the same policy");
}
