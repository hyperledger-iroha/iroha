//! ABI hash tests ensure the hash is stable for the same policy.
use ivm::syscalls::compute_abi_hash;
const ABI_V1_HASH_GOLDEN: &str = "0ce37cad372e184a4fc1810d6b7ce3224e9793b829079505f4c6aac8f512c6fb";
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
#[test]
fn abi_hash_has_valid_iroha_hash_marker() {
    let hash = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(
        hash[hash.len() - 1] & 1,
        1,
        "ABI hash must not be an invalid-surface diagnostic sentinel"
    );
}
