//! ABI v2 scaffold test
//!
//! This placeholder asserts that ABI hashing is stable for existing policies.
//! When introducing `SyscallPolicy::V2`, extend this test to check separation
//! from V1 and update goldens per docs in `docs/syscalls.md`.

#[test]
fn abi_hash_is_stable_for_v1() {
    use ivm::syscalls::compute_abi_hash;
    let h1 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(h1, h2);
}
