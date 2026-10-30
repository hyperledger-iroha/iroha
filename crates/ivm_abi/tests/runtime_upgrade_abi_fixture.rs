//! The shared ABI v1 hash fixture, authenticated by the canonical IVM ABI owner.

#[path = "../../iroha_data_model/tests/fixtures/abi_v1_hash.rs"]
mod abi_v1_fixture;

use ivm_abi::{SyscallPolicy, syscalls::compute_abi_hash};

#[test]
fn canonical_abi_v1_fixture_matches_the_owning_surface() {
    let current = compute_abi_hash(SyscallPolicy::AbiV1);
    assert_eq!(abi_v1_fixture::abi_v1_hash(), current);
    assert_eq!(hex::encode(current), abi_v1_fixture::ABI_V1_HASH_GOLDEN);
}
